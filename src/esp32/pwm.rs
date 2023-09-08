use esp_idf_hal::gpio::AnyIOPin;
use esp_idf_hal::gpio::Pin;
use esp_idf_hal::ledc::{
    config::TimerConfig, LedcDriver, LedcTimerDriver, SpeedMode, CHANNEL0, CHANNEL1, CHANNEL2,
    CHANNEL3, CHANNEL4, CHANNEL5, CHANNEL6, CHANNEL7, TIMER0, TIMER1, TIMER2, TIMER3,
};
use esp_idf_hal::peripheral::Peripheral;
use esp_idf_hal::prelude::FromValueType;
use esp_idf_sys::{
    ledc_bind_channel_timer, ledc_channel_t_LEDC_CHANNEL_MAX, ledc_get_freq, ledc_set_freq,
    ledc_timer_t, EspError,
};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;

static LEDC_MANAGER: Lazy<Mutex<LedcManager>> = Lazy::new(|| Mutex::new(LedcManager::new()));

pub(crate) fn create_pwm_driver(
    pin: AnyIOPin,
    starting_frequency_hz: u32,
) -> Result<PwmDriver<'static>, Esp32PwmError> {
    let chan = LEDC_MANAGER.lock().unwrap().take_next_channel(pin.pin())?;
    PwmDriver::new(pin, chan, starting_frequency_hz)
}

#[derive(Debug, Error)]
pub enum Esp32PwmError {
    #[error("{0}")]
    EspError(EspError),
    #[error("only 4 different PWM frequencies allowed, available freq: {0}, {1}, {2}, {3}")]
    NoTimersAvailable(u32, u32, u32, u32),
    #[error("CHANNEL{0} already in use by pin {1}")]
    ChannelAlreadyInUse(i32, i32),
    #[error("Could not find TIMER{0}")]
    TimerNotFound(usize),
    #[error("Could not find CHANNEL{0}")]
    ChannelNotFound(i32),
    #[error("no more pwm channels available")]
    NoChannelsAvailable,
}

impl From<EspError> for Esp32PwmError {
    fn from(value: EspError) -> Esp32PwmError {
        Esp32PwmError::EspError(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PwmChannel {
    C0,
    C1,
    C2,
    C3,
    C4,
    C5,
    C6,
    C7,
}

impl PwmChannel {
    fn bind_to_timer(&self, timer_number: u8) -> Result<(), Esp32PwmError> {
        match self {
            PwmChannel::C0 => {
                esp_idf_sys::esp!(unsafe {
                    ledc_bind_channel_timer(SpeedMode::LowSpeed.into(), timer_number.into(), 0)
                })?;
            }
            PwmChannel::C1 => {
                esp_idf_sys::esp!(unsafe {
                    ledc_bind_channel_timer(SpeedMode::LowSpeed.into(), timer_number.into(), 1)
                })?;
            }
            PwmChannel::C2 => {
                esp_idf_sys::esp!(unsafe {
                    ledc_bind_channel_timer(SpeedMode::LowSpeed.into(), timer_number.into(), 2)
                })?;
            }
            PwmChannel::C3 => {
                esp_idf_sys::esp!(unsafe {
                    ledc_bind_channel_timer(SpeedMode::LowSpeed.into(), timer_number.into(), 3)
                })?;
            }
            PwmChannel::C4 => {
                esp_idf_sys::esp!(unsafe {
                    ledc_bind_channel_timer(SpeedMode::LowSpeed.into(), timer_number.into(), 4)
                })?;
            }
            PwmChannel::C5 => {
                esp_idf_sys::esp!(unsafe {
                    ledc_bind_channel_timer(SpeedMode::LowSpeed.into(), timer_number.into(), 5)
                })?;
            }
            PwmChannel::C6 => {
                esp_idf_sys::esp!(unsafe {
                    ledc_bind_channel_timer(SpeedMode::LowSpeed.into(), timer_number.into(), 6)
                })?;
            }
            PwmChannel::C7 => {
                esp_idf_sys::esp!(unsafe {
                    ledc_bind_channel_timer(SpeedMode::LowSpeed.into(), timer_number.into(), 7)
                })?;
            }
        };
        Ok(())
    }
}

fn get_ledc_driver_by_channel<'a>(
    channel: PwmChannel,
    timer_number: usize,
    starting_frequency_hz: u32,
    pin: AnyIOPin,
) -> Result<LedcDriver<'a>, Esp32PwmError> {
    esp_idf_hal::into_ref!(pin);
    let pwm_tconf = TimerConfig::default().frequency(starting_frequency_hz.Hz());
    let timer = match timer_number {
        0 => LedcTimerDriver::new(unsafe { TIMER0::new() }, &pwm_tconf)?,
        1 => LedcTimerDriver::new(unsafe { TIMER1::new() }, &pwm_tconf)?,
        2 => LedcTimerDriver::new(unsafe { TIMER2::new() }, &pwm_tconf)?,
        3 => LedcTimerDriver::new(unsafe { TIMER3::new() }, &pwm_tconf)?,
        _ => return Err(Esp32PwmError::TimerNotFound(timer_number)),
    };
    Ok(match channel {
        PwmChannel::C0 => LedcDriver::new(unsafe { CHANNEL0::new() }, timer, pin)?,
        PwmChannel::C1 => LedcDriver::new(unsafe { CHANNEL1::new() }, timer, pin)?,
        PwmChannel::C2 => LedcDriver::new(unsafe { CHANNEL2::new() }, timer, pin)?,
        PwmChannel::C3 => LedcDriver::new(unsafe { CHANNEL3::new() }, timer, pin)?,
        PwmChannel::C4 => LedcDriver::new(unsafe { CHANNEL4::new() }, timer, pin)?,
        PwmChannel::C5 => LedcDriver::new(unsafe { CHANNEL5::new() }, timer, pin)?,
        PwmChannel::C6 => LedcDriver::new(unsafe { CHANNEL6::new() }, timer, pin)?,
        PwmChannel::C7 => LedcDriver::new(unsafe { CHANNEL7::new() }, timer, pin)?,
    })
}

pub(crate) struct PwmDriver<'a> {
    // the timer property on this LedcDriver is unreliable due to the logic below
    // in set_timer_frequency
    ledc_driver: LedcDriver<'a>,
    timer_number: usize,
    channel: PwmChannel,
}
impl<'a> PwmDriver<'a> {
    fn new(
        pin: AnyIOPin,
        channel: PwmChannel,
        starting_frequency_hz: u32,
    ) -> Result<PwmDriver<'a>, Esp32PwmError> {
        let mut ledc_manager = LEDC_MANAGER.lock().unwrap();
        let timer_number = ledc_manager.take_timer(starting_frequency_hz, channel)?;
        let ledc_driver =
            get_ledc_driver_by_channel(channel, timer_number, starting_frequency_hz, pin)?;

        Ok(PwmDriver {
            ledc_driver,
            timer_number,
            channel,
        })
    }

    pub fn set_ledc_duty_pct(&mut self, pct: f64) -> Result<(), Esp32PwmError> {
        let max_duty = self.ledc_driver.get_max_duty();
        self.ledc_driver
            .set_duty(((max_duty as f64) * pct.abs()).floor() as u32)?;
        Ok(())
    }

    pub fn get_ledc_duty_pct(&self) -> f64 {
        let max_duty = self.ledc_driver.get_max_duty();
        (self.ledc_driver.get_duty() as f64) / (max_duty as f64)
    }

    pub fn get_timer_frequency(&self) -> u32 {
        let timer: ledc_timer_t = (self.timer_number as u8).into();
        unsafe { ledc_get_freq(SpeedMode::LowSpeed.into(), timer) }
    }

    pub fn set_timer_frequency(&mut self, frequency_hz: u32) -> Result<(), Esp32PwmError> {
        let mut ledc_manager = LEDC_MANAGER.lock().unwrap();
        let channels_sharing_timer =
            ledc_manager.number_of_channels_by_timer(self.timer_number) - 1;
        // if we're not in danger of unexpectedly changing the frequency of another PWM signal
        // sharing the same timer, simply change the frequency of the current timer
        if channels_sharing_timer == 0 {
            ledc_manager.set_frequency_on_timer(self.timer_number, frequency_hz)?;
            return Ok(());
        }
        let timer_number = ledc_manager.take_timer(frequency_hz, self.channel)?;
        let timer: ledc_timer_t = (timer_number as u8).into();
        match esp_idf_sys::esp!(unsafe {
            ledc_set_freq(SpeedMode::LowSpeed.into(), timer, frequency_hz)
        }) {
            Ok(val) => Ok(val),
            Err(err) => {
                ledc_manager.drop_timer(timer_number, self.channel);
                Err(err)
            }
        }?;
        if timer_number != self.timer_number {
            self.channel.bind_to_timer(timer_number as u8)?;
        }
        ledc_manager.drop_timer(self.timer_number, self.channel);
        self.timer_number = timer_number;
        Ok(())
    }
}

impl<'a> Drop for PwmDriver<'a> {
    fn drop(&mut self) {
        let mut ledc_manager = LEDC_MANAGER.lock().unwrap();
        ledc_manager.drop_timer(self.timer_number, self.channel);
    }
}

struct LedcManager {
    channel_pin_subscriptions: HashMap<i32, i32>,
    timer_frequencies: [Option<u32>; 4],
    timer_channels: [Vec<PwmChannel>; 4],
}

impl LedcManager {
    fn new() -> Self {
        Self {
            channel_pin_subscriptions: HashMap::new(),
            timer_frequencies: [None, None, None, None],
            timer_channels: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
        }
    }

    fn find_timer_by_frequency(&mut self, frequency_hz: u32) -> Option<usize> {
        self.timer_frequencies
            .iter()
            .position(|&x| x == Some(frequency_hz))
    }

    fn set_frequency_on_timer(
        &mut self,
        timer_number: usize,
        frequency_hz: u32,
    ) -> Result<(), Esp32PwmError> {
        let timer: ledc_timer_t = (timer_number as u8).into();
        esp_idf_sys::esp!(unsafe {
            ledc_set_freq(SpeedMode::LowSpeed.into(), timer, frequency_hz)
        })?;
        self.timer_frequencies[timer_number] = Some(frequency_hz);
        Ok(())
    }

    fn number_of_channels_by_timer(&self, timer_number: usize) -> usize {
        self.timer_channels[timer_number].len()
    }

    fn take_timer(
        &mut self,
        frequency_hz: u32,
        channel: PwmChannel,
    ) -> Result<usize, Esp32PwmError> {
        match self.find_timer_by_frequency(frequency_hz) {
            Some(timer_number) => {
                self.timer_channels[timer_number].push(channel);
                Ok(timer_number)
            }
            None => {
                if self.timer_frequencies.iter().all(|&x| x.is_some()) {
                    Err(Esp32PwmError::NoTimersAvailable(
                        self.timer_frequencies[0].unwrap(),
                        self.timer_frequencies[1].unwrap(),
                        self.timer_frequencies[2].unwrap(),
                        self.timer_frequencies[3].unwrap(),
                    ))
                } else {
                    let timer_number = match self.timer_frequencies.iter().position(|x| x.is_none())
                    {
                        Some(timer_number) => timer_number,
                        None => unreachable!(),
                    };
                    self.timer_frequencies[timer_number] = Some(frequency_hz);
                    self.timer_channels[timer_number].push(channel);
                    Ok(timer_number)
                }
            }
        }
    }

    fn drop_timer(&mut self, timer_number: usize, channel: PwmChannel) {
        let timer_channels = &mut self.timer_channels[timer_number];
        if let Some(index) = timer_channels.iter().position(|&x| x == channel) {
            timer_channels.swap_remove(index);
            if self.timer_channels[timer_number].is_empty() {
                self.timer_frequencies[timer_number] = None;
            };
        };
    }

    fn take_channel(&mut self, n: i32, pin: i32) -> Result<PwmChannel, Esp32PwmError> {
        if let Some(&pin_using_channel) = self.channel_pin_subscriptions.get(&n) {
            return Err(Esp32PwmError::ChannelAlreadyInUse(n, pin_using_channel));
        }
        let res = match n {
            0 => PwmChannel::C0,
            1 => PwmChannel::C1,
            2 => PwmChannel::C2,
            3 => PwmChannel::C3,
            4 => PwmChannel::C4,
            5 => PwmChannel::C5,
            6 => PwmChannel::C6,
            7 => PwmChannel::C7,
            _ => {
                return Err(Esp32PwmError::ChannelNotFound(n));
            }
        };
        self.channel_pin_subscriptions.insert(n, pin);
        Ok(res)
    }

    fn take_next_channel(&mut self, pin: i32) -> Result<PwmChannel, Esp32PwmError> {
        for i in 0..(ledc_channel_t_LEDC_CHANNEL_MAX as i32) {
            match self.take_channel(i, pin) {
                Ok(ret) => {
                    return Ok(ret);
                }
                Err(err) => match err {
                    Esp32PwmError::ChannelAlreadyInUse(_, _) => {}
                    _ => return Err(err),
                },
            };
        }
        Err(Esp32PwmError::NoChannelsAvailable)
    }
}
