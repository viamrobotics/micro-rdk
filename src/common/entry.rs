use super::registry::ComponentRegistry;
use super::robot::LocalRobot;

pub enum RobotRepresentation {
    WithRobot(LocalRobot),
    WithRegistry(Box<ComponentRegistry>),
}
