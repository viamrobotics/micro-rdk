#![allow(dead_code)]
use std::{
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs},
    pin::Pin,
    time::{Duration, Instant},
};

use async_io::Timer;
use bytecodec::{DecodeExt, EncodeExt};
use bytes::{Bytes, BytesMut};
use thiserror::Error;

use futures_lite::{Future, FutureExt};
use rand::{
    distr::{Alphanumeric, SampleString},
    rng,
};

use stun_codec::{
    rfc5245,
    rfc5389::{self, methods::BINDING},
    Message, MessageClass, TransactionId,
};

use crate::{common::webrtc::candidates::CandidatePairState, IceAttribute};

use super::{
    api::AtomicSync,
    candidates::{Candidate, CandidateError, CandidatePair, CandidateType},
    udp_mux::UdpMux,
};

#[derive(Clone, Debug)]
pub struct ICECredentials {
    pub(crate) u_frag: String,
    pub(crate) pwd: String,
}

impl Default for ICECredentials {
    fn default() -> Self {
        Self {
            u_frag: Alphanumeric.sample_string(&mut rng(), 8),
            pwd: Alphanumeric.sample_string(&mut rng(), 22),
        }
    }
}

impl ICECredentials {
    pub(crate) fn new(u_frag: String, pwd: String) -> Self {
        Self { u_frag, pwd }
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum IceError {
    #[error("candidate channel closed")]
    IceCandidateChannelClosed,
    #[error("ice transport closed")]
    IceTransportClosed,
    #[error("server is not ipv4")]
    IceStunServerNotIPV4,
    #[error("io error from transport")]
    IceIoError,
    #[error("missing xor_mapped address")]
    IceMissingXorMappedAddress,
    #[error("xor mapped address is ipv6")]
    IceXorMappedAddressIsIPV6,
    #[error("missing {0} username")]
    IceMissingUserName(&'static str),
    #[error("failed username check")]
    IceFailedUsernameCheck,
    #[error("invalid stun message")]
    IceInvalidStunMessage,
    #[error("no local candidate")]
    IceNoLocalCandidates,
    #[error("no pair for this stun response")]
    IceNoPairForThisStunResponse,
    #[error("can't encode stun packet")]
    IceStunEncodingError,
    #[error("can't decode stun packet")]
    IceStunDecodingError,
    #[error("ice operation timeout")]
    IceTimeout,
    #[error(transparent)]
    IceCandidateError(#[from] CandidateError),
}

enum IceEvent {
    CandidateReceived(Candidate),
    StunPacketReceived((usize, SocketAddrV4)),
}

/// ICE Agent implementation for micro-RDK, the goal is to keep it lightweight. Therefore it doesn't
/// implement the full RFC5245
/// Notable omissions:
/// * Only support ICE-CONTROLLED
/// * Doesn't resolve local mDNS candidate presented
/// * Doesn't do a best effort to find a better pair once one was nominated
/// * Doesn't support Ice Restart
/// * Doesn't support freeing candidates
/// * Can only do trickle ice
/// * Adding/Removing tracks
pub struct ICEAgent {
    pub(crate) local_candidates: Vec<Candidate>,
    remote_candidates: Vec<Candidate>,
    remote_candidates_chan: async_channel::Receiver<Candidate>,
    transport: UdpMux,
    candidate_pairs: Vec<CandidatePair>,
    local_credentials: ICECredentials,
    remote_credentials: ICECredentials,
    state: ICEAgentState,
    local_ip: Ipv4Addr,
}

impl Drop for ICEAgent {
    fn drop(&mut self) {
        let _ = self.remote_candidates_chan.close();
    }
}

#[derive(Eq, Debug, PartialEq)]
enum ICEAgentState {
    Checking,
    Connected,
}

impl ICEAgent {
    pub(crate) fn new(
        remote_candidates_chan: async_channel::Receiver<Candidate>,
        transport: UdpMux,
        local_credentials: ICECredentials,
        remote_credentials: ICECredentials,
        local_ip: Ipv4Addr,
    ) -> Self {
        Self {
            local_candidates: vec![],
            remote_candidates: vec![],
            remote_candidates_chan,
            transport,
            candidate_pairs: vec![],
            local_ip,
            local_credentials,
            remote_credentials,
            state: ICEAgentState::Checking,
        }
    }

    /// Gather local candidates, it will only generate one host and one server reflexive,
    /// relay candidates are not supported yet
    pub async fn local_candidates(&mut self) -> Result<(), IceError> {
        if !self.local_candidates.is_empty() {
            return Ok(());
        }

        log::debug!("local_candidates: registering intrinsic local candidate");
        let our_ip = SocketAddrV4::new(
            self.local_ip,
            self.transport
                .local_address()
                .map_err(|_| IceError::IceIoError)?
                .port(),
        );
        let local_cand = Candidate::new_host_candidate(our_ip);
        self.local_candidates.push(local_cand);

        log::debug!("local_candidates: looking for srv reflexive candidate");

        let message = stun_codec::Message::<stun_codec::rfc5389::Attribute>::new(
            stun_codec::MessageClass::Request,
            stun_codec::rfc5389::methods::BINDING,
            stun_codec::TransactionId::new(rand::random()),
        );

        let mut encoder = stun_codec::MessageEncoder::new();
        let bytes = Bytes::from(encoder.encode_into_bytes(message).unwrap());

        // TODO(RSDK-3063) Twilio address is hard-coded, we should support additional server via WebRTCOptions
        let mut stun_ip = match "global.stun.twilio.com:3478".to_socket_addrs() {
            Ok(stun_ip) => stun_ip,
            Err(err) => {
                log::warn!("Failed trying to resolve STUN server address; no reflexive candidate will be generated: {}", err);
                return Ok(());
            }
        };

        let stun_ip = match stun_ip.next() {
            Some(stun_ip) => stun_ip,
            None => {
                log::warn!("STUN server address resolution found no records; no reflexive candidate will be generated");
                return Ok(());
            }
        };

        let stun_ip = match stun_ip {
            SocketAddr::V4(v4) => v4,
            _ => {
                return Err(IceError::IceStunServerNotIPV4);
            }
        };

        let mut buf = BytesMut::zeroed(256);
        let (buf_len, _addr) = loop {
            let _r = self
                .transport
                .send_to(&bytes, stun_ip.into())
                .await
                .unwrap();
            let response = self
                .transport
                .recv_from(&mut buf)
                .or(async {
                    Timer::after(Duration::from_secs(1)).await;
                    Err(io::Error::new(io::ErrorKind::TimedOut, ""))
                })
                .await;

            match response {
                Ok(rsp) => break rsp,
                Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
                Err(_) => return Err(IceError::IceIoError),
            };
        };
        let mut decoder = stun_codec::MessageDecoder::<stun_codec::rfc5389::Attribute>::new();

        let decoded = decoder
            .decode_from_bytes(&buf[..buf_len])
            .map_err(|_| IceError::IceStunDecodingError)?
            .unwrap();

        let xor_mapped_addr =
            match decoded.get_attribute::<stun_codec::rfc5389::attributes::XorMappedAddress>() {
                Some(addr) => addr.address(),
                None => return Err(IceError::IceMissingXorMappedAddress),
            };

        let rflx_addr = match xor_mapped_addr {
            SocketAddr::V4(v4) => v4,
            SocketAddr::V6(_) => return Err(IceError::IceXorMappedAddressIsIPV6),
        };

        let srflx_candidate = Candidate::new_srflx_candidate(rflx_addr, our_ip);
        self.local_candidates.push(srflx_candidate);

        Ok(())
    }

    /// run the ice agent, processing incoming STUN packet and emitting STUN request
    // TODO remove dependency on &mut self so ICEAgent can be closed without relying on the AtomicSync
    pub(crate) async fn run(&mut self, done: AtomicSync, stop: AtomicSync) {
        log::debug!("Running ICE Agent");

        let error = loop {
            let stop = stop.clone();
            for pair in &mut self.candidate_pairs {
                pair.update_pair_status();
                // TODO(npm) check for nomination flag before we are actually connected
                // note: nomitation flag isn't set yet
                if self.state != ICEAgentState::Connected
                    && *pair.state() == CandidatePairState::Succeeded
                {
                    // When at least one pair is succeeded we go in the connected state
                    // we will not attempt to find a better candidate pair
                    self.state = ICEAgentState::Connected;
                    // this is a work around to tell the WebRTCAPI that signaling can be
                    // stopped and DTLS should be started
                    done.done();
                }
            }

            let req = self.next_stun_request();
            if let Some(req) = req {
                if let Ok(msg) = self.make_stun_request(req.0) {
                    if self.transport.send_to(&msg, req.1.into()).await.is_err() {
                        break IceError::IceTransportClosed;
                    }
                }
            }

            let mut buf = BytesMut::zeroed(256);

            let f1: Pin<Box<dyn Future<Output = Result<IceEvent, IceError>> + Send>> =
                if !self.remote_candidates_chan.is_closed() {
                    Box::pin(async {
                        self.remote_candidates_chan
                            .recv()
                            .await
                            .map(IceEvent::CandidateReceived)
                            .map_err(|_| IceError::IceCandidateChannelClosed)
                    })
                } else {
                    Box::pin(async {
                        stop.await;
                        Err(IceError::IceTransportClosed)
                    })
                };
            let f2 = Box::pin(async {
                self.transport
                    .recv_from(&mut buf)
                    .await
                    .map(|(len, addr)| {
                        //TODO deal with IpV6
                        let addr = match addr {
                            SocketAddr::V4(addr) => addr,
                            _ => panic!(),
                        };
                        IceEvent::StunPacketReceived((len, addr))
                    })
                    .map_err(|_| IceError::IceTransportClosed)
            });

            let event = futures_lite::future::or(f1, f2)
                .or(async {
                    // TODO we should take the min time for next candidate pair check
                    Timer::after(Duration::from_millis(500)).await;
                    Err(IceError::IceTimeout)
                })
                .await;

            let event = match event {
                Ok(r) => r,
                Err(IceError::IceCandidateChannelClosed) | Err(IceError::IceTimeout) => {
                    continue;
                }
                Err(e) => {
                    break e;
                }
            };
            match event {
                IceEvent::CandidateReceived(c) => {
                    self.remote_candidates.push(c);
                    self.form_pairs(self.remote_candidates.len() - 1);
                    for pair in &self.candidate_pairs {
                        log::debug!(
                            "Pair list is {:?} -> {:?} ",
                            self.local_candidates[pair.local].address(),
                            self.remote_candidates[pair.remote].address()
                        )
                    }
                }
                IceEvent::StunPacketReceived((len, addr)) => {
                    let mut decoder = stun_codec::MessageDecoder::<IceAttribute>::new();
                    let decoded = match decoder.decode_from_bytes(&buf[..len]).unwrap() {
                        Ok(e) => e,
                        Err(e) => {
                            log::error!("dropping stun msg {:?}", e);
                            buf.clear();
                            continue;
                        }
                    };
                    buf.clear();

                    match decoded.class() {
                        MessageClass::Request => {
                            log::debug!("processing a stun request");
                            if let Ok(msg) = self.process_stun_request(&decoded, &addr) {
                                if self.transport.send_to(&msg, addr.into()).await.is_err() {
                                    break IceError::IceTransportClosed;
                                }
                            }
                        }
                        MessageClass::SuccessResponse => {
                            if let Err(e) = self.process_stun_response(Instant::now(), decoded) {
                                // could be caused by multiple response for one request
                                log::error!("unable to properly process stun response {:?}", e);
                            }
                        }

                        MessageClass::ErrorResponse => {
                            //TODO(RSDK-3064)
                            log::error!("received a stun error");
                        }
                        MessageClass::Indication => {
                            //TODO(RSDK-3064)
                            log::error!("received a stun indication")
                        }
                    }
                }
            }
        };

        log::error!("closing ice agent with error {:?}", error);
    }

    /// next_stun_request finds the next suitable pair to do a connection check on
    /// to do so it parses the pair list in the following manner
    /// 1) If a pair has no pending STUN request it generates an TransactionId and attach to the pair
    /// 2) If a pair has a pending STUN request and its timeout is elapsed it will resend
    ///    the generated TransactionId
    /// 3) Otherwise it moves to the next candidate pair
    fn next_stun_request(&mut self) -> Option<(TransactionId, SocketAddrV4)> {
        let instant = Instant::now();
        for pair in &mut self.candidate_pairs {
            log::debug!("processing pair {:?}", pair);
            let id = pair.create_new_binding_request(instant);
            if let Some(id) = id {
                log::debug!(
                    "will attempt to make a stun request from {:?} to {:?}",
                    self.local_candidates[pair.local],
                    self.remote_candidates[pair.remote]
                );
                return Some((id, *self.remote_candidates[pair.remote].address()));
            }
        }
        None
    }

    fn form_pairs(&mut self, remote_idx: usize) {
        for (local_idx, local) in self.local_candidates.iter().enumerate() {
            // Assumption, ipv6 candidates are rejected by default
            let remote = &self.remote_candidates[remote_idx];

            // TODO(RSDK-3065) srflx candidate should be replaced with their base
            // see 5.7.3.  Pruning the Pairs
            if local.candidate_type == CandidateType::ServerReflexive {
                continue;
            }

            let pair = match CandidatePair::new(local, remote, local_idx, remote_idx) {
                Err(e) => {
                    log::error!("Couldn't form pair {:?}", e);
                    continue;
                }
                Ok(c) => c,
            };
            let _ = match self
                .candidate_pairs
                .binary_search_by(|other| pair.cmp(other))
            {
                Ok(idx) => {
                    log::debug!(
                        "pair with same prio already exists from {:?} to {:?} against {:?} {:?}",
                        self.local_candidates[self.candidate_pairs[idx].local],
                        self.remote_candidates[self.candidate_pairs[idx].remote],
                        local,
                        remote
                    );
                    idx
                }
                Err(idx) => {
                    self.candidate_pairs.insert(idx, pair);
                    idx
                }
            };
            // TODO(RSDK-3066) prune the pairs
        }

        log::debug!(
            "our candidates checkliste size is {}",
            self.candidate_pairs.len()
        );
    }

    /// insert candidate pair, if one with the same prio exists the inserted pair in dropped
    fn insert_candidate_pair(&mut self, pair: CandidatePair) -> Option<usize> {
        match self
            .candidate_pairs
            .binary_search_by(|other| pair.cmp(other))
        {
            Ok(_idx) => {
                // TODO(npm) consider replacing the pair
                // this would help when discovering peer reflexive candidates
                log::debug!("pair with same prio already exists");
                None
            }
            Err(idx) => {
                self.candidate_pairs.insert(idx, pair);
                Some(idx)
            }
        }
    }

    /// Validate a stun message, note it doesn't check the integrity
    fn validate_stun_message(&self, stun: &Message<IceAttribute>) -> Result<(), IceError> {
        log::debug!("processing {:?}", &stun);
        if let BINDING = stun.method() {
            let mut creds = stun
                .get_attribute::<rfc5389::attributes::Username>()
                .unwrap()
                .name()
                .split(':');
            let local_u = creds.next().ok_or(IceError::IceMissingUserName("local"))?;
            let remote_u = creds.next().ok_or(IceError::IceMissingUserName("remote"))?;

            if local_u != self.local_credentials.u_frag {
                return Err(IceError::IceFailedUsernameCheck);
            }
            if remote_u != self.remote_credentials.u_frag {
                return Err(IceError::IceFailedUsernameCheck);
            }

            return Ok(());
        }
        Err(IceError::IceInvalidStunMessage)
    }

    fn process_stun_request(
        &mut self,
        stun: &Message<IceAttribute>,
        from: &SocketAddrV4,
    ) -> Result<Vec<u8>, IceError> {
        let use_candidate = if stun
            .get_attribute::<rfc5245::attributes::UseCandidate>()
            .is_some()
        {
            log::debug!("received a use candidate");
            true
        } else {
            false
        };

        let id = stun.transaction_id();
        if stun
            .get_attribute::<rfc5245::attributes::IceControlling>()
            .is_none()
        {
            log::debug!("we should have had the controlling attribute")
            // TODO(RSDK-3067) probably should error out here
        };

        let have_as_remote_candidate = match self
            .remote_candidates
            .iter()
            .enumerate()
            .position(|(_, c)| c.address() == from)
        {
            Some(idx) => idx,
            None => {
                log::debug!("received a peer reflexive address, we are going to add it to our list of remote candidates");
                let prio = stun
                    .get_attribute::<rfc5245::attributes::Priority>()
                    .ok_or(IceError::IceInvalidStunMessage)?
                    .prio();
                let candidate = Candidate::new_peer_reflexive(*from, Some(prio));

                self.remote_candidates.push(candidate);
                self.remote_candidates.len() - 1
            }
        };

        let local_host = self
            .local_candidates
            .iter()
            .enumerate()
            .position(|(_, c)| c.candidate_type() == CandidateType::Host)
            .ok_or(IceError::IceNoLocalCandidates)?;
        let pair_idx = match self
            .candidate_pairs
            .iter()
            .position(|c| c.local == local_host && c.remote == have_as_remote_candidate)
        {
            Some(idx) => Some(idx),
            None => {
                let local_c = &self.local_candidates[local_host];
                let remote_c = &self.remote_candidates[have_as_remote_candidate];

                let pair =
                    CandidatePair::new(local_c, remote_c, local_host, have_as_remote_candidate)
                        .map_err(IceError::IceCandidateError)?;

                self.insert_candidate_pair(pair)
            }
        };
        if let Some(pair_idx) = pair_idx {
            if use_candidate {
                log::debug!(
                    "should nominate Pair {:?} L:{:?} R:{:?}",
                    self.candidate_pairs[pair_idx],
                    &self.local_candidates[local_host],
                    &self.remote_candidates[have_as_remote_candidate]
                );
            }
            self.candidate_pairs[pair_idx].binding_req_recv += 1;

            return self.stun_success_response((*from).into(), id);
        }
        Err(IceError::IceNoPairForThisStunResponse)
    }

    // send the response to a binding request
    fn stun_success_response(
        &self,
        from: SocketAddr,
        id: TransactionId,
    ) -> Result<Vec<u8>, IceError> {
        let mut message = Message::<IceAttribute>::new(MessageClass::SuccessResponse, BINDING, id);
        message.add_attribute(IceAttribute::XorMappedAddress(
            rfc5389::attributes::XorMappedAddress::new(from),
        ));
        message.add_attribute(IceAttribute::MessageIntegrity(
            stun_codec::rfc5389::attributes::MessageIntegrity::new_short_term_credential(
                &message,
                &self.local_credentials.pwd,
            )
            .map_err(|_| IceError::IceStunEncodingError)?,
        ));
        message.add_attribute(IceAttribute::Fingerprint(
            stun_codec::rfc5389::attributes::Fingerprint::new(&message)
                .map_err(|_| IceError::IceStunEncodingError)?,
        ));
        let mut encoder = stun_codec::MessageEncoder::new();
        encoder
            .encode_into_bytes(message)
            .map_err(|_| IceError::IceStunEncodingError)
    }

    // process a response to a request
    fn process_stun_response(
        &mut self,
        now: Instant,
        stun: Message<IceAttribute>,
    ) -> Result<(), IceError> {
        let id = stun.transaction_id();
        log::debug!("processing id {:?}", id);
        let _pair = self
            .candidate_pairs
            .iter_mut()
            .find_map(|p| {
                if p.binding_response(&now, &id) {
                    return Some(p);
                }
                None
            })
            .ok_or(IceError::IceStunEncodingError)?;
        Ok(())
    }

    fn make_stun_request(&self, id: TransactionId) -> Result<Vec<u8>, IceError> {
        let mut message = Message::<IceAttribute>::new(MessageClass::Request, BINDING, id);
        message.add_attribute(IceAttribute::Username(
            rfc5389::attributes::Username::new(format!(
                "{}:{}",
                self.remote_credentials.u_frag, self.local_credentials.u_frag
            ))
            .map_err(|_| IceError::IceStunEncodingError)?,
        ));
        message.add_attribute(IceAttribute::IceControlled(
            rfc5245::attributes::IceControlled::new(0),
        ));
        message.add_attribute(IceAttribute::Priority(rfc5245::attributes::Priority::new(
            5_u32 << 24 | (u32::from(0xFFFF_u16) << 8) | (256 - 1_u32),
        )));
        message.add_attribute(IceAttribute::MessageIntegrity(
            rfc5389::attributes::MessageIntegrity::new_short_term_credential(
                &message,
                &self.remote_credentials.pwd,
            )
            .unwrap(),
        ));
        message.add_attribute(IceAttribute::Fingerprint(
            rfc5389::attributes::Fingerprint::new(&message).unwrap(),
        ));
        let mut encoder = stun_codec::MessageEncoder::new();
        encoder
            .encode_into_bytes(message)
            .map_err(|_| IceError::IceStunEncodingError)
    }
}

#[cfg(test)]
mod tests {
    use async_executor::Executor;
    use async_io::Async;
    use futures_lite::future::block_on;
    use std::net::UdpSocket;
    use std::sync::Arc;

    use crate::common::webrtc::ice::{ICEAgent, ICECredentials};

    use crate::common::webrtc::{candidates::Candidate, io::WebRtcTransport};

    use super::IceError;

    #[test_log::test]
    fn test_pair_form() -> Result<(), IceError> {
        let r1 = "candidate:2230659787 1 udp 2130706431 10.1.2.3 54182 typ host".to_owned();
        let r1 = TryInto::<Candidate>::try_into(r1).unwrap();
        let r2 = "candidate:830412194 1 udp 1694498815 71.167.39.185 49701 typ srflx raddr 0.0.0.0 rport 49701".to_owned();
        let r2 = TryInto::<Candidate>::try_into(r2).unwrap();
        let r3 = "candidate:830412194 1 udp 1694498815 71.167.39.185 49701 typ relay raddr 0.0.0.0 rport 49701".to_owned();
        let r3 = TryInto::<Candidate>::try_into(r3).unwrap();

        let executor = Executor::new();

        let udp = block_on(
            executor.run(async { Async::new(UdpSocket::bind("0.0.0.0:0").unwrap()).unwrap() }),
        );

        let transport = WebRtcTransport::new(Arc::new(udp));

        let (tx, rx) = async_channel::unbounded();
        let ice_transport = transport.get_stun_channel().unwrap();

        let our_ip = match local_ip_address::local_ip().unwrap() {
            std::net::IpAddr::V4(v4) => v4,
            _ => {
                return Err(IceError::IceStunServerNotIPV4);
            }
        };

        let mut ice_agent = ICEAgent::new(
            rx,
            ice_transport,
            ICECredentials::default(),
            ICECredentials::default(),
            our_ip,
        );
        let ret = block_on(executor.run(async { ice_agent.local_candidates().await }));

        assert!(ret.is_ok());

        assert!(!ice_agent.local_candidates.is_empty());

        assert!(tx.send_blocking(r1).is_ok());
        assert!(tx.send_blocking(r2).is_ok());
        assert!(tx.send_blocking(r3).is_ok());

        Ok(())
    }
}
