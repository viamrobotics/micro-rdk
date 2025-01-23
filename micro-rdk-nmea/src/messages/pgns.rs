use micro_rdk_nmea_macros::{FieldsetDerive, PgnMessageDerive};

use crate::parse_helpers::{
    enums::{Gns, GnsIntegrity, GnsMethod, RangeResidualMode, SatelliteStatus, TemperatureSource},
    parsers::{DataCursor, FieldSet},
};

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

#[derive(FieldsetDerive, Clone, Debug)]
pub struct ReferenceStation {
    #[bits = 12]
    reference_station_id: u16,
    #[scale = 0.01]
    age_of_dgnss_corrections: u16,
}

#[derive(PgnMessageDerive, Clone, Debug)]
pub struct GnssPositionData {
    source_id: u8,
    date: u16,
    #[scale = 0.0001]
    time: u32,
    #[scale = 1e-16]
    latitude: i64,
    #[scale = 1e-16]
    longitude: i64,
    #[scale = 1e-06]
    altitude: i64,
    #[lookup]
    #[bits = 4]
    gnss_type: Gns,
    #[lookup]
    #[bits = 4]
    method: GnsMethod,
    #[lookup]
    #[bits = 2]
    integrity: GnsIntegrity,
    #[offset = 6]
    number_of_svs: u8,
    #[scale = 0.01]
    hdop: i16,
    #[scale = 0.01]
    pdop: i16,
    #[scale = 0.01]
    geoidal_separation: i32,
    reference_stations: u8,
    #[fieldset]
    #[length_field = "reference_stations"]
    reference_station_structs: Vec<ReferenceStation>,
}

#[derive(FieldsetDerive, Clone, Debug)]
pub struct Satellite {
    prn: u8,
    #[scale = 0.0001]
    #[unit = "deg"]
    elevation: i16,
    #[scale = 0.0001]
    #[unit = "deg"]
    azimuth: u16,
    #[scale = 0.01]
    snr: u16,
    range_residuals: i32,
    #[lookup]
    #[bits = 4]
    status: SatelliteStatus,
    // normally we would handle "reserved" fields by using the offset attribute
    // on the next field, but in the edge case of a reserved field being the last
    // field of a fieldset we need to include it
    #[bits = 4]
    reserved: u8,
}

#[derive(PgnMessageDerive, Clone, Debug)]
pub struct GnssSatsInView {
    #[lookup]
    #[bits = 2]
    range_residual_mode: RangeResidualMode,
    #[offset = 6]
    sats_in_view: u8,
    #[fieldset]
    #[length_field = "sats_in_view"]
    satellites: Vec<Satellite>,
}
