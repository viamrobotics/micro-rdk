use std::{fmt::Debug, marker::PhantomData, sync::Arc, sync::Mutex, time::Duration};

use crate::{
    common::board::Board,
    common::robot::LocalRobot,
    google::rpc::Status,
    proto::{self, component, robot},
};
use bytes::{BufMut, BytesMut};
use futures_lite::{future, Future};
use hyper::{
    body::{self, Bytes, HttpBody},
    http::HeaderValue,
    service::Service,
    Body, HeaderMap, Request, Response,
};
use log::*;
use prost::Message;
use smol_timeout::TimeoutExt;
use std::cell::RefCell;
use std::error::Error;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};

use super::webrtc::grpc::WebRtcGrpcService;

#[cfg(feature = "camera")]
static GRPC_BUFFER_SIZE: usize = 10240;
#[cfg(not(feature = "camera"))]
static GRPC_BUFFER_SIZE: usize = 4096;

#[derive(Clone, Debug)]
pub struct GrpcBody {
    _marker: PhantomData<*const ()>,
    data: Option<Bytes>,
    trailers: Option<HeaderMap<HeaderValue>>,
}

impl GrpcBody {
    pub fn new() -> Self {
        let mut trailers = HeaderMap::new();
        trailers.insert("grpc-status", "0".parse().unwrap());
        GrpcBody {
            data: None,
            trailers: Some(trailers),
            _marker: PhantomData,
        }
    }
}

impl GrpcResponse for GrpcBody {
    fn put_data(&mut self, data: Bytes) {
        let _ = self.data.insert(data);
    }
    fn insert_trailer(&mut self, key: &'static str, value: &'_ str) {
        self.trailers
            .as_mut()
            .unwrap()
            .insert(key, value.parse().unwrap());
    }
    fn set_status(&mut self, code: i32, message: Option<&'_ str>) {
        self.trailers
            .as_mut()
            .unwrap()
            .insert("grpc-status", code.into());
        if let Some(message) = message {
            self.trailers
                .as_mut()
                .unwrap()
                .insert("grpc-message", message.parse().unwrap());
        }
    }
    fn get_data(&mut self) -> Bytes {
        self.data.take().unwrap()
    }
}

impl Default for GrpcBody {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for GrpcBody {
    fn drop(&mut self) {
        debug!("Dropping body");
    }
}

impl HttpBody for GrpcBody {
    type Data = Bytes;
    type Error = hyper::http::Error;

    fn poll_data(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Data, Self::Error>>> {
        Poll::Ready(self.get_mut().data.take().map(Ok))
    }
    fn poll_trailers(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Result<Option<HeaderMap>, Self::Error>> {
        Poll::Ready(Ok(self.get_mut().trailers.take()))
    }
}

pub trait GrpcResponse {
    fn put_data(&mut self, data: Bytes);
    fn insert_trailer(&mut self, key: &'static str, value: &'_ str);
    fn set_status(&mut self, code: i32, message: Option<&'_ str>);
    fn get_data(&mut self) -> Bytes;
}

#[derive(Clone)]
pub struct GrpcServer<R> {
    pub(crate) response: R,
    pub(crate) buffer: Rc<RefCell<BytesMut>>,
    robot: Arc<Mutex<LocalRobot>>,
}

impl<R> Debug for GrpcServer<R>
where
    R: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "GrpcServer {{ response : {:?} }}, {{ buffer : {:?} }}",
            self.response, self.buffer
        )
    }
}

impl<R> GrpcServer<R>
where
    R: GrpcResponse,
{
    pub fn new(robot: Arc<Mutex<LocalRobot>>, body: R) -> Self {
        info!("Making server");
        GrpcServer {
            response: body,
            buffer: Rc::new(RefCell::new(BytesMut::with_capacity(GRPC_BUFFER_SIZE))),
            robot,
        }
    }

    fn validate_rpc(message: &Bytes) -> anyhow::Result<&[u8]> {
        // Per https://github.com/grpc/grpc/blob/master/doc/PROTOCOL-HTTP2.md, we're expecting a
        // 5-byte header followed by the actual protocol buffer data. The 5 bytes in the header are
        // 1 null byte (indicating we're not using compression), and 4 bytes of a big-endian
        // integer describing the length of the rest of the data.
        anyhow::ensure!(message.len() >= 5, "Message too short");
        let (header, rest) = message.split_at(5);
        let (use_compression, expected_len) = header.split_at(1);
        anyhow::ensure!(use_compression[0] == 0, "Compression not supported");
        let expected_len = u32::from_be_bytes(expected_len.try_into().unwrap());
        anyhow::ensure!(expected_len == rest.len() as u32, "Incorrect payload size");
        Ok(rest)
    }

    pub(crate) fn handle_request(&mut self, path: &str, payload: &[u8]) -> anyhow::Result<()> {
        match path {
            "/viam.component.base.v1.BaseService/SetPower" => self.base_set_power(payload),
            "/viam.component.base.v1.BaseService/Stop" => self.base_stop(payload),
            "/viam.component.base.v1.BaseService/MoveStraight" => self.base_move_straight(payload),
            "/viam.component.base.v1.BaseService/Spin" => self.base_spin(payload),
            "/viam.component.base.v1.BaseService/SetVelocity" => self.base_set_velocity(payload),
            "/viam.component.board.v1.BoardService/GetDigitalinterruptValue" => {
                self.board_get_digital_interrupt_value(payload)
            }
            "/viam.component.board.v1.BoardService/GetGPIO" => self.board_get_pin(payload),
            "/viam.component.board.v1.BoardService/PWM" => self.board_pwm(payload),
            "/viam.component.board.v1.BoardService/PWMFrequency" => {
                self.board_pwm_frequency(payload)
            }
            "/viam.component.board.v1.BoardService/ReadAnalogReader" => {
                self.board_read_analog_reader(payload)
            }
            "/viam.component.board.v1.BoardService/SetGPIO" => self.board_set_pin(payload),
            "/viam.component.board.v1.BoardService/SetPWM" => self.board_set_pwm(payload),
            "/viam.component.board.v1.BoardService/SetPWMFrequency" => {
                self.board_set_pwm_frequency(payload)
            }
            "/viam.component.board.v1.BoardService/Status" => self.board_status(payload),
            "/viam.component.board.v1.BoardService/SetPowerMode" => {
                self.board_set_power_mode(payload)
            }
            #[cfg(feature = "camera")]
            "/viam.component.camera.v1.CameraService/GetImage" => self.camera_get_frame(payload),
            #[cfg(feature = "camera")]
            "/viam.component.camera.v1.CameraService/GetPointCloud" => {
                self.camera_get_point_cloud(payload)
            }
            #[cfg(feature = "camera")]
            "/viam.component.camera.v1.CameraService/GetProperties" => {
                self.camera_get_properties(payload)
            }
            #[cfg(feature = "camera")]
            "/viam.component.camera.v1.CameraService/RenderFrame" => {
                self.camera_render_frame(payload)
            }
            "/viam.component.motor.v1.MotorService/GetPosition" => self.motor_get_position(payload),
            "/viam.component.motor.v1.MotorService/GetProperties" => {
                self.motor_get_properties(payload)
            }
            "/viam.component.motor.v1.MotorService/GoFor" => self.motor_go_for(payload),
            "/viam.component.motor.v1.MotorService/GoTo" => self.motor_go_to(payload),
            "/viam.component.motor.v1.MotorService/IsPowered" => self.motor_is_powered(payload),
            "/viam.component.motor.v1.MotorService/ResetZeroPosition" => {
                self.motor_reset_zero_position(payload)
            }
            "/viam.component.motor.v1.MotorService/SetPower" => self.motor_set_power(payload),
            "/viam.component.motor.v1.MotorService/Stop" => self.motor_stop(payload),
            "/viam.robot.v1.RobotService/ResourceNames" => self.resource_names(payload),
            "/viam.robot.v1.RobotService/GetStatus" => self.robot_status(payload),
            "/viam.robot.v1.RobotService/StreamStatus" => self.robot_status_stream(payload),
            "/viam.robot.v1.RobotService/GetOperations" => self.robot_get_oprations(payload),
            "/proto.rpc.v1.AuthService/Authenticate" => self.auth_service_authentificate(payload),
            "/viam.component.sensor.v1.SensorService/GetReadings" => {
                self.sensor_get_readings(payload)
            }
            "/viam.component.movementsensor.v1.MovementSensorService/GetPosition" => {
                self.movement_sensor_get_position(payload)
            }
            "/viam.component.movementsensor.v1.MovementSensorService/GetLinearVelocity" => {
                self.movement_sensor_get_linear_velocity(payload)
            }
            "/viam.component.movementsensor.v1.MovementSensorService/GetAngularVelocity" => {
                self.movement_sensor_get_angular_velocity(payload)
            }
            "/viam.component.movementsensor.v1.MovementSensorService/GetLinearAcceleration" => {
                self.movement_sensor_get_linear_acceleration(payload)
            }
            "/viam.component.movementsensor.v1.MovementSensorService/GetCompassHeading" => {
                self.movement_sensor_get_compass_heading(payload)
            }
            "/viam.component.movementsensor.v1.MovementSensorService/GetProperties" => {
                self.movement_sensor_get_properties(payload)
            }
            "/viam.component.movementsensor.v1.MovementSensorService/GetOrientation" => {
                self.movement_sensor_get_orientation(payload)
            }
            "/viam.component.movementsensor.v1.MovementSensorService/GetAccuracy" => {
                self.movement_sensor_get_accuracy(payload)
            }
            "/viam.component.encoder.v1.EncoderService/GetPosition" => {
                self.encoder_get_position(payload)
            }
            "/viam.component.encoder.v1.EncoderService/ResetPosition" => {
                self.encoder_reset_position(payload)
            }
            "/viam.component.encoder.v1.EncoderService/GetProperties" => {
                self.encoder_get_properties(payload)
            }
            _ => anyhow::bail!("unimplemented method"),
        }
    }

    fn process_request(&mut self, path: &str, msg: Bytes) {
        let payload = Self::validate_rpc(&msg);
        if payload.is_err() || self.handle_request(path, payload.unwrap()).is_err() {
            self.response.set_status(12, Some("unimplemented"));
        }
    }

    fn motor_get_position(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: motor_get_position")
    }

    fn motor_get_properties(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: motor_get_properties")
    }

    fn motor_go_for(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: motor_go_for")
    }

    fn motor_go_to(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: motor_go_to")
    }

    fn motor_is_powered(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: motor_is_powered")
    }

    fn motor_reset_zero_position(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: motor_reset_zero_position")
    }

    fn auth_service_authentificate(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let _req = proto::rpc::v1::AuthenticateRequest::decode(message)?;
        let resp = proto::rpc::v1::AuthenticateResponse {
            access_token: "esp32".to_string(),
        };
        self.encode_message(resp)
    }

    fn motor_set_power(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::motor::v1::SetPowerRequest::decode(message)?;
        let motor = match self.robot.lock().unwrap().get_motor_by_name(req.name) {
            Some(m) => m,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        motor.lock().unwrap().set_power(req.power_pct)?;
        let resp = component::motor::v1::SetPowerResponse {};
        self.encode_message(resp)
    }

    fn motor_stop(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::motor::v1::StopRequest::decode(message)?;
        let motor = match self.robot.lock().unwrap().get_motor_by_name(req.name) {
            Some(m) => m,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        motor.lock().unwrap().set_power(0.0)?;
        let resp = component::motor::v1::StopResponse {};
        self.encode_message(resp)
    }

    fn board_get_digital_interrupt_value(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: board_get_digital_interrupt_value")
    }

    fn board_status(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::board::v1::StatusRequest::decode(message)?;
        let board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        let status = board.lock().unwrap().get_board_status()?;
        let status = component::board::v1::StatusResponse {
            status: Some(status),
        };
        self.encode_message(status)
    }

    fn board_pwm(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: board_pwm")
    }

    fn board_pwm_frequency(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: board_pwm_frequency")
    }

    fn board_read_analog_reader(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::board::v1::ReadAnalogReaderRequest::decode(message)?;
        let board = match self.robot.lock().unwrap().get_board_by_name(req.board_name) {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        let reader = board.get_analog_reader_by_name(req.analog_reader_name)?;
        let resp = component::board::v1::ReadAnalogReaderResponse {
            value: reader.borrow_mut().read()? as i32,
        };
        self.encode_message(resp)
    }

    fn board_set_pin(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::board::v1::SetGpioRequest::decode(message)?;
        let board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };

        let pin: i32 = req.pin.parse::<i32>().unwrap();
        let is_high = req.high;
        board.lock().unwrap().set_gpio_pin_level(pin, is_high)?;
        let resp = component::board::v1::SetGpioResponse {};
        self.encode_message(resp)
    }

    fn board_set_pwm(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: board_set_pwm")
    }

    fn board_set_pwm_frequency(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: board_set_pwm_frequency")
    }

    fn board_set_power_mode(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::board::v1::SetPowerModeRequest::decode(message)?;
        let pm = req.power_mode();

        anyhow::ensure!(
            pm != component::board::v1::PowerMode::Unspecified,
            "unimplemented: board_set_power_mode: cannot honor the 'unspecified' power mode"
        );

        let dur = match req.duration {
            Some(dur) => match Duration::try_from(dur) {
                Ok(converted) => Some(converted),
                Err(e) => return Err(e.into()),
            },
            None => None,
        };

        let board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };

        board.lock().unwrap().set_power_mode(pm, dur)?;

        let resp = component::board::v1::SetPowerModeResponse {};
        self.encode_message(resp)
    }

    fn board_get_pin(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::board::v1::GetGpioRequest::decode(message)?;
        let board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };

        let pin: i32 = req.pin.parse::<i32>().unwrap();
        let level = board.lock().unwrap().get_gpio_level(pin)?;
        let resp = component::board::v1::GetGpioResponse { high: level };
        self.encode_message(resp)
    }

    fn sensor_get_readings(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::sensor::v1::GetReadingsRequest::decode(message)?;
        let sensor = match self.robot.lock().unwrap().get_sensor_by_name(req.name) {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };

        let readings = sensor.lock().unwrap().get_generic_readings()?;
        let resp = component::sensor::v1::GetReadingsResponse { readings };
        self.encode_message(resp)
    }

    fn movement_sensor_get_position(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::movement_sensor::v1::GetPositionRequest::decode(message)?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        let position = m_sensor.lock().unwrap().get_position()?;
        let resp = component::movement_sensor::v1::GetPositionResponse::from(position);
        self.encode_message(resp)
    }

    fn movement_sensor_get_linear_velocity(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::movement_sensor::v1::GetLinearVelocityRequest::decode(message)?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        let l_vel = m_sensor.lock().unwrap().get_linear_velocity()?;
        let l_vel_msg = proto::common::v1::Vector3::from(l_vel);
        let resp = component::movement_sensor::v1::GetLinearVelocityResponse {
            linear_velocity: Some(l_vel_msg),
        };
        self.encode_message(resp)
    }

    fn movement_sensor_get_angular_velocity(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::movement_sensor::v1::GetAngularVelocityRequest::decode(message)?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        let a_vel = m_sensor.lock().unwrap().get_angular_velocity()?;
        let a_vel_msg = proto::common::v1::Vector3::from(a_vel);
        let resp = component::movement_sensor::v1::GetAngularVelocityResponse {
            angular_velocity: Some(a_vel_msg),
        };
        self.encode_message(resp)
    }

    fn movement_sensor_get_linear_acceleration(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::movement_sensor::v1::GetLinearAccelerationRequest::decode(message)?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        let l_acc = m_sensor.lock().unwrap().get_linear_acceleration()?;
        let l_acc_msg = proto::common::v1::Vector3::from(l_acc);
        let resp = component::movement_sensor::v1::GetLinearAccelerationResponse {
            linear_acceleration: Some(l_acc_msg),
        };
        self.encode_message(resp)
    }

    fn movement_sensor_get_compass_heading(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::movement_sensor::v1::GetCompassHeadingRequest::decode(message)?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        let heading = m_sensor.lock().unwrap().get_compass_heading()?;
        let resp = component::movement_sensor::v1::GetCompassHeadingResponse { value: heading };
        self.encode_message(resp)
    }

    fn movement_sensor_get_properties(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::movement_sensor::v1::GetPropertiesRequest::decode(message)?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        let props = m_sensor.lock().unwrap().get_properties();
        let resp = component::movement_sensor::v1::GetPropertiesResponse::from(props);
        self.encode_message(resp)
    }

    fn movement_sensor_get_accuracy(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented movement_sensor_get_accuracy")
    }

    fn movement_sensor_get_orientation(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented movement_sensor_get_orientation")
    }

    fn base_move_straight(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: base_move_straight")
    }

    fn base_spin(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: base_spin")
    }

    fn base_set_velocity(&mut self, _: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: base_set_velocity")
    }

    fn base_set_power(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::base::v1::SetPowerRequest::decode(message)?;
        let base = match self.robot.lock().unwrap().get_base_by_name(req.name) {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        base.lock().unwrap().set_power(
            &req.linear.unwrap_or_default(),
            &req.angular.unwrap_or_default(),
        )?;
        let resp = component::base::v1::SetPowerResponse {};
        self.encode_message(resp)
    }

    fn base_stop(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::base::v1::StopRequest::decode(message)?;
        let base = match self.robot.lock().unwrap().get_base_by_name(req.name) {
            Some(b) => b,
            None => return Err(anyhow::anyhow!("resource not found")),
        };

        base.lock().unwrap().stop()?;
        let resp = component::base::v1::StopResponse {};
        self.encode_message(resp)
    }

    fn encoder_get_properties(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::encoder::v1::GetPropertiesRequest::decode(message)?;
        let enc = match self.robot.lock().unwrap().get_encoder_by_name(req.name) {
            Some(e) => e,
            None => return Err(anyhow::anyhow!("resource not found")),
        };

        let props = enc.lock().unwrap().get_properties();
        let resp = component::encoder::v1::GetPropertiesResponse::from(props);
        self.encode_message(resp)
    }

    fn encoder_get_position(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::encoder::v1::GetPositionRequest::decode(message)?;
        let name = req.name.clone();
        let pos_type = req.position_type();
        let enc = match self.robot.lock().unwrap().get_encoder_by_name(name) {
            Some(e) => e,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        let pos = enc.lock().unwrap().get_position(pos_type.into())?;
        let resp = component::encoder::v1::GetPositionResponse::from(pos);
        self.encode_message(resp)
    }

    fn encoder_reset_position(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::encoder::v1::ResetPositionRequest::decode(message)?;
        let enc = match self.robot.lock().unwrap().get_encoder_by_name(req.name) {
            Some(e) => e,
            None => return Err(anyhow::anyhow!("resource not found")),
        };
        enc.lock().unwrap().reset_position()?;
        let resp = component::encoder::v1::ResetPositionResponse {};
        self.encode_message(resp)
    }

    // This is a placeholder for status stream; app.viam.com requires it when
    // connecting over webrtc and in this implementation we return one response only.
    fn robot_status_stream(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = robot::v1::StreamStatusRequest::decode(message)?;
        // fake a GetStatusRequest because local robot expect this
        let req = robot::v1::GetStatusRequest {
            resource_names: req.resource_names,
        };
        let status = robot::v1::StreamStatusResponse {
            status: self.robot.lock().unwrap().get_status(req)?,
        };
        self.encode_message(status)
    }

    // robot_get_operations returns an empty response since operations are not yet
    // supported on micro-rdk
    fn robot_get_oprations(&mut self, _: &[u8]) -> anyhow::Result<()> {
        let operation = robot::v1::GetOperationsResponse::default();
        self.encode_message(operation)
    }

    fn robot_status(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = robot::v1::GetStatusRequest::decode(message)?;
        let status = robot::v1::GetStatusResponse {
            status: self.robot.lock().unwrap().get_status(req)?,
        };
        self.encode_message(status)
    }

    #[cfg(feature = "camera")]
    fn camera_get_frame(&mut self, message: &[u8]) -> anyhow::Result<()> {
        let req = component::camera::v1::GetImageRequest::decode(message)?;
        if let Some(camera) = self.robot.lock().unwrap().get_camera_by_name(req.name) {
            // TODO: Modify `get_frame` to return a data structure that can be passed into
            // `encode_message`, rather than re-implementing `encode_message` here. See
            // https://viam.atlassian.net/browse/RSDK-824
            let mut buffer = RefCell::borrow_mut(&self.buffer).split_off(0);
            buffer.put_u8(0);
            buffer.put_u32(0.try_into().unwrap());
            let msg = buffer.split_off(5);
            let msg = camera.lock().unwrap().get_frame(msg)?;
            let len = msg.len().to_be_bytes();
            buffer[1] = len[0];
            buffer[2] = len[1];
            buffer[3] = len[2];
            buffer[4] = len[3];
            buffer.unsplit(msg);
            self.response.put_data(buffer.freeze());
            return Ok(());
        }
        Err(anyhow::anyhow!("resource not found"))
    }

    #[cfg(feature = "camera")]
    fn camera_get_point_cloud(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: camera_get_point_cloud")
    }

    #[cfg(feature = "camera")]
    fn camera_get_properties(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: camera_get_properties")
    }

    #[cfg(feature = "camera")]
    fn camera_render_frame(&mut self, _message: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("unimplemented: camera_render_frame")
    }

    fn resource_names(&mut self, _unused_message: &[u8]) -> anyhow::Result<()> {
        let rr = self.robot.lock().unwrap().get_resource_names()?;
        let rr = robot::v1::ResourceNamesResponse { resources: rr };
        self.encode_message(rr)
    }

    fn encode_message<M: Message>(&mut self, m: M) -> anyhow::Result<()> {
        let mut buffer = RefCell::borrow_mut(&self.buffer).split_off(0);
        // The buffer will have a null byte, then 4 bytes containing the big-endian length of the
        // data (*not* including this 5-byte header), and then the data from the message itself.
        if 5 + m.encoded_len() > buffer.capacity() {
            return Err(anyhow::anyhow!("not enough space"));
        }
        buffer.put_u8(0);
        buffer.put_u32(m.encoded_len().try_into().unwrap());
        let mut msg = buffer.split();
        m.encode(&mut msg)?;
        buffer.unsplit(msg);
        self.response.put_data(buffer.freeze());
        Ok(())
    }
}

impl<R> WebRtcGrpcService for GrpcServer<R>
where
    R: GrpcResponse + 'static,
{
    fn unary_rpc(
        &mut self,
        method: &str,
        data: &Bytes,
    ) -> Result<Bytes, crate::google::rpc::Status> {
        {
            let cap = RefCell::borrow(&self.buffer).capacity();
            let len = RefCell::borrow(&self.buffer).len();
            log::info!("current status of buffer is cap: {:?} len: {:?}", cap, len);
        }
        debug!("webRTC");
        {
            RefCell::borrow_mut(&self.buffer).reserve(GRPC_BUFFER_SIZE);
        }
        log::info!("req is {:?}, ", method);
        match self.handle_request(method, data) {
            Err(e) => {
                log::error!("received error {:?}", e);
                Err(Status {
                    code: 12,
                    message: "unimplemented".to_owned(),
                    ..Default::default()
                })
            }
            Ok(_) => Ok(self.response.get_data().split_off(5)),
        }
    }
}

impl<R> Service<Request<Body>> for GrpcServer<R>
where
    R: GrpcResponse + HttpBody + Clone + 'static,
{
    type Response = Response<R>;
    type Error = MyErr;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        #[cfg(debug_assertions)]
        debug!("clone in Servive GRPC");
        {
            RefCell::borrow_mut(&self.buffer).reserve(GRPC_BUFFER_SIZE);
        }
        let mut svc = self.clone();
        #[cfg(debug_assertions)]
        log::debug!("processing {:?}", req);
        Box::pin(async move {
            let (path, body) = req.into_parts();
            let msg = body::to_bytes(body).await.map_err(|_| MyErr)?;
            let path = match path.uri.path_and_query() {
                Some(path) => path.as_str(),
                None => return Err(MyErr),
            };
            svc.process_request(path, msg);
            Response::builder()
                .header("content-type", "application/grpc")
                .status(200)
                .body(svc.response.clone())
                .map_err(|_| MyErr {})
        })
    }

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
impl<R> Drop for GrpcServer<R> {
    fn drop(&mut self) {
        debug!("Server dropped");
    }
}

#[derive(Debug, Default)]
pub struct MyErr;

impl std::error::Error for MyErr {}

impl std::fmt::Display for MyErr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("<todo> error types")
    }
}

pub struct MakeSvcGrpcServer {
    server: GrpcServer<GrpcBody>,
}

impl MakeSvcGrpcServer {
    #[allow(dead_code)]
    pub fn new(robot: Arc<Mutex<LocalRobot>>) -> Self {
        MakeSvcGrpcServer {
            server: GrpcServer::new(robot, GrpcBody::new()),
        }
    }
}

impl<T> Service<T> for MakeSvcGrpcServer {
    type Response = GrpcServer<GrpcBody>;
    type Error = MyErr;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Ok(()).into()
    }

    fn call(&mut self, _: T) -> Self::Future {
        {
            info!("reserve memory");
            RefCell::borrow_mut(&self.server.buffer).reserve(10240);
        }
        future::ready(Ok(self.server.clone()))
    }
}

pub struct Timeout<T> {
    inner: T,
    timeout: Duration,
}

impl<T> Timeout<T> {
    #[allow(dead_code)]
    pub fn new(inner: T, timeout: Duration) -> Timeout<T> {
        Timeout { inner, timeout }
    }
}

// The error returned if processing a request timed out
#[derive(Debug)]
pub struct Expired;

impl std::fmt::Display for Expired {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "expired")
    }
}

impl<T> Service<Request<Body>> for Timeout<T>
where
    T: Service<Request<Body>>,
    T::Error: Into<Box<dyn Error + Send + Sync>> + 'static,
    T::Future: 'static,
{
    type Response = T::Response;
    type Error = Box<dyn Error + Send + Sync>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let fut = self.inner.call(req);
        let timeout = self.timeout;
        let f = async move {
            if let Some(req) = fut.timeout(timeout).await {
                return req.map_err(Into::into);
            }
            info!("timeout");
            Err(Box::new(MyErr))
        };
        Box::pin(f)
    }
}
