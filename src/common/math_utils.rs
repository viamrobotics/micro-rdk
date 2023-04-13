#![allow(dead_code)]
use crate::proto::common;

#[derive(Clone, Copy, Debug, Default)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3 {
    pub fn new() -> Self {
        Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl From<Vector3> for common::v1::Vector3 {
    fn from(vector: Vector3) -> Self {
        common::v1::Vector3 {
            x: vector.x,
            y: vector.y,
            z: vector.z,
        }
    }
}
