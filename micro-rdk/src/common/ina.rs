/// Package ina implements ina power sensors to measure voltage, current, and power
/// INA219 datasheet: https://www.ti.com/lit/ds/symlink/ina219.pdf
/// INA226 datasheet: https://www.ti.com/lit/ds/symlink/ina226.pdf
///
/// The voltage, current and power can be read as
/// 16 bit big endian integers from their given registers.
/// This value is multiplied by the register LSB to get the reading in nanounits.
///
/// Voltage LSB: 1.25 mV for INA226, 4 mV for INA219
/// Current LSB: maximum expected current of the system / (1 << 15)
/// Power LSB: 25 * Current LSB for INA226, 20 * Current LSB for INA219
///
/// The calibration register is programmed to measure current and power properly.
/// The calibration register is set to: calibratescale / (current_lsb * sense_resistor)
use crate::common::i2c::I2CHandle;
use core::fmt;
use std::sync::{Arc, Mutex};

use super::{
    board::Board,
    config::ConfigType,
    i2c::{I2CErrors, I2cHandleType},
    power_sensor::{Current, PowerSensor, PowerSensorType, PowerSupplyType, Voltage},
    registry::{get_board_from_dependencies, ComponentRegistry, Dependency},
    sensor::SensorError,
    status::Status,
};

const DEFAULT_I2C_ADDRESS: u8 = 0x40;
const DEFAULT_SHUNT_RESISTANCE_OHMS: f64 = 0.1;

const INA_219_CALIBRATION_SCALE: f64 = 0.04096;
const INA_226_CALIBRATION_SCALE: f64 = 0.00512;

const DEFAULT_CONFIG_REGISTER_VALUE: u16 = 0x399F;
const CALIBRATION_REGISTER: u8 = 0x05;
const CONFIG_REGISTER: u8 = 0x00;
const VOLTAGE_REGISTER: [u8; 1] = [0x02];
const CURRENT_AMPERES_REGISTER: [u8; 1] = [0x04];
const POWER_REGISTER: [u8; 1] = [0x03];

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_power_sensor("ina219", &ina219_from_config)
        .is_err()
    {
        log::error!("gpio model is already registered")
    }
    if registry
        .register_power_sensor("ina226", &ina226_from_config)
        .is_err()
    {
        log::error!("gpio model is already registered")
    }
}

fn ina219_from_config(
    cfg: ConfigType,
    deps: Vec<Dependency>,
) -> Result<PowerSensorType, SensorError> {
    Ok(Arc::new(Mutex::new(from_config(Model::Ina219, cfg, deps)?)))
}

fn ina226_from_config(
    cfg: ConfigType,
    deps: Vec<Dependency>,
) -> Result<PowerSensorType, SensorError> {
    Ok(Arc::new(Mutex::new(from_config(Model::Ina226, cfg, deps)?)))
}

fn from_config(
    model: Model,
    cfg: ConfigType,
    dependencies: Vec<Dependency>,
) -> Result<Ina<I2cHandleType>, SensorError> {
    let i2c_address = cfg
        .get_attribute::<u8>("i2c_address")
        .unwrap_or(DEFAULT_I2C_ADDRESS);

    let default_max_current_amperes = match &model {
        Model::Ina219 => 3.2,
        Model::Ina226 => 20.0,
    };
    let max_current_amperes = cfg
        .get_attribute::<f64>("max_current_amps")
        .unwrap_or(default_max_current_amperes);
    let max_current_nano_amperes = (max_current_amperes * 1e9) as i64;

    let shunt_resistance_ohms = cfg
        .get_attribute::<f64>("shunt_resistance")
        .unwrap_or(DEFAULT_SHUNT_RESISTANCE_OHMS);
    let shunt_resistance_nano_ohms = (shunt_resistance_ohms * 1e9) as i64;

    let i2c_name = cfg.get_attribute::<String>("i2c_bus").map_err(|_| {
        SensorError::ConfigError("i2c_bus is a required attribute for power sensor")
    })?;
    let board = get_board_from_dependencies(dependencies).ok_or(SensorError::ConfigError(
        "missing board attribute for Ina sensor",
    ))?;
    let i2c_handle = board.get_i2c_by_name(i2c_name)?;

    Ina::new(
        model,
        i2c_handle,
        i2c_address,
        max_current_nano_amperes,
        shunt_resistance_nano_ohms,
    )
}

#[derive(Clone, Copy)]
enum Model {
    Ina219,
    Ina226,
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let model_str = match self {
            Model::Ina219 => "INA219",
            Model::Ina226 => "INA226",
        };
        write!(f, "{model_str}")
    }
}

#[derive(DoCommand, PowerSensorReadings)]
struct Ina<H: I2CHandle> {
    model: Model,
    i2c_handle: H,
    i2c_address: u8,
    max_current_nano_amperes: i64,
    power_reading_lsb: i64,
}

impl<H: I2CHandle> Ina<H> {
    fn new(
        model: Model,
        i2c_handle: H,
        i2c_address: u8,
        max_current_nano_amperes: i64,
        shunt_resistance_nano_ohms: i64,
    ) -> Result<Self, SensorError> {
        let current_reading_lsb = max_current_nano_amperes / (1 << 15);
        let (unadjusted_calibration_scale, power_reading_lsb) = match model {
            Model::Ina219 => {
                let power_lsb = (max_current_nano_amperes * 20 + (1 << 14)) / (1 << 15);
                (INA_219_CALIBRATION_SCALE, power_lsb)
            }
            Model::Ina226 => (INA_226_CALIBRATION_SCALE, 25 * current_reading_lsb),
        };
        let calibration_scale = (unadjusted_calibration_scale
            / ((current_reading_lsb as f64) * (shunt_resistance_nano_ohms as f64 * 1e-9)))
            as i64;
        if calibration_scale >= (1 << 16) {
            return Err(SensorError::ConfigError(
                "ina calibration scale exceeds limit of 1 << 16",
            ));
        }
        let mut calibration_scale_bytes = (calibration_scale as u16).to_be_bytes();
        let mut res = Self {
            model,
            i2c_handle,
            i2c_address,
            max_current_nano_amperes,
            power_reading_lsb,
        };
        res.calibrate(&mut calibration_scale_bytes)?;
        Ok(res)
    }

    fn calibrate(&mut self, calibration_scale_bytes: &mut [u8; 2]) -> Result<(), I2CErrors> {
        // set scaling factor for current and power registers by writing adjusted
        // calibration scale to the appropriate register
        self.write_to_register(CALIBRATION_REGISTER, calibration_scale_bytes)?;
        // set the sensor into its normal operating mode 111 (continously reading voltage, current and power),
        // 0s indicate that the corresponding measurement will onyl be made in response to an event
        let default_config_register_bytes = DEFAULT_CONFIG_REGISTER_VALUE.to_be_bytes();
        self.write_to_register(CONFIG_REGISTER, &default_config_register_bytes)
    }

    fn write_to_register(&mut self, register: u8, buffer: &[u8]) -> Result<(), I2CErrors> {
        let mut byte_vec: Vec<u8> = vec![register];
        byte_vec.extend_from_slice(buffer);
        self.i2c_handle.write_i2c(self.i2c_address, &byte_vec)
    }
}

impl<H: I2CHandle> PowerSensor for Ina<H> {
    fn get_voltage(&mut self) -> Result<Voltage, SensorError> {
        let mut voltage_bytes: [u8; 2] = [0; 2];
        self.i2c_handle
            .write_read_i2c(self.i2c_address, &VOLTAGE_REGISTER, &mut voltage_bytes)?;
        let volts = match self.model {
            Model::Ina226 => (i16::from_be_bytes(voltage_bytes) as f64) * 1.25e-3,
            Model::Ina219 => ((i16::from_be_bytes(voltage_bytes) >> 3) as f64) / 250.0,
        };
        Ok(Voltage {
            volts,
            power_supply_type: PowerSupplyType::DC,
        })
    }

    fn get_current(&mut self) -> Result<Current, SensorError> {
        let current_reading_lsb = self.max_current_nano_amperes / (1 << 15);
        let mut current_amperes_bytes: [u8; 2] = [0; 2];
        self.i2c_handle.write_read_i2c(
            self.i2c_address,
            &CURRENT_AMPERES_REGISTER,
            &mut current_amperes_bytes,
        )?;
        let current_nano_amperes =
            (i16::from_be_bytes(current_amperes_bytes) as i64) * current_reading_lsb;
        let amperes = (current_nano_amperes as f64) * 1e-9;
        Ok(Current {
            amperes,
            power_supply_type: PowerSupplyType::DC,
        })
    }

    fn get_power(&mut self) -> Result<f64, SensorError> {
        let mut power_bytes: [u8; 2] = [0; 2];
        self.i2c_handle
            .write_read_i2c(self.i2c_address, &POWER_REGISTER, &mut power_bytes)?;
        let power_nano_watts = (i16::from_be_bytes(power_bytes) as i64) * self.power_reading_lsb;
        Ok((power_nano_watts as f64) * 1e-9)
    }
}

impl<H: I2CHandle> Status for Ina<H> {
    fn get_status(&self) -> anyhow::Result<Option<crate::google::protobuf::Struct>> {
        Ok(None)
    }
}
