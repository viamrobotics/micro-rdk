pub mod enums {
    include!(concat!(env!("OUT_DIR"), "/nmea_gen/enums.rs"));
}

pub mod polymorphisms {
    include!(concat!(env!("OUT_DIR"), "/nmea_gen/polymorphic_types.rs"));
}

#[allow(dead_code)]
pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/nmea_gen/messages.rs"));
}
