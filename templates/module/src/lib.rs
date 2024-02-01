use std::sync::{Arc, Mutex};
use micro_rdk::DoCommand;
use micro_rdk::common::status::Status;
use micro_rdk::common::registry::{ComponentRegistry, RegistryError};
{% if starting_component == "Motor" %}
use micro_rdk::common::{actuator::Actuator, motor::{Motor, MotorType}};
{% elsif starting_component == "Base" %}
use micro_rdk::common::{actuator::Actuator, base::{Base, BaseType}};
{% elsif starting_component == "MovementSensor" %}
use micro_rdk::MovementSensorReadings;
use micro_rdk::common::movement_sensor::{MovementSensor, MovementSensorType};
{% elsif starting_component == "PowerSensor" %}
use micro_rdk::PowerSensorReadings;
use micro_rdk::common::power_sensor::{PowerSensor, PowerSensorType};
{% elsif starting_component == "Sensor" %}
use micro_rdk::common::sensor::{Sensor, SensorType, Readings};
{% elsif starting_component == "Servo" %}
use micro_rdk::common::{actuator::Actuator, servo::{Servo, ServoType}};
{% elsif starting_component == "GenericComponent" %}
use micro_rdk::common::generic::{GenericComponent, GenericComponentType};
{% elsif starting_component == "Encoder" %}
use micro_rdk::common::encoder::{Encoder, EncoderType};
{% else %}
{% endif %}

pub fn register_models(registry: &mut ComponentRegistry) -> anyhow::Result<(), RegistryError> {
    {% if starting_component == "Motor" %}registry.register_motor("my_motor", &My{{starting_component}}::from_config){% elsif starting_component == "Base" %}registry.register_base("my_base", &My{{starting_component}}::from_config){% elsif starting_component == "MovementSensor" %}registry.register_movement_sensor("my_movement_sensor", &My{{starting_component}}::from_config){% elsif starting_component == "PowerSensor" %}registry.register_power_sensor("my_power_sensor", &My{{starting_component}}::from_config){% elsif starting_component == "Sensor" %}registry.register_sensor("my_sensor", &My{{starting_component}}::from_config){% elsif starting_component == "Servo" %}registry.register_servo("my_servo", &My{{starting_component}}::from_config){% elsif starting_component == "GenericComponent" %}registry.register_generic_component("my_generic_component", &My{{starting_component}}::from_config){% elsif starting_component == "Encoder" %}registry.register_encoder("my_encoder", &My{{starting_component}}::from_config){% else %}Ok(()){% endif %}
}

{% if starting_component != "None" %}
#[derive(DoCommand{% if starting_component == "MovementSensor" %}, MovementSensorReadings{% elsif starting_component == "PowerSensor" %}, PowerSensorReadings{% else %}{% endif %})]
pub struct My{{starting_component}} {}

impl My{{starting_component}} {
    pub fn from_config(cfg: ConfigType, deps: Vec<Dependency>) -> anyhow::Result<{{starting_component}}Type> {
        Ok(Arc::new(Mutex::new(My{{starting_component}} {})))
    }
}

impl Status for My{{starting_component}} {
    fn get_status(&self) -> anyhow::Result<Option<micro_rdk::google::protobuf::Struct>> {
        Ok(Some(micro_rdk::google::protobuf::Struct {
            fields: HashMap::new(),
        }))
    }
}
{% endif %}
