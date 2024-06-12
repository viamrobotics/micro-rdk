use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use micro_rdk::DoCommand;

use micro_rdk::common::{
    board::{BoardType},
    config::{ConfigType},
    motor::{Motor, MotorSupportedProperties, MotorType, MotorError },
    registry::{self, ComponentRegistry, Dependency, RegistryError, },
    status::{Status, StatusError}, 
    actuator::{Actuator, ActuatorError}
};

/// This driver is for a water pump and optional led
#[derive(DoCommand)]
pub struct WaterPump {
    board_handle: BoardType,
    pin: i32,
    led: Option<i32>,
}

pub fn register_models(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
    registry.register_motor("water_pump", &WaterPump::from_config)?;
    log::info!("water_pump motor registration ok");
    Ok(())
}

impl WaterPump {
    pub fn from_config(cfg: ConfigType, deps: Vec<Dependency>) -> Result<MotorType, MotorError> {
        let board_handle = registry::get_board_from_dependencies(deps)
            .expect("failed to get board from dependencies");
        let pin = cfg.get_attribute::<i32>("pin").map_err(|_| MotorError::ConfigError("failed to get pin from board"))?;
        let led = cfg.get_attribute::<i32>("led").ok();
        Ok(Arc::new(Mutex::new(Self {
            board_handle,
            pin,
            led,
        })))
    }
}

impl Motor for WaterPump {
    fn set_power(&mut self, pct: f64) -> Result<(), MotorError> {
        let pct = pct.clamp(-1.0, 1.0);
        if pct > 0.0 {
            // high
            self.board_handle
                .lock()
                .unwrap()
                .set_gpio_pin_level(self.pin, true)?;
            if let Some(pin) = self.led {
                self.board_handle
                    .lock()
                    .unwrap()
                    .set_gpio_pin_level(pin, true)?;
            }
        } else {
            // low
            self.board_handle
                .lock()
                .unwrap()
                .set_gpio_pin_level(self.pin, false)?;
            if let Some(pin) = self.led {
                self.board_handle
                    .lock()
                    .unwrap()
                    .set_gpio_pin_level(pin, false)?;
            }
        };
        Ok(())
    }
    fn get_position(&mut self) -> Result<i32, MotorError> {
        unimplemented!();
    }
    fn go_for(
        &mut self,
        _rpm: f64,
        _revolutions: f64,
    ) -> Result<Option<std::time::Duration>, MotorError> {
        unimplemented!();
    }

    fn get_properties(&mut self) -> MotorSupportedProperties {
        MotorSupportedProperties {
            position_reporting: false,
        }
    }
}

impl Actuator for WaterPump {
    fn is_moving(&mut self) -> Result<bool, ActuatorError> {
        self.board_handle
            .lock()
            .unwrap()
            .get_gpio_level(self.pin).map_err(ActuatorError::BoardError)
    }
    fn stop(&mut self) -> Result<(), ActuatorError> {
        self.set_power(0.0).map_err(|_|ActuatorError::CouldntStop)
    }
}

impl Status for WaterPump {
    fn get_status(&self) -> Result<Option<micro_rdk::google::protobuf::Struct>, StatusError> {
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
