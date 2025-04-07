use rcgen::{date_time_ymd, CertificateParams, DistinguishedName};
use sha2::{Digest, Sha256};

use crate::common::webrtc::certificate::{Certificate, Fingerprint};

#[derive(Clone)]
pub struct WebRtcCertificate {
    serialized_der: Vec<u8>,
    key_pair: Vec<u8>,
    fingerprint: Fingerprint,
}

impl WebRtcCertificate {
    pub fn new() -> Self {
        let mut param: CertificateParams = Default::default();
        param.not_before = date_time_ymd(2021, 5, 19);
        param.not_after = date_time_ymd(4096, 1, 1);
        param.distinguished_name = DistinguishedName::new();

        let kp = rcgen::KeyPair::generate().unwrap();
        let kp_der = kp.serialize_der();

        let cert = param.self_signed(&kp).unwrap();
        let cert_der = cert.der();

        let fp_hashed = Sha256::new_with_prefix(cert_der)
            .finalize()
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<String>>()
            .join(":");
        let fingerprint = Fingerprint::new("sha-256".to_owned(), fp_hashed);

        Self {
            serialized_der: cert_der.to_vec(),
            key_pair: kp_der,
            fingerprint,
        }
    }
}

impl Default for WebRtcCertificate {
    fn default() -> Self {
        Self::new()
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
