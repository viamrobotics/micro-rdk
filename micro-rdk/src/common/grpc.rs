use core::fmt;
use std::{
    convert::Infallible,
    fmt::Debug,
    marker::PhantomData,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::{
    common::{
        analog::AnalogReader, board::Board, motor::Motor, robot::LocalRobot,
        webrtc::grpc::WebRtcGrpcService,
    },
    google::rpc::Status,
    proto::{self, component, robot},
};
use bytes::{BufMut, BytesMut};
use futures_lite::{future, Future};
use http_body_util::BodyExt;
use hyper::{
    body::{self, Body, Bytes, Frame},
    http::HeaderValue,
    service::Service,
    HeaderMap, Request, Response,
};
use log::*;
use prost::Message;
use std::{
    cell::RefCell,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
use thiserror::Error;

#[cfg(feature = "camera")]
static GRPC_BUFFER_SIZE: usize = 1024 * 30; // 30KB
#[cfg(not(feature = "camera"))]
static GRPC_BUFFER_SIZE: usize = 9216;

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
    fn set_status(&mut self, code: i32, message: Option<String>) {
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

impl Body for GrpcBody {
    type Data = Bytes;
    type Error = hyper::http::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<body::Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        if let Some(data) = this.data.take() {
            return Poll::Ready(Some(Ok(Frame::data(data))));
        }
        if let Some(trailer) = this.trailers.take() {
            return Poll::Ready(Some(Ok(Frame::trailers(trailer))));
        }
        Poll::Pending
    }
}

pub trait GrpcResponse {
    fn put_data(&mut self, data: Bytes);
    fn insert_trailer(&mut self, key: &'static str, value: &'_ str);
    fn set_status(&mut self, code: i32, message: Option<String>);
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
        GrpcServer {
            response: body,
            buffer: Rc::new(RefCell::new(BytesMut::with_capacity(GRPC_BUFFER_SIZE))),
            robot,
        }
    }

    fn validate_rpc(message: &Bytes) -> Result<&[u8], GrpcError> {
        // Per https://github.com/grpc/grpc/blob/master/doc/PROTOCOL-HTTP2.md, we're expecting a
        // 5-byte header followed by the actual protocol buffer data. The 5 bytes in the header are
        // 1 null byte (indicating we're not using compression), and 4 bytes of a big-endian
        // integer describing the length of the rest of the data.
        if message.len() < 5 {
            return Err(GrpcError::RpcFailedPrecondition);
        }
        let (header, rest) = message.split_at(5);
        let (use_compression, expected_len) = header.split_at(1);
        if use_compression[0] != 0 {
            return Err(GrpcError::RpcFailedPrecondition);
        }
        let expected_len = u32::from_be_bytes(expected_len.try_into().unwrap());
        if expected_len != rest.len() as u32 {
            return Err(GrpcError::RpcInvalidArgument);
        }
        Ok(rest)
    }

    pub(crate) fn handle_rpc_stream(
        &mut self,
        path: &str,
        payload: &[u8],
    ) -> Result<std::time::Instant, ServerError> {
        match path {
            "/viam.robot.v1.RobotService/StreamStatus" => self.robot_status_stream(payload),
            _ => Err(ServerError::from(GrpcError::RpcUnavailable)),
        }
    }

    pub(crate) fn handle_request(&mut self, path: &str, payload: &[u8]) -> Result<(), ServerError> {
        match path {
            "/viam.component.base.v1.BaseService/SetPower" => self.base_set_power(payload),
            "/viam.component.base.v1.BaseService/Stop" => self.base_stop(payload),
            "/viam.component.base.v1.BaseService/MoveStraight" => self.base_move_straight(payload),
            "/viam.component.base.v1.BaseService/Spin" => self.base_spin(payload),
            "/viam.component.base.v1.BaseService/SetVelocity" => self.base_set_velocity(payload),
            "/viam.component.base.v1.BaseService/IsMoving" => self.base_is_moving(payload),
            "/viam.component.board.v1.BoardService/GetDigitalInterruptValue" => {
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
            "/viam.component.board.v1.BoardService/SetPowerMode" => {
                self.board_set_power_mode(payload)
            }
            "/viam.component.board.v1.BoardService/DoCommand" => self.board_do_command(payload),
            "/viam.component.generic.v1.GenericService/DoCommand" => {
                self.generic_component_do_command(payload)
            }
            #[cfg(feature = "camera")]
            "/viam.component.camera.v1.CameraService/GetImage" => self.camera_get_image(payload),
            #[cfg(feature = "camera")]
            "/viam.component.camera.v1.CameraService/RenderFrame" => {
                self.camera_render_frame(payload)
            }
            #[cfg(feature = "camera")]
            "/viam.component.camera.v1.CameraService/DoCommand" => self.camera_do_command(payload),
            "/viam.component.motor.v1.MotorService/GetPosition" => self.motor_get_position(payload),
            "/viam.component.motor.v1.MotorService/GetProperties" => {
                self.motor_get_properties(payload)
            }
            "/viam.component.motor.v1.MotorService/GoFor" => self.motor_go_for(payload),
            "/viam.component.motor.v1.MotorService/GoTo" => self.motor_go_to(payload),
            "/viam.component.motor.v1.MotorService/IsPowered" => self.motor_is_powered(payload),
            "/viam.component.motor.v1.MotorService/IsMoving" => self.motor_is_moving(payload),
            "/viam.component.motor.v1.MotorService/ResetZeroPosition" => {
                self.motor_reset_zero_position(payload)
            }
            "/viam.component.motor.v1.MotorService/SetPower" => self.motor_set_power(payload),
            "/viam.component.motor.v1.MotorService/Stop" => self.motor_stop(payload),
            "/viam.component.motor.v1.MotorService/SetRPM" => self.motor_set_rpm(payload),
            "/viam.component.motor.v1.MotorService/DoCommand" => self.motor_do_command(payload),
            "/viam.robot.v1.RobotService/ResourceNames" => self.resource_names(payload),
            "/viam.robot.v1.RobotService/GetStatus" => self.robot_status(payload),
            "/viam.robot.v1.RobotService/GetOperations" => self.robot_get_operations(payload),
            "/viam.robot.v1.RobotService/Shutdown" => self.robot_shutdown(payload),
            "/proto.rpc.v1.AuthService/Authenticate" => self.auth_service_authentificate(payload),
            "/viam.component.sensor.v1.SensorService/GetReadings" => {
                self.sensor_get_readings(payload)
            }
            "/viam.component.sensor.v1.SensorService/DoCommand" => self.sensor_do_command(payload),
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
            "/viam.component.movementsensor.v1.MovementSensorService/DoCommand" => {
                self.movement_sensor_do_command(payload)
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
            "/viam.component.encoder.v1.EncoderService/DoCommand" => {
                self.encoder_do_command(payload)
            }
            "/viam.component.powersensor.v1.PowerSensorService/GetVoltage" => {
                self.power_sensor_get_voltage(payload)
            }
            "/viam.component.powersensor.v1.PowerSensorService/GetCurrent" => {
                self.power_sensor_get_current(payload)
            }
            "/viam.component.powersensor.v1.PowerSensorService/GetPower" => {
                self.power_sensor_get_power(payload)
            }
            "/viam.component.powersensor.v1.PowerSensorService/DoCommand" => {
                self.power_sensor_do_command(payload)
            }
            "/viam.component.servo.v1.ServoService/Move" => self.servo_move(payload),
            "/viam.component.servo.v1.ServoService/GetPosition" => self.servo_get_position(payload),
            "/viam.component.servo.v1.ServoService/IsMoving" => self.servo_is_moving(payload),
            "/viam.component.servo.v1.ServoService/Stop" => self.servo_stop(payload),
            "/viam.component.servo.v1.ServoService/DoCommand" => self.servo_do_command(payload),
            _ => Err(ServerError::from(GrpcError::RpcUnimplemented)),
        }
    }

    fn process_request(&mut self, path: &str, msg: Bytes) {
        let payload = Self::validate_rpc(&msg).map_err(ServerError::from);
        match payload.and_then(|payload| self.handle_request(path, payload)) {
            Ok(_) => {}
            Err(e) => {
                let message = Some(e.to_string());
                self.response.set_status(e.status_code(), message);
            }
        }
    }

    fn motor_get_position(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::motor::v1::GetPositionRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let motor = match self.robot.lock().unwrap().get_motor_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let pos = motor
            .lock()
            .unwrap()
            .get_position()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::motor::v1::GetPositionResponse {
            position: pos as f64,
        };
        self.encode_message(resp)
    }

    fn motor_get_properties(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::motor::v1::GetPropertiesRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let motor = match self.robot.lock().unwrap().get_motor_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let props: component::motor::v1::GetPropertiesResponse =
            motor.lock().unwrap().get_properties().into();
        self.encode_message(props)
    }

    fn motor_go_for(&mut self, _message: &[u8]) -> Result<(), ServerError> {
        // TODO: internal go_for can't wait without blocking executor, must be waited from here.
        // requires refactoring this function (and its callers) to be async
        /*
        let req = component::motor::v1::GoForRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let motor = match self.robot.lock().unwrap().get_motor_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let mut motor = motor.lock().unwrap();

        if let Some(dur) =  motor.go_for(req.rpm, req.revolutions).map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err)))? {
            // async wait for duration
        }
        motor.lock().unwrap();

        let resp = component::motor::v1::GoForResponse {};
        self.encode_message(resp)
        */
        Err(ServerError::from(GrpcError::RpcUnimplemented))
    }

    fn motor_go_to(&mut self, _message: &[u8]) -> Result<(), ServerError> {
        Err(ServerError::from(GrpcError::RpcUnimplemented))
    }

    fn motor_is_powered(&mut self, _message: &[u8]) -> Result<(), ServerError> {
        Err(ServerError::from(GrpcError::RpcUnimplemented))
    }

    fn motor_is_moving(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::motor::v1::IsMovingRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let motor = match self.robot.lock().unwrap().get_motor_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let resp = component::motor::v1::IsMovingResponse {
            is_moving: motor
                .lock()
                .unwrap()
                .is_moving()
                .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?,
        };
        self.encode_message(resp)
    }

    fn motor_reset_zero_position(&mut self, _message: &[u8]) -> Result<(), ServerError> {
        Err(ServerError::from(GrpcError::RpcUnimplemented))
    }

    fn motor_do_command(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = proto::common::v1::DoCommandRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let motor = match self.robot.lock().unwrap().get_motor_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let res = motor
            .lock()
            .unwrap()
            .do_command(req.command)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let resp = proto::common::v1::DoCommandResponse { result: res };
        self.encode_message(resp)
    }

    fn auth_service_authentificate(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let _req = proto::rpc::v1::AuthenticateRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let resp = proto::rpc::v1::AuthenticateResponse {
            access_token: "esp32".to_string(),
        };
        self.encode_message(resp)
    }

    fn motor_set_power(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::motor::v1::SetPowerRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let motor = match self.robot.lock().unwrap().get_motor_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        motor
            .lock()
            .unwrap()
            .set_power(req.power_pct)
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::motor::v1::SetPowerResponse {};
        self.encode_message(resp)
    }

    fn motor_set_rpm(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::motor::v1::SetRpmRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let mut motor = match self.robot.lock().unwrap().get_motor_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        motor
            .set_rpm(req.rpm)
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::motor::v1::SetRpmResponse {};
        self.encode_message(resp)
    }

    fn motor_stop(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::motor::v1::StopRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let motor = match self.robot.lock().unwrap().get_motor_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        motor
            .lock()
            .unwrap()
            .stop()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::motor::v1::StopResponse {};
        self.encode_message(resp)
    }

    fn servo_move(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::servo::v1::MoveRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let servo = match self.robot.lock().unwrap().get_servo_by_name(req.name) {
            Some(s) => s,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        servo
            .lock()
            .unwrap()
            .move_to(req.angle_deg)
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::servo::v1::MoveResponse {};
        self.encode_message(resp)
    }

    fn servo_get_position(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::servo::v1::GetPositionRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let servo = match self.robot.lock().unwrap().get_servo_by_name(req.name) {
            Some(s) => s,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let pos = servo
            .lock()
            .unwrap()
            .get_position()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::servo::v1::GetPositionResponse { position_deg: pos };
        self.encode_message(resp)
    }

    fn servo_is_moving(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::servo::v1::IsMovingRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let servo = match self.robot.lock().unwrap().get_servo_by_name(req.name) {
            Some(s) => s,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let resp = component::servo::v1::IsMovingResponse {
            is_moving: servo
                .lock()
                .unwrap()
                .is_moving()
                .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?,
        };
        self.encode_message(resp)
    }

    fn servo_stop(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::servo::v1::StopRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let servo = match self.robot.lock().unwrap().get_servo_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        servo
            .lock()
            .unwrap()
            .stop()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::servo::v1::StopResponse {};
        self.encode_message(resp)
    }

    fn servo_do_command(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = proto::common::v1::DoCommandRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let servo = match self.robot.lock().unwrap().get_servo_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let res = servo
            .lock()
            .unwrap()
            .do_command(req.command)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let resp = proto::common::v1::DoCommandResponse { result: res };
        self.encode_message(resp)
    }

    fn board_get_digital_interrupt_value(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::board::v1::GetDigitalInterruptValueRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let board = match self.robot.lock().unwrap().get_board_by_name(req.board_name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let interrupt_pin = req
            .digital_interrupt_name
            .parse::<i32>()
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let value = board
            .get_digital_interrupt_value(interrupt_pin)
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?
            .into();
        let resp = component::board::v1::GetDigitalInterruptValueResponse { value };
        self.encode_message(resp)
    }

    fn board_pwm(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::board::v1::PwmRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let pin: i32 = req
            .pin
            .parse::<i32>()
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let duty_cycle_pct = board.get_pwm_duty(pin);
        let resp = component::board::v1::PwmResponse { duty_cycle_pct };
        self.encode_message(resp)
    }

    fn board_pwm_frequency(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::board::v1::PwmFrequencyRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let pin: i32 = req
            .pin
            .parse::<i32>()
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let frequency_hz = board
            .get_pwm_frequency(pin)
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::board::v1::PwmFrequencyResponse { frequency_hz };
        self.encode_message(resp)
    }

    fn board_read_analog_reader(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::board::v1::ReadAnalogReaderRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let board = match self.robot.lock().unwrap().get_board_by_name(req.board_name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let mut reader = board
            .get_analog_reader_by_name(req.analog_reader_name)
            .map_err(|err| ServerError::new(GrpcError::RpcUnavailable, Some(err.into())))?;
        let resolution = reader.resolution();
        let resp = component::board::v1::ReadAnalogReaderResponse {
            value: reader
                .read()
                .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?
                as i32,
            min_range: resolution.min_range,
            max_range: resolution.max_range,
            step_size: resolution.step_size,
        };
        self.encode_message(resp)
    }

    fn board_set_pin(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::board::v1::SetGpioRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };

        let pin: i32 = req
            .pin
            .parse::<i32>()
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let is_high = req.high;
        board
            .lock()
            .unwrap()
            .set_gpio_pin_level(pin, is_high)
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::board::v1::SetGpioResponse {};
        self.encode_message(resp)
    }

    fn board_set_pwm(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::board::v1::SetPwmRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let mut board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let pin: i32 = req
            .pin
            .parse::<i32>()
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;

        // ignore error to match behavior on RDK
        let _ = board.set_pwm_duty(pin, req.duty_cycle_pct);

        let resp = component::board::v1::SetPwmResponse {};
        self.encode_message(resp)
    }

    fn board_set_pwm_frequency(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::board::v1::SetPwmFrequencyRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let mut board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let pin: i32 = req
            .pin
            .parse::<i32>()
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;

        // ignore error to match behavior on RDK
        let _ = board.set_pwm_frequency(pin, req.frequency_hz);
        let resp = component::board::v1::SetPwmFrequencyResponse {};
        self.encode_message(resp)
    }

    fn board_set_power_mode(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::board::v1::SetPowerModeRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let pm = req.power_mode();

        if pm == component::board::v1::PowerMode::Unspecified {
            return Err(ServerError::from(GrpcError::RpcInvalidArgument));
        }

        let dur = match req.duration {
            Some(dur) => match Duration::try_from(dur) {
                Ok(converted) => Some(converted),
                Err(_) => return Err(ServerError::from(GrpcError::RpcInvalidArgument)),
            },
            None => None,
        };

        let board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };

        board
            .lock()
            .unwrap()
            .set_power_mode(pm, dur)
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;

        let resp = component::board::v1::SetPowerModeResponse {};
        self.encode_message(resp)
    }

    fn board_get_pin(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::board::v1::GetGpioRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };

        let pin: i32 = req
            .pin
            .parse::<i32>()
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let level = board
            .lock()
            .unwrap()
            .get_gpio_level(pin)
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::board::v1::GetGpioResponse { high: level };
        self.encode_message(resp)
    }

    fn board_do_command(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = proto::common::v1::DoCommandRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let board = match self.robot.lock().unwrap().get_board_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let res = board
            .lock()
            .unwrap()
            .do_command(req.command)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let resp = proto::common::v1::DoCommandResponse { result: res };
        self.encode_message(resp)
    }

    fn generic_component_do_command(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = proto::common::v1::DoCommandRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let component = match self
            .robot
            .lock()
            .unwrap()
            .get_generic_component_by_name(req.name)
        {
            Some(c) => c,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let res = component
            .lock()
            .unwrap()
            .do_command(req.command)
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = proto::common::v1::DoCommandResponse { result: res };
        self.encode_message(resp)
    }

    fn sensor_get_readings(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = proto::common::v1::GetReadingsRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let sensor = match self.robot.lock().unwrap().get_sensor_by_name(req.name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };

        let readings = sensor
            .lock()
            .unwrap()
            .get_generic_readings()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = proto::common::v1::GetReadingsResponse { readings };
        self.encode_message(resp)
    }

    fn sensor_do_command(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = proto::common::v1::DoCommandRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let sensor = match self.robot.lock().unwrap().get_sensor_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let res = sensor
            .lock()
            .unwrap()
            .do_command(req.command)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let resp = proto::common::v1::DoCommandResponse { result: res };
        self.encode_message(resp)
    }

    fn movement_sensor_get_position(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::movement_sensor::v1::GetPositionRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let position = m_sensor
            .lock()
            .unwrap()
            .get_position()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::movement_sensor::v1::GetPositionResponse::from(position);
        self.encode_message(resp)
    }

    fn movement_sensor_get_linear_velocity(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::movement_sensor::v1::GetLinearVelocityRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let l_vel = m_sensor
            .lock()
            .unwrap()
            .get_linear_velocity()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let l_vel_msg = proto::common::v1::Vector3::from(l_vel);
        let resp = component::movement_sensor::v1::GetLinearVelocityResponse {
            linear_velocity: Some(l_vel_msg),
        };
        self.encode_message(resp)
    }

    fn movement_sensor_get_angular_velocity(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::movement_sensor::v1::GetAngularVelocityRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let a_vel = m_sensor
            .lock()
            .unwrap()
            .get_angular_velocity()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let a_vel_msg = proto::common::v1::Vector3::from(a_vel);
        let resp = component::movement_sensor::v1::GetAngularVelocityResponse {
            angular_velocity: Some(a_vel_msg),
        };
        self.encode_message(resp)
    }

    fn movement_sensor_get_linear_acceleration(
        &mut self,
        message: &[u8],
    ) -> Result<(), ServerError> {
        let req = component::movement_sensor::v1::GetLinearAccelerationRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let l_acc = m_sensor
            .lock()
            .unwrap()
            .get_linear_acceleration()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let l_acc_msg = proto::common::v1::Vector3::from(l_acc);
        let resp = component::movement_sensor::v1::GetLinearAccelerationResponse {
            linear_acceleration: Some(l_acc_msg),
        };
        self.encode_message(resp)
    }

    fn movement_sensor_get_compass_heading(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::movement_sensor::v1::GetCompassHeadingRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let heading = m_sensor
            .lock()
            .unwrap()
            .get_compass_heading()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::movement_sensor::v1::GetCompassHeadingResponse { value: heading };
        self.encode_message(resp)
    }

    fn movement_sensor_get_properties(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::movement_sensor::v1::GetPropertiesRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let m_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let props = m_sensor.lock().unwrap().get_properties();
        let resp = component::movement_sensor::v1::GetPropertiesResponse::from(props);
        self.encode_message(resp)
    }

    fn movement_sensor_get_accuracy(&mut self, _message: &[u8]) -> Result<(), ServerError> {
        Err(ServerError::from(GrpcError::RpcUnimplemented))
    }

    fn movement_sensor_get_orientation(&mut self, _message: &[u8]) -> Result<(), ServerError> {
        Err(ServerError::from(GrpcError::RpcUnimplemented))
    }

    fn movement_sensor_do_command(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = proto::common::v1::DoCommandRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let movement_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_movement_sensor_by_name(req.name)
        {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let res = movement_sensor
            .lock()
            .unwrap()
            .do_command(req.command)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let resp = proto::common::v1::DoCommandResponse { result: res };
        self.encode_message(resp)
    }

    fn base_move_straight(&mut self, _message: &[u8]) -> Result<(), ServerError> {
        Err(ServerError::from(GrpcError::RpcUnimplemented))
    }

    fn base_spin(&mut self, _message: &[u8]) -> Result<(), ServerError> {
        Err(ServerError::from(GrpcError::RpcUnimplemented))
    }

    fn base_set_velocity(&mut self, _: &[u8]) -> Result<(), ServerError> {
        Err(ServerError::from(GrpcError::RpcUnimplemented))
    }

    fn base_is_moving(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::base::v1::IsMovingRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let base = match self.robot.lock().unwrap().get_base_by_name(req.name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let resp = component::base::v1::IsMovingResponse {
            is_moving: base
                .lock()
                .unwrap()
                .is_moving()
                .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?,
        };
        self.encode_message(resp)
    }

    fn base_set_power(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::base::v1::SetPowerRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let base = match self.robot.lock().unwrap().get_base_by_name(req.name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        base.lock()
            .unwrap()
            .set_power(
                &req.linear.unwrap_or_default(),
                &req.angular.unwrap_or_default(),
            )
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::base::v1::SetPowerResponse {};
        self.encode_message(resp)
    }

    fn base_stop(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::base::v1::StopRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let base = match self.robot.lock().unwrap().get_base_by_name(req.name) {
            Some(b) => b,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };

        base.lock()
            .unwrap()
            .stop()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::base::v1::StopResponse {};
        self.encode_message(resp)
    }

    fn encoder_get_properties(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::encoder::v1::GetPropertiesRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let enc = match self.robot.lock().unwrap().get_encoder_by_name(req.name) {
            Some(e) => e,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };

        let props = enc.lock().unwrap().get_properties();
        let resp = component::encoder::v1::GetPropertiesResponse::from(props);
        self.encode_message(resp)
    }

    fn encoder_get_position(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::encoder::v1::GetPositionRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let name = req.name.clone();
        let pos_type = req.position_type();
        let enc = match self.robot.lock().unwrap().get_encoder_by_name(name) {
            Some(e) => e,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let pos = enc
            .lock()
            .unwrap()
            .get_position(pos_type.into())
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::encoder::v1::GetPositionResponse::from(pos);
        self.encode_message(resp)
    }

    fn encoder_reset_position(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::encoder::v1::ResetPositionRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let enc = match self.robot.lock().unwrap().get_encoder_by_name(req.name) {
            Some(e) => e,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        enc.lock()
            .unwrap()
            .reset_position()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let resp = component::encoder::v1::ResetPositionResponse {};
        self.encode_message(resp)
    }

    fn encoder_do_command(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = proto::common::v1::DoCommandRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let encoder = match self.robot.lock().unwrap().get_encoder_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let res = encoder
            .lock()
            .unwrap()
            .do_command(req.command)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let resp = proto::common::v1::DoCommandResponse { result: res };
        self.encode_message(resp)
    }

    fn power_sensor_get_voltage(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::power_sensor::v1::GetVoltageRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let power_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_power_sensor_by_name(req.name)
        {
            Some(s) => s,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let resp: component::power_sensor::v1::GetVoltageResponse = power_sensor
            .lock()
            .unwrap()
            .get_voltage()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?
            .into();
        self.encode_message(resp)
    }

    fn power_sensor_get_current(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::power_sensor::v1::GetCurrentRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let power_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_power_sensor_by_name(req.name)
        {
            Some(s) => s,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let resp: component::power_sensor::v1::GetCurrentResponse = power_sensor
            .lock()
            .unwrap()
            .get_current()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?
            .into();
        self.encode_message(resp)
    }

    fn power_sensor_get_power(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::power_sensor::v1::GetPowerRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let power_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_power_sensor_by_name(req.name)
        {
            Some(s) => s,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let resp = component::power_sensor::v1::GetPowerResponse {
            watts: power_sensor
                .lock()
                .unwrap()
                .get_power()
                .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?,
        };
        self.encode_message(resp)
    }

    fn power_sensor_do_command(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = proto::common::v1::DoCommandRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let power_sensor = match self
            .robot
            .lock()
            .unwrap()
            .get_power_sensor_by_name(req.name)
        {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let res = power_sensor
            .lock()
            .unwrap()
            .do_command(req.command)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let resp = proto::common::v1::DoCommandResponse { result: res };
        self.encode_message(resp)
    }

    fn robot_status_stream(&mut self, message: &[u8]) -> Result<std::time::Instant, ServerError> {
        let req = robot::v1::StreamStatusRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let duration = Instant::now()
            + TryInto::<Duration>::try_into(req.every.unwrap())
                .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        // fake a GetStatusRequest because local robot expect this
        let req = robot::v1::GetStatusRequest {
            resource_names: req.resource_names,
        };
        let status = robot::v1::StreamStatusResponse {
            status: self
                .robot
                .lock()
                .unwrap()
                .get_status(req)
                .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?,
        };
        self.encode_message(status).map(|_| duration)
    }

    // robot_get_operations returns an empty response since operations are not yet
    // supported on micro-rdk
    fn robot_get_operations(&mut self, _: &[u8]) -> Result<(), ServerError> {
        let operation = robot::v1::GetOperationsResponse::default();
        self.encode_message(operation)
    }

    // robot_shutdown will not return anything because will restart
    fn robot_shutdown(&mut self, _: &[u8]) -> ! {
        #[cfg(feature = "native")]
        std::process::exit(0);
        #[cfg(feature = "esp32")]
        unsafe {
            crate::esp32::esp_idf_svc::sys::esp_restart();
        }
    }

    fn robot_status(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = robot::v1::GetStatusRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let status = robot::v1::GetStatusResponse {
            status: self
                .robot
                .lock()
                .unwrap()
                .get_status(req)
                .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?,
        };
        self.encode_message(status)
    }

    #[cfg(feature = "camera")]
    fn camera_get_image(&mut self, message: &[u8]) -> Result<(), ServerError> {
        // TODO: Modify camera methods (ie `get_image`, `render_frame`) to return a data structure that can be passed into
        // `encode_message`, rather than re-implementing `encode_message` here. See
        // https://viam.atlassian.net/browse/RSDK-824
        let req = component::camera::v1::GetImageRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;

        let camera = self
            .robot
            .lock()
            .unwrap()
            .get_camera_by_name(req.name)
            .ok_or(GrpcError::RpcUnavailable)?;

        let mut buffer = RefCell::borrow_mut(&self.buffer).split_off(0);
        let msg_buf = buffer.split_off(5);

        let msg_buf = camera
            .lock()
            .unwrap()
            .get_image(msg_buf)
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;

        buffer.put_u8(0);
        buffer.put_u32(msg_buf.len() as u32);
        buffer.unsplit(msg_buf);
        self.response.put_data(buffer.freeze());
        Ok(())
    }

    #[cfg(feature = "camera")]
    fn camera_render_frame(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = component::camera::v1::RenderFrameRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;

        let camera = self
            .robot
            .lock()
            .unwrap()
            .get_camera_by_name(req.name)
            .ok_or(GrpcError::RpcUnavailable)?;

        let mut buffer = RefCell::borrow_mut(&self.buffer).split_off(0);
        let msg_buf = buffer.split_off(5);

        let msg_buf = camera
            .lock()
            .unwrap()
            .render_frame(msg_buf)
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;

        buffer.put_u8(0);
        buffer.put_u32(msg_buf.len() as u32);
        buffer.unsplit(msg_buf);
        self.response.put_data(buffer.freeze());
        Ok(())
    }

    #[cfg(feature = "camera")]
    fn camera_do_command(&mut self, message: &[u8]) -> Result<(), ServerError> {
        let req = proto::common::v1::DoCommandRequest::decode(message)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let camera = match self.robot.lock().unwrap().get_camera_by_name(req.name) {
            Some(m) => m,
            None => return Err(ServerError::from(GrpcError::RpcUnavailable)),
        };
        let res = camera
            .lock()
            .unwrap()
            .do_command(req.command)
            .map_err(|_| ServerError::from(GrpcError::RpcInvalidArgument))?;
        let resp = proto::common::v1::DoCommandResponse { result: res };
        self.encode_message(resp)
    }

    fn resource_names(&mut self, _unused_message: &[u8]) -> Result<(), ServerError> {
        let rr = self
            .robot
            .lock()
            .unwrap()
            .get_resource_names()
            .map_err(|err| ServerError::new(GrpcError::RpcInternal, Some(err.into())))?;
        let rr = robot::v1::ResourceNamesResponse { resources: rr };
        self.encode_message(rr)
    }

    fn encode_message<M: Message>(&mut self, m: M) -> Result<(), ServerError> {
        let mut buffer = RefCell::borrow_mut(&self.buffer).split_off(0);
        // The buffer will have a null byte, then 4 bytes containing the big-endian length of the
        // data (*not* including this 5-byte header), and then the data from the message itself.
        if 5 + m.encoded_len() > buffer.capacity() {
            return Err(GrpcError::RpcResourceExhausted.into());
        }
        buffer.put_u8(0);
        buffer.put_u32(m.encoded_len().try_into().unwrap());
        let mut msg = buffer.split_off(5);
        m.encode(&mut msg)
            .map_err(|_| ServerError::from(GrpcError::RpcInternal))?;
        buffer.unsplit(msg);
        self.response.put_data(buffer.freeze());
        Ok(())
    }
}

impl<R> WebRtcGrpcService for GrpcServer<R>
where
    R: GrpcResponse + 'static,
{
    fn unary_rpc(&mut self, method: &str, data: &Bytes) -> Result<Bytes, ServerError> {
        {
            RefCell::borrow_mut(&self.buffer).reserve(GRPC_BUFFER_SIZE);
        }
        self.handle_request(method, data)
            .map(|_| self.response.get_data().split_off(5))
    }
    fn server_stream_rpc(
        &mut self,
        method: &str,
        data: &Bytes,
    ) -> Result<(Bytes, Instant), ServerError> {
        {
            RefCell::borrow_mut(&self.buffer).reserve(GRPC_BUFFER_SIZE);
        }
        log::debug!("stream req is {:?}, ", method);
        self.handle_rpc_stream(method, data)
            .map(|dur| (self.response.get_data().split_off(5), dur))
    }
}

impl<R> Service<Request<body::Incoming>> for GrpcServer<R>
where
    R: GrpcResponse + Body + Clone + 'static,
{
    type Response = Response<R>;
    type Error = GrpcError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn call(&self, req: Request<body::Incoming>) -> Self::Future {
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
            let msg = body
                .collect()
                .await
                .map_err(|_| GrpcError::RpcFailedPrecondition)?
                .to_bytes();

            let path = match path.uri.path_and_query() {
                Some(path) => path.as_str(),
                None => return Err(GrpcError::RpcInvalidArgument),
            };
            svc.process_request(path, msg);
            Response::builder()
                .header("content-type", "application/grpc")
                .status(200)
                .body(svc.response.clone())
                .map_err(|_| GrpcError::RpcFailedPrecondition)
        })
    }
}
impl<R> Drop for GrpcServer<R> {
    fn drop(&mut self) {
        debug!("Server dropped");
    }
}
#[derive(Error, Debug, Clone, Copy)]
pub enum GrpcError {
    #[error("canceled rpc")]
    RpcCanceled = 1,
    #[error("unknown rpc")]
    Unknown = 2,
    #[error("invalid argument for this rpc")]
    RpcInvalidArgument = 3,
    #[error("rpc deadline exceeded")]
    RpcDeadlineExceeded = 4,
    #[error("rpc not found")]
    RpcNotFound = 5,
    #[error("rpc already exists")]
    RpcAlreadyExists = 6,
    #[error("permission denied")]
    RpcPermissionDenied = 7,
    #[error("resource exhausted")]
    RpcResourceExhausted = 8,
    #[error("failed precondition")]
    RpcFailedPrecondition = 9,
    #[error("aborted")]
    RpcAborted = 10,
    #[error("out of range")]
    RpcOutOfRange = 11,
    #[error("Unimplemented")]
    RpcUnimplemented = 12,
    #[error("internal")]
    RpcInternal = 13,
    #[error("unavailable")]
    RpcUnavailable = 14,
    #[error("data loss")]
    RpcDataLoss = 15,
    #[error("unauthenticated")]
    RpcUnauthenticated = 16,
}

impl GrpcError {
    pub fn to_status(self, message: String) -> Status {
        Status {
            code: self as i32,
            message,
            details: vec![],
        }
    }
}

impl From<Infallible> for GrpcError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

#[derive(Debug, Error)]
pub struct ServerError {
    grpc_error: GrpcError,
    #[source]
    cause: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl ServerError {
    pub fn new(
        grpc_error: GrpcError,
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self { grpc_error, cause }
    }

    pub fn to_status(&self) -> Status {
        self.grpc_error.to_status(self.to_string())
    }

    pub fn status_code(&self) -> i32 {
        self.grpc_error as i32
    }
}

impl From<GrpcError> for ServerError {
    fn from(grpc_error: GrpcError) -> Self {
        Self {
            grpc_error,
            cause: None,
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            Some(err) => write!(f, "{}: {}", self.grpc_error, err),
            None => std::fmt::Display::fmt(&self.grpc_error, f),
        }
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
    type Error = GrpcError;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;

    fn call(&self, _: T) -> Self::Future {
        {
            info!("reserve memory");
            RefCell::borrow_mut(&self.server.buffer).reserve(10240);
        }
        future::ready(Ok(self.server.clone()))
    }
}
