use micro_rdk::common::registry::{ComponentRegistry, RegistryError};

pub mod gen;
pub mod messages;
pub mod parse_helpers;
#[cfg(generate_nmea_definitions)]
pub mod viamboat;

#[allow(unused_variables)]
pub fn register_models(registry: &mut ComponentRegistry) -> Result<(), RegistryError> {
    #[cfg(generate_nmea_definitions)]
    let res = viamboat::register_models(registry);
    #[cfg(not(generate_nmea_definitions))]
    let res = Ok(());
    res
}

#[cfg(generate_nmea_definitions)]
#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose, Engine};

    use crate::{
        gen::messages::{Pgn128267Message, MESSAGE_DATA_OFFSET},
        messages::message::Message,
        parse_helpers::{errors::NumberFieldError, parsers::DataCursor},
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
        let message = Pgn128267Message::from_cursor(cursor);
        assert!(message.is_ok());
        let message = message.unwrap();
        assert_eq!(message.sid().unwrap(), 255);
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
        let message = Pgn128267Message::from_cursor(cursor);
        assert!(message.is_ok());
        let message = message.unwrap();
        let sid = message.sid();
        assert!(sid.is_ok());
        assert_eq!(sid.unwrap(), 0);
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
}
