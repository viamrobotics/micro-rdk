use std::{
    ffi::{c_void, CStr, CString},
    fmt::{Debug, Display},
    mem::MaybeUninit,
};

use chrono::{NaiveDate, NaiveDateTime};
use esp_idf_svc::sys::{
    mbedtls_ctr_drbg_context, mbedtls_ctr_drbg_free, mbedtls_ctr_drbg_init,
    mbedtls_ctr_drbg_random, mbedtls_ctr_drbg_seed, mbedtls_ecp_gen_key,
    mbedtls_ecp_group_id_MBEDTLS_ECP_DP_SECP256R1, mbedtls_ecp_keypair, mbedtls_entropy_context,
    mbedtls_entropy_free, mbedtls_entropy_func, mbedtls_entropy_init, mbedtls_high_level_strerr,
    mbedtls_low_level_strerr, mbedtls_md_type_t_MBEDTLS_MD_SHA256, mbedtls_pk_context,
    mbedtls_pk_free, mbedtls_pk_info_from_type, mbedtls_pk_init, mbedtls_pk_setup,
    mbedtls_pk_type_t_MBEDTLS_PK_ECKEY, mbedtls_pk_write_key_der, mbedtls_x509write_cert,
    mbedtls_x509write_crt_der, mbedtls_x509write_crt_free, mbedtls_x509write_crt_init,
    mbedtls_x509write_crt_set_issuer_key, mbedtls_x509write_crt_set_issuer_name,
    mbedtls_x509write_crt_set_md_alg, mbedtls_x509write_crt_set_serial_raw,
    mbedtls_x509write_crt_set_subject_key, mbedtls_x509write_crt_set_subject_name,
    mbedtls_x509write_crt_set_validity, mbedtls_x509write_crt_set_version,
    MBEDTLS_ERR_PK_BAD_INPUT_DATA, MBEDTLS_X509_CRT_VERSION_3, SHA_TYPE_SHA2_256,
};

use crate::common::webrtc::certificate::{Certificate, Fingerprint};

#[derive(Clone)]
pub struct WebRtcCertificate {
    serialized_der: Vec<u8>,
    priv_key: Vec<u8>,
    fingerprint: Fingerprint,
}

impl WebRtcCertificate {
    pub fn new(serialized_der: Vec<u8>, key_pair: Vec<u8>, fingerprint: &str) -> Self {
        Self {
            serialized_der,
            priv_key: key_pair,
            fingerprint: Fingerprint::try_from(fingerprint).unwrap(),
        }
    }
}

impl Certificate for WebRtcCertificate {
    fn get_der_certificate(&self) -> &'_ [u8] {
        &self.serialized_der
    }
    fn get_der_keypair(&self) -> &'_ [u8] {
        &self.priv_key
    }
    fn get_fingerprint(&self) -> &'_ Fingerprint {
        &self.fingerprint
    }
}

pub struct MbedTLSError(i32);

impl MbedTLSError {
    fn to_unit_result(code: i32) -> Result<(), Self> {
        match code {
            0 => Ok(()),
            _ => Err(code.into()),
        }
    }
    fn to_result(code: i32) -> Result<i32, Self> {
        match code {
            0..=i32::MAX => Ok(code),
            _ => Err(code.into()),
        }
    }
}

impl From<i32> for MbedTLSError {
    fn from(value: i32) -> Self {
        MbedTLSError(value)
    }
}

impl Display for MbedTLSError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut code = self.0.abs();

        write!(f, "MBEDTLS ERROR ({:#04X}) ", code)?;
        if (code & 0xFF80) > 0 {
            let high_level_description = unsafe { mbedtls_high_level_strerr(code) };
            if !high_level_description.is_null() {
                let err_str = unsafe { CStr::from_ptr(high_level_description) }
                    .to_str()
                    .unwrap();
                write!(f, "HE : {} ", err_str)?;
            }
        }
        code &= !0xFF80;
        if code > 0 {
            let low_level_description = unsafe { mbedtls_low_level_strerr(code) };
            if !low_level_description.is_null() {
                let err_str = unsafe { CStr::from_ptr(low_level_description) }
                    .to_str()
                    .unwrap();
                write!(f, "LE : {} ", err_str)?;
            }
        }
        Ok(())
    }
}

impl Debug for MbedTLSError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

// TODO consider moving this function in it's own rust wrapper to be reused elsewhere
// or patch esp-idf-sys upstream to expose it via the bindings
extern "C" {
    fn esp_sha(sha_type: esp_idf_svc::sys::SHA_TYPE, input: *const u8, len: usize, output: *mut u8);
}

pub struct GeneratedWebRtcCertificateBuilder {
    oid_cn: String,
    oid_c: String,
    oid_o: String,
    not_before: NaiveDateTime,
    not_after: NaiveDateTime,
    drbg_context: mbedtls_ctr_drbg_context,
    entropy: mbedtls_entropy_context,
    kp_context: mbedtls_pk_context,
    crt_context: mbedtls_x509write_cert,
}

impl GeneratedWebRtcCertificateBuilder {
    pub fn with_common_name(&mut self, cn: String) -> &mut Self {
        self.oid_cn = cn;
        self
    }
    pub fn with_country(&mut self, c: String) -> &mut Self {
        self.oid_c = c;
        self
    }
    pub fn with_organization(&mut self, org: String) -> &mut Self {
        self.oid_o = org;
        self
    }
    pub fn with_notbefore(&mut self, not_before: NaiveDateTime) -> &mut Self {
        self.not_before = not_before;
        self
    }
    pub fn with_notafter(&mut self, not_after: NaiveDateTime) -> &mut Self {
        self.not_after = not_after;
        self
    }
    pub fn build(mut self) -> Result<WebRtcCertificate, MbedTLSError> {
        unsafe {
            MbedTLSError::to_unit_result(mbedtls_ctr_drbg_seed(
                &mut self.drbg_context as *mut mbedtls_ctr_drbg_context,
                Some(mbedtls_entropy_func),
                &mut self.entropy as *mut _ as *mut c_void,
                std::ptr::null(),
                0,
            ))
        }?;

        let key_type_info =
            unsafe { mbedtls_pk_info_from_type(mbedtls_pk_type_t_MBEDTLS_PK_ECKEY) };
        if key_type_info.is_null() {
            return Err(MbedTLSError(MBEDTLS_ERR_PK_BAD_INPUT_DATA));
        }

        unsafe {
            MbedTLSError::to_unit_result(mbedtls_pk_setup(
                &mut self.kp_context as *mut mbedtls_pk_context,
                key_type_info,
            ))
        }?;

        // TODO(RSDK-10196): The `pk_ctx` field doesn't exist in ESP-IDF 5. Maybe it should be `private_kp_ctx`?
        // we should use the mbedtls_ecp_keypair *mbedtls_pk_ec(const mbedtls_pk_context pk) but it's defined as static inline
        // to access it we would need to change bindgen invocation to export it
        let ecp_keypair = self.kp_context.private_pk_ctx;

        unsafe {
            MbedTLSError::to_unit_result(mbedtls_ecp_gen_key(
                mbedtls_ecp_group_id_MBEDTLS_ECP_DP_SECP256R1,
                ecp_keypair as *mut _ as *mut mbedtls_ecp_keypair,
                Some(mbedtls_ctr_drbg_random),
                &mut self.drbg_context as *mut _ as *mut c_void,
            ))
        }?;

        unsafe {
            mbedtls_x509write_crt_set_subject_key(
                &mut self.crt_context as *mut mbedtls_x509write_cert,
                &mut self.kp_context as *mut mbedtls_pk_context,
            );
            mbedtls_x509write_crt_set_issuer_key(
                &mut self.crt_context as *mut mbedtls_x509write_cert,
                &mut self.kp_context as *mut mbedtls_pk_context,
            );
            mbedtls_x509write_crt_set_version(
                &mut self.crt_context as *mut mbedtls_x509write_cert,
                MBEDTLS_X509_CRT_VERSION_3 as i32,
            );
            mbedtls_x509write_crt_set_md_alg(
                &mut self.crt_context as *mut mbedtls_x509write_cert,
                mbedtls_md_type_t_MBEDTLS_MD_SHA256,
            );
        };

        let subject_name = {
            CString::new(format!(
                "CN={},O={},C={}",
                self.oid_cn, self.oid_o, self.oid_c
            ))
            .unwrap()
        };
        unsafe {
            MbedTLSError::to_unit_result(mbedtls_x509write_crt_set_issuer_name(
                &mut self.crt_context as *mut mbedtls_x509write_cert,
                subject_name.as_ptr(),
            ))
        }?;

        unsafe {
            MbedTLSError::to_unit_result(mbedtls_x509write_crt_set_subject_name(
                &mut self.crt_context as *mut mbedtls_x509write_cert,
                subject_name.as_ptr(),
            ))
        }?;

        let not_before = CString::new(self.not_before.format("%Y%m%d%H%M%S").to_string()).unwrap();
        let not_after = CString::new(self.not_after.format("%Y%m%d%H%M%S").to_string()).unwrap();

        unsafe {
            MbedTLSError::to_unit_result(mbedtls_x509write_crt_set_validity(
                &mut self.crt_context as *mut mbedtls_x509write_cert,
                not_before.as_ptr(),
                not_after.as_ptr(),
            ))
        }?;

        let mut serial_number = [0x0_u8; 1];
        unsafe {
            MbedTLSError::to_unit_result(mbedtls_x509write_crt_set_serial_raw(
                &mut self.crt_context as *mut mbedtls_x509write_cert,
                serial_number.as_mut_ptr(),
                serial_number.len(),
            ))
        }?;

        let mut work_buffer = vec![0_u8; 2048];

        let ret = unsafe {
            MbedTLSError::to_result(mbedtls_x509write_crt_der(
                &mut self.crt_context as *mut mbedtls_x509write_cert,
                work_buffer.as_mut_ptr(),
                work_buffer.len(),
                Some(mbedtls_ctr_drbg_random),
                &mut self.drbg_context as *mut _ as *mut c_void,
            ))
        }?;
        let der_certificate = work_buffer[2048 - ret as usize..].to_vec();
        debug_assert_eq!(der_certificate.len(), ret as usize);

        unsafe {
            esp_sha(
                SHA_TYPE_SHA2_256,
                der_certificate.as_ptr(),
                der_certificate.len(),
                work_buffer.as_mut_ptr(),
            )
        };
        let der_fp = work_buffer
            .iter()
            .take(32)
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<String>>()
            .join(":");
        debug_assert_eq!(der_fp.len(), 95);
        let fingerprint = Fingerprint::new("sha-256".to_owned(), der_fp);

        let ret = unsafe {
            MbedTLSError::to_result(mbedtls_pk_write_key_der(
                &mut self.kp_context as *mut _,
                work_buffer.as_mut_ptr(),
                work_buffer.len(),
            ))
        }?;

        let der_pk = work_buffer[2048 - ret as usize..].to_vec();
        debug_assert_eq!(der_pk.len(), ret as usize);

        Ok(WebRtcCertificate {
            serialized_der: der_certificate,
            priv_key: der_pk,
            fingerprint,
        })
    }
}

impl Default for GeneratedWebRtcCertificateBuilder {
    fn default() -> Self {
        let drbg_context = unsafe {
            let mut drbg_context = MaybeUninit::uninit();
            mbedtls_ctr_drbg_init(drbg_context.as_mut_ptr());
            drbg_context.assume_init()
        };
        let entropy = unsafe {
            let mut entropy = MaybeUninit::uninit();
            mbedtls_entropy_init(entropy.as_mut_ptr());
            entropy.assume_init()
        };
        let kp_context = unsafe {
            let mut kp_context = MaybeUninit::uninit();
            mbedtls_pk_init(kp_context.as_mut_ptr());
            kp_context.assume_init()
        };
        let crt_context = unsafe {
            let mut crt_context = MaybeUninit::uninit();
            mbedtls_x509write_crt_init(crt_context.as_mut_ptr());
            crt_context.assume_init()
        };

        Self {
            oid_cn: String::from("VIAM WebRTC"),
            oid_c: String::from("US"),
            oid_o: String::from("VIAM"),
            not_before: NaiveDate::from_ymd_opt(2024, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            not_after: NaiveDate::from_ymd_opt(2034, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            drbg_context,
            entropy,
            kp_context,
            crt_context,
        }
    }
}
impl Drop for GeneratedWebRtcCertificateBuilder {
    fn drop(&mut self) {
        unsafe {
            mbedtls_x509write_crt_free(&mut self.crt_context as *mut _);
            mbedtls_pk_free(&mut self.kp_context as *mut _);
            mbedtls_ctr_drbg_free(&mut self.drbg_context as *mut _);
            mbedtls_entropy_free(&mut self.entropy as *mut _);
        }
    }
}
