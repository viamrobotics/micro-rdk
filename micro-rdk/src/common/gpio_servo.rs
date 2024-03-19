//! A generalized servo implementation that uses a PWM signal on a GPIO pin
//! via methods on a board component
//!
//! # Creating a GPIO pin servo and turning it 90 degrees
//!
//! ```ignore
//!
//! let board = FakeBoard::new(vec![]);
//!
//! let servo_settings = GpioServoSettings {
//!     min_angle_deg: 0,
//!     max_angle_deg: 180,
//!     min_period_us: 500,
//!     max_period_us: 2500,
//!     pwm_resolution: 0,
//!     frequency: 300,
//! }
//!
//! let mut servo = GpioServo::new(board, 12, servo_settings);
//!
//! servo.move_to(90).unwrap()
//!
//! ```

use std::sync::{Arc, Mutex};

use super::{
    actuator::{Actuator, ActuatorError},
    board::{Board, BoardType},
    config::{AttributeError, ConfigType},
    registry::{get_board_from_dependencies, ComponentRegistry, Dependency},
    servo::{Servo, ServoType},
    status::Status,
};

/// Minimum and maximum period widths that should be safe limits for
/// most servos. It is recommended you configure the servo with
/// the limits provided by its datasheet if possible
const SAFE_PERIOD_WIDTH_LIMITS: (u32, u32) = (500, 2500);
/// Minimum and maximum angular positions that should be safe limits
/// for most servos. It is recommended you configure the servo with
/// the limits provided by its datasheet if possible
const SAFE_ANGULAR_POSITION_LIMITS: (u32, u32) = (0, 180);
/// Default PWM frequency that should be acceptable for most servos.
/// It is recommended you configure the servo with the limits
/// provided by its datasheet if possible
const SAFE_DEFAULT_FREQUENCY_HZ: u32 = 300;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry.register_servo("gpio", &from_config).is_err() {
        log::error!("gpio model is already registered")
    }
}

pub(crate) fn from_config(
    cfg: ConfigType,
    dependencies: Vec<Dependency>,
) -> anyhow::Result<ServoType> {
    let board = get_board_from_dependencies(dependencies).ok_or(anyhow::anyhow!(
        "configuration for 'gpio' servo is missing the required board dependency"
    ))?;
    let servo_settings = GpioServoSettings::from_config(&cfg)?;
    let pin = cfg.get_attribute::<i32>("pin").map_err(|err| match err {
        AttributeError::ConversionImpossibleError => {
            anyhow::anyhow!(
                "could not convert pin attribute to integer when configuring gpio servo"
            )
        }
        _ => anyhow::anyhow!("error configuring gpio servo: {:?}", err),
    })?;
    Ok(Arc::new(Mutex::new(GpioServo::<BoardType>::new(
        board.clone(),
        pin,
        servo_settings,
    )?)))
}

#[derive(Debug)]
pub(crate) struct GpioServoSettings {
    pub min_angle_deg: u32,
    pub max_angle_deg: u32,
    pub min_period_us: u32,
    pub max_period_us: u32,
    pub frequency: u32,
    /// when 0, pwm_resolution is not considered when calculating the PWM duty cycle
    /// necessary to move the servo to particular angular position
    pub pwm_resolution: u32,
}

impl GpioServoSettings {
    pub fn from_config(cfg: &ConfigType) -> anyhow::Result<Self> {
        let min_angle_deg = cfg
            .get_attribute::<u32>("min_angle_deg")
            .unwrap_or(SAFE_ANGULAR_POSITION_LIMITS.0);
        let max_angle_deg = cfg
            .get_attribute::<u32>("max_angle_deg")
            .unwrap_or(SAFE_ANGULAR_POSITION_LIMITS.1);
        let min_period_us = cfg
            .get_attribute::<u32>("min_width_us")
            .unwrap_or(SAFE_PERIOD_WIDTH_LIMITS.0);
        let max_period_us = cfg
            .get_attribute::<u32>("max_width_us")
            .unwrap_or(SAFE_PERIOD_WIDTH_LIMITS.1);
        let frequency = cfg
            .get_attribute::<u32>("frequency_hz")
            .unwrap_or(SAFE_DEFAULT_FREQUENCY_HZ);
        let pwm_resolution = cfg
            .get_attribute::<u32>("pwm_resolution")
            .unwrap_or_default();
        Ok(Self {
            min_angle_deg,
            max_angle_deg,
            min_period_us,
            max_period_us,
            frequency,
            pwm_resolution,
        })
    }
}

#[derive(DoCommand)]
pub struct GpioServo<B> {
    board: B,
    pin: i32,
    min_angle_deg: u32,
    max_angle_deg: u32,
    min_period_us: u32,
    max_period_us: u32,
    frequency: u32,
    pwm_resolution: u32,
}

impl<B> GpioServo<B>
where
    B: Board,
{
    pub(crate) fn new(board: B, pin: i32, settings: GpioServoSettings) -> anyhow::Result<Self> {
        if settings.frequency == 0 {
            return Err(anyhow::anyhow!(
                "PWM frequency cannot be zero for 'gpio' servo"
            ));
        }
        let mut res = Self {
            board,
            pin,
            min_angle_deg: settings.min_angle_deg,
            max_angle_deg: settings.max_angle_deg,
            min_period_us: settings.min_period_us,
            max_period_us: settings.max_period_us,
            frequency: settings.frequency,
            pwm_resolution: settings.pwm_resolution,
        };
        res.board.set_pwm_frequency(pin, res.frequency as u64)?;
        Ok(res)
    }

    pub fn angle_to_duty_pct(&self, angle_deg: u32) -> f64 {
        let period = 1.0 / (self.frequency as f64);
        let angle_range = (self.max_angle_deg - self.min_angle_deg) as f64;
        let period_range = (self.max_period_us - self.min_period_us) as f64;
        let period_per_angle = period_range / angle_range;
        let pwm_width: f64 = (self.min_period_us as f64)
            + ((angle_deg - self.min_angle_deg) as f64) * period_per_angle;
        pwm_width / 1000000.0 / period
    }

    fn duty_pct_to_angle(&self, duty_pct: f64) -> u32 {
        let period = 1.0 / (self.frequency as f64);
        let pwm_width = (duty_pct * period * 1000000.0)
            .clamp(self.min_period_us as f64, self.max_period_us as f64)
            as u32;
        let angle_range = (self.max_angle_deg - self.min_angle_deg) as f64;
        let period_range: f64 = (self.max_period_us - self.min_period_us) as f64;
        let angle_per_period = angle_range / period_range;
        let location_in_period = (pwm_width - self.min_period_us) as f64;
        ((self.min_angle_deg as f64) + (location_in_period * angle_per_period)) as u32
    }
}

impl<B> Servo for GpioServo<B>
where
    B: Board,
{
    // this implementation of move_to clamps the angle to the range determined
    // by min_angle_deg and max_angle_deg, rather than raising an error for out of range
    // values
    fn move_to(&mut self, angle_deg: u32) -> anyhow::Result<()> {
        let angle_deg = angle_deg.clamp(self.min_angle_deg, self.max_angle_deg);
        let mut duty_cycle_pct = self.angle_to_duty_pct(angle_deg);
        if self.pwm_resolution != 0 {
            let real_tick = (duty_cycle_pct * (self.pwm_resolution as f64)).round();
            duty_cycle_pct = real_tick / (self.pwm_resolution as f64);
        }
        self.board.set_pwm_duty(self.pin, duty_cycle_pct)?;
        Ok(())
    }
    fn get_position(&mut self) -> anyhow::Result<u32> {
        let duty_pct = self.board.get_pwm_duty(self.pin);
        Ok(self.duty_pct_to_angle(duty_pct))
    }
}

impl<B> Actuator for GpioServo<B>
where
    B: Board,
{
    fn is_moving(&mut self) -> Result<bool, ActuatorError> {
        Ok(self.board.get_pwm_duty(self.pin) != 0.0)
    }
    fn stop(&mut self) -> Result<(), ActuatorError> {
        Ok(self.board.set_pwm_duty(self.pin, 0.0)?)
    }
}

impl<B> Status for GpioServo<B>
where
    B: Board,
{
    fn get_status(&self) -> anyhow::Result<Option<crate::google::protobuf::Struct>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::common::board::{Board, FakeBoard};
    use crate::common::gpio_servo::{GpioServo, GpioServoSettings};
    use crate::common::servo::Servo;
    use std::sync::{Arc, Mutex};

    #[test_log::test]
    fn test_move_to_with_no_pwm_resolution() -> anyhow::Result<()> {
        let board = Arc::new(Mutex::new(FakeBoard::new(vec![])));
        let servo_settings = GpioServoSettings {
            min_angle_deg: 90,
            max_angle_deg: 270,
            min_period_us: 500,
            max_period_us: 2500,
            frequency: 300,
            pwm_resolution: 0,
        };
        let mut servo = GpioServo::new(board.clone(), 2, servo_settings)?;

        // clamp to minimum test case (80 -> 90 internally)
        servo.move_to(80)?;
        assert_eq!(board.get_pwm_frequency(2)?, 300);
        assert_eq!(board.get_pwm_duty(2), 0.15);

        // clamp to maximum test case, (280 -> 270 internally)
        servo.move_to(280)?;
        assert_eq!(board.get_pwm_frequency(2)?, 300);
        assert_eq!(board.get_pwm_duty(2), 0.75);

        // angle: 90 -> duty: 0.45
        servo.move_to(180)?;
        assert_eq!(board.get_pwm_frequency(2)?, 300);
        assert!((board.get_pwm_duty(2) - 0.45).abs() < 0.0001);
        Ok(())
    }

    #[test_log::test]
    fn test_get_position() -> anyhow::Result<()> {
        let mut board = Arc::new(Mutex::new(FakeBoard::new(vec![])));
        let servo_settings = GpioServoSettings {
            min_angle_deg: 90,
            max_angle_deg: 270,
            min_period_us: 500,
            max_period_us: 2500,
            frequency: 300,
            pwm_resolution: 0,
        };
        let mut servo = GpioServo::new(board.clone(), 2, servo_settings)?;

        board.set_pwm_duty(2, 0.15)?;
        assert_eq!(servo.get_position()?, 90);

        board.set_pwm_duty(2, 0.75)?;
        assert_eq!(servo.get_position()?, 270);

        board.set_pwm_duty(2, 0.45)?;
        assert_eq!(servo.get_position()?, 180);
        Ok(())
    }

    #[test_log::test]
    fn test_move_to_with_pwm_resolution() -> anyhow::Result<()> {
        let board = Arc::new(Mutex::new(FakeBoard::new(vec![])));
        let servo_settings = GpioServoSettings {
            min_angle_deg: 90,
            max_angle_deg: 270,
            min_period_us: 500,
            max_period_us: 2500,
            frequency: 300,
            pwm_resolution: 10,
        };
        let mut servo = GpioServo::new(board.clone(), 2, servo_settings)?;

        // clamp to minimum test case (80 -> 90 internally)
        servo.move_to(80)?;
        assert_eq!(board.get_pwm_frequency(2)?, 300);
        assert_eq!(board.get_pwm_duty(2), 0.20);

        // clamp to maximum test case, (280 -> 270 internally)
        servo.move_to(280)?;
        assert_eq!(board.get_pwm_frequency(2)?, 300);
        assert_eq!(board.get_pwm_duty(2), 0.8);
        Ok(())
    }
}
