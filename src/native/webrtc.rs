// pub struct WebRTCSignaling {
//     signaling_tx: GrpcMessageSender<AnswerResponse>,
//     signaling_rx: GrpcMessageStream<AnswerRequest>,
//     done: Arc<AtomicBool>,
//     certificate: Arc<PeerCertificate>,
// }

// impl WebRTCSignaling {
//     pub fn new(
//         tx_half: GrpcMessageSender<AnswerResponse>,
//         rx_half: GrpcMessageStream<AnswerRequest>,
//         certificate: Arc<PeerCertificate>,
//     ) -> Self {
//         Self {
//             signaling_tx: tx_half,
//             signaling_rx: rx_half,
//             done: Arc::new(AtomicBool::new(false)),
//             certificate,
//         }
//     }
//     async fn process_signaling_request(
//         &mut self,
//         req: AnswerRequest,
//     ) -> anyhow::Result<Option<AnswerResponse>> {
//         if let Some(stage) = req.stage {
//             match stage {
//                 rpc::webrtc::v1::answer_request::Stage::Init(s) => {
//                     self.uuid = req.uuid;
//                     let sdp_decoded = self.engine.decode(s.sdp)?;
//                     let sdp_decoded: SdpOffer = serde_json::from_slice(sdp_decoded.as_slice())?;
//                     log::debug!(
//                         "Getting offer {:?} of type {:?}",
//                         &sdp_decoded.sdp,
//                         &sdp_decoded.r#type
//                     );
//                     let mut cursor = Cursor::new(sdp_decoded.sdp);
//                     log::debug!("we have an offer - there should be no PeerConnection");
//                     if self.peer_connection.lock().unwrap().is_some() {
//                         return Err(anyhow::anyhow!("a peer connection already exists"));
//                     }
//                     let sdp = sdp::SessionDescription::unmarshal(&mut cursor)?;
//                     let _ = self
//                         .peer_connection
//                         .lock()
//                         .unwrap()
//                         .insert(PeerConnection::new(sdp));

//                     let answer = if let Some(cp) = self.peer_connection.lock().unwrap().as_ref() {
//                         cp.answer()
//                     } else {
//                         return Err(anyhow::anyhow!(
//                             "No peerconnection found but we just created is"
//                         ));
//                     };

//                     log::debug!("We are answering {:?}", &answer);

//                     let answer = SdpOffer {
//                         r#type: "answer".to_owned(),
//                         sdp: answer.marshal(),
//                     };

//                     let answer = self.engine.encode(serde_json::to_string(&answer)?);

//                     let answer = AnswerResponse {
//                         uuid: self.uuid.clone(),
//                         stage: Some(rpc::webrtc::v1::answer_response::Stage::Init(
//                             AnswerResponseInitStage { sdp: answer },
//                         )),
//                     };
//                     return Ok(Some(answer));
//                 }
//                 rpc::webrtc::v1::answer_request::Stage::Update(u) => {
//                     if let Some(c) = u.candidate {
//                         //log::info!("getting {:?}", &c.candidate);
//                         let candidate: Candidate = c.candidate.try_into()?;
//                         if let Some(c) = &self.remote_candidate_chan_tx {
//                             //log::info!("sendind candidate");
//                             c.send(candidate).await?;
//                         }
//                     }
//                 }
//                 _ => return Err(anyhow::anyhow!("not yet implemented")),
//             }
//         }
//         Ok(None)
//     }

//     async fn run_signaling(&mut self) {
//         while !self.done.load(std::sync::atomic::Ordering::Relaxed) {
//             let req = self
//                 .signaling_rx
//                 .next()
//                 .timeout(Duration::from_millis(100))
//                 .await;
//             if let Some(req) = req {
//                 let resp = match req {
//                     Some(req) => self.process_signaling_request(req).await,
//                     None => Err(anyhow::anyhow!("no request is link down?")),
//                 };

//                 match resp {
//                     Ok(resp) => {
//                         if let Some(answer) = resp {
//                             self.signaling_tx.send_message(answer).unwrap();
//                         }
//                     }
//                     Err(e) => {
//                         log::error!("error received was {:?}", e);
//                     }
//                 };
//             };
//         }
//     }
// }

// pub struct WebRTC<'a> {
//     executor: NativeExecutor<'a>,
//     peer_connection: Rc<Mutex<Option<PeerConnection>>>,
//     //ice_agent: ICEAgent,
//     signaling_tx: GrpcMessageSender<AnswerResponse>,
//     signaling_rx: GrpcMessageStream<AnswerRequest>,
//     engine: base64::engine::general_purpose::GeneralPurpose,
//     remote_candidate_chan_tx: Option<smol::channel::Sender<Candidate>>,
//     uuid: String,
//     transport: WebRTCTransport,
// }

// impl<'a> WebRTC<'a> {
//     pub(crate) fn new(
//         executor: NativeExecutor<'a>,
//         tx_half: GrpcMessageSender<AnswerResponse>,
//         rx_half: GrpcMessageStream<AnswerRequest>,
//     ) -> Self {
//         let udp = block_on(executor.run(async { UdpSocket::bind("0.0.0.0:61203").await.unwrap() }));
//         Self {
//             executor: executor.clone(),
//             peer_connection: Rc::new(Mutex::new(None)),
//             //ice_agent: ICEAgent::new(),
//             signaling_tx: tx_half,
//             signaling_rx: rx_half,
//             engine: base64::engine::general_purpose::STANDARD,
//             remote_candidate_chan_tx: None,
//             uuid: uuid::Uuid::new_v4().to_string(),
//             transport: WebRTCTransport::new(udp),
//         }
//     }
//     async fn send_local_candidate(&mut self, candidate: Candidate) -> anyhow::Result<()> {
//         let ufrag = if let Some(pc) = self.peer_connection.lock().unwrap().as_ref() {
//             pc.ice_ufrag.clone()
//         } else {
//             return Err(anyhow::anyhow!(
//                 "the pc doesn't exists yet we are sending candidates?"
//             ));
//         };
//         log::info!("Sending candidate");
//         let answer = AnswerResponse {
//             uuid: self.uuid.clone(),
//             stage: Some(rpc::webrtc::v1::answer_response::Stage::Update(
//                 rpc::webrtc::v1::AnswerResponseUpdateStage {
//                     candidate: Some(IceCandidate {
//                         candidate: candidate.to_string(),
//                         sdp_mid: Some("".to_owned()),
//                         sdpm_line_index: Some(0),
//                         username_fragment: Some(ufrag),
//                     }),
//                 },
//             )),
//         };
//         log::info!("Returning following answer {:?}", &answer);

//         self.signaling_tx.send_message(answer)
//     }
//     async fn send_done(&mut self) -> anyhow::Result<()> {
//         let answer = AnswerResponse {
//             uuid: self.uuid.clone(),
//             stage: Some(rpc::webrtc::v1::answer_response::Stage::Done(
//                 rpc::webrtc::v1::AnswerResponseDoneStage {},
//             )),
//         };
//         self.signaling_tx.send_message(answer)
//     }
//     pub fn run(&mut self) -> anyhow::Result<()> {
//         let (sender, receiver) = smol::channel::bounded(1);
//         let _ = self.remote_candidate_chan_tx.insert(sender);
//         let pc = self.peer_connection.clone();

//         let mut once = false;
//         //let mut agent = ICEAgent::new(receiver, peer_connection, transport, local_credentials, remote_credentials)
//         let tx = self.transport.clone();
//         let rx = self.transport.clone();
//         block_on(self.executor.clone().run(async {
//             self.executor
//                 .spawn(async move { tx.read_loop().await })
//                 .detach();
//             self.executor
//                 .spawn(async move { rx.write_loop().await })
//                 .detach();

//             loop {
//                 if !once {
//                     let mut agent = if let Some(pc) = self.peer_connection.lock().unwrap().as_ref()
//                     {
//                         let dtls_t = self.transport.get_dtls_channel().unwrap();
//                         let cert = pc.peer_certificate.clone();

//                         let dtls = Dtls::new(Rc::new(cert), dtls_t).unwrap();

//                         let dtls_stream = dtls.accept().await.unwrap();
//                         let (c_tx, c_rx) = async_channel::unbounded();

//                         let mut sctp = Sctp2::new(dtls_stream, self.executor.clone(), c_tx);

//                         sctp.listen().await.unwrap();
//                         self.executor
//                             .spawn(async move {
//                                 sctp.run().await;
//                             })
//                             .detach();

//                         let mut c = c_rx.recv().await.unwrap();
//                         self.executor
//                             .spawn(async move {
//                                 loop {
//                                     use futures_lite::AsyncReadExt;
//                                     let mut buf = [0; 1500];
//                                     let ret = c.read(&mut buf).await;
//                                     match ret {
//                                         Err(e) => log::info!("error echoing {:?}", e),
//                                         Ok(len) => {
//                                             log::info!(
//                                                 "Echoing from server {} internal len {}",
//                                                 len,
//                                                 u16::from_be_bytes(buf[1..3].try_into().unwrap())
//                                             );
//                                             let buf2 = &buf[..59];
//                                             let msg = webrtc::v1::Request::decode(buf2).unwrap();
//                                             log::info!("request {:?} {}", msg, msg.encoded_len());
//                                         }
//                                     }
//                                 }
//                             })
//                             .detach();

//                         once = true;
//                         //TODO (npm) should be ICEcredential[]s
//                         let remote_creds = ICECredentials {
//                             u_frag: pc.get_remote_ice_ufrag().unwrap().to_string(),
//                             pwd: pc.get_remote_ice_pwd().unwrap().to_string(),
//                         };
//                         let local_creds = ICECredentials {
//                             u_frag: pc.ice_ufrag.clone(),
//                             pwd: pc.ice_pwd.clone(),
//                         };
//                         Some(ICEAgent::new(
//                             receiver.clone(),
//                             self.peer_connection.clone(),
//                             self.transport.get_stun_channel().unwrap(),
//                             local_creds,
//                             remote_creds,
//                         ))
//                     } else {
//                         None
//                     };
//                     if let Some(mut agent) = agent {
//                         log::info!("gathering local cands");
//                         agent.local_candidates().await.unwrap();
//                         for c in &agent.local_candidates {
//                             self.send_local_candidate(c.clone()).await.unwrap();
//                         }
//                         //self.send_done().await.unwrap();
//                         log::info!("sent local candidates");
//                         self.executor
//                             .clone()
//                             .spawn(async move {
//                                 log::info!("spwaning agent");
//                                 agent.run().await;
//                             })
//                             .detach();
//                     };
//                 }
//                 let req = self
//                     .signaling_rx
//                     .next()
//                     .timeout(Duration::from_millis(100))
//                     .await;
//                 if let Some(req) = req {
//                     let resp = match req {
//                         Some(req) => self.process_signaling_request(req).await,
//                         None => Err(anyhow::anyhow!("no request is link down?")),
//                     };

//                     match resp {
//                         Ok(resp) => {
//                             if let Some(answer) = resp {
//                                 self.signaling_tx.send_message(answer).unwrap();
//                             }
//                         }
//                         Err(e) => {
//                             log::error!("error received was {:?}", e);
//                         }
//                     };
//                 };
//             }
//         }));
//         Ok(())
//     }
//     async fn process_signaling_request(
//         &mut self,
//         req: AnswerRequest,
//     ) -> anyhow::Result<Option<AnswerResponse>> {
//         if let Some(stage) = req.stage {
//             match stage {
//                 rpc::webrtc::v1::answer_request::Stage::Init(s) => {
//                     self.uuid = req.uuid;
//                     let sdp_decoded = self.engine.decode(s.sdp)?;
//                     let sdp_decoded: SdpOffer = serde_json::from_slice(sdp_decoded.as_slice())?;
//                     log::debug!(
//                         "Getting offer {:?} of type {:?}",
//                         &sdp_decoded.sdp,
//                         &sdp_decoded.r#type
//                     );
//                     let mut cursor = Cursor::new(sdp_decoded.sdp);
//                     log::debug!("we have an offer - there should be no PeerConnection");
//                     if self.peer_connection.lock().unwrap().is_some() {
//                         return Err(anyhow::anyhow!("a peer connection already exists"));
//                     }
//                     let sdp = sdp::SessionDescription::unmarshal(&mut cursor)?;
//                     let _ = self
//                         .peer_connection
//                         .lock()
//                         .unwrap()
//                         .insert(PeerConnection::new(sdp));

//                     let answer = if let Some(cp) = self.peer_connection.lock().unwrap().as_ref() {
//                         cp.answer()
//                     } else {
//                         return Err(anyhow::anyhow!(
//                             "No peerconnection found but we just created is"
//                         ));
//                     };

//                     log::debug!("We are answering {:?}", &answer);

//                     let answer = SdpOffer {
//                         r#type: "answer".to_owned(),
//                         sdp: answer.marshal(),
//                     };

//                     let answer = self.engine.encode(serde_json::to_string(&answer)?);

//                     let answer = AnswerResponse {
//                         uuid: self.uuid.clone(),
//                         stage: Some(rpc::webrtc::v1::answer_response::Stage::Init(
//                             AnswerResponseInitStage { sdp: answer },
//                         )),
//                     };
//                     return Ok(Some(answer));
//                 }
//                 rpc::webrtc::v1::answer_request::Stage::Update(u) => {
//                     if let Some(c) = u.candidate {
//                         //log::info!("getting {:?}", &c.candidate);
//                         let candidate: Candidate = c.candidate.try_into()?;
//                         if let Some(c) = &self.remote_candidate_chan_tx {
//                             //log::info!("sendind candidate");
//                             c.send(candidate).await?;
//                         }
//                     }
//                 }
//                 _ => return Err(anyhow::anyhow!("not yet implemented")),
//             }
//         }
//         Ok(None)
//     }
// }
