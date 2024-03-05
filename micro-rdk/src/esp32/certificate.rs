use crate::common::webrtc::certificate::{Certificate, Fingerprint};

#[derive(Clone)]
pub struct WebRtcCertificate {
    serialized_der: Vec<u8>,
    key_pair: Vec<u8>,
    fingerprint: Fingerprint,
}

impl<'a> WebRtcCertificate {
    pub fn new(serialized_der: Vec<u8>, key_pair: Vec<u8>, fingerprint: &'a str) -> Self {
        Self {
            serialized_der,
            key_pair,
            fingerprint: Fingerprint::try_from(fingerprint).unwrap(),
        }
    }
}

impl Certificate for WebRtcCertificate {
    fn get_der_certificate(&self) -> &'_ [u8] {
        &self.serialized_der
    }
    fn get_der_keypair(&self) -> &'_ [u8] {
        &self.key_pair
    }
    fn get_fingerprint(&self) -> &'_ Fingerprint {
        &self.fingerprint
    }
}
