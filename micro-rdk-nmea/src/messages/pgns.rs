use micro_rdk_nmea_macros::PgnMessageDerive;

use crate::parse_helpers::enums::TemperatureSource;

#[derive(PgnMessageDerive, Debug)]
pub struct WaterDepth {
    source_id: u8,
    #[scale = 0.01]
    depth: u32,
    #[scale = 0.001]
    offset: i16,
    #[scale = 10.0]
    range: u8,
}

#[derive(PgnMessageDerive, Debug)]
pub struct TemperatureExtendedRange {
    source_id: u8,
    instance: u8,
    #[lookup]
    source: TemperatureSource,
    #[bits = 24]
    #[scale = 0.001]
    #[unit = "C"]
    temperature: u32,
    #[scale = 0.1]
    set_temperature: u16,
}
