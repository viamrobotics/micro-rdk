pub mod common {
    pub mod adxl345;
    pub mod analog;
    pub mod app_client;
    pub mod base;
    pub mod board;
    pub mod camera;
    pub mod config;
    pub mod digital_interrupt;
    pub mod encoder;
    pub mod entry;
    pub mod grpc;
    pub mod grpc_client;
    pub mod i2c;
    pub mod math_utils;
    pub mod moisture_sensor;
    pub mod motor;
    pub mod movement_sensor;
    pub mod mpu6050;
    pub mod registry;
    pub mod robot;
    pub mod sensor;
    pub mod status;
    pub mod stop;
    pub mod webrtc {
        pub mod api;
        pub mod candidates;
        pub mod certificate;
        pub mod dtls;
        pub mod exec;
        pub mod grpc;
        pub mod ice;
        pub mod io;
        pub mod sctp;
    }
    pub mod conn {
        pub mod mdns;
        pub mod server;
    }
}

#[cfg(feature = "esp32")]
pub mod esp32 {
    pub mod analog;
    pub mod base;
    pub mod board;
    #[cfg(feature = "camera")]
    pub mod camera;
    pub mod certificate;
    pub mod dtls;
    pub mod encoder;
    pub mod entry;
    pub mod exec;
    pub mod i2c;
    pub mod motor;
    pub mod pin;
    pub mod pulse_counter;
    pub mod single_encoded_motor;
    pub mod single_encoder;
    pub mod tcp;
    pub mod tls;
    pub mod utils;
    pub mod webhook;
    pub mod conn {
        pub mod mdns;
    }
}

#[cfg(feature = "native")]
pub mod native {
    pub mod certificate;
    pub mod dtls;
    pub mod entry;
    pub mod exec;
    pub mod tcp;
    pub mod tls;
    pub mod conn {
        pub mod mdns;
    }
}

pub mod google {
    pub mod rpc {
        #![allow(clippy::derive_partial_eq_without_eq)]
        include!("gen/google.rpc.rs");
    }
}

pub mod proto {
    pub mod common {
        pub mod v1 {
            #![allow(clippy::derive_partial_eq_without_eq)]
            include!("gen/viam.common.v1.rs");
        }
    }

    pub mod app {
        pub mod v1 {
            #![allow(clippy::derive_partial_eq_without_eq)]
            include!("gen/viam.app.v1.rs");
        }
    }

    pub mod rpc {
        pub mod v1 {
            #![allow(clippy::derive_partial_eq_without_eq)]
            include!("gen/proto.rpc.v1.rs");
        }
        pub mod webrtc {
            pub mod v1 {
                #![allow(clippy::derive_partial_eq_without_eq)]
                include!("gen/proto.rpc.webrtc.v1.rs");
            }
        }
        pub mod examples {
            pub mod echo {
                pub mod v1 {
                    #![allow(clippy::derive_partial_eq_without_eq)]
                    include!("gen/proto.rpc.examples.echo.v1.rs");
                }
            }
        }
    }

    pub mod robot {
        pub mod v1 {
            #![allow(clippy::derive_partial_eq_without_eq)]
            include!("gen/viam.robot.v1.rs");
        }
    }
    pub mod component {
        pub mod board {
            pub mod v1 {
                #![allow(clippy::derive_partial_eq_without_eq)]
                include!("gen/viam.component.board.v1.rs");
            }
        }
        pub mod motor {
            pub mod v1 {
                #![allow(clippy::derive_partial_eq_without_eq)]
                include!("gen/viam.component.motor.v1.rs");
            }
        }
        pub mod camera {
            pub mod v1 {
                #![allow(clippy::derive_partial_eq_without_eq)]
                include!("gen/viam.component.camera.v1.rs");
            }
        }
        pub mod base {
            pub mod v1 {
                #![allow(clippy::derive_partial_eq_without_eq)]
                include!("gen/viam.component.base.v1.rs");
            }
        }

        pub mod encoder {
            pub mod v1 {
                #![allow(clippy::derive_partial_eq_without_eq)]
                include!("gen/viam.component.encoder.v1.rs");
            }
        }
        pub mod sensor {
            pub mod v1 {
                #![allow(clippy::derive_partial_eq_without_eq)]
                include!("gen/viam.component.sensor.v1.rs");
            }
        }

        pub mod movement_sensor {
            pub mod v1 {
                #![allow(clippy::derive_partial_eq_without_eq)]
                include!("gen/viam.component.movementsensor.v1.rs");
            }
        }
    }
}

#[macro_use]
extern crate trackable;

use stun_codec::rfc5245::attributes::*;
use stun_codec::rfc5389::attributes::*;
stun_codec::define_attribute_enums!(
    IceAttribute,
    AttributeDecoder,
    AttributeEncoder,
    [
        Username,
        MessageIntegrity,
        ErrorCode,
        XorMappedAddress,
        Fingerprint,
        IceControlled,
        IceControlling,
        Priority,
        UseCandidate
    ]
);
