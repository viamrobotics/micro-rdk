#![allow(dead_code)]
use std::{
    fmt::Display,
    net::{Ipv4Addr, SocketAddrV4},
    time::{Duration, Instant},
};

use stun_codec::TransactionId;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum CandidateError {
    #[error("cannot parse candidate")]
    CannotParseCandidate,
    #[error("not UDP based")]
    NotUDPCandidate,
    #[error("unsupported candidate type")]
    UnsupportedType,
    #[error("cannot form candidate pair")]
    CannotFormCandidatePair,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateType {
    Host,
    ServerReflexive,
    PeerReflexive,
    Relay,
}

impl Display for CandidateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Host => write!(f, "host"),
            Self::PeerReflexive => write!(f, "prflx"),
            Self::Relay => write!(f, "relay"),
            Self::ServerReflexive => write!(f, "srflx"),
        }
    }
}

impl CandidateType {
    /// Returns the preference weight of a `CandidateType`.
    ///
    /// 4.1.2.2.  Guidelines for Choosing Type and Local Preferences
    /// The RECOMMENDED values are 126 for host candidates, 100
    /// for server reflexive candidates, 110 for peer reflexive candidates,
    /// and 0 for relayed candidates.
    pub const fn preference(&self) -> u16 {
        match self {
            Self::Host => 126,
            Self::PeerReflexive => 110,
            Self::ServerReflexive => 100,
            Self::Relay => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum NetworkType {
    // Support only UDP network
    UDP,
}

impl Display for NetworkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UDP => write!(f, "UDP"),
        }
    }
}

/// Represents an ICE candidate
#[derive(Clone, Debug)]
pub struct Candidate {
    /// Underlying network protocol
    pub network_type: NetworkType,
    pub candidate_type: CandidateType,
    pub component: u16,
    pub address: SocketAddrV4, //Socket addr??
    pub raddr: Option<String>,
    pub rport: Option<u16>,
    /// The foundation is an identifier, scoped within a session
    /// It is the same for two candidates that
    /// have the same type, base IP address, protocol (UDP, TCP, etc.),
    /// and STUN or TURN server.
    pub foundation: Option<String>,
    pub priority: Option<u32>,
    // TODO(npm) add base address for srflx
}

impl Candidate {
    /// Creates a new server reflexive candidate
    /// Only supports IpV4
    pub fn new_srflx_candidate(ip_v4: SocketAddrV4, _base: SocketAddrV4) -> Self {
        Self {
            network_type: NetworkType::UDP,
            candidate_type: CandidateType::ServerReflexive,
            component: 1,
            address: ip_v4,
            raddr: Some("0.0.0.0".to_owned()),
            rport: Some(0),
            foundation: None,
            priority: None,
        }
    }
    /// Creates a new host candidate
    /// Only supports IpV4
    pub fn new_host_candidate(ip_v4: SocketAddrV4) -> Self {
        Self {
            network_type: NetworkType::UDP, //Always UDP
            candidate_type: CandidateType::Host,
            component: 1, // Always a single strem
            address: ip_v4,
            raddr: None,
            rport: None,
            foundation: None,
            priority: None,
        }
    }
    /// Creates a new peer reflexive candidate
    /// Only supports IpV4
    pub fn new_peer_reflexive(ip_v4: SocketAddrV4, _priority: Option<u32>) -> Self {
        Self {
            network_type: NetworkType::UDP,
            candidate_type: CandidateType::PeerReflexive,
            component: 1,
            address: ip_v4,
            raddr: None,
            rport: None,
            foundation: None,
            //We should be passing the priority along, but that can break with the filtering done on the pair making side.
            priority: None,
        }
    }
    fn fondation(&self) -> String {
        if let Some(f) = &self.foundation {
            return f.clone();
        }
        match self.candidate_type {
            CandidateType::Host => "0".to_owned(),
            CandidateType::ServerReflexive => "1".to_owned(),
            CandidateType::PeerReflexive => "5".to_owned(),
            CandidateType::Relay => "2".to_owned(),
        }
    }

    fn component(&self) -> u16 {
        self.component
    }

    fn network_type_string(&self) -> String {
        "UDP".to_owned()
    }

    pub(crate) fn address(&self) -> &SocketAddrV4 {
        &self.address
    }

    fn port(&self) -> u16 {
        self.address.port()
    }

    fn priority(&self) -> u32 {
        if let Some(p) = self.priority {
            return p;
        }
        u32::from(self.candidate_type.preference()) << 24
            | (u32::from(0xFFFF_u16) << 8)
            | (256 - u32::from(self.component))
    }
    pub(crate) fn candidate_type(&self) -> CandidateType {
        self.candidate_type
    }

    fn raddr(&self) -> Option<String> {
        self.raddr.clone()
    }
    fn rport(&self) -> Option<u16> {
        self.rport
    }
}
/// Format the candidate so it's suitable to send over signaling
impl Display for Candidate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "candidate:{} {} {} {} {} {} typ {}",
            self.fondation(),
            self.component(),
            self.network_type_string(),
            self.priority(),
            self.address().ip(),
            self.port(),
            self.candidate_type()
        )?;

        if let Some(raddr) = self.raddr() {
            write!(f, " raddr {} rport {}", raddr, self.rport().unwrap())
        } else {
            Ok(())
        }
    }
}

/// Attempt to create a candidate from a string received via signaling
impl TryFrom<String> for Candidate {
    type Error = CandidateError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let split = value.split_whitespace().collect::<Vec<&str>>();
        if split.len() < 8 {
            return Err(CandidateError::CannotParseCandidate);
        }

        let fondation = split[0].to_owned();

        let component = split[1]
            .parse::<u16>()
            .map_err(|_| CandidateError::CannotParseCandidate)?;

        // we reject candidate that are not over UDP
        if split[2] != "UDP" && split[2] != "udp" {
            return Err(CandidateError::NotUDPCandidate);
        }

        let priority = split[3]
            .parse::<u32>()
            .map_err(|_| CandidateError::CannotParseCandidate)?;

        let address = split[4].to_owned();

        // if the candidate we receive is Ipv6 mDNS we reject it
        // mDNS candidate will be discovered as peer reflexive during connectivity check
        let address = address
            .parse::<Ipv4Addr>()
            .map_err(|_| CandidateError::CannotParseCandidate)?;

        let port = split[5]
            .parse::<u16>()
            .map_err(|_| CandidateError::CannotParseCandidate)?;

        let typ = split[7];

        let (raddr, rport) = if split.len() == 12 {
            (
                Some(split[9].to_owned()),
                Some(
                    split[11]
                        .parse::<u16>()
                        .map_err(|_| CandidateError::CannotParseCandidate)?,
                ),
            )
        } else {
            (None, None)
        };
        match typ {
            "host" => Ok(Candidate {
                foundation: Some(fondation),
                component,
                address: SocketAddrV4::new(address, port),
                priority: Some(priority),
                raddr,
                rport,
                candidate_type: CandidateType::Host,
                network_type: NetworkType::UDP,
            }),
            "srflx" => Ok(Candidate {
                foundation: Some(fondation),
                component,
                address: SocketAddrV4::new(address, port),
                priority: Some(priority),
                raddr,
                rport,
                candidate_type: CandidateType::ServerReflexive,
                network_type: NetworkType::UDP,
            }),
            "prflx" => Ok(Candidate {
                foundation: Some(fondation),
                component,
                address: SocketAddrV4::new(address, port),
                priority: Some(priority),
                raddr,
                rport,
                candidate_type: CandidateType::PeerReflexive,
                network_type: NetworkType::UDP,
            }),
            "relay" => Ok(Candidate {
                foundation: Some(fondation),
                component,
                address: SocketAddrV4::new(address, port),
                priority: Some(priority),
                raddr,
                rport,
                candidate_type: CandidateType::Relay,
                network_type: NetworkType::UDP,
            }),
            _ => Err(CandidateError::UnsupportedType),
        }
    }
}

/// Represent the state of a candidate pair
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CandidatePairState {
    Waiting,
    InProgress,
    Succeeded,
    Failed,
    Frozen,
}

impl Display for CandidatePairState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Failed => write!(f, "failed"),
            Self::Frozen => write!(f, "frozen"),
            Self::Succeeded => write!(f, "sucess"),
            Self::Waiting => write!(f, "waiting"),
            Self::InProgress => write!(f, "progess"),
        }
    }
}

/// Represent a pair of candidate that may be able to communicate with each other.
#[derive(Eq, Debug)]
pub struct CandidatePair {
    pub(crate) local: usize,
    pub(crate) remote: usize,
    prio: u64,
    state: CandidatePairState,
    nominated: bool,
    /// track the current binding request that was sent at least once
    /// this request will be sent again if no response was received after at least Ta*1ms
    current_binding_request: Option<BindingRequests>,
    /// binding request received on this pair
    pub(crate) binding_req_recv: u32,
    binding_req_sent: u32,
    /// successful binding requests on this pair
    pub(crate) binding_resp_recv: u32,
}

impl CandidatePair {
    pub(crate) fn new(
        local: &Candidate,
        remote: &Candidate,
        local_idx: usize,
        remote_idx: usize,
    ) -> Result<Self, CandidateError> {
        // Only support ipv4 & udp so just need to check component id is correct
        if local.component() != remote.component() {
            return Err(CandidateError::CannotFormCandidatePair);
        }
        // Remote is always the controlling agent
        // 5.7.2.  Computing Pair Priority and Ordering Pairs
        let prio: u64 = 2_u64.pow(32) * (std::cmp::min(local.priority(), remote.priority()) as u64)
            + (2 * std::cmp::max(local.priority(), remote.priority()) as u64)
            + u64::from(local.priority() > remote.priority());

        Ok(Self {
            local: local_idx,
            remote: remote_idx,
            prio,
            state: CandidatePairState::Waiting,
            nominated: false,
            binding_req_recv: 0,
            current_binding_request: None, // store last 4 attempts
            binding_resp_recv: 0,
            binding_req_sent: 0,
        })
    }
    pub(crate) fn state(&self) -> &CandidatePairState {
        &self.state
    }
    /// create a new binding request if None have been created already other returns the
    /// TransactionId of the last request
    pub(crate) fn create_new_binding_request(&mut self, now: Instant) -> Option<TransactionId> {
        match self.state {
            CandidatePairState::Frozen => {
                return None;
            }
            CandidatePairState::Failed => {
                return None;
            }
            CandidatePairState::Waiting => {
                self.state = CandidatePairState::InProgress;
            }
            CandidatePairState::InProgress | CandidatePairState::Succeeded => {
                if let Some(req) = self.current_binding_request.as_mut() {
                    // Retry while pair is InProgress, Ta is set a 500ms.
                    if now - req.req_time < Duration::from_millis(250) {
                        return None;
                    }
                    self.binding_req_sent += 1;
                    req.req_time = now;
                    return Some(req.id);
                }
            }
        }
        let id = TransactionId::new(rand::random());
        let _ = self.current_binding_request.insert(BindingRequests {
            id,
            req_time: now,
            resp_recv: false,
        });

        self.binding_req_sent += 1;
        Some(id)
    }

    /// Check if the CandidatePair should be set to fail
    pub fn update_pair_status(&mut self) {
        if self.state != CandidatePairState::Failed
            && self.binding_req_sent > self.binding_req_recv
            && self.binding_req_sent - self.binding_req_recv > 50
        {
            // after 20 failed attempts mark the pair as failed
            self.state = CandidatePairState::Failed;
        }
    }
    /// Check if a binding response belongs to this Pair
    pub fn binding_response(&mut self, _now: &Instant, id: &TransactionId) -> bool {
        if let Some(req) = self.current_binding_request.as_ref() {
            if req.id == *id {
                self.current_binding_request = None;
                self.binding_req_recv += 1;
                self.state = CandidatePairState::Succeeded;
                log::debug!("Pair succeeded {:?}", self);
                return true;
            }
        }
        false
    }
}

impl PartialEq for CandidatePair {
    fn eq(&self, other: &Self) -> bool {
        self.local == other.local && self.remote == other.remote && self.prio == other.prio
    }
}

impl PartialOrd for CandidatePair {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CandidatePair {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.prio.cmp(&other.prio)
    }
}

// Represent a particular binding requests for a CandidatePair
#[derive(Eq, Debug)]
pub struct BindingRequests {
    id: TransactionId,
    req_time: Instant, // When the request was generated (does not track if it qwas sent but we assume so)
    resp_recv: bool,   //Wether a succes response was received for this particular request
}

impl PartialEq for BindingRequests {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddrV4;

    use super::Candidate;
    use super::CandidateType;

    #[test_log::test]
    fn test_parse_candidate_string() {
        let c1 = "candidate:2230659787 1 udp 2130706431 0df946e3-c405-403b-8c7b-1dc8ff69b55a.local 54182 typ host".to_owned();
        let ret = TryInto::<Candidate>::try_into(c1);
        assert!(ret.is_err());

        let c1 =
            "candidate:830412194 1 udp 1694498815 ::1 49701 typ host raddr 0.0.0.0 rport 49701"
                .to_owned();
        let ret = TryInto::<Candidate>::try_into(c1);
        assert!(ret.is_err());

        let c1 = "candidate:2230659787 1 udp 2130706431 10.1.2.3 54182 typ host".to_owned();
        let ret = TryInto::<Candidate>::try_into(c1);
        assert!(ret.is_ok());

        let c1 = ret.unwrap();
        assert_eq!(c1.candidate_type, CandidateType::Host);
        assert_eq!(
            c1.address,
            SocketAddrV4::new("10.1.2.3".parse().unwrap(), 54182)
        );
        assert_eq!(c1.priority.unwrap(), 2130706431);
        assert_eq!(c1.component, 1);
        assert_eq!(c1.foundation.unwrap(), "candidate:2230659787");
        assert!(c1.raddr.is_none());
        assert!(c1.rport.is_none());

        let c1 = "candidate:830412194 1 udp 1694498815 71.167.39.185 49701 typ srflx raddr 0.0.0.0 rport 49701".to_owned();
        let ret = TryInto::<Candidate>::try_into(c1);
        assert!(ret.is_ok());

        let c1 = ret.unwrap();
        assert_eq!(c1.candidate_type, CandidateType::ServerReflexive);
        assert_eq!(
            c1.address,
            SocketAddrV4::new("71.167.39.185".parse().unwrap(), 49701)
        );
        assert_eq!(c1.priority.unwrap(), 1694498815);
        assert_eq!(c1.component, 1);
        assert_eq!(c1.foundation.unwrap(), "candidate:830412194");
        assert_eq!(c1.raddr.unwrap(), "0.0.0.0");
        assert_eq!(c1.rport.unwrap(), 49701);
    }

    #[test_log::test]
    fn test_candidate_to_string() {
        let c1 =
            Candidate::new_host_candidate(SocketAddrV4::new("127.0.0.1".parse().unwrap(), 61322));

        let r = format!("{c1}");

        assert_eq!("candidate:0 1 UDP 2130706431 127.0.0.1 61322 typ host", r);

        let c1 = Candidate::new_srflx_candidate(
            SocketAddrV4::new("89.72.32.132".parse().unwrap(), 61322),
            SocketAddrV4::new("127.0.0.1".parse().unwrap(), 61322),
        );

        let r = format!("{c1}");

        assert_eq!(
            "candidate:1 1 UDP 1694498815 89.72.32.132 61322 typ srflx raddr 0.0.0.0 rport 0",
            r
        );
    }
}
