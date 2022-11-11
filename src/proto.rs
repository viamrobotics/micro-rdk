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
}
