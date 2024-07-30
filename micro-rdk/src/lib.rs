// TODO this check is disabled but shouldn't
#![allow(clippy::arc_with_non_send_sync)]
// The linkage feature is experimental, so it only works for
// the nightly channel, which we only use for ESP32 builds.
// See common/data_manager.rs for why we use this feature at all.
#![cfg_attr(feature = "data-upload-hook-unstable", feature(linkage))]

pub mod common;

#[cfg(feature = "esp32")]
pub mod esp32;

#[cfg(feature = "native")]
pub mod native;

#[macro_use]
pub extern crate micro_rdk_macros;

#[cfg(all(feature = "esp32", feature = "builtin-components"))]
#[macro_use(defer)]
extern crate scopeguard;

#[cfg(all(feature = "data", not(feature = "builtin-components")))]
#[macro_use(defer)]
extern crate scopeguard;

pub use micro_rdk_macros::DoCommand;
pub use micro_rdk_macros::MovementSensorReadings;
pub use micro_rdk_macros::PowerSensorReadings;

/// gRPC protobuf utilities, auto-generated
pub mod google {
    #[cfg(feature = "camera")]
    pub mod api {
        #![allow(clippy::derive_partial_eq_without_eq)]
        include!("gen/google.api.rs");
    }
    pub mod rpc {
        #![allow(clippy::derive_partial_eq_without_eq)]
        include!("gen/google.rpc.rs");
    }
    pub mod protobuf {
        #![allow(clippy::derive_partial_eq_without_eq)]
        include!("gen/google.protobuf.rs");
    }
}

/// gRPC prototypes from definitions in [api repository](https://github.com/viamrobotics/api/tree/main/proto/viam), auto-generated
pub mod proto {

    // Don't bother to clippy generated proto code
    #![allow(clippy::all)]

    pub mod provisioning {
        pub mod v1 {
            include!("gen/viam.provisioning.v1.rs");
        }
    }
    pub mod common {
        pub mod v1 {
            include!("gen/viam.common.v1.rs");
        }
    }

    pub mod app {
        pub mod v1 {
            include!("gen/viam.app.v1.rs");
        }
        pub mod packages {
            pub mod v1 {
                include!("gen/viam.app.packages.v1.rs");
            }
        }
        #[cfg(feature = "data")]
        pub mod data_sync {
            pub mod v1 {
                include!("gen/viam.app.datasync.v1.rs");
            }
        }
        // REVISIT(?): This is only being imported so that the code in gen/viam.app.v1.rs
        // can successfully compile, but this seems wrong. We need to confer with the team
        // responsible for the proto dependency
        pub mod mltraining {
            pub mod v1 {
                include!("gen/viam.app.mltraining.v1.rs");
            }
        }
    }

    pub mod rpc {
        pub mod v1 {
            include!("gen/proto.rpc.v1.rs");
        }
        pub mod webrtc {
            pub mod v1 {
                include!("gen/proto.rpc.webrtc.v1.rs");
            }
        }
        pub mod examples {
            pub mod echo {
                pub mod v1 {
                    include!("gen/proto.rpc.examples.echo.v1.rs");
                }
            }
        }
    }

    pub mod robot {
        pub mod v1 {
            include!("gen/viam.robot.v1.rs");
        }
    }
    pub mod component {
        pub mod board {
            pub mod v1 {
                include!("gen/viam.component.board.v1.rs");
            }
        }
        pub mod motor {
            pub mod v1 {
                include!("gen/viam.component.motor.v1.rs");
            }
        }
        pub mod camera {
            pub mod v1 {
                include!("gen/viam.component.camera.v1.rs");
            }
        }
        pub mod base {
            pub mod v1 {
                include!("gen/viam.component.base.v1.rs");
            }
        }

        pub mod encoder {
            pub mod v1 {
                include!("gen/viam.component.encoder.v1.rs");
            }
        }

        pub mod movement_sensor {
            pub mod v1 {
                include!("gen/viam.component.movementsensor.v1.rs");
            }
        }

        pub mod servo {
            pub mod v1 {
                include!("gen/viam.component.servo.v1.rs");
            }
        }

        pub mod power_sensor {
            pub mod v1 {
                include!("gen/viam.component.powersensor.v1.rs");
            }
        }
    }
}

#[macro_use]
extern crate trackable;

use std::time;

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

pub struct DurationParseFailure;

impl TryFrom<google::protobuf::Duration> for time::Duration {
    type Error = DurationParseFailure;
    fn try_from(duration: google::protobuf::Duration) -> Result<Self, Self::Error> {
        if duration.seconds >= 0 && duration.nanos >= 0 {
            Ok(time::Duration::new(
                duration.seconds as u64,
                duration.nanos as u32,
            ))
        } else {
            Err(DurationParseFailure)
        }
    }
}
