use crate::common::webrtc::certificate::{Certificate, Fingerprint};

#[derive(Clone)]
pub struct WebRTCCertificate {
    serialized_der: &'static [u8],
    key_pair: &'static [u8],
    fingerprint: Fingerprint,
}

impl WebRTCCertificate {
    pub fn new(
        serialized_der: &'static [u8],
        key_pair: &'static [u8],
        fingerprint: &'static str,
    ) -> Self {
        Self {
            serialized_der,
            key_pair,
            fingerprint: Fingerprint::try_from(fingerprint).unwrap(),
        }
    }
}

impl Certificate for WebRTCCertificate {
    fn get_der_certificate(&self) -> &'_ [u8] {
        self.serialized_der
    }
    fn get_der_keypair(&self) -> &'_ [u8] {
        self.key_pair
    }
    fn get_fingerprint(&self) -> &'_ Fingerprint {
        &self.fingerprint
    }
}
