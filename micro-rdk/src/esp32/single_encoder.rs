use embedded_hal::digital::InputPin;

use super::pin::PinExt;
use super::pulse_counter::{get_unit, isr_install, isr_installed, isr_remove_unit};

use crate::common::config::{AttributeError, ConfigType};
use crate::common::encoder::{
    Direction, Encoder, EncoderError, EncoderPosition, EncoderPositionType,
    EncoderSupportedRepresentations, EncoderType, SingleEncoder,
};
use crate::common::registry::{ComponentRegistry, Dependency};
use crate::google;

use crate::esp32::esp_idf_svc::hal::gpio::{AnyInputPin, PinDriver};
use crate::esp32::esp_idf_svc::sys::pcnt_channel_edge_action_t_PCNT_CHANNEL_EDGE_ACTION_DECREASE as pcnt_count_dec;
use crate::esp32::esp_idf_svc::sys::pcnt_channel_edge_action_t_PCNT_CHANNEL_EDGE_ACTION_INCREASE as pcnt_count_inc;
use crate::esp32::esp_idf_svc::sys::pcnt_channel_level_action_t_PCNT_CHANNEL_LEVEL_ACTION_KEEP as pcnt_mode_keep;
use crate::esp32::esp_idf_svc::sys::pcnt_channel_t_PCNT_CHANNEL_0 as pcnt_channel_0;
use crate::esp32::esp_idf_svc::sys::pcnt_config_t;
use crate::esp32::esp_idf_svc::sys::pcnt_evt_type_t_PCNT_EVT_H_LIM as pcnt_evt_h_lim;
use crate::esp32::esp_idf_svc::sys::pcnt_evt_type_t_PCNT_EVT_L_LIM as pcnt_evt_l_lim;
use crate::esp32::esp_idf_svc::sys::{esp, ESP_OK};
use core::ffi::{c_short, c_ulong};

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crate::common::status::{Status, StatusError};

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_encoder("single", &Esp32SingleEncoder::from_config)
        .is_err()
    {
        log::error!("single model is already registered")
    }
}

// TODO: Make configurable?
const MAX_GLITCH_MICROSEC: u16 = 1;

// TODO: Move this type to common once we have a single encoder
// implementation for another board
pub(crate) type SingleEncoderType = Arc<Mutex<dyn SingleEncoder>>;

struct PulseStorage {
    acc: Arc<AtomicI32>,
    unit: i32,
    moving_forwards: Arc<AtomicBool>,
}

#[derive(DoCommand)]
pub struct Esp32SingleEncoder {
    pulse_counter: Box<PulseStorage>,
    config: pcnt_config_t,
    dir: Direction,
}

impl Esp32SingleEncoder {
    pub fn new(encoder_pin: impl InputPin + PinExt, dir_flip: bool) -> Result<Self, EncoderError> {
        let unit = get_unit();
        log::debug!("pulse counter unit received in single encoder: {:?}", unit);
        let pcnt = Box::new(PulseStorage {
            acc: Arc::new(AtomicI32::new(0)),
            unit,
            moving_forwards: Arc::new(AtomicBool::new(true)),
        });
        let mut enc = Esp32SingleEncoder {
            pulse_counter: pcnt,
            config: pcnt_config_t {
                pulse_gpio_num: encoder_pin.pin(),
                ctrl_gpio_num: -1,
                pos_mode: pcnt_count_inc,
                neg_mode: pcnt_count_inc,
                lctrl_mode: pcnt_mode_keep,
                hctrl_mode: pcnt_mode_keep,
                counter_h_lim: 100,
                counter_l_lim: -100,
                channel: pcnt_channel_0,
                unit,
            },
            dir: Direction::StoppedForwards,
        };
        if dir_flip {
            enc.dir = Direction::StoppedBackwards
        }
        enc.setup_pcnt()?;
        enc.start()?;
        Ok(enc)
    }

    pub(crate) fn from_config(
        cfg: ConfigType,
        _: Vec<Dependency>,
    ) -> Result<EncoderType, EncoderError> {
        let pin_num = cfg.get_attribute::<i32>("pin")?;
        let pin = PinDriver::input(unsafe { AnyInputPin::new(pin_num) })
            .map_err(|err| EncoderError::EncoderCodeError(err.code()))?;
        let dir_flip = match cfg.get_attribute::<bool>("dir_flip") {
            Ok(flip) => flip,
            Err(err) => match err {
                AttributeError::KeyNotFound(_) => false,
                _ => {
                    return Err(EncoderError::EncoderConfigAttributeError(err));
                }
            },
        };
        Ok(Arc::new(Mutex::new(Esp32SingleEncoder::new(
            pin, dir_flip,
        )?)))
    }

    pub fn start(&self) -> Result<(), EncoderError> {
        unsafe {
            match crate::esp32::esp_idf_svc::sys::pcnt_counter_resume(self.config.unit) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
        }
        Ok(())
    }
    pub fn stop(&self) -> Result<(), EncoderError> {
        unsafe {
            match crate::esp32::esp_idf_svc::sys::pcnt_counter_pause(self.config.unit) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
        }
        Ok(())
    }
    pub fn reset(&self) -> Result<(), EncoderError> {
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
    pub fn get_counter_value(&self) -> Result<i32, EncoderError> {
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
        let sign: i32 = match self.dir {
            Direction::Forwards | Direction::StoppedForwards => 1,
            Direction::Backwards | Direction::StoppedBackwards => -1,
        };
        let tot = self.pulse_counter.acc.load(Ordering::Relaxed) * 100 + (i32::from(ctr) * sign);
        Ok(tot)
    }
    pub fn setup_pcnt(&mut self) -> Result<(), EncoderError> {
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
        .map_err(|err| EncoderError::EncoderCodeError(err.code()))?;

        unsafe {
            match crate::esp32::esp_idf_svc::sys::pcnt_set_filter_value(
                self.config.unit,
                MAX_GLITCH_MICROSEC * 80,
            ) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
            match crate::esp32::esp_idf_svc::sys::pcnt_filter_enable(self.config.unit) {
                ESP_OK => {}
                err => return Err(EncoderError::EncoderCodeError(err)),
            }
        }

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
        if arg.moving_forwards.load(Ordering::Relaxed) {
            if status & pcnt_evt_h_lim != 0 {
                arg.acc.fetch_add(1, Ordering::SeqCst);
            }
        } else if status & pcnt_evt_l_lim != 0 {
            arg.acc.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Encoder for Esp32SingleEncoder {
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

impl SingleEncoder for Esp32SingleEncoder {
    fn get_direction(&self) -> Result<Direction, EncoderError> {
        Ok(self.dir)
    }
    fn set_direction(&mut self, dir: Direction) -> Result<(), EncoderError> {
        let mut reconfigure = false;
        match dir {
            Direction::Forwards | Direction::StoppedForwards => {
                if !self.dir.is_forwards() {
                    self.config.neg_mode = pcnt_count_inc;
                    self.config.pos_mode = pcnt_count_inc;
                    reconfigure = true;
                    self.pulse_counter
                        .moving_forwards
                        .store(true, Ordering::Relaxed);
                }
            }
            Direction::Backwards | Direction::StoppedBackwards => {
                if self.dir.is_forwards() {
                    self.config.neg_mode = pcnt_count_dec;
                    self.config.pos_mode = pcnt_count_dec;
                    reconfigure = true;
                    self.pulse_counter
                        .moving_forwards
                        .store(false, Ordering::Relaxed);
                }
            }
        };
        self.dir = dir;
        let isr_is_installed = isr_installed();
        if reconfigure && isr_is_installed {
            unsafe {
                match crate::esp32::esp_idf_svc::sys::pcnt_counter_pause(self.config.unit) {
                    ESP_OK => {}
                    err => return Err(EncoderError::EncoderCodeError(err)),
                }

                match crate::esp32::esp_idf_svc::sys::pcnt_unit_config(
                    &self.config as *const pcnt_config_t,
                ) {
                    ESP_OK => {}
                    err => return Err(EncoderError::EncoderCodeError(err)),
                }
            }
            unsafe {
                match crate::esp32::esp_idf_svc::sys::pcnt_set_filter_value(
                    self.config.unit,
                    MAX_GLITCH_MICROSEC * 80,
                ) {
                    ESP_OK => {}
                    err => return Err(EncoderError::EncoderCodeError(err)),
                }
                match crate::esp32::esp_idf_svc::sys::pcnt_filter_enable(self.config.unit) {
                    ESP_OK => {}
                    err => return Err(EncoderError::EncoderCodeError(err)),
                }
            }

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
                match crate::esp32::esp_idf_svc::sys::pcnt_counter_resume(self.config.unit) {
                    ESP_OK => {}
                    err => return Err(EncoderError::EncoderCodeError(err)),
                }
            }
        }
        Ok(())
    }
}

impl Status for Esp32SingleEncoder {
    fn get_status(&self) -> Result<Option<google::protobuf::Struct>, StatusError> {
        Ok(Some(google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}

impl Drop for Esp32SingleEncoder {
    fn drop(&mut self) {
        if isr_installed() {
            unsafe {
                crate::esp32::esp_idf_svc::sys::pcnt_isr_handler_remove(self.config.unit);
            }
            isr_remove_unit();
        }
    }
}
