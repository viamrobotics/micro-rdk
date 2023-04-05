#![allow(dead_code)]
use core::ffi::{c_short, c_ulong};
use esp_idf_hal::gpio::{AnyOutputPin, Output, PinDriver};
use esp_idf_hal::ledc::config::TimerConfig;
use esp_idf_hal::ledc::{LedcDriver, LedcTimerDriver, CHANNEL0, CHANNEL1, CHANNEL2};
use esp_idf_sys as espsys;
use espsys::pcnt_channel_edge_action_t_PCNT_CHANNEL_EDGE_ACTION_DECREASE as pcnt_count_dec;
use espsys::pcnt_channel_edge_action_t_PCNT_CHANNEL_EDGE_ACTION_INCREASE as pcnt_count_inc;
use espsys::pcnt_channel_level_action_t_PCNT_CHANNEL_LEVEL_ACTION_INVERSE as pcnt_mode_reverse;
use espsys::pcnt_channel_level_action_t_PCNT_CHANNEL_LEVEL_ACTION_KEEP as pcnt_mode_keep;
use espsys::pcnt_channel_t_PCNT_CHANNEL_0 as pcnt_channel_0;
use espsys::pcnt_channel_t_PCNT_CHANNEL_1 as pcnt_channel_1;
use espsys::pcnt_config_t;
use espsys::pcnt_evt_type_t_PCNT_EVT_H_LIM as pcnt_evt_h_lim;
use espsys::pcnt_evt_type_t_PCNT_EVT_L_LIM as pcnt_evt_l_lim;

use super::pin::PinExt;
use crate::common::board::BoardType;
use crate::common::config::{Component, ConfigType};
use crate::common::motor::Position;
use crate::common::motor::{Motor, MotorConfig, MotorType};
use crate::common::registry::ComponentRegistry;
use crate::common::status::Status;
use espsys::{esp, EspError, ESP_ERR_INVALID_STATE, ESP_OK};
use log::*;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use embedded_hal::digital::v2::{InputPin, OutputPin};
use embedded_hal::PwmPin;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_motor(
            "gpio",
            &MotorEsp32::<
                PinDriver<'_, AnyOutputPin, Output>,
                PinDriver<'_, AnyOutputPin, Output>,
                LedcDriver<'_>,
            >::from_config,
        )
        .is_err()
    {
        log::error!("gpio model is already registered")
    }
}

pub struct Esp32Encoder<A, B> {
    acc: Arc<AtomicI32>,
    config: pcnt_config_t,
    a: A,
    b: B,
    unit: u32,
}

impl<A, B> Esp32Encoder<A, B>
where
    A: InputPin + PinExt,
    B: InputPin + PinExt,
{
    pub fn new(a: A, b: B) -> Self {
        Esp32Encoder {
            acc: Arc::new(AtomicI32::new(0)),
            config: pcnt_config_t {
                pulse_gpio_num: a.pin(),
                ctrl_gpio_num: b.pin(),
                pos_mode: pcnt_count_inc,
                neg_mode: pcnt_count_dec,
                lctrl_mode: pcnt_mode_reverse,
                hctrl_mode: pcnt_mode_keep,
                counter_h_lim: 100,
                counter_l_lim: -100,
                channel: pcnt_channel_0,
                unit: 0,
            },
            a,
            b,
            unit: 0,
        }
    }
    pub fn start(&self) -> anyhow::Result<()> {
        unsafe {
            match esp_idf_sys::pcnt_counter_resume(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        Ok(())
    }
    pub fn stop(&self) -> anyhow::Result<()> {
        unsafe {
            match esp_idf_sys::pcnt_counter_pause(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        Ok(())
    }
    pub fn reset(&self) -> anyhow::Result<()> {
        self.stop()?;
        unsafe {
            match esp_idf_sys::pcnt_counter_clear(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        self.start()?;
        Ok(())
    }
    pub fn get_counter_value(&self) -> anyhow::Result<i32> {
        let mut ctr: i16 = 0;
        unsafe {
            match esp_idf_sys::pcnt_get_counter_value(self.config.unit, &mut ctr as *mut c_short) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        let tot = self.acc.load(Ordering::Relaxed) * 100 + i32::from(ctr);
        Ok(tot)
    }
    pub fn setup_pcnt(&mut self) -> anyhow::Result<()> {
        unsafe {
            match esp_idf_sys::pcnt_unit_config(&self.config as *const pcnt_config_t) {
                ESP_OK | ESP_ERR_INVALID_STATE => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        self.config.pulse_gpio_num = self.b.pin();
        self.config.ctrl_gpio_num = self.a.pin();
        self.config.channel = pcnt_channel_1;
        self.config.pos_mode = pcnt_count_dec;
        self.config.neg_mode = pcnt_count_inc;
        unsafe {
            match esp_idf_sys::pcnt_unit_config(&self.config as *const pcnt_config_t) {
                ESP_OK | ESP_ERR_INVALID_STATE => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }

        unsafe {
            match esp_idf_sys::pcnt_counter_pause(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
            match esp_idf_sys::pcnt_counter_clear(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }

        unsafe {
            match esp_idf_sys::pcnt_isr_service_install(0) {
                ESP_OK | ESP_ERR_INVALID_STATE => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }

        esp!(unsafe {
            esp_idf_sys::pcnt_isr_handler_add(
                self.config.unit,
                Some(Self::irq_handler),
                self as *mut Self as *mut _,
            )
        })?;

        unsafe {
            match esp_idf_sys::pcnt_event_enable(self.config.unit, pcnt_evt_h_lim) {
                ESP_OK | ESP_ERR_INVALID_STATE => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
            match esp_idf_sys::pcnt_event_enable(self.config.unit, pcnt_evt_l_lim) {
                ESP_OK | ESP_ERR_INVALID_STATE => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }

        Ok(())
    }
    #[inline(always)]
    #[link_section = ".iram1.pcnt_srv"]
    unsafe extern "C" fn irq_handler(arg: *mut core::ffi::c_void) {
        let arg: &mut Esp32Encoder<A, B> = &mut *(arg as *mut _);
        let mut status = 0;
        esp_idf_sys::pcnt_get_event_status(arg.unit, &mut status as *mut c_ulong);
        if status & pcnt_evt_h_lim != 0 {
            arg.acc.fetch_add(1, Ordering::Relaxed);
        }
        if status & pcnt_evt_l_lim != 0 {
            arg.acc.fetch_sub(1, Ordering::Relaxed);
        }
    }
}
impl<A, B> Position for Esp32Encoder<A, B>
where
    A: InputPin + PinExt,
    B: InputPin + PinExt,
{
    fn position(&self) -> anyhow::Result<i32> {
        self.get_counter_value()
    }
}

pub struct MotorEncodedEsp32<Enc, A, B, PWM> {
    a: A,
    b: B,
    pwm: PWM,
    enc: Enc,
}

impl<Enc, A, B, PWM> MotorEncodedEsp32<Enc, A, B, PWM>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
    Enc: Position,
{
    pub fn new(a: A, b: B, pwm: PWM, enc: Enc) -> Self {
        MotorEncodedEsp32 { a, b, pwm, enc }
    }
}
impl<Enc, A, B, PWM> Motor for MotorEncodedEsp32<Enc, A, B, PWM>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
    Enc: Position,
{
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        if !(-1.0..=1.0).contains(&pct) {
            anyhow::bail!("power outside limit")
        }
        let max_duty = self.pwm.get_max_duty();
        if pct < 0.0 {
            self.a
                .set_high()
                .map_err(|_| anyhow::anyhow!("error setting A pin"))?;
            self.b
                .set_low()
                .map_err(|_| anyhow::anyhow!("error setting B pin"))?;
        } else {
            self.a
                .set_low()
                .map_err(|_| anyhow::anyhow!("error setting A pin"))?;
            self.b
                .set_high()
                .map_err(|_| anyhow::anyhow!("error setting B pin"))?;
        }
        info!(
            "Setting pwr {} translate to {} out of {}",
            &pct,
            ((max_duty as f64) * pct.abs()).floor() as u32,
            max_duty
        );
        self.pwm
            .set_duty(((max_duty as f64) * pct.abs()).floor() as u32);
        Ok(())
    }
    fn get_position(&mut self) -> anyhow::Result<i32> {
        self.enc.position()
    }
}
use std::collections::BTreeMap;

impl<Enc, A, B, PWM> Status for MotorEncodedEsp32<Enc, A, B, PWM>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
    Enc: Position,
{
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        let pos = self.enc.position()? as f64;
        bt.insert(
            "position".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(pos)),
            },
        );
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

pub struct MotorEsp32<A, B, PWM> {
    a: A,
    b: B,
    pwm: PWM,
}

impl<A, B, PWM> MotorEsp32<A, B, PWM>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
{
    pub fn new(a: A, b: B, pwm: PWM) -> Self {
        MotorEsp32 { a, b, pwm }
    }
    pub(crate) fn from_config(cfg: ConfigType, _: Option<BoardType>) -> anyhow::Result<MotorType> {
        match cfg {
            ConfigType::Static(cfg) => {
                if let Ok(pins) = cfg.get_attribute::<MotorConfig>("pins") {
                    if pins.a.is_some() && pins.b.is_some() {
                        use esp_idf_hal::units::FromValueType;
                        let pwm_tconf = TimerConfig::default().frequency(10.kHz().into());
                        let timer = LedcTimerDriver::new(
                            unsafe { esp_idf_hal::ledc::TIMER0::new() },
                            &pwm_tconf,
                        )?;
                        let pwm_pin = unsafe { AnyOutputPin::new(pins.pwm) };
                        let chan = PWMCHANNELS.lock().unwrap().take_next_channel()?;
                        let chan = match chan {
                            PwmChannel::C0(c) => LedcDriver::new(c, timer, pwm_pin)?,
                            PwmChannel::C1(c) => LedcDriver::new(c, timer, pwm_pin)?,
                            PwmChannel::C2(c) => LedcDriver::new(c, timer, pwm_pin)?,
                        };
                        return Ok(Arc::new(Mutex::new(MotorEsp32::new(
                            PinDriver::output(unsafe { AnyOutputPin::new(pins.a.unwrap()) })?,
                            PinDriver::output(unsafe { AnyOutputPin::new(pins.b.unwrap()) })?,
                            chan,
                        ))));
                    }
                }
            }
        }
        Err(anyhow::anyhow!("cannot build motor"))
    }
}
impl<A, B, PWM> Motor for MotorEsp32<A, B, PWM>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
{
    fn set_power(&mut self, pct: f64) -> anyhow::Result<()> {
        if !(-1.0..=1.0).contains(&pct) {
            anyhow::bail!("power outside limit")
        }
        let max_duty = self.pwm.get_max_duty();
        if pct < 0.0 {
            self.a
                .set_high()
                .map_err(|_| anyhow::anyhow!("error setting A pin"))?;
            self.b
                .set_low()
                .map_err(|_| anyhow::anyhow!("error setting B pin"))?;
        } else {
            self.a
                .set_low()
                .map_err(|_| anyhow::anyhow!("error setting A pin"))?;
            self.b
                .set_high()
                .map_err(|_| anyhow::anyhow!("error setting B pin"))?;
        }
        self.pwm
            .set_duty(((max_duty as f64) * pct.abs()).floor() as u32);
        Ok(())
    }
    fn get_position(&mut self) -> anyhow::Result<i32> {
        Ok(0)
    }
}

impl<A, B, PWM> Status for MotorEsp32<A, B, PWM>
where
    A: OutputPin + PinExt,
    B: OutputPin + PinExt,
    PWM: PwmPin<Duty = u32>,
{
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        let mut bt = BTreeMap::new();
        let pos = 0.0;
        bt.insert(
            "position".to_string(),
            prost_types::Value {
                kind: Some(prost_types::value::Kind::NumberValue(pos)),
            },
        );
        Ok(Some(prost_types::Struct { fields: bt }))
    }
}

/// Below is a first attempt as an approach to runtime configuration, the problem raise with runtime configuration is
/// enforcing single instance of any peripheral at any point in the program. For example say you have a Motor that uses pins 33,34 and 35 and
/// a AnalogReader that uses pin 35. This situation is wrong since two objects own pin 35. In embedded rust this is avoided by having any hardware
/// peripherals be singleton and leveraging the borrow checker so that single ownership rules are enforced. When dealing with runtime configuration,
/// the borrow checker cannot help us. We can however follow the singleton approach and wrap peripherals into options that will be 'taken out' when
/// something needs an hardware component. Following is an implementation of the proposed approach, a significant limitation is that the hardware can
/// only be taken once and can never be returned.
enum PwmChannel {
    C0(CHANNEL0),
    C1(CHANNEL1),
    C2(CHANNEL2),
}
struct PwmChannels {
    channel0: Option<CHANNEL0>,
    channel1: Option<CHANNEL1>,
    channel2: Option<CHANNEL2>,
}

impl PwmChannels {
    fn take_channel(&mut self, n: i32) -> anyhow::Result<PwmChannel> {
        match n {
            0 => {
                if self.channel0.is_some() {
                    let chan = self.channel0.take().unwrap();
                    return Ok(PwmChannel::C0(chan));
                }
                Err(anyhow::anyhow!("channel 0 already taken"))
            }
            1 => {
                if self.channel1.is_some() {
                    let chan = self.channel1.take().unwrap();
                    return Ok(PwmChannel::C1(chan));
                }
                Err(anyhow::anyhow!("channel 1 already taken"))
            }
            2 => {
                if self.channel2.is_some() {
                    let chan = self.channel2.take().unwrap();
                    return Ok(PwmChannel::C2(chan));
                }
                Err(anyhow::anyhow!("channel 2 already taken"))
            }
            _ => Err(anyhow::anyhow!("no channel {}", n)),
        }
    }
    fn take_next_channel(&mut self) -> anyhow::Result<PwmChannel> {
        for i in 0..2 {
            let ret = self.take_channel(i);
            if ret.is_ok() {
                return ret;
            }
        }
        Err(anyhow::anyhow!("no more channel available"))
    }
}

lazy_static::lazy_static! {
    static ref PWMCHANNELS: Mutex<PwmChannels> = Mutex::new(PwmChannels {
        channel0: Some(unsafe { CHANNEL0::new() }),
        channel1: Some(unsafe { CHANNEL1::new() }),
        channel2: Some(unsafe { CHANNEL2::new() }),
    });
}
