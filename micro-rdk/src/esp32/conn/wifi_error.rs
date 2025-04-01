#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
use esp_idf_svc::sys::*;

/// Wrapper around (`wifi_err_reason_t`)[https://github.com/espressif/esp-idf/blob/a3864c088dafb0b8ce94dba272685b850b46c837/components/esp_wifi/include/esp_wifi_types_generic.h#L175]
#[derive(Debug)]
pub enum WifiErrReason {
    Unspecified,
    AuthExpire,
    AuthLeave,
    AssocExpire,
    AssocTooMany,
    NotAuthed,
    NotAssoced,
    AssocLeave,
    AssocNotAuthed,
    DisassocPwrcapBad,
    DisassocSupchanBad,
    BssTransitionDisassoc,
    IeInvalid,
    MicFailure,
    FourWayHandshakeTimeout,
    GroupKeyUpdateTimeout,
    IeInFourWayDiffers,
    GroupCipherInvalid,
    PairwiseCipherInvalid,
    AkmpInvalid,
    UnsuppRsnIeVersion,
    InvalidRsnIeCap,
    AuthFailed802_1x,
    CipherSuiteRejected,
    TDlsPeerUnreachable,
    TDlsUnspecified,
    SspRequestedDisassoc,
    NoSspRoamingAgreement,
    BadCipherOrAkm,
    NotAuthorizedThisLocation,
    ServiceChangePercludesTs,
    UnspecifiedQos,
    NotEnoughBandwidth,
    MissingAcks,
    ExceededTxOp,
    StaLeaving,
    EndBA,
    UnknownBA,
    Timeout,
    PeerInitiated,
    ApInitiated,
    InvalidFtActionFrameCount,
    InvalidPmkid,
    InvalidMde,
    InvalidFte,
    TransmissionLinkEstablishFailed,
    AlternativeChannelOccupied,
    BeaconTimeout,
    NoApFound,
    AuthFail,
    AssocFail,
    HandshakeTimeout,
    ConnectionFail,
    ApTsfReset,
    Roaming,
    AssocComebackTimeTooLong,
    SaQueryTimeout,
    NoApFoundWithCompatibleSecurity,
    NoApFoundInAuthModeThreshold,
    NoApFoundInRssiThreshold,
    Unrecognized(u32),
}

impl std::fmt::Display for WifiErrReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unrecognized(v) => write!(f, "Unrecognized - internal code {}", v),
            _ => write!(f, "{:?}", self),
        }
    }
}

impl From<u16> for WifiErrReason {
    fn from(value: u16) -> Self {
        (value as u32).into()
    }
}

impl From<u32> for WifiErrReason {
    fn from(value: u32) -> Self {
        match value {
            wifi_err_reason_t_WIFI_REASON_UNSPECIFIED => Self::Unspecified,
            wifi_err_reason_t_WIFI_REASON_AUTH_EXPIRE => Self::AuthExpire,
            wifi_err_reason_t_WIFI_REASON_AUTH_LEAVE => Self::AuthLeave,
            wifi_err_reason_t_WIFI_REASON_ASSOC_EXPIRE => Self::AssocExpire,
            wifi_err_reason_t_WIFI_REASON_ASSOC_TOOMANY => Self::AssocTooMany,
            wifi_err_reason_t_WIFI_REASON_NOT_AUTHED => Self::NotAuthed,
            wifi_err_reason_t_WIFI_REASON_NOT_ASSOCED => Self::NotAssoced,
            wifi_err_reason_t_WIFI_REASON_ASSOC_LEAVE => Self::AssocLeave,
            wifi_err_reason_t_WIFI_REASON_ASSOC_NOT_AUTHED => Self::AssocNotAuthed,
            wifi_err_reason_t_WIFI_REASON_DISASSOC_PWRCAP_BAD => Self::DisassocPwrcapBad,
            wifi_err_reason_t_WIFI_REASON_DISASSOC_SUPCHAN_BAD => Self::DisassocSupchanBad,
            wifi_err_reason_t_WIFI_REASON_BSS_TRANSITION_DISASSOC => Self::BssTransitionDisassoc,
            wifi_err_reason_t_WIFI_REASON_IE_INVALID => Self::IeInvalid,
            wifi_err_reason_t_WIFI_REASON_MIC_FAILURE => Self::MicFailure,
            wifi_err_reason_t_WIFI_REASON_4WAY_HANDSHAKE_TIMEOUT => Self::FourWayHandshakeTimeout,
            wifi_err_reason_t_WIFI_REASON_GROUP_KEY_UPDATE_TIMEOUT => Self::GroupKeyUpdateTimeout,
            wifi_err_reason_t_WIFI_REASON_IE_IN_4WAY_DIFFERS => Self::IeInFourWayDiffers,
            wifi_err_reason_t_WIFI_REASON_GROUP_CIPHER_INVALID => Self::GroupCipherInvalid,
            wifi_err_reason_t_WIFI_REASON_PAIRWISE_CIPHER_INVALID => Self::PairwiseCipherInvalid,
            wifi_err_reason_t_WIFI_REASON_AKMP_INVALID => Self::AkmpInvalid,
            wifi_err_reason_t_WIFI_REASON_UNSUPP_RSN_IE_VERSION => Self::UnsuppRsnIeVersion,
            wifi_err_reason_t_WIFI_REASON_INVALID_RSN_IE_CAP => Self::InvalidRsnIeCap,
            wifi_err_reason_t_WIFI_REASON_802_1X_AUTH_FAILED => Self::AuthFailed802_1x,
            wifi_err_reason_t_WIFI_REASON_CIPHER_SUITE_REJECTED => Self::CipherSuiteRejected,
            wifi_err_reason_t_WIFI_REASON_TDLS_PEER_UNREACHABLE => Self::TDlsPeerUnreachable,
            wifi_err_reason_t_WIFI_REASON_TDLS_UNSPECIFIED => Self::TDlsUnspecified,
            wifi_err_reason_t_WIFI_REASON_SSP_REQUESTED_DISASSOC => Self::SspRequestedDisassoc,
            wifi_err_reason_t_WIFI_REASON_NO_SSP_ROAMING_AGREEMENT => Self::NoSspRoamingAgreement,
            wifi_err_reason_t_WIFI_REASON_BAD_CIPHER_OR_AKM => Self::BadCipherOrAkm,
            wifi_err_reason_t_WIFI_REASON_NOT_AUTHORIZED_THIS_LOCATION => {
                Self::NotAuthorizedThisLocation
            }
            wifi_err_reason_t_WIFI_REASON_SERVICE_CHANGE_PERCLUDES_TS => {
                Self::ServiceChangePercludesTs
            }
            wifi_err_reason_t_WIFI_REASON_UNSPECIFIED_QOS => Self::UnspecifiedQos,
            wifi_err_reason_t_WIFI_REASON_NOT_ENOUGH_BANDWIDTH => Self::NotEnoughBandwidth,
            wifi_err_reason_t_WIFI_REASON_MISSING_ACKS => Self::MissingAcks,
            wifi_err_reason_t_WIFI_REASON_EXCEEDED_TXOP => Self::ExceededTxOp,
            wifi_err_reason_t_WIFI_REASON_STA_LEAVING => Self::StaLeaving,
            wifi_err_reason_t_WIFI_REASON_END_BA => Self::EndBA,
            wifi_err_reason_t_WIFI_REASON_UNKNOWN_BA => Self::UnknownBA,
            wifi_err_reason_t_WIFI_REASON_TIMEOUT => Self::Timeout,
            wifi_err_reason_t_WIFI_REASON_PEER_INITIATED => Self::PeerInitiated,
            wifi_err_reason_t_WIFI_REASON_AP_INITIATED => Self::ApInitiated,
            wifi_err_reason_t_WIFI_REASON_INVALID_FT_ACTION_FRAME_COUNT => {
                Self::InvalidFtActionFrameCount
            }
            wifi_err_reason_t_WIFI_REASON_INVALID_PMKID => Self::InvalidPmkid,
            wifi_err_reason_t_WIFI_REASON_INVALID_MDE => Self::InvalidMde,
            wifi_err_reason_t_WIFI_REASON_INVALID_FTE => Self::InvalidFte,
            wifi_err_reason_t_WIFI_REASON_TRANSMISSION_LINK_ESTABLISH_FAILED => {
                Self::TransmissionLinkEstablishFailed
            }
            wifi_err_reason_t_WIFI_REASON_ALTERATIVE_CHANNEL_OCCUPIED => {
                Self::AlternativeChannelOccupied
            }
            wifi_err_reason_t_WIFI_REASON_BEACON_TIMEOUT => Self::BeaconTimeout,
            wifi_err_reason_t_WIFI_REASON_NO_AP_FOUND => Self::NoApFound,
            wifi_err_reason_t_WIFI_REASON_AUTH_FAIL => Self::AuthFail,
            wifi_err_reason_t_WIFI_REASON_ASSOC_FAIL => Self::AssocFail,
            wifi_err_reason_t_WIFI_REASON_HANDSHAKE_TIMEOUT => Self::HandshakeTimeout,
            wifi_err_reason_t_WIFI_REASON_CONNECTION_FAIL => Self::ConnectionFail,
            wifi_err_reason_t_WIFI_REASON_AP_TSF_RESET => Self::ApTsfReset,
            wifi_err_reason_t_WIFI_REASON_ROAMING => Self::Roaming,
            wifi_err_reason_t_WIFI_REASON_ASSOC_COMEBACK_TIME_TOO_LONG => {
                Self::AssocComebackTimeTooLong
            }
            wifi_err_reason_t_WIFI_REASON_SA_QUERY_TIMEOUT => Self::SaQueryTimeout,
            wifi_err_reason_t_WIFI_REASON_NO_AP_FOUND_W_COMPATIBLE_SECURITY => {
                Self::NoApFoundWithCompatibleSecurity
            }
            wifi_err_reason_t_WIFI_REASON_NO_AP_FOUND_IN_AUTHMODE_THRESHOLD => {
                Self::NoApFoundInAuthModeThreshold
            }
            wifi_err_reason_t_WIFI_REASON_NO_AP_FOUND_IN_RSSI_THRESHOLD => {
                Self::NoApFoundInRssiThreshold
            }
            _ => Self::Unrecognized(value),
        }
    }
}
