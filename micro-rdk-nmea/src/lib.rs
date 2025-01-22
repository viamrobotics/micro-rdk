pub mod messages;
pub mod parse_helpers;

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose, Engine};

    use crate::{
        messages::pgns::{TemperatureExtendedRange, WaterDepth},
        parse_helpers::{enums::TemperatureSource, errors::NumberFieldError},
    };

    #[test]
    fn water_depth_parse() {
        let water_depth_str = "C/UBAHg+gD/TL/RmAAAAAFZODAAAAAAACAD/ABMAAwD/1AAAAAAA/w==";
        let mut data = Vec::<u8>::new();
        let res = general_purpose::STANDARD.decode_vec(water_depth_str, &mut data);
        assert!(res.is_ok());
        let message = WaterDepth::from_bytes(data[33..].to_vec(), Some(13));
        assert!(message.is_ok());
        let message = message.unwrap();
        assert_eq!(message.source_id(), 13);
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
        let message = WaterDepth::from_bytes(data[33..].to_vec(), Some(13));
        assert!(message.is_ok());
        let message = message.unwrap();
        assert_eq!(message.source_id(), 13);
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

        let message = TemperatureExtendedRange::from_bytes(data[33..].to_vec(), Some(23));
        assert!(message.is_ok());
        let message = message.unwrap();
        assert_eq!(message.source_id(), 23);
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
}
