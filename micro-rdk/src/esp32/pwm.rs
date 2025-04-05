use crate::esp32::esp_idf_svc::hal::gpio::AnyIOPin;
use crate::esp32::esp_idf_svc::hal::gpio::Pin;
use crate::esp32::esp_idf_svc::hal::ledc::{
    config::TimerConfig, LedcDriver, LedcTimer, LedcTimerDriver, LowSpeed, SpeedMode, CHANNEL0,
    CHANNEL1, CHANNEL2, CHANNEL3, CHANNEL4, CHANNEL5, TIMER0, TIMER1, TIMER2, TIMER3,
};

use crate::esp32::esp_idf_svc::hal::peripheral::Peripheral;
use crate::esp32::esp_idf_svc::hal::prelude::FromValueType;
use crate::esp32::esp_idf_svc::sys::{
    ledc_bind_channel_timer, ledc_get_freq, ledc_timer_t, EspError,
};
use bitfield::{bitfield, Bit, BitMut};
use once_cell::sync::Lazy;
use std::cell::OnceCell;
use std::fmt::Debug;
use std::sync::Mutex;
use thiserror::Error;

#[cfg(any(esp32, esp32s2, esp32s3))]
use crate::esp32::esp_idf_svc::hal::ledc::{CHANNEL6, CHANNEL7};

static LEDC_MANAGER: Lazy<Mutex<LedcManager>> = Lazy::new(|| Mutex::new(LedcManager::new()));

#[derive(Debug, Error)]
pub enum Esp32PwmError {
    #[error("{0}")]
    EspError(EspError),
    #[error("all timer are used try a different frequency")]
    NoTimersAvailable,
    #[error("CHANNEL{0} already in use by pin {1}")]
    ChannelAlreadyInUse(i32, i32),
    #[error("Could not find TIMER{0}")]
    TimerNotFound(usize),
    #[error("Could not find CHANNEL{0}")]
    ChannelNotFound(i32),
    #[error("no more pwm channels available")]
    NoChannelsAvailable,
    #[error("invalid timer number {0}")]
    InvalidTimerNumber(i32),
    #[error("one or more channel are bind to the timer")]
    OtherChannelsBindToTimer,
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

    #[cfg(any(esp32, esp32s2, esp32s3))]
    C6,

    #[cfg(any(esp32, esp32s2, esp32s3))]
    C7,
}

impl PwmChannel {
    fn into_ledc_driver<'a, T>(
        self,
        timer: &LedcTimerDriver<'a, T>,
        pin: AnyIOPin,
    ) -> Result<LedcDriver<'a>, Esp32PwmError>
    where
        T: LedcTimer<SpeedMode = LowSpeed>,
    {
        crate::esp32::esp_idf_svc::hal::into_ref!(pin);

        Ok(match self {
            Self::C0 => LedcDriver::new(unsafe { CHANNEL0::new() }, timer, pin)?,
            Self::C1 => LedcDriver::new(unsafe { CHANNEL1::new() }, timer, pin)?,
            Self::C2 => LedcDriver::new(unsafe { CHANNEL2::new() }, timer, pin)?,
            Self::C3 => LedcDriver::new(unsafe { CHANNEL3::new() }, timer, pin)?,
            Self::C4 => LedcDriver::new(unsafe { CHANNEL4::new() }, timer, pin)?,
            Self::C5 => LedcDriver::new(unsafe { CHANNEL5::new() }, timer, pin)?,

            #[cfg(any(esp32, esp32s2, esp32s3))]
            Self::C6 => LedcDriver::new(unsafe { CHANNEL6::new() }, timer, pin)?,

            #[cfg(any(esp32, esp32s2, esp32s3))]
            Self::C7 => LedcDriver::new(unsafe { CHANNEL7::new() }, timer, pin)?,
        })
    }
}

fn get_ledc_driver_by_channel_by_timer<'a>(
    channel: PwmChannel,
    timer_opt: &LedcTimerOption<'a>,
    pin: AnyIOPin,
) -> Result<LedcDriver<'a>, Esp32PwmError> {
    match timer_opt {
        LedcTimerOption::Timer0(timer) => channel.into_ledc_driver(timer, pin),
        LedcTimerOption::Timer1(timer) => channel.into_ledc_driver(timer, pin),
        LedcTimerOption::Timer2(timer) => channel.into_ledc_driver(timer, pin),
        LedcTimerOption::Timer3(timer) => channel.into_ledc_driver(timer, pin),
    }
}

bitfield! {
    struct PwmChannelInUse(u8);
    impl Debug;
    channels, _ : 7,0;
}

impl From<u8> for PwmChannel {
    fn from(value: u8) -> Self {
        match value {
            0 => PwmChannel::C0,
            1 => PwmChannel::C1,
            2 => PwmChannel::C2,
            3 => PwmChannel::C3,
            4 => PwmChannel::C4,
            5 => PwmChannel::C5,

            #[cfg(any(esp32, esp32s2, esp32s3))]
            6 => PwmChannel::C6,

            #[cfg(any(esp32, esp32s2, esp32s3))]
            7 => PwmChannel::C7,

            _ => unreachable!(),
        }
    }
}

impl From<PwmChannel> for usize {
    fn from(value: PwmChannel) -> Self {
        match value {
            PwmChannel::C0 => 0,
            PwmChannel::C1 => 1,
            PwmChannel::C2 => 2,
            PwmChannel::C3 => 3,
            PwmChannel::C4 => 4,
            PwmChannel::C5 => 5,
            #[cfg(any(esp32, esp32s2, esp32s3))]
            PwmChannel::C6 => 6,

            #[cfg(any(esp32, esp32s2, esp32s3))]
            PwmChannel::C7 => 7,
        }
    }
}

pub(crate) struct PwmDriver<'a> {
    // the timer property on this LedcDriver is unreliable due to the logic below
    // in set_timer_frequency
    ledc_driver: LedcDriver<'a>,
    timer_number: usize,
    channel: PwmChannel,
}
impl<'a> PwmDriver<'a> {
    pub fn new(pin: AnyIOPin, starting_frequency_hz: u32) -> Result<PwmDriver<'a>, Esp32PwmError> {
        let mut ledc_manager = LEDC_MANAGER.lock().unwrap();
        let channel = ledc_manager.allocate_pin(pin.pin(), starting_frequency_hz)?;
        let timer = ledc_manager.get_configure_timer_instance(channel.1);
        let ledc_driver = get_ledc_driver_by_channel_by_timer(channel.0, timer, pin)?;
        Ok(PwmDriver {
            ledc_driver,
            timer_number: channel.1 as usize,
            channel: channel.0,
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
        unsafe { ledc_get_freq(LowSpeed::SPEED_MODE, timer) }
    }

    pub fn set_timer_frequency(&mut self, frequency_hz: u32) -> Result<(), Esp32PwmError> {
        let mut ledc_manager = LEDC_MANAGER.lock().unwrap();
        let timer_number =
            ledc_manager.set_timer_frequency(self.timer_number, frequency_hz, self.channel)?;
        self.timer_number = timer_number;
        Ok(())
    }
}

impl Drop for PwmDriver<'_> {
    fn drop(&mut self) {
        let mut ledc_manager = LEDC_MANAGER.lock().unwrap();
        ledc_manager.release_channel_and_timer(self.channel, self.timer_number);
    }
}

pub enum LedcTimerOption<'a> {
    Timer0(LedcTimerDriver<'a, TIMER0>),
    Timer1(LedcTimerDriver<'a, TIMER1>),
    Timer2(LedcTimerDriver<'a, TIMER2>),
    Timer3(LedcTimerDriver<'a, TIMER3>),
}

impl<'a> LedcTimerOption<'a> {
    pub fn new(timer: u8, conf: &TimerConfig) -> Result<Self, Esp32PwmError> {
        match timer {
            0 => {
                let driver = LedcTimerDriver::new(unsafe { TIMER0::new() }, conf)
                    .map_err(Esp32PwmError::EspError)?;
                Ok(Self::Timer0(driver))
            }
            1 => {
                let driver = LedcTimerDriver::new(unsafe { TIMER1::new() }, conf)
                    .map_err(Esp32PwmError::EspError)?;
                Ok(Self::Timer1(driver))
            }
            2 => {
                let driver = LedcTimerDriver::new(unsafe { TIMER2::new() }, conf)
                    .map_err(Esp32PwmError::EspError)?;
                Ok(Self::Timer2(driver))
            }
            3 => {
                let driver = LedcTimerDriver::new(unsafe { TIMER3::new() }, conf)
                    .map_err(Esp32PwmError::EspError)?;
                Ok(Self::Timer3(driver))
            }
            _ => unreachable!(),
        }
    }

    pub fn timer(&'a self) -> ledc_timer_t {
        match self {
            Self::Timer0(_) => TIMER0::timer(),
            Self::Timer1(_) => TIMER1::timer(),
            Self::Timer2(_) => TIMER2::timer(),
            Self::Timer3(_) => TIMER3::timer(),
        }
    }
}

#[derive(Debug)]
struct LedcManager<'a> {
    used_channel: PwmChannelInUse,
    associated_pins: [u8; 8],
    timer_allocation: [LedcTimerWrapper<'a>; 4],
}

struct LedcTimerWrapper<'a> {
    frequency: u32,
    count: u8,
    timer: OnceCell<LedcTimerOption<'a>>,
}

impl Debug for LedcTimerWrapper<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedcTimerWrapper")
            .field("count", &self.count)
            .field("frequency", &self.frequency)
            .field("timer", &self.timer.get().unwrap().timer())
            .finish()
    }
}

impl LedcTimerWrapper<'_> {
    fn new(id: u8, frequency_hz: u32) -> Result<Self, Esp32PwmError> {
        let timer_config = TimerConfig::default().frequency(frequency_hz.Hz());
        let timer = OnceCell::new();
        let _ = timer.set(LedcTimerOption::new(id, &timer_config)?);
        Ok(Self {
            count: 0,
            frequency: frequency_hz,
            timer,
        })
    }
    fn set_frequency(&mut self, frequency_hz: u32) -> Result<(), Esp32PwmError> {
        if self.frequency == frequency_hz {
            return Ok(());
        }
        if self.count > 0 {
            return Err(Esp32PwmError::OtherChannelsBindToTimer);
        }

        let id = {
            let timer = self.timer.take();
            timer.unwrap().timer() as u8
        };
        let timer_config = TimerConfig::default().frequency(frequency_hz.Hz());

        // The configured clock source for the timer may not be able to achieve the target frequency.
        // We have to reconfigure the timer so the appropriate clock source for that frequency may be
        // selected. If no appropriate clock source exists the previous timer frequency
        // will be retained
        match LedcTimerOption::new(id, &timer_config) {
            Ok(driver) => {
                let _ = self.timer.set(driver);
                self.frequency = frequency_hz;
                Ok(())
            }
            Err(err) => {
                let timer_config = TimerConfig::default().frequency(self.frequency.Hz());
                let _ =
                    self.timer
                        .set(LedcTimerOption::new(id, &timer_config).unwrap_or_else(|_| {
                            panic!("bad frequency previously set on timer {:?}", id)
                        }));
                Err(err)
            }
        }
    }
    fn inc(&mut self) {
        self.count += 1;
    }
    fn dec(&mut self) {
        self.count -= 1;
    }
}

impl<'a> LedcManager<'a> {
    fn new() -> Self {
        let timer_allocation = [
            LedcTimerWrapper::new(0, 1000).unwrap(),
            LedcTimerWrapper::new(1, 1000).unwrap(),
            LedcTimerWrapper::new(2, 1000).unwrap(),
            LedcTimerWrapper::new(3, 1000).unwrap(),
        ];
        Self {
            used_channel: PwmChannelInUse(0),
            associated_pins: [0_u8; 8],
            timer_allocation,
        }
    }

    fn get_configure_timer_instance<'d>(&'d self, timer_number: u32) -> &'d LedcTimerOption<'a> {
        self.timer_allocation[timer_number as usize]
            .timer
            .get()
            .unwrap()
    }

    fn next_available_timer(&mut self, frequency_hz: u32) -> Result<usize, Esp32PwmError> {
        // Timer with same frequency exist?
        if let Some(timer) = self.find_timer_by_frequency(frequency_hz) {
            return Ok(timer);
        }
        // Free Timer?
        let res = self
            .timer_allocation
            .iter()
            .enumerate()
            .find_map(|(i, t)| if t.count == 0 { Some(i) } else { None })
            .ok_or(Esp32PwmError::NoTimersAvailable);
        let timer_number = match res {
            Ok(t) => {
                self.timer_allocation[t].set_frequency(frequency_hz)?;
                t
            }
            // if no timer are free then match with the nearest pwm frequency
            Err(_) => self
                .timer_allocation
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    (a.frequency as i32 - frequency_hz as i32)
                        .abs()
                        .cmp(&(b.frequency as i32 - frequency_hz as i32).abs())
                })
                .map(|(idx, _)| idx)
                .unwrap(),
        };
        Ok(timer_number)
    }

    fn find_timer_by_frequency(&mut self, frequency_hz: u32) -> Option<usize> {
        self.timer_allocation
            .iter()
            .position(|t| t.frequency == frequency_hz)
    }

    fn set_timer_frequency(
        &mut self,
        timer_number: usize,
        frequency_hz: u32,
        channel: PwmChannel,
    ) -> Result<usize, Esp32PwmError> {
        if timer_number > 3 {
            return Err(Esp32PwmError::InvalidTimerNumber(timer_number as i32));
        }

        // We decrease and then e=increase the timer count to check if
        // the new frequency can be achieved on the assigned timer (ie no other channels are depending on it)
        // After that we have 3 case
        // 1) another timer as the target frequency to assign the channel to it
        // 2) an timer is free so assigned the channel to it
        // 3) Neither 1 or 2 are possible so frequency remains unchanged (note we could find the nearest frequency there)
        self.timer_allocation[timer_number].dec();
        let res = self.timer_allocation[timer_number].set_frequency(frequency_hz);
        self.timer_allocation[timer_number].inc();

        match res {
            Ok(()) => Ok(timer_number),
            Err(_) => {
                let new_timer = self
                    .timer_allocation
                    .iter_mut()
                    .enumerate()
                    .find_map(|(i, t)| match t.set_frequency(frequency_hz) {
                        Ok(()) => Some(i),
                        Err(_) => None,
                    })
                    .ok_or(Esp32PwmError::NoTimersAvailable)?;
                unsafe {
                    ledc_bind_channel_timer(
                        LowSpeed::SPEED_MODE,
                        Into::<usize>::into(channel) as u32,
                        new_timer as u32,
                    )
                };
                self.bind_channel_to_timer(Some(timer_number), new_timer)
                    .map(|_| new_timer)
            }
        }
    }
    fn bind_channel_to_timer(
        &mut self,
        old_timer: Option<usize>,
        new_timer: usize,
    ) -> Result<(), Esp32PwmError> {
        if let Some(old_timer) = old_timer {
            if old_timer > self.timer_allocation.len() - 1 {
                return Err(Esp32PwmError::InvalidTimerNumber(old_timer as i32));
            }
            if old_timer == new_timer {
                return Ok(());
            }
            self.timer_allocation[old_timer].dec();
        }
        if new_timer > self.timer_allocation.len() - 1 {
            return Err(Esp32PwmError::InvalidTimerNumber(new_timer as i32));
        }
        self.timer_allocation[new_timer].inc();
        Ok(())
    }
    fn next_available_channel(&mut self, pin: i32) -> Result<PwmChannel, Esp32PwmError> {
        let mut channel: Option<PwmChannel> = None;
        for i in 0..8 {
            if !self.used_channel.bit(i) {
                let _ = channel.insert((i as u8).into());
                self.used_channel.set_bit(i, true);
                self.associated_pins[i] = pin as u8;
                break;
            }
        }
        channel.ok_or(Esp32PwmError::NoChannelsAvailable)
    }
    fn release_channel_and_timer(&mut self, channel: PwmChannel, timer_number: usize) {
        self.used_channel.set_bit(channel.into(), false);
        if timer_number < self.timer_allocation.len() - 1 {
            self.timer_allocation[timer_number].dec();
        }
    }
    fn allocate_pin(
        &mut self,
        pin: i32,
        frequency_hz: u32,
    ) -> Result<(PwmChannel, u32), Esp32PwmError> {
        let channel = self.next_available_channel(pin)?;
        let timer = self.next_available_timer(frequency_hz)?;
        self.bind_channel_to_timer(None, timer)?;
        Ok((channel, timer as u32))
    }
}
