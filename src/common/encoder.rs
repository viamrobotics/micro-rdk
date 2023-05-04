use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use crate::proto::component::encoder::v1::GetPositionResponse;
use crate::proto::component::encoder::v1::GetPropertiesResponse;
use crate::proto::component::encoder::v1::PositionType;

use super::board::BoardType;
use super::config::Component;
use super::config::ConfigType;
use super::registry::ComponentRegistry;
use super::status::Status;

pub(crate) fn register_models(registry: &mut ComponentRegistry) {
    if registry
        .register_encoder("fake", &FakeEncoder::from_config)
        .is_err()
    {
        log::error!("fake type is already registered");
    }
    if registry
        .register_encoder("fake_incremental", &FakeIncrementalEncoder::from_config)
        .is_err()
    {
        log::error!("fake_incremental type is already registered");
    }
}

pub struct EncoderSupportedRepresentations {
    pub ticks_count_supported: bool,
    pub angle_degrees_supported: bool,
}

impl From<EncoderSupportedRepresentations> for GetPropertiesResponse {
    fn from(repr_struct: EncoderSupportedRepresentations) -> Self {
        GetPropertiesResponse {
            ticks_count_supported: repr_struct.ticks_count_supported,
            angle_degrees_supported: repr_struct.angle_degrees_supported,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum EncoderPositionType {
    UNSPECIFIED,
    TICKS,
    DEGREES,
}

impl EncoderPositionType {
    pub fn wrap_value(self, value: f32) -> EncoderPosition {
        EncoderPosition {
            position_type: self,
            value,
        }
    }
}

impl From<EncoderPositionType> for PositionType {
    fn from(pt: EncoderPositionType) -> Self {
        match pt {
            EncoderPositionType::UNSPECIFIED => PositionType::Unspecified,
            EncoderPositionType::TICKS => PositionType::TicksCount,
            EncoderPositionType::DEGREES => PositionType::AngleDegrees,
        }
    }
}

impl From<PositionType> for EncoderPositionType {
    fn from(mpt: PositionType) -> Self {
        match mpt {
            PositionType::Unspecified => EncoderPositionType::UNSPECIFIED,
            PositionType::AngleDegrees => EncoderPositionType::DEGREES,
            PositionType::TicksCount => EncoderPositionType::TICKS,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct EncoderPosition {
    pub position_type: EncoderPositionType,
    pub value: f32,
}

impl From<EncoderPosition> for GetPositionResponse {
    fn from(pos: EncoderPosition) -> Self {
        GetPositionResponse {
            value: pos.value,
            position_type: PositionType::from(pos.position_type).into(),
        }
    }
}

pub trait Encoder: Status {
    fn get_properties(&mut self) -> EncoderSupportedRepresentations;
    fn get_position(
        &mut self,
        position_type: EncoderPositionType,
    ) -> anyhow::Result<EncoderPosition>;
    fn reset_position(&mut self) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: encoder_reset_position")
    }
}

pub(crate) type EncoderType = Arc<Mutex<dyn Encoder>>;

pub struct FakeIncrementalEncoder {
    pub ticks: f32,
}

impl Default for FakeIncrementalEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeIncrementalEncoder {
    pub fn new() -> Self {
        Self { ticks: 0.0 }
    }
    pub(crate) fn from_config(
        cfg: ConfigType,
        _: Option<BoardType>,
    ) -> anyhow::Result<EncoderType> {
        match cfg {
            ConfigType::Static(cfg) => {
                let mut enc: FakeIncrementalEncoder = Default::default();
                if let Ok(fake_ticks) = cfg.get_attribute::<f32>("fake_ticks") {
                    enc.ticks = fake_ticks;
                }
                Ok(Arc::new(Mutex::new(enc)))
            }
        }
    }
}

impl Encoder for FakeIncrementalEncoder {
    fn get_properties(&mut self) -> EncoderSupportedRepresentations {
        EncoderSupportedRepresentations {
            ticks_count_supported: true,
            angle_degrees_supported: false,
        }
    }
    fn get_position(
        &mut self,
        position_type: EncoderPositionType,
    ) -> anyhow::Result<EncoderPosition> {
        match position_type {
            EncoderPositionType::TICKS | EncoderPositionType::UNSPECIFIED => {
                Ok(EncoderPositionType::TICKS.wrap_value(self.ticks))
            }
            EncoderPositionType::DEGREES => {
                anyhow::bail!("FakeIncrementalEncoder does not support returning angular position")
            }
        }
    }
    fn reset_position(&mut self) -> anyhow::Result<()> {
        self.ticks = 0.0;
        Ok(())
    }
}

impl Status for FakeIncrementalEncoder {
    fn get_status(&mut self) -> anyhow::Result<Option<prost_types::Struct>> {
        Ok(Some(prost_types::Struct {
            fields: BTreeMap::new(),
        }))
    }
}

pub struct FakeEncoder {
    pub angle_degrees: f32,
    pub ticks_per_rotation: u32,
}

impl Default for FakeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeEncoder {
    pub fn new() -> Self {
        Self {
            angle_degrees: 0.0,
            ticks_per_rotation: 1,
        }
    }

    pub(crate) fn from_config(
        cfg: ConfigType,
        _: Option<BoardType>,
    ) -> anyhow::Result<EncoderType> {
        match cfg {
            ConfigType::Static(cfg) => {
                let mut enc: FakeEncoder = Default::default();
                if let Ok(fake_deg) = cfg.get_attribute::<f32>("fake_deg") {
                    enc.angle_degrees = fake_deg;
                }
                if let Ok(ticks_per_rotation) = cfg.get_attribute::<u32>("ticks_per_rotation") {
                    enc.ticks_per_rotation = ticks_per_rotation
                }
                Ok(Arc::new(Mutex::new(enc)))
            }
        }
    }
}

impl Encoder for FakeEncoder {
    fn get_properties(&mut self) -> EncoderSupportedRepresentations {
        EncoderSupportedRepresentations {
            ticks_count_supported: true,
            angle_degrees_supported: true,
        }
    }
    fn get_position(
        &mut self,
        position_type: EncoderPositionType,
    ) -> anyhow::Result<EncoderPosition> {
        match position_type {
            EncoderPositionType::UNSPECIFIED => {
                anyhow::bail!("must specify position_type to get FakeEncoder position")
            }
            EncoderPositionType::DEGREES => Ok(position_type.wrap_value(self.angle_degrees)),
            EncoderPositionType::TICKS => {
                let value: f32 = (self.angle_degrees / 360.0) * (self.ticks_per_rotation as f32);
                Ok(position_type.wrap_value(value))
            }
        }
    }
}

impl Status for FakeEncoder {
    fn get_status(&mut self) -> anyhow::Result<Option<prost_types::Struct>> {
        Ok(Some(prost_types::Struct {
            fields: BTreeMap::new(),
        }))
    }
}

impl<A> Encoder for Mutex<A>
where
    A: ?Sized + Encoder,
{
    fn get_properties(&mut self) -> EncoderSupportedRepresentations {
        self.get_mut().unwrap().get_properties()
    }
    fn reset_position(&mut self) -> anyhow::Result<()> {
        self.get_mut().unwrap().reset_position()
    }
    fn get_position(
        &mut self,
        position_type: EncoderPositionType,
    ) -> anyhow::Result<EncoderPosition> {
        self.get_mut().unwrap().get_position(position_type)
    }
}

impl<A> Encoder for Arc<Mutex<A>>
where
    A: ?Sized + Encoder,
{
    fn get_properties(&mut self) -> EncoderSupportedRepresentations {
        self.lock().unwrap().get_properties()
    }
    fn reset_position(&mut self) -> anyhow::Result<()> {
        self.lock().unwrap().reset_position()
    }
    fn get_position(
        &mut self,
        position_type: EncoderPositionType,
    ) -> anyhow::Result<EncoderPosition> {
        self.lock().unwrap().get_position(position_type)
    }
}
