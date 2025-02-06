use micro_rdk::common::registry::{ComponentRegistry, RegistryError};

pub mod messages;
pub mod parse_helpers;
pub mod viamboat;

pub fn register_models(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
    viamboat::register_models(registry)
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose, Engine};

    use crate::{
        messages::{
            message::Message,
            pgns::{
                GnssPositionData, GnssSatsInView, NmeaMessage, NmeaMessageBody,
                PositionRapidUpdate, TemperatureExtendedRange, WaterDepth, MESSAGE_DATA_OFFSET,
            },
        },
        parse_helpers::{
            enums::{
                Gns, GnsIntegrity, GnsMethod, RangeResidualMode, SatelliteStatus, TemperatureSource,
            },
            errors::NumberFieldError,
            parsers::DataCursor,
        },
    };

    // The strings in the below test represent base64-encoded data examples taken from raw results
    // posted by an active CAN sensor. The first 32 bytes involve a header that is represented in serialized
    // form by parse_helpers::parsers::NmeaMessageMetadata.

    #[test]
    fn water_depth_parse() {
        let water_depth_str = "C/UBAHg+gD/TL/RmAAAAAFZODAAAAAAACAD/ABMAAwD/1AAAAAAA/w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(water_depth_str, &mut data);
        assert!(res.is_ok());
        let cursor = DataCursor::new(data[MESSAGE_DATA_OFFSET..].to_vec());
        let message = WaterDepth::from_cursor(cursor);
        assert!(message.is_ok());
        let message = message.unwrap();
        assert_eq!(message.source_id().unwrap(), 255);
        let depth = message.depth();
        assert!(depth.is_ok());
        assert_eq!(depth.unwrap(), 2.12);
        let offset = message.offset();
        assert!(offset.is_ok());
        assert_eq!(offset.unwrap(), 0.0);
        let range = message.range();
        assert!(range.is_err_and(|err| {
            matches!(err, NumberFieldError::FieldNotPresent(x) if x.as_str() == "range")
        }));
    }

    #[test]
    fn water_depth_parse_2() {
        let water_depth_str = "C/UBAHg+gD8l2A2A/////40fszsAAAAACAD/AAIAAwAAhgEAALwC/w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(water_depth_str, &mut data);
        assert!(res.is_ok());
        let cursor = DataCursor::new(data[MESSAGE_DATA_OFFSET..].to_vec());
        let message = WaterDepth::from_cursor(cursor);
        assert!(message.is_ok());
        let message = message.unwrap();
        let source_id = message.source_id();
        assert!(source_id.is_ok());
        assert_eq!(source_id.unwrap(), 0);
        let depth = message.depth();
        assert!(depth.is_ok());
        assert_eq!(depth.unwrap(), 3.9);
        let offset = message.offset();
        assert!(offset.is_ok());
        assert_eq!(offset.unwrap(), 0.7000000000000001);
        let range = message.range();
        assert!(range.is_err_and(|err| {
            matches!(err, NumberFieldError::FieldNotPresent(x) if x.as_str() == "range")
        }));
    }

    #[test]
    fn temperature_parse() {
        let temp_str = "DP0BAHg+gD8QkDZnAAAAABLFBAAAAAAACAD/ACMABQD/AADzmwT//w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(temp_str, &mut data);
        assert!(res.is_ok());

        let cursor = DataCursor::new(data[MESSAGE_DATA_OFFSET..].to_vec());
        let message = TemperatureExtendedRange::from_cursor(cursor);
        assert!(message.is_ok());
        let message = message.unwrap();
        let source_id = message.source_id();
        assert!(source_id.is_ok());
        assert_eq!(source_id.unwrap(), 255);

        let temp = message.temperature();
        assert!(temp.is_ok());
        let temp = temp.unwrap();
        assert_eq!(temp, 28.91700000000003);

        let instance = message.instance();
        assert!(instance.is_ok());
        let instance = instance.unwrap();
        assert_eq!(instance, 0);
        assert!(matches!(
            message.source(),
            TemperatureSource::SeaTemperature
        ));
    }

    #[test]
    fn gnss_parse() {
        let gnss_str = "BfgBAHg+gD9ugwBnAAAAAIS5CQAAAAAAKwD/AAMAAwA6IU4Ar0sAANWDfvaZpwWAW1SoTaa69XxJjf7/////JPwePACEAOby//8A";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(gnss_str, &mut data);
        assert!(res.is_ok());
        let cursor = DataCursor::new(data[MESSAGE_DATA_OFFSET..].to_vec());
        let message = GnssPositionData::from_cursor(cursor);
        assert!(message.is_ok());
        let message = message.unwrap();

        let source_id = message.source_id();
        assert!(source_id.is_ok());
        assert_eq!(source_id.unwrap(), 58);

        let altitude = message.altitude();
        assert!(altitude.is_ok());
        let altitude = altitude.unwrap();
        assert_eq!(altitude, -24.295043999999997);

        let gnss_type = message.gnss_type();
        assert!(matches!(gnss_type, Gns::GpsSbasWaasGlonass));

        // could not find an example containing any reference stations
        let ref_stations = message.reference_station_structs();
        assert_eq!(ref_stations.len(), 0);

        let latitude = message.latitude();
        assert!(latitude.is_ok());
        let latitude = latitude.unwrap();
        assert_eq!(latitude, 40.746357526389275);

        let longitude = message.longitude();
        assert!(longitude.is_ok());
        let longitude = longitude.unwrap();
        assert_eq!(longitude, -74.0096336282232);

        let method = message.method();
        assert!(matches!(method, GnsMethod::DgnssFix));

        let integrity = message.integrity();
        assert!(matches!(integrity, GnsIntegrity::NoIntegrityChecking));
    }

    #[test]
    fn gnss_sats_in_view_parse() {
        let msg_str = "BPoBAHg+gD/wh5FnAAAAAJwaDAAAAAAAwwD/AAUAAgCi/RAGlimH6ocR////f/UJdAV+Q20N////f/ELFiJwuR0R////f/UOxROhZ2gQ////f/URlhpnMNIQ////f/UT0CRzFOsQ////f/UUORlaiOsR////f/UYfwe2thwO////f/VE6ArQJG0M////f/VJ3Bc3gvcN////f/VKOSjlr0IO////f/VLCxHZ2qYO////f/VVuREIxcsN////f/VFCxFESGEQ////f/UGlimH6tIP////f/ULFiJwucwQ////f/U=";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(msg_str, &mut data);
        assert!(res.is_ok());

        let nmea_message = NmeaMessage::try_from(data);
        assert!(nmea_message.is_ok());

        let nmea_message = nmea_message.unwrap();
        let message = match nmea_message.data {
            NmeaMessageBody::GnssSatsInView(val) => Some(val),
            _ => None,
        };
        assert!(message.is_some());
        let message = message.unwrap();

        println!("message: {:?}", message);

        let source_id = message.source_id();
        assert!(source_id.is_ok());
        assert_eq!(source_id.unwrap(), 162);

        let range_residual_mode = message.range_residual_mode();
        println!("range_residual_mode: {:?}", range_residual_mode);
        assert!(matches!(
            range_residual_mode,
            RangeResidualMode::PostCalculation
        ));

        let sats_in_view = message.sats_in_view();
        println!("sats in view: {:?}", sats_in_view);
        assert!(sats_in_view.is_ok());
        let sats_in_view = sats_in_view.unwrap();
        assert_eq!(sats_in_view, 16);

        let sats = message.satellites();
        println!("sats: {:?}", sats);
        assert_eq!(sats.len(), sats_in_view as usize);

        let azimuth_1 = sats[1].azimuth();
        assert!(azimuth_1.is_ok());
        let azimuth_1 = azimuth_1.unwrap();
        assert_eq!(azimuth_1, 98.99564784270363);

        let elevation_1 = sats[1].elevation();
        assert!(elevation_1.is_ok());
        let elevation_1 = elevation_1.unwrap();
        assert_eq!(elevation_1, 7.9984908200262925);

        let prn_1 = sats[1].prn();
        assert!(prn_1.is_ok());
        let prn_1 = prn_1.unwrap();
        assert_eq!(prn_1, 9);

        let snr_1 = sats[1].snr();
        assert!(snr_1.is_ok());
        let snr_1 = snr_1.unwrap();
        assert_eq!(snr_1, 34.37);

        let status_1 = sats[1].status();
        assert!(matches!(status_1, SatelliteStatus::Tracked));

        let azimuth_2 = sats[2].azimuth();
        assert!(azimuth_2.is_ok());
        let azimuth_2 = azimuth_2.unwrap();
        assert_eq!(azimuth_2, 271.9945245045044);

        let elevation_2 = sats[2].elevation();
        assert!(elevation_2.is_ok());
        let elevation_2 = elevation_2.unwrap();
        assert_eq!(elevation_2, 49.99629720311564);

        let prn_2 = sats[2].prn();
        assert!(prn_2.is_ok());
        let prn_2 = prn_2.unwrap();
        assert_eq!(prn_2, 11);

        let snr_2 = sats[2].snr();
        assert!(snr_2.is_ok());
        let snr_2 = snr_2.unwrap();
        assert_eq!(snr_2, 43.81);

        let status_2 = sats[2].status();
        assert!(matches!(status_2, SatelliteStatus::UsedDiff));
    }
}
