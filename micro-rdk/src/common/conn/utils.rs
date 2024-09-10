use crate::common::webrtc::dtls::{DtlsError, DtlsStream, IntoDtlsStream};

impl IntoDtlsStream for futures_lite::future::Pending<Result<Box<dyn DtlsStream>, DtlsError>> {}
