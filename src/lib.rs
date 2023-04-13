pub mod common {
    pub mod analog;
    pub mod base;
    pub mod board;
    pub mod camera;
    pub mod config;
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
}

#[cfg(feature = "esp32")]
pub mod esp32 {
    pub mod analog;
    pub mod base;
    pub mod board;
    #[cfg(feature = "camera")]
    pub mod camera;
    pub mod exec;
    pub mod i2c;
    pub mod motor;
    pub mod pin;
    pub mod robot_client;
    pub mod server;
    pub mod tcp;
    pub mod tls;
}

#[cfg(feature = "native")]
pub mod native {
    pub mod exec;
    pub mod robot_client;
    pub mod server;
    pub mod tcp;
    pub mod tls;
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
