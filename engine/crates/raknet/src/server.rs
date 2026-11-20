//! Async Tokio UDP server implementing the clean-room 2016 Roblox RakNet handshake & session lifecycle.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use revival_core::Part;
use tokio::{
    net::UdpSocket,
    sync::{mpsc, RwLock},
};
use tracing::{debug, info, warn};

use crate::{
    inspect_packet,
    packets::{
        encode_parts_replication_data, is_offline_message, ChatAllPacket, ConnectedPing,
        ConnectedPong, ConnectionRequest, ConnectionRequestAccepted, MarkerPacket,
        NewIncomingConnection, OpenConnectionReply1, OpenConnectionReply2, OpenConnectionRequest1,
        OpenConnectionRequest2, PacketId, PlaceIdVerification, ProtocolSync, SetGlobals,
        SubmitTicket, UnconnectedPing, UnconnectedPong, DEFAULT_ROBLOX_2016_PROTOCOL_VERSION,
    },
    reliability::{
        decode_ack, encode_ack, fragment_payload, AckRange, DataDatagram, EncapsulatedPacket,
        RakNetFlags, Reliability, SplitAssembler,
    },
    schema::{NetworkSchema, PhysicsPacket},
};

/// Configuration for the 2016 legacy RakNet UDP server.
#[derive(Debug, Clone)]
pub struct RakNetConfig {
    pub bind_addr: SocketAddr,
    pub place_id: u64,
    pub place_name: String,
    pub server_guid: u64,
    pub protocol_version: u32,
    pub trace_packets: bool,
}

impl RakNetConfig {
    pub fn new(bind_addr: SocketAddr, place_name: impl Into<String>) -> Self {
        Self {
            bind_addr,
            place_id: 1818, // Classic Crossroads default
            place_name: place_name.into(),
            server_guid: 0x2016_B10C_BB10_C001,
            protocol_version: DEFAULT_ROBLOX_2016_PROTOCOL_VERSION,
            trace_packets: std::env::var("RAKNET_TRACE").is_ok(),
        }
    }
}

/// High-level events emitted by the RakNet server to the host (`revival-server`).
#[derive(Debug, Clone)]
pub enum RakNetEvent {
    PeerJoined {
        addr: SocketAddr,
        user_id: i32,
        username: String,
    },
    PeerChat {
        addr: SocketAddr,
        username: String,
        message: String,
    },
    PeerPhysics {
        addr: SocketAddr,
        user_id: i32,
        position: [f32; 3],
    },
    PeerLeft {
        addr: SocketAddr,
        username: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerState {
    Handshaking,
    Connecting,
    Connected,
    Joined,
}

struct PeerSession {
    addr: SocketAddr,
    client_guid: u64,
    mtu: u16,
    state: PeerState,
    user_id: i32,
    username: String,
    next_datagram_seq: u32,
    next_reliable_index: u32,
    next_order_index: u32,
    next_split_id: u16,
    split_assembler: SplitAssembler,
    last_seen: Instant,
}

impl PeerSession {
    fn new(addr: SocketAddr, mtu: u16) -> Self {
        Self {
            addr,
            client_guid: 0,
            mtu,
            state: PeerState::Handshaking,
            user_id: 0,
            username: format!("Guest_{}", addr.port()),
            next_datagram_seq: 0,
            next_reliable_index: 0,
            next_order_index: 0,
            next_split_id: 0,
            split_assembler: SplitAssembler::default(),
            last_seen: Instant::now(),
        }
    }

    fn alloc_datagram_seq(&mut self) -> u32 {
        let s = self.next_datagram_seq;
        self.next_datagram_seq = (self.next_datagram_seq + 1) & 0x00FF_FFFF;
        s
    }

    fn alloc_reliable_index(&mut self) -> u32 {
        let i = self.next_reliable_index;
        self.next_reliable_index = (self.next_reliable_index + 1) & 0x00FF_FFFF;
        i
    }

    fn alloc_order_index(&mut self) -> u32 {
        let i = self.next_order_index;
        self.next_order_index = (self.next_order_index + 1) & 0x00FF_FFFF;
        i
    }

    fn alloc_split_id(&mut self) -> u16 {
        let id = self.next_split_id;
        self.next_split_id = self.next_split_id.wrapping_add(1);
        id
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Runs the clean-room 2016 RakNet UDP server loop on a bound `UdpSocket`.
pub async fn run_raknet_server(
    socket: Arc<UdpSocket>,
    config: RakNetConfig,
    parts_provider: Arc<RwLock<Vec<Part>>>,
    events_tx: mpsc::UnboundedSender<RakNetEvent>,
) -> Result<()> {
    let local_addr = socket.local_addr()?;
    info!(
        "2016 RakNet RCC server listening on udp://{local_addr} (place='{}', protocol={})",
        config.place_name, config.protocol_version
    );

    let mut peers: HashMap<SocketAddr, PeerSession> = HashMap::new();
    let mut recv_buf = vec![0u8; 4096];

    loop {
        let (len, src) = socket.recv_from(&mut recv_buf).await?;
        let data = &recv_buf[..len];
        if data.is_empty() {
            continue;
        }

        if config.trace_packets {
            info!("[RAKNET RX {src}] {}", inspect_packet(data));
        }

        // 1. Unconnected / Offline Handshake packets
        if is_offline_message(data) || data[0] == PacketId::UnconnectedPing as u8 || data[0] == PacketId::UnconnectedPingOpenConnections as u8 {
            handle_offline_packet(&socket, &config, &mut peers, src, data).await;
            continue;
        }

        // 2. Connected RakNet datagrams (flags bit 0x80 set)
        let flags = RakNetFlags(data[0]);
        if !flags.is_valid() {
            debug!("ignoring non-RakNet UDP packet from {src} (first byte 0x{:02X})", data[0]);
            continue;
        }

        if flags.is_ack() || flags.is_nak() {
            if let Ok((is_nak, ranges)) = decode_ack(data) {
                debug!(
                    "received {} from {src}: {} range(s)",
                    if is_nak { "NAK" } else { "ACK" },
                    ranges.len()
                );
            }
            continue;
        }

        // 3. Connected Data Datagram
        let datagram = match DataDatagram::decode(data) {
            Ok(dg) => dg,
            Err(e) => {
                warn!("failed to decode RakNet data datagram from {src}: {e}");
                continue;
            }
        };

        // Immediately ACK the received datagram sequence number.
        let ack = encode_ack(false, &[AckRange::single(datagram.sequence_number)]);
        let _ = socket.send_to(&ack, src).await;

        let peer = peers
            .entry(src)
            .or_insert_with(|| PeerSession::new(src, 1400));
        peer.last_seen = Instant::now();

        let mut disconnected = false;
        let mut broadcast_chat: Option<(String, String)> = None;

        for encap in datagram.packets {
            let payload = if let Some(split) = encap.split {
                match peer.split_assembler.push(split, encap.payload) {
                    Some(full) => full,
                    None => continue, // Waiting for remaining fragments
                }
            } else {
                encap.payload
            };

            if payload.is_empty() {
                continue;
            }

            let pid = payload[0];
            if config.trace_packets {
                info!(
                    "[RAKNET APP {src}] {} (0x{pid:02X}, {} bytes)",
                    PacketId::name(pid),
                    payload.len()
                );
            }

            match pid {
                0x00 => {
                    // ID_CONNECTED_PING -> ID_CONNECTED_PONG
                    if let Ok(ping) = ConnectedPing::decode(&payload) {
                        let pong = ConnectedPong {
                            ping_time: ping.ping_time,
                            pong_time: now_millis(),
                        };
                        send_reliable(&socket, peer, &pong.encode(), 0, config.trace_packets).await;
                    }
                }
                0x09 => {
                    // ID_CONNECTION_REQUEST -> ID_CONNECTION_REQUEST_ACCEPTED
                    if let Ok(req) = ConnectionRequest::decode(&payload) {
                        peer.client_guid = req.client_guid;
                        peer.state = PeerState::Connecting;
                        let accepted = ConnectionRequestAccepted {
                            client_address: src,
                            system_index: 0,
                            request_timestamp: req.request_timestamp,
                            accepted_timestamp: now_millis(),
                        };
                        send_reliable(&socket, peer, &accepted.encode(), 0, config.trace_packets)
                            .await;
                    }
                }
                0x13 => {
                    // ID_NEW_INCOMING_CONNECTION -> send 2016 ProtocolSync + SchemaSync + SetGlobals + PlaceIdVerification
                    if NewIncomingConnection::decode(&payload).is_ok() {
                        peer.state = PeerState::Connected;
                        info!("2016 RakNet peer connected: {src} (guid=0x{:016X})", peer.client_guid);

                        let sync = ProtocolSync {
                            protocol_version: config.protocol_version,
                            api_dictionary_compression: false,
                        };
                        send_reliable(&socket, peer, &sync.encode(), 0, config.trace_packets).await;

                        let schema = NetworkSchema::default_2016().encode(PacketId::SchemaSync);
                        send_reliable(&socket, peer, &schema, 0, config.trace_packets).await;

                        let globals = SetGlobals {
                            streaming_enabled: false,
                            place_id: config.place_id,
                        };
                        send_reliable(&socket, peer, &globals.encode(), 0, config.trace_packets)
                            .await;

                        let verify = PlaceIdVerification {
                            place_id: config.place_id,
                        };
                        send_reliable(&socket, peer, &verify.encode(), 0, config.trace_packets)
                            .await;
                    }
                }
                0x90 => {
                    // ID_PROTOCOL_SYNC from client -> confirm protocol version
                    if let Ok(client_sync) = ProtocolSync::decode(&payload) {
                        debug!(
                            "peer {src} requested protocol_version={}",
                            client_sync.protocol_version
                        );
                        let reply = ProtocolSync {
                            protocol_version: config.protocol_version,
                            api_dictionary_compression: false,
                        };
                        send_reliable(&socket, peer, &reply.encode(), 0, config.trace_packets)
                            .await;
                    }
                }
                0x8A => {
                    // ID_SUBMIT_TICKET -> accept player, replicate DataModel parts, send ID_MARKER
                    if let Ok(ticket) = SubmitTicket::decode(&payload) {
                        peer.user_id = ticket.user_id;
                        peer.username = ticket.username.clone();
                        peer.state = PeerState::Joined;
                        info!(
                            "2016 RakNet player joined: '{}' (user_id={}, addr={src})",
                            peer.username, peer.user_id
                        );
                        let _ = events_tx.send(RakNetEvent::PeerJoined {
                            addr: src,
                            user_id: peer.user_id,
                            username: peer.username.clone(),
                        });

                        // Replicate current workspace Parts in an ID_DATA stream (auto-fragmented if > MTU)
                        let parts = parts_provider.read().await.clone();
                        let data_pkt = encode_parts_replication_data(&parts);
                        send_reliable(&socket, peer, &data_pkt, 1, config.trace_packets).await;

                        // Signal initial join-data completion via ID_MARKER
                        let marker = MarkerPacket { marker_id: 1 };
                        send_reliable(&socket, peer, &marker.encode(), 1, config.trace_packets)
                            .await;
                    }
                }
                0x1B | 0x85 => {
                    // Optional ID_TIMESTAMP (9-byte prefix) + ID_PHYSICS (0x85)
                    let phys_slice = if pid == 0x1B && payload.len() > 9 {
                        &payload[9..]
                    } else {
                        &payload[..]
                    };
                    if let Ok(phys) = PhysicsPacket::decode(phys_slice) {
                        if let Some(first) = phys.parts.first() {
                            let _ = events_tx.send(RakNetEvent::PeerPhysics {
                                addr: src,
                                user_id: peer.user_id,
                                position: first.position,
                            });
                        }
                    }
                }
                0x87 => {
                    // ID_CHAT_ALL -> notify host & broadcast to other joined RakNet peers
                    if let Ok(chat) = ChatAllPacket::decode(&payload) {
                        let sender = if chat.sender.is_empty() {
                            peer.username.clone()
                        } else {
                            chat.sender
                        };
                        let _ = events_tx.send(RakNetEvent::PeerChat {
                            addr: src,
                            username: sender.clone(),
                            message: chat.message.clone(),
                        });
                        broadcast_chat = Some((sender, chat.message));
                    }
                }
                0x15 => {
                    // ID_DISCONNECTION_NOTIFICATION
                    disconnected = true;
                }
                _ => {
                    debug!(
                        "unhandled 2016 RakNet packet {} (0x{pid:02X}, {} bytes) from {src}",
                        PacketId::name(pid),
                        payload.len()
                    );
                }
            }
        }

        if let Some((sender, message)) = broadcast_chat {
            let encoded = ChatAllPacket { sender, message }.encode();
            for other in peers.values_mut() {
                if other.state == PeerState::Joined {
                    send_reliable(&socket, other, &encoded, 2, config.trace_packets).await;
                }
            }
        }

        if disconnected {
            if let Some(removed) = peers.remove(&src) {
                info!("2016 RakNet peer disconnected: '{}' ({src})", removed.username);
                let _ = events_tx.send(RakNetEvent::PeerLeft {
                    addr: src,
                    username: removed.username,
                });
            }
        }
    }
}

async fn handle_offline_packet(
    socket: &UdpSocket,
    config: &RakNetConfig,
    peers: &mut HashMap<SocketAddr, PeerSession>,
    src: SocketAddr,
    data: &[u8],
) {
    match data[0] {
        0x01 | 0x02 => {
            if let Ok(ping) = UnconnectedPing::decode(data) {
                let pong = UnconnectedPong {
                    send_ping_time: ping.send_ping_time,
                    server_guid: config.server_guid,
                    motd: format!("BlockBlox 2016 RCC;{};{}", config.place_name, peers.len()),
                };
                let _ = socket.send_to(&pong.encode(), src).await;
            }
        }
        0x05 => {
            if let Ok(req1) = OpenConnectionRequest1::decode(data) {
                let mtu = req1.mtu_size.clamp(576, 1492);
                peers.insert(src, PeerSession::new(src, mtu));
                let reply1 = OpenConnectionReply1 {
                    server_guid: config.server_guid,
                    use_security: false,
                    mtu_size: mtu,
                };
                let raw = reply1.encode();
                if config.trace_packets {
                    info!("[RAKNET TX {src}] {}", inspect_packet(&raw));
                }
                let _ = socket.send_to(&raw, src).await;
            }
        }
        0x07 => {
            if let Ok(req2) = OpenConnectionRequest2::decode(data) {
                let mtu = req2.mtu_size.clamp(576, 1492);
                let peer = peers
                    .entry(src)
                    .or_insert_with(|| PeerSession::new(src, mtu));
                peer.mtu = mtu;
                peer.client_guid = req2.client_guid;

                let reply2 = OpenConnectionReply2 {
                    server_guid: config.server_guid,
                    client_address: src,
                    mtu_size: mtu,
                    use_encryption: false,
                };
                let raw = reply2.encode();
                if config.trace_packets {
                    info!("[RAKNET TX {src}] {}", inspect_packet(&raw));
                }
                let _ = socket.send_to(&raw, src).await;
            }
        }
        _ => {}
    }
}

async fn send_reliable(
    socket: &UdpSocket,
    peer: &mut PeerSession,
    payload: &[u8],
    channel: u8,
    trace: bool,
) {
    let order_idx = peer.alloc_order_index();
    let split_id = peer.alloc_split_id();
    let mtu = peer.mtu;
    let packets = fragment_payload(
        payload,
        mtu,
        split_id,
        || peer.alloc_reliable_index(),
        order_idx,
        channel,
    );

    for pkt in packets {
        let dg = DataDatagram {
            flags: RakNetFlags::data(),
            sequence_number: peer.alloc_datagram_seq(),
            packets: vec![pkt],
        };
        let raw = dg.encode();
        if trace {
            info!("[RAKNET TX {}] {}", peer.addr, inspect_packet(&raw));
        }
        let _ = socket.send_to(&raw, peer.addr).await;
    }
}

/// Helper for building a single `ReliableOrdered` datagram (used by tests and client tools).
pub fn wrap_reliable_datagram(seq: u32, reliable_idx: u32, order_idx: u32, payload: Vec<u8>) -> Vec<u8> {
    DataDatagram {
        flags: RakNetFlags::data(),
        sequence_number: seq,
        packets: vec![EncapsulatedPacket {
            reliability: Reliability::ReliableOrdered,
            reliable_index: Some(reliable_idx),
            sequence_index: None,
            order_index: Some(order_idx),
            order_channel: Some(0),
            split: None,
            payload,
        }],
    }
    .encode()
}

#[cfg(test)]
mod tests {
    use super::*;
    use revival_core::DataModel;
    use std::time::Duration;

    async fn recv_next_app_payload(
        client: &UdpSocket,
        assembler: &mut SplitAssembler,
    ) -> Vec<u8> {
        let mut buf = vec![0u8; 4096];
        loop {
            let (n, _) = tokio::time::timeout(Duration::from_secs(2), client.recv_from(&mut buf))
                .await
                .expect("timed out waiting for server packet")
                .unwrap();
            let raw = &buf[..n];
            let flags = RakNetFlags(raw[0]);
            if flags.is_ack() || flags.is_nak() {
                continue;
            }
            let dg = DataDatagram::decode(raw).unwrap();
            for pkt in dg.packets {
                if let Some(split) = pkt.split {
                    if let Some(full) = assembler.push(split, pkt.payload) {
                        return full;
                    }
                } else {
                    return pkt.payload;
                }
            }
        }
    }

    #[tokio::test]
    async fn full_2016_raknet_handshake_and_replication_loopback() {
        let server_sock = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let server_addr = server_sock.local_addr().unwrap();

        let dm = DataModel::new_baseplate();
        let parts = Arc::new(RwLock::new(dm.parts));
        let (ev_tx, mut ev_rx) = mpsc::unbounded_channel();

        let config = RakNetConfig::new(server_addr, "Crossroads 2016");
        let server_handle = tokio::spawn(run_raknet_server(
            server_sock.clone(),
            config,
            parts,
            ev_tx,
        ));

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mut buf = vec![0u8; 4096];

        // 1. ID_OPEN_CONNECTION_REQUEST_1 -> ID_OPEN_CONNECTION_REPLY_1
        let req1 = OpenConnectionRequest1 {
            protocol_version: 5,
            mtu_size: 1200,
        };
        client.send_to(&req1.encode(), server_addr).await.unwrap();
        let (n, _) = client.recv_from(&mut buf).await.unwrap();
        let rep1 = OpenConnectionReply1::decode(&buf[..n]).unwrap();
        assert_eq!(rep1.mtu_size, 1200);
        assert!(!rep1.use_security);

        // 2. ID_OPEN_CONNECTION_REQUEST_2 -> ID_OPEN_CONNECTION_REPLY_2
        let req2 = OpenConnectionRequest2 {
            server_address: server_addr,
            mtu_size: 1200,
            client_guid: 0xDEAD_BEEF_CAFE_0001,
        };
        client.send_to(&req2.encode(), server_addr).await.unwrap();
        let (n, _) = client.recv_from(&mut buf).await.unwrap();
        let rep2 = OpenConnectionReply2::decode(&buf[..n]).unwrap();
        assert_eq!(rep2.mtu_size, 1200);
        assert!(!rep2.use_encryption);

        let mut assembler = SplitAssembler::default();

        // 3. ID_CONNECTION_REQUEST -> ID_CONNECTION_REQUEST_ACCEPTED
        let conn_req = ConnectionRequest {
            client_guid: 0xDEAD_BEEF_CAFE_0001,
            request_timestamp: 123456,
            use_security: false,
        };
        client
            .send_to(&wrap_reliable_datagram(0, 0, 0, conn_req.encode()), server_addr)
            .await
            .unwrap();
        let p = recv_next_app_payload(&client, &mut assembler).await;
        let accepted = ConnectionRequestAccepted::decode(&p).unwrap();
        assert_eq!(accepted.request_timestamp, 123456);

        // 4. ID_NEW_INCOMING_CONNECTION -> ProtocolSync + SetGlobals + PlaceIdVerification
        let nic = NewIncomingConnection {
            server_address: server_addr,
            ping_timestamp: 123456,
            pong_timestamp: accepted.accepted_timestamp,
        };
        client
            .send_to(&wrap_reliable_datagram(1, 1, 1, nic.encode()), server_addr)
            .await
            .unwrap();
        let p1 = recv_next_app_payload(&client, &mut assembler).await;
        assert_eq!(p1[0], PacketId::ProtocolSync as u8);
        let p_schema = recv_next_app_payload(&client, &mut assembler).await;
        let schema = NetworkSchema::decode(&p_schema).unwrap();
        assert_eq!(schema.schema_version, 2016);
        let p2 = recv_next_app_payload(&client, &mut assembler).await;
        assert_eq!(p2[0], PacketId::SetGlobals as u8);
        let p3 = recv_next_app_payload(&client, &mut assembler).await;
        assert_eq!(p3[0], PacketId::PlaceIdVerification as u8);

        // 5. ID_SUBMIT_TICKET -> ID_DATA (parts) + ID_MARKER + PeerJoined event
        let ticket = SubmitTicket {
            user_id: 42,
            protocol_version: DEFAULT_ROBLOX_2016_PROTOCOL_VERSION,
            username: "BuilderMan2016".into(),
            ticket: "local-dev-ticket".into(),
        };
        client
            .send_to(&wrap_reliable_datagram(2, 2, 2, ticket.encode()), server_addr)
            .await
            .unwrap();
        let data_pkt = recv_next_app_payload(&client, &mut assembler).await;
        assert_eq!(data_pkt[0], PacketId::Data as u8);
        let marker_pkt = recv_next_app_payload(&client, &mut assembler).await;
        assert_eq!(MarkerPacket::decode(&marker_pkt).unwrap().marker_id, 1);

        match ev_rx.recv().await.unwrap() {
            RakNetEvent::PeerJoined {
                user_id, username, ..
            } => {
                assert_eq!(user_id, 42);
                assert_eq!(username, "BuilderMan2016");
            }
            other => panic!("expected PeerJoined, got {other:?}"),
        }

        // 6. ID_CONNECTED_PING -> ID_CONNECTED_PONG
        let ping = ConnectedPing { ping_time: 99999 };
        client
            .send_to(&wrap_reliable_datagram(3, 3, 3, ping.encode()), server_addr)
            .await
            .unwrap();
        let pong_raw = recv_next_app_payload(&client, &mut assembler).await;
        let pong = ConnectedPong::decode(&pong_raw).unwrap();
        assert_eq!(pong.ping_time, 99999);

        // 7. ID_PHYSICS -> PeerPhysics event
        let phys = PhysicsPacket {
            parts: vec![crate::schema::PhysicsPartState {
                referent_id: 1,
                position: [12.0, 5.0, -3.5],
                velocity: [0.0, 0.0, 0.0],
                angular_velocity: [0.0, 0.0, 0.0],
                humanoid_state: 8,
            }],
        };
        client
            .send_to(&wrap_reliable_datagram(4, 4, 4, phys.encode()), server_addr)
            .await
            .unwrap();
        match ev_rx.recv().await.unwrap() {
            RakNetEvent::PeerPhysics {
                user_id, position, ..
            } => {
                assert_eq!(user_id, 42);
                assert_eq!(position, [12.0, 5.0, -3.5]);
            }
            other => panic!("expected PeerPhysics, got {other:?}"),
        }

        // 8. ID_CHAT_ALL -> echoed back + PeerChat event
        let chat = ChatAllPacket {
            sender: "BuilderMan2016".into(),
            message: "Hello 2016 RCC!".into(),
        };
        client
            .send_to(&wrap_reliable_datagram(5, 5, 5, chat.encode()), server_addr)
            .await
            .unwrap();
        let chat_echo = recv_next_app_payload(&client, &mut assembler).await;
        assert_eq!(ChatAllPacket::decode(&chat_echo).unwrap().message, "Hello 2016 RCC!");

        server_handle.abort();
    }
}
