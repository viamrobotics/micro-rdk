use std::fmt::Display;

/// Fingerprint of a certificate
#[derive(Clone, Default)]
pub struct Fingerprint {
    /// hashing algorithm
    algo: String,
    /// digest of the certificate
    hash: String,
}

impl Fingerprint {
    pub fn new(algo: String, hash: String) -> Self {
        Self { algo, hash }
    }
    pub fn get_algo(&self) -> &str {
        &self.algo
    }
    pub fn get_hash(&self) -> &str {
        &self.hash
    }
}

impl<'a> TryFrom<&'a str> for Fingerprint {
    type Error = ();
    fn try_from(s: &'a str) -> Result<Self, Self::Error> {
        let mut part = s.split(' ');
        let hash = part.next().ok_or(())?;
        let fp = part.next().ok_or(())?;
        Ok(Self {
            algo: String::from(hash),
            hash: String::from(fp),
        })
    }
}

impl Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.algo, self.hash)
    }
}

/// Certificate are implemented differently on each supported platform
/// This trait can be used by the common and specific part of the webrtc stack implementation
pub trait Certificate {
    /// returns the fingerprint of the certificate to be used when answering the offer
    fn get_fingerprint(&self) -> &'_ Fingerprint;
    /// returns the certificate in DER format
    fn get_der_certificate(&self) -> &'_ [u8];
    /// returns the private key of the certificate in DER format
    fn get_der_keypair(&self) -> &'_ [u8];
}
