use super::pin::PinExt;
use super::pulse_counter::{get_unit, isr_install, isr_remove_unit};

use core::ffi::{c_short, c_ulong};
use esp_idf_hal::gpio::{AnyInputPin, Input, PinDriver};
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
use espsys::{esp, EspError, ESP_OK};

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::common::board::BoardType;
use crate::common::config::ConfigType;
use crate::common::encoder::{
    Encoder, EncoderPosition, EncoderPositionType, EncoderSupportedRepresentations, EncoderType,
};
use crate::common::registry::ComponentRegistry;
use crate::common::status::Status;

use embedded_hal::digital::v2::InputPin;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_encoder(
            "incremental",
            &Esp32Encoder::<
                PinDriver<'_, AnyInputPin, Input>,
                PinDriver<'_, AnyInputPin, Input>,
            >::from_config,
        )
        .is_err()
    {
        log::error!("incremental model is already registered")
    }
}

pub struct PulseStorage {
    pub acc: Arc<AtomicI32>,
    pub unit: u32,
}

pub struct Esp32Encoder<A, B> {
    pulse_counter: Box<PulseStorage>,
    config: pcnt_config_t,
    a: A,
    b: B,
}

impl<A, B> Esp32Encoder<A, B>
where
    A: InputPin + PinExt,
    B: InputPin + PinExt,
{
    pub fn new(a: A, b: B) -> anyhow::Result<Self> {
        let unit = get_unit()?;
        let pcnt = Box::new(PulseStorage {
            acc: Arc::new(AtomicI32::new(0)),
            unit,
        });
        let mut enc = Esp32Encoder {
            pulse_counter: pcnt,
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
                unit,
            },
            a,
            b,
        };
        enc.setup_pcnt()?;
        enc.start()?;
        Ok(enc)
    }

    pub(crate) fn from_config(
        cfg: ConfigType,
        _: Option<BoardType>,
    ) -> anyhow::Result<EncoderType> {
        let pin_a_num = match cfg.get_attribute::<i32>("a") {
            Ok(num) => num,
            Err(_) => return Err(anyhow::anyhow!("cannot build encoder, need 'a' pin")),
        };
        let pin_b_num = match cfg.get_attribute::<i32>("b") {
            Ok(num) => num,
            Err(_) => return Err(anyhow::anyhow!("cannot build encoder, need 'b' pin")),
        };
        let a = match PinDriver::input(unsafe { AnyInputPin::new(pin_a_num) }) {
            Ok(a) => a,
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "cannot build encoder, could not initialize pin {:?} as pin 'a': {:?}",
                    pin_a_num,
                    err
                ))
            }
        };
        let b = match PinDriver::input(unsafe { AnyInputPin::new(pin_b_num) }) {
            Ok(b) => b,
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "cannot build encoder, could not initialize pin {:?} as pin 'b': {:?}",
                    pin_b_num,
                    err
                ))
            }
        };
        Ok(Arc::new(Mutex::new(Esp32Encoder::new(a, b)?)))
    }

    fn start(&self) -> anyhow::Result<()> {
        unsafe {
            match esp_idf_sys::pcnt_counter_resume(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        Ok(())
    }
    fn stop(&self) -> anyhow::Result<()> {
        unsafe {
            match esp_idf_sys::pcnt_counter_pause(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        Ok(())
    }
    fn reset(&self) -> anyhow::Result<()> {
        self.stop()?;
        unsafe {
            match esp_idf_sys::pcnt_counter_clear(self.config.unit) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        self.pulse_counter.acc.store(0, Ordering::Relaxed);
        self.start()?;
        Ok(())
    }
    fn get_counter_value(&self) -> anyhow::Result<i32> {
        let mut ctr: i16 = 0;
        unsafe {
            match esp_idf_sys::pcnt_get_counter_value(self.config.unit, &mut ctr as *mut c_short) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }
        let tot = self.pulse_counter.acc.load(Ordering::Relaxed) * 100 + i32::from(ctr);
        Ok(tot)
    }
    fn setup_pcnt(&mut self) -> anyhow::Result<()> {
        unsafe {
            match esp_idf_sys::pcnt_unit_config(&self.config as *const pcnt_config_t) {
                ESP_OK => {}
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
                ESP_OK => {}
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

        isr_install()?;

        esp!(unsafe {
            esp_idf_sys::pcnt_isr_handler_add(
                self.config.unit,
                Some(Self::irq_handler),
                self.pulse_counter.as_mut() as *mut PulseStorage as *mut _,
            )
        })?;

        unsafe {
            match esp_idf_sys::pcnt_event_enable(self.config.unit, pcnt_evt_h_lim) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
            match esp_idf_sys::pcnt_event_enable(self.config.unit, pcnt_evt_l_lim) {
                ESP_OK => {}
                err => return Err(EspError::from(err).unwrap().into()),
            }
        }

        Ok(())
    }
    #[inline(always)]
    #[link_section = ".iram1.pcnt_srv"]
    unsafe extern "C" fn irq_handler(arg: *mut core::ffi::c_void) {
        let arg: &mut PulseStorage = &mut *(arg as *mut _);
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

impl<A, B> Encoder for Esp32Encoder<A, B>
where
    A: InputPin + PinExt,
    B: InputPin + PinExt,
{
    fn get_properties(&mut self) -> EncoderSupportedRepresentations {
        EncoderSupportedRepresentations {
            ticks_count_supported: true,
            angle_degrees_supported: false,
        }
    }
    fn get_position(&self, position_type: EncoderPositionType) -> anyhow::Result<EncoderPosition> {
        match position_type {
            EncoderPositionType::TICKS | EncoderPositionType::UNSPECIFIED => {
                let count = self.get_counter_value()?;
                Ok(EncoderPositionType::TICKS.wrap_value(count as f32))
            }
            EncoderPositionType::DEGREES => {
                anyhow::bail!("Esp32Encoder does not support returning angular position")
            }
        }
    }
    fn reset_position(&mut self) -> anyhow::Result<()> {
        self.reset()
    }
}

impl<A, B> Status for Esp32Encoder<A, B>
where
    A: InputPin + PinExt,
    B: InputPin + PinExt,
{
    fn get_status(&self) -> anyhow::Result<Option<prost_types::Struct>> {
        Ok(Some(prost_types::Struct {
            fields: BTreeMap::new(),
        }))
    }
}

impl<A, B> Drop for Esp32Encoder<A, B> {
    fn drop(&mut self) {
        isr_remove_unit();
    }
}
