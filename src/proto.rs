pub mod common {
    pub mod v1 {
        include!("gen/viam.common.v1.rs");
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
}
