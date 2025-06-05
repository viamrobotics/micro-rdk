use std::{
    cell::UnsafeCell,
    ffi::c_void,
    ops::Shl,
    ptr::addr_of,
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};

use chrono::{DateTime, TimeDelta};
use micro_rdk::{
    common::{
        config::{AttributeError, ConfigType, Kind},
        i2c::I2cHandleType,
        registry::{self, ComponentRegistry, Dependency, RegistryError},
        sensor::{GenericReadingsResult, Readings, Sensor, SensorError, SensorType},
    },
    esp32::esp_idf_svc::{hal::ulp::UlpDriver, sys::EspError},
    DoCommand,
};
use micro_rdk::{
    google::protobuf::{value, Timestamp, Value},
    proto::app::data_sync::v1::{MimeType, SensorData, SensorMetadata},
};
use thiserror::Error;

// contains the address of global variables defined in the ULP program
include!(concat!(env!("OUT_DIR"), "/ulp.rs"));

static ULP_PROGRAM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/ulp.bin"));

struct UnsafeRtcMemory<T>(UnsafeCell<T>);
unsafe impl<T> Sync for UnsafeRtcMemory<T> {}

static SAMPLE_ARRAY_SIZE: usize = 1024;
#[unsafe(link_section = ".rtc.force_slow")]
static SAMPLE_ARRAY: UnsafeRtcMemory<[u32; SAMPLE_ARRAY_SIZE]> =
    UnsafeRtcMemory(UnsafeCell::new([0xFFFFFFFF_u32; SAMPLE_ARRAY_SIZE]));
#[unsafe(link_section = ".rtc.force_slow")]
static RTC_TIME_START: UnsafeRtcMemory<i64> = UnsafeRtcMemory(UnsafeCell::new(0_i64));

#[derive(Error, Debug)]
pub enum BME280Error {
    #[error("pin {0} is not an RTC pin")]
    BME280UlpInvalidRTCPin(i32),
    #[error("pin {0} is not valid pin")]
    BME280UlpInvalidPin(i32),
    #[error("error configuring driver {0}")]
    BME280I2cDriverError(EspError),
    #[error("cannot instantiate UlpDriver: {0}")]
    BME280UlpDriverError(EspError),
    #[error("error loading Ulp Program: {0}")]
    BME280UlpLoadError(EspError),
    #[error("unable to configure rtc pin {0} cause {1}")]
    BME280UlpRtcPinConfigError(i32, EspError),
    #[error("cannot start ulp program {0}")]
    BME280UlpCannotStart(EspError),
    #[error("invalid config value")]
    BME280InvalidConfigValue,
}

struct ULPResultsMemory<'a> {
    memory: &'a mut [u32],
    offset: usize,
}

impl<'a> From<&'a UnsafeRtcMemory<[u32; SAMPLE_ARRAY_SIZE]>> for ULPResultsMemory<'a> {
    fn from(value: &'a UnsafeRtcMemory<[u32; SAMPLE_ARRAY_SIZE]>) -> Self {
        let ptr = unsafe { &mut *value.0.get() };
        Self {
            memory: ptr,
            offset: 0,
        }
    }
}

impl Iterator for ULPResultsMemory<'_> {
    type Item = RawMeasurement;
    fn next(&mut self) -> Option<Self::Item> {
        let next_offset = self.offset + 8; // 8 words per measurement
        if next_offset > self.memory.len() || self.memory[self.offset] == 0xFFFFFFFF {
            return None;
        }
        let raw: [u8; 8] = self.memory[self.offset..next_offset]
            .iter()
            .map(|v| (*v & 0xFF) as u8)
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap();
        // not exactly correct because one could read some measurement (less than stored) but this code
        // will make it impossible to read again unless user keep the iterator around
        self.memory[self.offset] = 0xFFFFFFFF;
        self.offset = next_offset;
        Some(raw.into())
    }
}

#[allow(clippy::upper_case_acronyms)]
struct ULP {
    ulp: UlpDriver<'static>,
    rtc_pin_scl: u32,
    rtc_pin_sda: u32,
    cfg: ULPConfig,
}

struct ULPConfig {
    pin_scl: i32,
    pin_sda: i32,
    period: Duration,
    sample: u32,
}

pub fn register_models(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
    registry.register_sensor("bme280_ulp", &BME280::from_config)
}

impl TryFrom<&Kind> for ULPConfig {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        if let Kind::StructValue(config) = value {
            let pin_scl = config
                .get("ulp_scl_pin")
                .ok_or(AttributeError::KeyNotFound("ulp_scl_pin".to_owned()))
                .and_then(|kind| {
                    if let Kind::NumberValue(value) = kind {
                        Ok(*value as i32)
                    } else {
                        Err(AttributeError::ValidationError(
                            "ulp_pin_scl should be a number".to_owned(),
                        ))
                    }
                })?;
            let pin_sda = config
                .get("ulp_sda_pin")
                .ok_or(AttributeError::KeyNotFound("ulp_sda_pin".to_owned()))
                .and_then(|kind| {
                    if let Kind::NumberValue(value) = kind {
                        Ok(*value as i32)
                    } else {
                        Err(AttributeError::ValidationError(
                            "ulp_pin_sda should be a number".to_owned(),
                        ))
                    }
                })?;
            let period = config
                .get("frequency_hz")
                .ok_or(AttributeError::KeyNotFound("frequency_hz".to_owned()))
                .and_then(|kind| {
                    if let Kind::NumberValue(value) = kind {
                        if *value == 0.0 {
                            Err(AttributeError::ValidationError(
                                "frequency_hz should not be 0".to_owned(),
                            ))
                        } else {
                            Ok(Duration::from_secs_f64(1.0 / (*value)))
                        }
                    } else {
                        Err(AttributeError::ValidationError(
                            "frequency_hz should be a number".to_owned(),
                        ))
                    }
                })?;
            let cfg_sample = config
                .get("sample")
                .ok_or(AttributeError::KeyNotFound("sample".to_owned()))
                .and_then(|kind| {
                    if let Kind::NumberValue(smp) = kind {
                        if *smp <= 0.0 {
                            Err(AttributeError::ValidationError(
                                "sample should ne strictly positive".to_owned(),
                            ))
                        } else {
			    if (*smp as u32) > (SAMPLE_ARRAY_SIZE as u32)/8 {
				log::warn!("bme280 ulp setting max number of samples to {}", SAMPLE_ARRAY_SIZE/8);
			    }
                            Ok((*smp as u32).min((SAMPLE_ARRAY_SIZE as u32)/8))
                        }
                    } else {
                        Err(AttributeError::ValidationError(
                            "sample should be a number".to_owned(),
                        ))
                    }
                }).inspect_err(|err| log::warn!("sample number wasn't set or is invalid reason : {} ulp will collect {} samples in deep sleep", err, SAMPLE_ARRAY_SIZE/8)).unwrap_or((SAMPLE_ARRAY_SIZE as u32)/8);
            Ok(Self {
                pin_scl,
                pin_sda,
                period,
                sample: cfg_sample,
            })
        } else {
            Err(AttributeError::ValidationError(
                "ulp_config should be a struct".to_string(),
            ))
        }
    }
}

impl ULP {
    fn new(cfg: ULPConfig) -> Result<Self, BME280Error> {
        let rtc_pin_scl = unsafe {
            micro_rdk::esp32::esp_idf_svc::sys::rtc_gpio_is_valid_gpio(cfg.pin_scl)
                .then(|| micro_rdk::esp32::esp_idf_svc::sys::rtc_io_number_get(cfg.pin_scl))
                .ok_or(BME280Error::BME280UlpInvalidRTCPin(cfg.pin_scl))
        }? as u32;
        let rtc_pin_sda = unsafe {
            micro_rdk::esp32::esp_idf_svc::sys::rtc_gpio_is_valid_gpio(cfg.pin_sda)
                .then(|| micro_rdk::esp32::esp_idf_svc::sys::rtc_io_number_get(cfg.pin_sda))
                .ok_or(BME280Error::BME280UlpInvalidRTCPin(cfg.pin_sda))
        }? as u32;

        let ulp = unsafe { micro_rdk::esp32::esp_idf_svc::hal::ulp::ULP::new() };
        let mut ulp = UlpDriver::new(ulp).map_err(BME280Error::BME280UlpDriverError)?;
        unsafe { ulp.load(ULP_PROGRAM) }.map_err(BME280Error::BME280UlpLoadError)?;

        Ok(Self {
            ulp,
            rtc_pin_scl,
            rtc_pin_sda,
            cfg,
        })
    }
    // 31      28 27   23 22   18 17     9
    // | OpCode |  High |  Low |  Unused | Addr
    // ESP32 Technical reference manual 30.4.13
    fn ulp_fsm_reg_rd_inst(rtc_reg: u32, low_bit: u32, bit_width: u32) -> u32 {
        use micro_rdk::esp32::esp_idf_svc::sys::DR_REG_RTCCNTL_BASE;
        (2_u32 << 28)
            | (((low_bit + bit_width - 1) & ((1 << 5) - 1)) << 23)
            | (((low_bit) & ((1 << 5) - 1)) << 18)
            | ((rtc_reg - DR_REG_RTCCNTL_BASE) / 4)
    }
    // 31      28 27   23 22   18 17  10 9
    // | OpCode |  High |  Low |  Data  | Addr
    // ESP32 Technical reference manual 30.4.14
    fn ulp_fsm_reg_wr_inst(rtc_reg: u32, low_bit: u32, bit_width: u32, data: u32) -> u32 {
        use micro_rdk::esp32::esp_idf_svc::sys::DR_REG_RTCCNTL_BASE;
        (1_u32 << 28)
            | (((low_bit + bit_width - 1) & ((1 << 5) - 1)) << 23)
            | (((low_bit) & ((1 << 5) - 1)) << 18)
            | (((data) & ((1 << 8) - 1)) << 10)
            | ((rtc_reg - DR_REG_RTCCNTL_BASE) / 4)
    }

    fn start_ulp(&mut self, mut cfg: Configuration) -> Result<(), BME280Error> {
        use micro_rdk::esp32::esp_idf_svc::sys::{
            esp, rtc_gpio_init,
            rtc_gpio_mode_t_RTC_GPIO_MODE_INPUT_OUTPUT as RTC_GPIO_MODE_INPUT_OUTPUT,
            rtc_gpio_set_direction, ulp_set_wakeup_period, RTC_GPIO_ENABLE_W1TC_REG,
            RTC_GPIO_ENABLE_W1TC_S, RTC_GPIO_ENABLE_W1TS_REG, RTC_GPIO_ENABLE_W1TS_S,
            RTC_GPIO_IN_NEXT_S, RTC_GPIO_IN_REG,
        };
        // configure pin so they can be accessed by ULP. Note that any pin should not be changed or used afterwards
        esp!(unsafe { rtc_gpio_init(self.cfg.pin_scl) })
            .and_then(|_| {
                esp!(unsafe {
                    rtc_gpio_set_direction(self.cfg.pin_scl, RTC_GPIO_MODE_INPUT_OUTPUT)
                })
            })
            .map_err(|err| BME280Error::BME280UlpRtcPinConfigError(self.cfg.pin_scl, err))?;
        esp!(unsafe { rtc_gpio_init(self.cfg.pin_sda) })
            .and_then(|_| {
                esp!(unsafe {
                    rtc_gpio_set_direction(self.cfg.pin_sda, RTC_GPIO_MODE_INPUT_OUTPUT)
                })
            })
            .map_err(|err| BME280Error::BME280UlpRtcPinConfigError(self.cfg.pin_sda, err))?;

        // config NOP reserved insts
        unsafe {
            *set_SCL = Self::ulp_fsm_reg_wr_inst(
                RTC_GPIO_ENABLE_W1TC_REG,
                RTC_GPIO_ENABLE_W1TC_S + self.rtc_pin_scl,
                1,
                1,
            );
            *clear_SCL = Self::ulp_fsm_reg_wr_inst(
                RTC_GPIO_ENABLE_W1TS_REG,
                RTC_GPIO_ENABLE_W1TS_S + self.rtc_pin_scl,
                1,
                1,
            );
            *read_SCL = Self::ulp_fsm_reg_rd_inst(
                RTC_GPIO_IN_REG,
                RTC_GPIO_IN_NEXT_S + self.rtc_pin_scl,
                1,
            );
            *set_SDA = Self::ulp_fsm_reg_wr_inst(
                RTC_GPIO_ENABLE_W1TC_REG,
                RTC_GPIO_ENABLE_W1TC_S + self.rtc_pin_sda,
                1,
                1,
            );
            *clear_SDA = Self::ulp_fsm_reg_wr_inst(
                RTC_GPIO_ENABLE_W1TS_REG,
                RTC_GPIO_ENABLE_W1TS_S + self.rtc_pin_sda,
                1,
                1,
            );
            *read_SDA = Self::ulp_fsm_reg_rd_inst(
                RTC_GPIO_IN_REG,
                RTC_GPIO_IN_NEXT_S + self.rtc_pin_sda,
                1,
            );

            *data_offset = ((addr_of!(SAMPLE_ARRAY) as *mut c_void)
                .offset_from(micro_rdk::esp32::esp_idf_svc::hal::ulp::ULP::MEM_START)
                as u32)
                / 4;

            *sample = self.cfg.sample; // each measurement takes 8 word of memory

            (*RTC_TIME_START.0.get()) = chrono::Local::now().fixed_offset().timestamp_millis();

            // ULP program starts as soon as start is called doing the first measurement, it will wake the cpu right
            // after the last sample was gathered. There the rendez-vous can be calculated as (sample - 1)*(period) + now
            log::info!(
                "ULP is configured to gather {} samples every {} s will wake up at {}",
                self.cfg.sample,
                self.cfg.period.as_secs(),
                DateTime::from_timestamp_millis(*(RTC_TIME_START.0.get()))
                    .unwrap()
                    .checked_add_signed(TimeDelta::milliseconds(
                        (self.cfg.period.as_millis() * ((self.cfg.sample - 1) as u128)) as i64
                    ))
                    .unwrap()
            );

            cfg.mode = Mode::FORCED;
            *ulp_ctrl_meas = cfg.ctrl_meas_value() as u32;
            *ulp_ctrl_hum = cfg.ctrl_hum_value() as u32;
            *ulp_ctrl_config = cfg.ctrl_config_value() as u32;

            ulp_set_wakeup_period(0, self.cfg.period.as_micros() as u32);

            self.ulp
                .start(entry.offset_from(
                    micro_rdk::esp32::esp_idf_svc::hal::ulp::ULP::MEM_START as *const u32,
                ) as *const u32)
                .map_err(BME280Error::BME280UlpCannotStart)?;
        }

        Ok(())
    }
}

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy)]
enum Register {
    CALIB_DATA1 = 0x88,
    CALIB_DATA2 = 0xE1,
    ID = 0xD0,
    RESET = 0xE0,
    CTRL_HUM = 0xF2,
    STATUS = 0xF3,
    CTRL_MEAS = 0xF4,
    CONFIG = 0xF5,
    PRES_REG = 0xF7,
    TEMP_REG = 0xFA,
    HUM_REG = 0xFD,
}

impl From<Register> for u8 {
    fn from(value: Register) -> Self {
        value as u8
    }
}

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Default, Debug)]
enum OverSampling {
    SKIPPED = 0,
    OVERSAMPLINGX1 = 0b001,
    OVERSAMPLINGX2 = 0b010,
    OVERSAMPLINGX4 = 0b011,
    OVERSAMPLINGX8 = 0b100,
    #[default]
    OVERSAMPLINGX16 = 0b101,
}
impl TryFrom<&Kind> for OverSampling {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        if let Kind::NumberValue(value) = value {
            return match *value as u8 {
                0 => Ok(Self::SKIPPED),
                0b001 => Ok(Self::OVERSAMPLINGX1),
                0b010 => Ok(Self::OVERSAMPLINGX2),
                0b011 => Ok(Self::OVERSAMPLINGX4),
                0b100 => Ok(Self::OVERSAMPLINGX8),
                0b101 => Ok(Self::OVERSAMPLINGX16),
                _ => Err(AttributeError::ValidationError(format!(
                    "not a valid oversampling value {}",
                    value
                ))),
            };
        }
        Err(AttributeError::ValidationError(
            "oversampling value should be a number".to_string(),
        ))
    }
}

impl From<OverSampling> for u8 {
    fn from(value: OverSampling) -> Self {
        value as u8
    }
}
impl Shl<u8> for OverSampling {
    type Output = u8;
    fn shl(self, rhs: u8) -> Self::Output {
        (self as u8) << rhs
    }
}

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Default, Debug)]
enum Mode {
    SLEEP = 0,
    FORCED = 0b01,
    #[default]
    NORMAL = 0b11,
}

impl From<Mode> for u8 {
    fn from(value: Mode) -> Self {
        value as u8
    }
}

impl TryFrom<&Kind> for Mode {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        if let Kind::NumberValue(value) = value {
            return match *value as u8 {
                0 => Ok(Self::SLEEP),
                0b01 => Ok(Self::FORCED),
                0b11 => Ok(Self::NORMAL),
                _ => Err(AttributeError::ValidationError(format!(
                    "not a valid mode value {}",
                    value
                ))),
            };
        }
        Err(AttributeError::ValidationError(
            "mode value should be a number".to_string(),
        ))
    }
}

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Default, Debug)]
enum Standby {
    #[default]
    STANDBY_0_5_MS = 0b000,
    STANDBY_62_5_MS = 0b001,
    STANDBY_125_0_MS = 0b010,
    STANDBY_250_0_MS = 0b011,
    STANDBY_500_0_MS = 0b100,
    STANDBY_1000_0_MS = 0b101,
    STANDBY_10_0_MS = 0b110,
    STANDBY_20_0_MS = 0b111,
}

impl From<Standby> for u8 {
    fn from(value: Standby) -> Self {
        value as u8
    }
}

impl TryFrom<&Kind> for Standby {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        if let Kind::NumberValue(value) = value {
            return match *value as u8 {
                0b000 => Ok(Self::STANDBY_0_5_MS),
                0b001 => Ok(Self::STANDBY_62_5_MS),
                0b010 => Ok(Self::STANDBY_125_0_MS),
                0b011 => Ok(Self::STANDBY_250_0_MS),
                0b100 => Ok(Self::STANDBY_500_0_MS),
                0b101 => Ok(Self::STANDBY_1000_0_MS),
                0b110 => Ok(Self::STANDBY_10_0_MS),
                0b111 => Ok(Self::STANDBY_20_0_MS),
                _ => Err(AttributeError::ValidationError(format!(
                    "not a valid standby value {}",
                    value
                ))),
            };
        }
        Err(AttributeError::ValidationError(
            "standby value should be a number".to_string(),
        ))
    }
}

impl Shl<u8> for Standby {
    type Output = u8;
    fn shl(self, rhs: u8) -> Self::Output {
        (self as u8) << rhs
    }
}

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Default, Debug)]
enum Filter {
    #[default]
    OFF = 0b000,
    FILTER_2 = 0b001,
    FILTER_4 = 0b010,
    FILTER_8 = 0b011,
    FILTER_16 = 0b100,
}

impl From<Filter> for u8 {
    fn from(value: Filter) -> Self {
        value as u8
    }
}
impl TryFrom<&Kind> for Filter {
    type Error = AttributeError;
    fn try_from(value: &Kind) -> Result<Self, Self::Error> {
        if let Kind::NumberValue(value) = value {
            return match *value as u8 {
                0b000 => Ok(Self::OFF),
                0b001 => Ok(Self::FILTER_2),
                0b010 => Ok(Self::FILTER_4),
                0b011 => Ok(Self::FILTER_8),
                0b100 => Ok(Self::FILTER_16),
                _ => Err(AttributeError::ValidationError(format!(
                    "not a valid filter value {}",
                    value
                ))),
            };
        }
        Err(AttributeError::ValidationError(
            "filter value should be a number".to_string(),
        ))
    }
}

impl Shl<u8> for Filter {
    type Output = u8;
    fn shl(self, rhs: u8) -> Self::Output {
        (self as u8) << rhs
    }
}

#[derive(Debug, Default)]
struct CalibrationData {
    dig_t1: u16,
    dig_t2: i16,
    dig_t3: i16,
    dig_p1: u16,
    dig_p2: i16,
    dig_p3: i16,
    dig_p4: i16,
    dig_p5: i16,
    dig_p6: i16,
    dig_p7: i16,
    dig_p8: i16,
    dig_p9: i16,
    dig_h1: u8,
    dig_h2: i16,
    dig_h3: u8,
    dig_h4: i16,
    dig_h5: i16,
    dig_h6: i8,
    t_fine: i32,
}

impl From<[u8; BME280_CALIB_LEN]> for CalibrationData {
    fn from(data: [u8; BME280_CALIB_LEN]) -> Self {
        Self {
            dig_t1: u16::from_le_bytes([data[0], data[1]]),
            dig_t2: i16::from_le_bytes([data[2], data[3]]),
            dig_t3: i16::from_le_bytes([data[4], data[5]]),
            dig_p1: u16::from_le_bytes([data[6], data[7]]),
            dig_p2: i16::from_le_bytes([data[8], data[9]]),
            dig_p3: i16::from_le_bytes([data[10], data[11]]),
            dig_p4: i16::from_le_bytes([data[12], data[13]]),
            dig_p5: i16::from_le_bytes([data[14], data[15]]),
            dig_p6: i16::from_le_bytes([data[16], data[17]]),
            dig_p7: i16::from_le_bytes([data[18], data[19]]),
            dig_p8: i16::from_le_bytes([data[20], data[21]]),
            dig_p9: i16::from_le_bytes([data[22], data[23]]),
            dig_h1: data[25],
            dig_h2: i16::from_le_bytes([data[26], data[27]]),
            dig_h3: data[28],
            dig_h4: ((i16::from(data[29]) << 4) | (i16::from(data[30]) & 0xf)),
            dig_h5: ((i16::from(data[31]) << 4) | ((i16::from(data[30]) & 0xf0) >> 4)),
            dig_h6: data[32] as i8,
            t_fine: 0,
        }
    }
}

#[derive(Default, Debug, Clone)]
struct Configuration {
    filter: Filter,
    smp_temp: OverSampling,
    smp_hum: OverSampling,
    smp_pres: OverSampling,
    t_sb: Standby,
    mode: Mode,
}

impl Configuration {
    fn ctrl_meas_value(&self) -> u8 {
        (self.smp_temp << 5) | (self.smp_pres << 2) | Into::<u8>::into(self.mode)
    }
    fn ctrl_hum_value(&self) -> u8 {
        self.smp_hum.into()
    }
    fn ctrl_config_value(&self) -> u8 {
        (self.t_sb << 5) | (self.filter << 2)
    }
}
#[allow(dead_code)]
struct Status {
    measuring: u8,
    im_update: u8,
}

impl From<u8> for Status {
    fn from(value: u8) -> Self {
        Self {
            im_update: value & 0x1,
            measuring: (value >> 3) & 0x1,
        }
    }
}

const BME280_DEV_ADDR: u8 = 0x76;
const BME280_CHIP_ID: u8 = 0x60;
const BME280_RESET: u8 = 0xB6;
const BME280_CALIB_LEN: usize = 33;
const BME280_CALIB_PART1: usize = 26;
const BME280_MEAS_LEN: usize = 8;
const BME280_INACTIVE_MEAS: u32 = 0x80000;

#[derive(DoCommand)]
struct BME280 {
    config: Configuration,
    driver: I2cHandleType,
    calib: CalibrationData,
    ulp: Option<ULP>,
}

impl Drop for BME280 {
    fn drop(&mut self) {
        if let Some(ulp) = &mut self.ulp {
            if let Err(err) = ulp.start_ulp(self.config.clone()) {
                log::error!("couldn't start BME280 ULP program reason : {}", err);
            } else {
                log::info!("started BME280 ULP program")
            }
        }
    }
}

#[derive(Debug)]
struct RawMeasurement {
    temp: Option<u32>,
    pressure: Option<u32>,
    humidity: Option<u32>,
}

impl From<[u8; 8]> for RawMeasurement {
    fn from(data: [u8; 8]) -> Self {
        let pressure = ((data[0] as u32) << 12) | ((data[1] as u32) << 4) | ((data[2] as u32) >> 4);
        let temperature =
            ((data[3] as u32) << 12) | ((data[4] as u32) << 4) | ((data[5] as u32) >> 4);
        let humidity = ((data[6] as u32) << 8) | (data[7] as u32);
        Self {
            temp: temperature.ne(&BME280_INACTIVE_MEAS).then_some(temperature),
            pressure: pressure.ne(&BME280_INACTIVE_MEAS).then_some(pressure),
            humidity: humidity
                .ne(&(BME280_INACTIVE_MEAS >> 2))
                .then_some(humidity),
        }
    }
}

#[derive(Debug)]
struct CompensatedMeasurement {
    temperature: Option<f64>,
    pressure: Option<f64>,
    humidity: Option<f64>,
}

impl CompensatedMeasurement {
    fn from_raw(raw: RawMeasurement, calib: &mut CalibrationData) -> Self {
        let temperature = raw
            .temp
            .map(|temp| CompensatedMeasurement::compensate_temperature(temp as i32, calib));
        let pressure = raw.pressure.and_then(|pressure| {
            CompensatedMeasurement::compensate_pressure(pressure as i32, calib)
        });
        let humidity = raw
            .humidity
            .map(|humidity| CompensatedMeasurement::compensate_humidity(humidity as i32, calib));
        Self {
            temperature,
            pressure,
            humidity,
        }
    }

    fn compensate_temperature(temp: i32, calib: &mut CalibrationData) -> f64 {
        let var1: f64 =
            ((temp as f64) / 16384.0 - (calib.dig_t1 as f64) / 1024.0) * (calib.dig_t2 as f64);
        let mut var2: f64 = (temp as f64) / 131072.0 - (calib.dig_t1 as f64) / 8192.0;
        var2 = (var2 * var2) * (calib.dig_t3 as f64);
        calib.t_fine = (var1 + var2) as i32;
        let temp: f64 = (var1 + var2) / 5120.0;
        temp.clamp(-40.0, 85.0)
    }
    fn compensate_humidity(hum: i32, calib: &CalibrationData) -> f64 {
        let var1: f64 = (calib.t_fine as f64) - 76800.0;
        let var2 = (calib.dig_h4 as f64) * 64.0 + ((calib.dig_h5 as f64) / 16384.0) * var1;
        let var3 = (hum as f64) - var2;
        let var4 = (calib.dig_h2 as f64) / 65536.0;
        let var5 = 1.0 + ((calib.dig_h3 as f64) / 67108864.0) * var1;

        let mut var6 = 1.0 + ((calib.dig_h6 as f64) / 67108864.0) * var1 * var5;
        var6 = var3 * var4 * (var5 * var6);
        let humidity = var6 * (1.0 - (calib.dig_h1 as f64) * var6 / 524288.0);
        humidity.clamp(0.0, 100.0)
    }
    fn compensate_pressure(pres: i32, calib: &CalibrationData) -> Option<f64> {
        let mut var1 = (calib.t_fine as f64 / 2.0) - 64000.0;
        let mut var2 = var1 * var1 * (calib.dig_p6 as f64) / 32768.0;
        var2 += var1 * (calib.dig_p5 as f64) * 2.0;
        var2 = (var2 / 4.0) + ((calib.dig_p4 as f64) * 65536.0);
        let var3 = (calib.dig_p3 as f64) * var1 * var1 / 524288.0;
        var1 = (var3 + (calib.dig_p2 as f64) * var1) / 524288.0;
        var1 = (1.0 + var1 / 32768.0) * (calib.dig_p1 as f64);
        if var1 <= 0.0 {
            return None;
        }
        let mut pressure = 1048576.0 - (pres as f64);
        pressure = (pressure - (var2 / 4096.0)) * 6250.0 / var1;
        var1 = (calib.dig_p9 as f64) * pressure * pressure / 2147483648.0;
        var2 = pressure * (calib.dig_p8 as f64) / 32768.0;
        pressure += (var1 + var2 + (calib.dig_p7 as f64)) / 16.0;
        Some(pressure.clamp(30000.0, 110000.0))
    }
}

impl BME280 {
    pub fn from_config(cfg: ConfigType, deps: Vec<Dependency>) -> Result<SensorType, SensorError> {
        let board_handle = registry::get_board_from_dependencies(deps)
            .ok_or(SensorError::ConfigError("BME280: couldn't find a board"))?;
        let i2c_bus = cfg.get_attribute::<String>("i2c_name")?;

        let smp_pres = cfg
            .get_attribute::<OverSampling>("pressure_sampling")
            .or_else(|err| {
                if matches!(err, AttributeError::KeyNotFound(_)) {
                    Ok(Default::default())
                } else {
                    Err(err)
                }
            })?;
        let smp_temp = cfg
            .get_attribute::<OverSampling>("temperature_sampling")
            .or_else(|err| {
                if matches!(err, AttributeError::KeyNotFound(_)) {
                    Ok(Default::default())
                } else {
                    Err(err)
                }
            })?;
        let smp_hum = cfg
            .get_attribute::<OverSampling>("humidity_sampling")
            .or_else(|err| {
                if matches!(err, AttributeError::KeyNotFound(_)) {
                    Ok(Default::default())
                } else {
                    Err(err)
                }
            })?;
        let mode = cfg.get_attribute::<Mode>("mode").or_else(|err| {
            if matches!(err, AttributeError::KeyNotFound(_)) {
                Ok(Default::default())
            } else {
                Err(err)
            }
        })?;
        let t_sb = cfg.get_attribute::<Standby>("standby").or_else(|err| {
            if matches!(err, AttributeError::KeyNotFound(_)) {
                Ok(Default::default())
            } else {
                Err(err)
            }
        })?;
        let filter = cfg.get_attribute::<Filter>("filter").or_else(|err| {
            if matches!(err, AttributeError::KeyNotFound(_)) {
                Ok(Default::default())
            } else {
                Err(err)
            }
        })?;

        let ulp = cfg.has_attribute("ulp_config").then(|| {
            cfg.get_attribute::<ULPConfig>("ulp_config")
                .and_then(|cfg| {
                    ULP::new(cfg).map_err(|err| AttributeError::ValidationError(err.to_string()))
                })
        });
        let ulp = ulp.transpose()?;

        let config = Configuration {
            filter,
            mode,
            smp_hum,
            smp_pres,
            smp_temp,
            t_sb,
        };

        let i2c_bus = board_handle.lock().unwrap().get_i2c_by_name(i2c_bus)?;

        let mut sensor = Self {
            config,
            driver: i2c_bus,
            ulp,
            calib: Default::default(),
        };
        let mut id = [0_u8; 1];
        sensor.driver.lock().unwrap().write_read_i2c(
            BME280_DEV_ADDR,
            &[Register::ID.into()],
            &mut id,
        )?;
        if id[0] != BME280_CHIP_ID {
            return Err(SensorError::SensorDriverError(format!(
                "the connected chip id is 0x{:x} expected 0x{:x}",
                id[0], BME280_CHIP_ID,
            )));
        }
        sensor.read_calibration()?;
        sensor.configure()?;
        Ok(Arc::new(Mutex::new(sensor)))
    }

    fn reset(&mut self) -> Result<(), SensorError> {
        self.driver
            .lock()
            .unwrap()
            .write_i2c(BME280_DEV_ADDR, &[Register::RESET.into(), BME280_RESET])?;
        // reset time is around 2ms
        sleep(Duration::from_millis(5));
        Ok(())
    }

    fn configure(&mut self) -> Result<(), SensorError> {
        let ctrl_meas = self.config.ctrl_meas_value();
        let ctrl_hum = self.config.ctrl_hum_value();
        let config = self.config.ctrl_config_value();

        self.reset()?;
        self.driver
            .lock()
            .unwrap()
            .write_i2c(BME280_DEV_ADDR, &[Register::CTRL_HUM.into(), ctrl_hum])?;
        self.driver
            .lock()
            .unwrap()
            .write_i2c(BME280_DEV_ADDR, &[Register::CTRL_MEAS.into(), ctrl_meas])?;
        self.driver
            .lock()
            .unwrap()
            .write_i2c(BME280_DEV_ADDR, &[Register::CONFIG.into(), config])?;
        Ok(())
    }

    fn read_calibration(&mut self) -> Result<(), SensorError> {
        let mut data = [0_u8; BME280_CALIB_LEN];
        self.driver.lock().unwrap().write_read_i2c(
            BME280_DEV_ADDR,
            &[Register::CALIB_DATA1.into()],
            &mut data[..BME280_CALIB_PART1],
        )?;
        self.driver.lock().unwrap().write_read_i2c(
            BME280_DEV_ADDR,
            &[Register::CALIB_DATA2.into()],
            &mut data[BME280_CALIB_PART1..],
        )?;

        self.calib = data.into();
        Ok(())
    }
    fn read_status(&mut self) -> Result<Status, SensorError> {
        let mut status = [0_u8; 1];
        self.driver.lock().unwrap().write_read_i2c(
            BME280_DEV_ADDR,
            &[Register::STATUS.into()],
            &mut status,
        )?;
        Ok(status[0].into())
    }
    fn read_raw_measurement(&mut self) -> Result<RawMeasurement, SensorError> {
        let mut raw = [0_u8; BME280_MEAS_LEN];

        if let Mode::FORCED = self.config.mode {
            let ctrl_meas = self.config.ctrl_meas_value();
            self.driver
                .lock()
                .unwrap()
                .write_i2c(BME280_DEV_ADDR, &[Register::CTRL_MEAS.into(), ctrl_meas])?;

            loop {
                if self.read_status()?.measuring == 0 {
                    break;
                }
            }
        }

        self.driver.lock().unwrap().write_read_i2c(
            BME280_DEV_ADDR,
            &[Register::PRES_REG.into()],
            &mut raw,
        )?;

        let raw: RawMeasurement = raw.into();
        Ok(raw)
    }

    fn get_calibrated_reading(&mut self, raw_measurement: RawMeasurement) -> GenericReadingsResult {
        let mut res = GenericReadingsResult::new();
        let measurement = CompensatedMeasurement::from_raw(raw_measurement, &mut self.calib);
        if let Some(temperature) = measurement.temperature {
            res.insert(
                "temperature".to_owned(),
                Value {
                    kind: Some(value::Kind::NumberValue(temperature)),
                },
            );
        }
        if let Some(pressure) = measurement.pressure {
            res.insert(
                "pressure".to_owned(),
                Value {
                    kind: Some(value::Kind::NumberValue(pressure)),
                },
            );
        }
        if let Some(humidity) = measurement.humidity {
            res.insert(
                "humidity".to_owned(),
                Value {
                    kind: Some(value::Kind::NumberValue(humidity)),
                },
            );
        }
        res
    }
}

impl Sensor for BME280 {}

impl Readings for BME280 {
    fn get_generic_readings(
        &mut self,
    ) -> Result<micro_rdk::common::sensor::GenericReadingsResult, SensorError> {
        let measurement = self.read_raw_measurement()?;
        Ok(self.get_calibrated_reading(measurement))
    }

    fn get_readings_sensor_data(&mut self) -> Result<Vec<SensorData>, SensorError> {
        let res = match self.ulp.as_ref() {
            None => {
                let reading_requested_dt = chrono::offset::Local::now().fixed_offset();
                let readings = self.get_generic_readings()?;
                let reading_received_dt = chrono::offset::Local::now().fixed_offset();

                vec![SensorData {
                    metadata: Some(SensorMetadata {
                        time_received: Some(Timestamp {
                            seconds: reading_requested_dt.timestamp(),
                            nanos: reading_requested_dt.timestamp_subsec_nanos() as i32,
                        }),
                        time_requested: Some(Timestamp {
                            seconds: reading_received_dt.timestamp(),
                            nanos: reading_received_dt.timestamp_subsec_nanos() as i32,
                        }),
                        annotations: None,
                        mime_type: MimeType::Unspecified.into(),
                    }),
                    data: Some(readings.into()),
                }]
            }
            Some(ulp) => {
                let period = ulp.cfg.period;
                let start_dt = unsafe {
                    DateTime::from_timestamp_millis(*(RTC_TIME_START.0.get()))
                        .unwrap()
                        .fixed_offset()
                };

                let res_mem = ULPResultsMemory::from(&SAMPLE_ARRAY);

                res_mem
                    .enumerate()
                    .map(|(idx, raw_measurement)| {
                        let reading_ts = start_dt + (period * (idx as u32));
                        let reading = self.get_calibrated_reading(raw_measurement);
                        SensorData {
                            metadata: Some(SensorMetadata {
                                time_received: Some(Timestamp {
                                    seconds: reading_ts.timestamp(),
                                    nanos: reading_ts.timestamp_subsec_nanos() as i32,
                                }),
                                time_requested: Some(Timestamp {
                                    seconds: reading_ts.timestamp(),
                                    nanos: reading_ts.timestamp_subsec_nanos() as i32,
                                }),
                                annotations: None,
                                mime_type: MimeType::Unspecified.into(),
                            }),
                            data: Some(reading.into()),
                        }
                    })
                    .collect()
            }
        };
        Ok(res)
    }
}
