#![allow(non_snake_case)]
use esp_idf_svc::sys::*;
use core::ffi::c_uint;

impl From<u16> for WifiErrReason {
    fn from(value: u16) -> Self {
        (value as c_uint).into()
    }
}

impl From<c_uint> for WifiErrReason {
    fn from(v: c_uint) -> Self {
        match v {
            wifi_err_reason_t_WIFI_REASON_UNSPECIFIED => Self::Unspecified(v),
            wifi_err_reason_t_WIFI_REASON_AUTH_EXPIRE => Self::AuthExpire(v),
            wifi_err_reason_t_WIFI_REASON_AUTH_LEAVE => Self::AuthLeave(v),
            wifi_err_reason_t_WIFI_REASON_ASSOC_EXPIRE => Self::AssocExpire(v),
            wifi_err_reason_t_WIFI_REASON_ASSOC_TOOMANY => Self::AssocTooMany(v),
            wifi_err_reason_t_WIFI_REASON_NOT_AUTHED => Self::NotAuthed(v),
            wifi_err_reason_t_WIFI_REASON_NOT_ASSOCED => Self::NotAssoced(v),
            wifi_err_reason_t_WIFI_REASON_ASSOC_LEAVE => Self::AssocLeave(v),
            wifi_err_reason_t_WIFI_REASON_ASSOC_NOT_AUTHED => Self::AssocNotAuthed(v),
            wifi_err_reason_t_WIFI_REASON_DISASSOC_PWRCAP_BAD => Self::DisassocPwrCapBad(v),
            wifi_err_reason_t_WIFI_REASON_DISASSOC_SUPCHAN_BAD => Self::DisassocSupchanBad(v),
            wifi_err_reason_t_WIFI_REASON_BSS_TRANSITION_DISASSOC => Self::BssTransitionDisassoc(v),
            wifi_err_reason_t_WIFI_REASON_IE_INVALID => Self::IeInvalid(v),
            wifi_err_reason_t_WIFI_REASON_MIC_FAILURE => Self::MicFailure(v),
            wifi_err_reason_t_WIFI_REASON_4WAY_HANDSHAKE_TIMEOUT => {
                Self::FourWayHandshakeTimeout(v)
            }
            wifi_err_reason_t_WIFI_REASON_GROUP_KEY_UPDATE_TIMEOUT => {
                Self::GroupKeyUpdateTimeout(v)
            }
            wifi_err_reason_t_WIFI_REASON_IE_IN_4WAY_DIFFERS => Self::IeInFourWayDiffers(v),
            wifi_err_reason_t_WIFI_REASON_GROUP_CIPHER_INVALID => Self::GroupCipherInvalid(v),
            wifi_err_reason_t_WIFI_REASON_PAIRWISE_CIPHER_INVALID => Self::PairwiseCipherInvalid(v),
            wifi_err_reason_t_WIFI_REASON_AKMP_INVALID => Self::AkmpInvalid(v),
            wifi_err_reason_t_WIFI_REASON_UNSUPP_RSN_IE_VERSION => Self::UnsuppRsnIeVersion(v),

            wifi_err_reason_t_WIFI_REASON_INVALID_RSN_IE_CAP => Self::InvalidRsnIeCap(v),
            wifi_err_reason_t_WIFI_REASON_802_1X_AUTH_FAILED => Self::AuthFailed802_1x(v),
            wifi_err_reason_t_WIFI_REASON_CIPHER_SUITE_REJECTED => Self::CipherSuiteRejected(v),
            wifi_err_reason_t_WIFI_REASON_TDLS_PEER_UNREACHABLE => Self::TDlsPeerUnreachable(v),
            wifi_err_reason_t_WIFI_REASON_TDLS_UNSPECIFIED => Self::TDlsUnspecified(v),

            wifi_err_reason_t_WIFI_REASON_SSP_REQUESTED_DISASSOC => Self::SspRequestedDisassoc(v),
            wifi_err_reason_t_WIFI_REASON_NO_SSP_ROAMING_AGREEMENT => {
                Self::NoSspRoamingAgreement(v)
            }
            wifi_err_reason_t_WIFI_REASON_BAD_CIPHER_OR_AKM => Self::BadCipherOrAkm(v),
            wifi_err_reason_t_WIFI_REASON_NOT_AUTHORIZED_THIS_LOCATION => {
                Self::NotAuthorizedThisLocation(v)
            }
            wifi_err_reason_t_WIFI_REASON_SERVICE_CHANGE_PERCLUDES_TS => {
                Self::ServiceChangePrecludesTs(v)
            }
            wifi_err_reason_t_WIFI_REASON_UNSPECIFIED_QOS => Self::UnspecifiedQos(v),
            wifi_err_reason_t_WIFI_REASON_NOT_ENOUGH_BANDWIDTH => Self::NotEnoughBandwidth(v),
            wifi_err_reason_t_WIFI_REASON_MISSING_ACKS => Self::MissingAcks(v),
            wifi_err_reason_t_WIFI_REASON_EXCEEDED_TXOP => Self::ExceededTxOp(v),
            wifi_err_reason_t_WIFI_REASON_STA_LEAVING => Self::StaLeaving(v),
            wifi_err_reason_t_WIFI_REASON_END_BA => Self::EndBA(v),
            wifi_err_reason_t_WIFI_REASON_UNKNOWN_BA => Self::UnknownBA(v),

            wifi_err_reason_t_WIFI_REASON_TIMEOUT => Self::Timeout(v),
            wifi_err_reason_t_WIFI_REASON_PEER_INITIATED => Self::PeerInitiated(v),
            wifi_err_reason_t_WIFI_REASON_AP_INITIATED => Self::ApInitiated(v),
            wifi_err_reason_t_WIFI_REASON_INVALID_FT_ACTION_FRAME_COUNT => {
                Self::InvalidFtActionFrameCount(v)
            }
            wifi_err_reason_t_WIFI_REASON_INVALID_PMKID => Self::InvalidPmkid(v),
            wifi_err_reason_t_WIFI_REASON_INVALID_MDE => Self::InvalidMde(v),
            wifi_err_reason_t_WIFI_REASON_INVALID_FTE => Self::InvalidFte(v),

            wifi_err_reason_t_WIFI_REASON_TRANSMISSION_LINK_ESTABLISH_FAILED => {
                Self::TransmissionLinkEstablishFailed(v)
            }
            wifi_err_reason_t_WIFI_REASON_ALTERATIVE_CHANNEL_OCCUPIED => {
                Self::AlternativeChannelOccupied(v)
            }
            wifi_err_reason_t_WIFI_REASON_BEACON_TIMEOUT => Self::BeaconTimeout(v),
            wifi_err_reason_t_WIFI_REASON_NO_AP_FOUND => Self::NoApFound(v),
            wifi_err_reason_t_WIFI_REASON_AUTH_FAIL => Self::AuthFail(v),
            wifi_err_reason_t_WIFI_REASON_ASSOC_FAIL => Self::AssocFail(v),
            wifi_err_reason_t_WIFI_REASON_HANDSHAKE_TIMEOUT => Self::HandshakeTimeout(v),
            wifi_err_reason_t_WIFI_REASON_CONNECTION_FAIL => Self::ConnectionFail(v),
            wifi_err_reason_t_WIFI_REASON_AP_TSF_RESET => Self::ApTsfReset(t),
            wifi_err_reason_t_WIFI_REASON_ROAMING => Self::Roaming(v),
            wifi_err_reason_t_WIFI_REASON_ASSOC_COMEBACK_TIME_TOO_LONG => {
                Self::AssocComebackTimeTooLong(v)
            }
            wifi_err_reason_t_WIFI_REASON_SA_QUERY_TIMEOUT => Self::SaQueryTimeout(v),

            wifi_err_reason_t_WIFI_REASON_NO_AP_FOUND_W_COMPATIBLE_SECURITY => {
                Self::NoApFoundWithCompatibleSecurity(v)
            }
            wifi_err_reason_t_WIFI_REASON_NO_AP_FOUND_IN_AUTHMODE_THRESHOLD => {
                Self::NoApFoundInAuthModeThreshold(v)
            }
            wifi_err_reason_t_WIFI_REASON_NO_AP_FOUND_IN_RSSI_THRESHOLD => {
                Self::NoApFoundInRssiThreshold(v)
            }
            _ => Self::Unrecognized(v),
        }
    }
}

#[derive(Debug)]
pub enum WifiErrReason {
    Unspecified(c_uint),
    AuthExpire(c_uint),
    AuthLeave(c_uint),
    AssocExpire(c_uint),
    AssocTooMany(c_uint),
    NotAuthed(c_uint),
    NotAssoced(c_uint),
    AssocLeave(c_uint),
    AssocNotAuthed(c_uint),
    DisassocPwrCapBad(c_uint),
    DisassocSupchanBad(c_uint),
    BssTransitionDisassoc(c_uint),
    IeInvalid(c_uint),
    MicFailure(c_uint),
    FourWayHandshakeTimeout(c_uint),
    GroupKeyUpdateTimeout(c_uint),
    IeInFourWayDiffers(c_uint),
    GroupCipherInvalid(c_uint),
    PairwiseCipherInvalid(c_uint),
    AkmpInvalid(c_uint),
    UnsuppRsnIeVersion(c_uint),
    InvalidRsnIeCap(c_uint),
    AuthFailed802_1x(c_uint),
    CipherSuiteRejected(c_uint),
    TDlsPeerUnreachable(c_uint),
    TDlsUnspecified(c_uint),
    SspRequestedDisassoc(c_uint),
    NoSspRoamingAgreement(c_uint),
    BadCipherOrAkm(c_uint),
    NotAuthorizedThisLocation(c_uint),
    ServiceChangePrecludesTs(c_uint),
    UnspecifiedQos(c_uint),
    NotEnoughBandwidth(c_uint),
    MissingAcks(c_uint),
    ExceededTxOp(c_uint),
    StaLeaving(c_uint),
    EndBA(c_uint),
    UnknownBA(c_uint),
    Timeout(c_uint),
    PeerInitiated(c_uint),
    ApInitiated(c_uint),
    InvalidFtActionFrameCount(c_uint),
    InvalidPmkid(c_uint),
    InvalidMde(c_uint),
    InvalidFte(c_uint),
    TransmissionLinkEstablishFailed(c_uint),
    AlternativeChannelOccupied(c_uint),
    BeaconTimeout(c_uint),
    NoApFound(c_uint),
    AuthFail(c_uint),
    AssocFail(c_uint),
    HandshakeTimeout(c_uint),
    ConnectionFail(c_uint),
    ApTsfReset(c_uint),
    Roaming(c_uint),
    AssocComebackTimeTooLong(c_uint),
    SaQueryTimeout(c_uint),
    NoApFoundWithCompatibleSecurity(c_uint),
    NoApFoundInAuthModeThreshold(c_uint),
    NoApFoundInRssiThreshold(c_uint),
    Unrecognized(c_uint),
}
