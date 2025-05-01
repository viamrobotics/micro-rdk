pub mod enums {
    include!(concat!(env!("OUT_DIR"), "/nmea_gen/enums.rs"));
}

pub mod polymorphic_types {
    include!(concat!(env!("OUT_DIR"), "/nmea_gen/polymorphic_types.rs"));
}
