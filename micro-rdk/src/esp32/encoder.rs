use super::pin::PinExt;
use super::pulse_counter::{get_unit, isr_install, isr_remove_unit};

use crate::esp32::esp_idf_svc::hal::gpio::{AnyInputPin, Input, PinDriver};
use crate::esp32::esp_idf_svc::sys::pcnt_channel_edge_action_t_PCNT_CHANNEL_EDGE_ACTION_DECREASE as pcnt_count_dec;
use crate::esp32::esp_idf_svc::sys::pcnt_channel_edge_action_t_PCNT_CHANNEL_EDGE_ACTION_INCREASE as pcnt_count_inc;
use crate::esp32::esp_idf_svc::sys::pcnt_channel_level_action_t_PCNT_CHANNEL_LEVEL_ACTION_INVERSE as pcnt_mode_reverse;
use crate::esp32::esp_idf_svc::sys::pcnt_channel_level_action_t_PCNT_CHANNEL_LEVEL_ACTION_KEEP as pcnt_mode_keep;
use crate::esp32::esp_idf_svc::sys::pcnt_channel_t_PCNT_CHANNEL_0 as pcnt_channel_0;
use crate::esp32::esp_idf_svc::sys::pcnt_channel_t_PCNT_CHANNEL_1 as pcnt_channel_1;
use crate::esp32::esp_idf_svc::sys::pcnt_config_t;
use crate::esp32::esp_idf_svc::sys::pcnt_evt_type_t_PCNT_EVT_H_LIM as pcnt_evt_h_lim;
use crate::esp32::esp_idf_svc::sys::pcnt_evt_type_t_PCNT_EVT_L_LIM as pcnt_evt_l_lim;
use crate::esp32::esp_idf_svc::sys::{esp, ESP_OK};
use core::ffi::{c_short, c_ulong};

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::common::config::ConfigType;
use crate::common::encoder::{
    Encoder, EncoderError, EncoderPosition, EncoderPositionType, EncoderSupportedRepresentations,
    EncoderType,
};
use crate::common::registry::{ComponentRegistry, Dependency};
use crate::common::status::{Status, StatusError};
use crate::google;

use embedded_hal::digital::InputPin;

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
    pub unit: i32,
}

#[derive(DoCommand)]
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
    pub fn new(a: A, b: B) -> Result<Self, EncoderError> {
        let unit = get_unit();
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
        _: Vec<Dependency>,
    ) -> Result<EncoderType, EncoderError> {
        let pin_a_num = cfg.get_attribute::<i32>("a")?;

        let pin_b_num = cfg.get_attribute::<i32>("b")?;
        let a = match PinDriver::input(unsafe { AnyInputPin::new(pin_a_num) }) {
            Ok(a) => a,
            Err(err) => return Err(EncoderError::EncoderCodeError(err.code())),
        };
        let b = match PinDriver::input(unsafe { AnyInputPin::new(pin_b_num) }) {
            Ok(b) => b,
            Err(err) => return Err(EncoderError::EncoderCodeError(err.code())),
        };
        Ok(Arc::new(Mutex::new(Esp32Encoder::new(a, b)?)))
    }

    fn start(&self) -> Result<(), EncoderError> {
        unsafe {
            match crate::esp32::esp_idf_svc::sys::pcnt_counter_resume(self.config.unit) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
        }
        Ok(())
    }
    fn stop(&self) -> Result<(), EncoderError> {
        unsafe {
            match crate::esp32::esp_idf_svc::sys::pcnt_counter_pause(self.config.unit) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
        }
        Ok(())
    }
    fn reset(&self) -> Result<(), EncoderError> {
        self.stop()?;
        unsafe {
            match crate::esp32::esp_idf_svc::sys::pcnt_counter_clear(self.config.unit) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
        }
        self.pulse_counter.acc.store(0, Ordering::Relaxed);
        self.start()?;
        Ok(())
    }
    fn get_counter_value(&self) -> Result<i32, EncoderError> {
        let mut ctr: i16 = 0;
        unsafe {
            match crate::esp32::esp_idf_svc::sys::pcnt_get_counter_value(
                self.config.unit,
                &mut ctr as *mut c_short,
            ) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
        }
        let tot = self.pulse_counter.acc.load(Ordering::Relaxed) * 100 + i32::from(ctr);
        Ok(tot)
    }
    fn setup_pcnt(&mut self) -> Result<(), EncoderError> {
        unsafe {
            match crate::esp32::esp_idf_svc::sys::pcnt_unit_config(
                &self.config as *const pcnt_config_t,
            ) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
        }
        self.config.pulse_gpio_num = self.b.pin();
        self.config.ctrl_gpio_num = self.a.pin();
        self.config.channel = pcnt_channel_1;
        self.config.pos_mode = pcnt_count_dec;
        self.config.neg_mode = pcnt_count_inc;
        unsafe {
            match crate::esp32::esp_idf_svc::sys::pcnt_unit_config(
                &self.config as *const pcnt_config_t,
            ) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
        }

        unsafe {
            match crate::esp32::esp_idf_svc::sys::pcnt_counter_pause(self.config.unit) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
            match crate::esp32::esp_idf_svc::sys::pcnt_counter_clear(self.config.unit) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
        }

        isr_install()?;

        esp!(unsafe {
            crate::esp32::esp_idf_svc::sys::pcnt_isr_handler_add(
                self.config.unit,
                Some(Self::irq_handler),
                self.pulse_counter.as_mut() as *mut PulseStorage as *mut _,
            )
        })
        .map_err(|e| EncoderError::EncoderCodeError(e.code()))?;

        unsafe {
            match crate::esp32::esp_idf_svc::sys::pcnt_event_enable(
                self.config.unit,
                pcnt_evt_h_lim,
            ) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
            match crate::esp32::esp_idf_svc::sys::pcnt_event_enable(
                self.config.unit,
                pcnt_evt_l_lim,
            ) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
        }

        Ok(())
    }
    #[inline(always)]
    #[link_section = ".iram1.pcnt_srv"]
    unsafe extern "C" fn irq_handler(arg: *mut core::ffi::c_void) {
        let arg: &mut PulseStorage = &mut *(arg as *mut _);
        let mut status = 0;
        crate::esp32::esp_idf_svc::sys::pcnt_get_event_status(
            arg.unit,
            &mut status as *mut c_ulong,
        );
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
    fn get_position(
        &self,
        position_type: EncoderPositionType,
    ) -> Result<EncoderPosition, EncoderError> {
        match position_type {
            EncoderPositionType::TICKS | EncoderPositionType::UNSPECIFIED => {
                let count = self.get_counter_value()?;
                Ok(EncoderPositionType::TICKS.wrap_value(count as f32))
            }
            EncoderPositionType::DEGREES => Err(EncoderError::EncoderAngularNotSupported),
        }
    }
    fn reset_position(&mut self) -> Result<(), EncoderError> {
        self.reset()
    }
}

impl<A, B> Status for Esp32Encoder<A, B>
where
    A: InputPin + PinExt,
    B: InputPin + PinExt,
{
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

impl<A, B> Drop for Esp32Encoder<A, B> {
    fn drop(&mut self) {
        isr_remove_unit();
    }
}
