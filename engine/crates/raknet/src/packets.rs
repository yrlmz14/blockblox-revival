//! RakNet control packets and 2016 Roblox legacy protocol packet definitions.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use anyhow::{bail, Result};
use revival_core::Part;

use crate::wire::{WireReader, WireWriter, OFFLINE_MESSAGE_ID};

/// Default Roblox 2016 game server UDP port.
pub const DEFAULT_RAKNET_PORT: u16 = 53640;

/// Default RakNet protocol version used by 2012–2016 Roblox builds (RakNet 4 / v5).
pub const DEFAULT_RAKNET_PROTOCOL_VERSION: u8 = 5;

/// Default 2016 Roblox network protocol version sent in `ID_PROTOCOL_SYNC`.
pub const DEFAULT_ROBLOX_2016_PROTOCOL_VERSION: u32 = 34;

/// Top-level RakNet & 2016 Roblox packet IDs (from open-source RakNet/Roblox dissector specs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketId {
    ConnectedPing = 0x00,
    UnconnectedPing = 0x01,
    UnconnectedPingOpenConnections = 0x02,
    ConnectedPong = 0x03,
    DetectLostConnections = 0x04,
    OpenConnectionRequest1 = 0x05,
    OpenConnectionReply1 = 0x06,
    OpenConnectionRequest2 = 0x07,
    OpenConnectionReply2 = 0x08,
    ConnectionRequest = 0x09,
    ConnectionRequestAccepted = 0x10,
    ConnectionAttemptFailed = 0x11,
    NewIncomingConnection = 0x13,
    DisconnectionNotification = 0x15,
    Timestamp = 0x1B,
    UnconnectedPong = 0x1C,
    // 2016 Roblox application packets (0x81..0x97)
    SetGlobals = 0x81,
    TeachDescriptorDictionaries = 0x82,
    Data = 0x83,
    Marker = 0x84,
    Physics = 0x85,
    Touches = 0x86,
    ChatAll = 0x87,
    ChatTeam = 0x88,
    ReportAbuse = 0x89,
    SubmitTicket = 0x8A,
    ChatGame = 0x8B,
    ChatPlayer = 0x8C,
    Cluster = 0x8D,
    ProtocolMismatch = 0x8E,
    PreferredSpawnName = 0x8F,
    ProtocolSync = 0x90,
    SchemaSync = 0x91,
    PlaceIdVerification = 0x92,
    DictionaryFormat = 0x93,
    HashMismatch = 0x94,
    SecurityKeyMismatch = 0x95,
    RequestStats = 0x96,
    NewSchema = 0x97,
}

impl PacketId {
    pub fn name(id: u8) -> &'static str {
        match id {
            0x00 => "ID_CONNECTED_PING",
            0x01 => "ID_UNCONNECTED_PING",
            0x02 => "ID_UNCONNECTED_PING_OPEN_CONNECTIONS",
            0x03 => "ID_CONNECTED_PONG",
            0x04 => "ID_DETECT_LOST_CONNECTIONS",
            0x05 => "ID_OPEN_CONNECTION_REQUEST_1",
            0x06 => "ID_OPEN_CONNECTION_REPLY_1",
            0x07 => "ID_OPEN_CONNECTION_REQUEST_2",
            0x08 => "ID_OPEN_CONNECTION_REPLY_2",
            0x09 => "ID_CONNECTION_REQUEST",
            0x10 => "ID_CONNECTION_REQUEST_ACCEPTED",
            0x11 => "ID_CONNECTION_ATTEMPT_FAILED",
            0x13 => "ID_NEW_INCOMING_CONNECTION",
            0x15 => "ID_DISCONNECTION_NOTIFICATION",
            0x1B => "ID_TIMESTAMP",
            0x1C => "ID_UNCONNECTED_PONG",
            0x81 => "ID_SET_GLOBALS",
            0x82 => "ID_TEACH_DESCRIPTOR_DICTIONARIES",
            0x83 => "ID_DATA",
            0x84 => "ID_MARKER",
            0x85 => "ID_PHYSICS",
            0x86 => "ID_TOUCHES",
            0x87 => "ID_CHAT_ALL",
            0x88 => "ID_CHAT_TEAM",
            0x89 => "ID_REPORT_ABUSE",
            0x8A => "ID_SUBMIT_TICKET",
            0x8B => "ID_CHAT_GAME",
            0x8C => "ID_CHAT_PLAYER",
            0x8D => "ID_CLUSTER",
            0x8E => "ID_PROTOCOL_MISMATCH",
            0x8F => "ID_PREFERRED_SPAWN_NAME",
            0x90 => "ID_PROTOCOL_SYNC",
            0x91 => "ID_SCHEMA_SYNC",
            0x92 => "ID_PLACEID_VERIFICATION",
            0x93 => "ID_DICTIONARY_FORMAT",
            0x94 => "ID_HASH_MISMATCH",
            0x95 => "ID_SECURITYKEY_MISMATCH",
            0x96 => "ID_REQUEST_STATS",
            0x97 => "ID_NEW_SCHEMA",
            _ => "ID_UNKNOWN",
        }
    }
}

/// Checks if a raw UDP datagram starts with a RakNet offline message (magic at bytes 1..17).
pub fn is_offline_message(data: &[u8]) -> bool {
    data.len() >= 1 + OFFLINE_MESSAGE_ID.len()
        && data[1..1 + OFFLINE_MESSAGE_ID.len()] == OFFLINE_MESSAGE_ID
}

// ============================================================================
// Offline Handshake Packets (Unconnected)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenConnectionRequest1 {
    pub protocol_version: u8,
    pub mtu_size: u16,
}

impl OpenConnectionRequest1 {
    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::OpenConnectionRequest1 as u8 {
            bail!("expected ID_OPEN_CONNECTION_REQUEST_1 (0x05), got 0x{id:02X}");
        }
        r.read_magic()?;
        let protocol_version = r.read_u8()?;
        // In RakNet, MTU is inferred from total UDP payload size + IP/UDP header (28 bytes).
        let mtu_size = (buf.len() + 28).clamp(576, 1492) as u16;
        Ok(Self {
            protocol_version,
            mtu_size,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let payload_len = (self.mtu_size as usize).saturating_sub(28).max(18);
        let mut w = WireWriter::with_capacity(payload_len);
        w.write_u8(PacketId::OpenConnectionRequest1 as u8);
        w.write_magic();
        w.write_u8(self.protocol_version);
        let pad = payload_len.saturating_sub(18);
        if pad > 0 {
            w.write_bytes(&vec![0u8; pad]);
        }
        w.into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenConnectionReply1 {
    pub server_guid: u64,
    pub use_security: bool,
    pub mtu_size: u16,
}

impl OpenConnectionReply1 {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(28);
        w.write_u8(PacketId::OpenConnectionReply1 as u8);
        w.write_magic();
        w.write_u64_be(self.server_guid);
        w.write_bool(self.use_security);
        w.write_u16_be(self.mtu_size);
        w.into_bytes()
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::OpenConnectionReply1 as u8 {
            bail!("expected ID_OPEN_CONNECTION_REPLY_1 (0x06), got 0x{id:02X}");
        }
        r.read_magic()?;
        let server_guid = r.read_u64_be()?;
        let use_security = r.read_bool()?;
        let mtu_size = r.read_u16_be()?;
        Ok(Self {
            server_guid,
            use_security,
            mtu_size,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenConnectionRequest2 {
    pub server_address: SocketAddr,
    pub mtu_size: u16,
    pub client_guid: u64,
}

impl OpenConnectionRequest2 {
    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::OpenConnectionRequest2 as u8 {
            bail!("expected ID_OPEN_CONNECTION_REQUEST_2 (0x07), got 0x{id:02X}");
        }
        r.read_magic()?;
        let server_address = r.read_system_address()?;
        let mtu_size = r.read_u16_be()?;
        let client_guid = r.read_u64_be()?;
        Ok(Self {
            server_address,
            mtu_size,
            client_guid,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(34);
        w.write_u8(PacketId::OpenConnectionRequest2 as u8);
        w.write_magic();
        w.write_system_address(self.server_address);
        w.write_u16_be(self.mtu_size);
        w.write_u64_be(self.client_guid);
        w.into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenConnectionReply2 {
    pub server_guid: u64,
    pub client_address: SocketAddr,
    pub mtu_size: u16,
    pub use_encryption: bool,
}

impl OpenConnectionReply2 {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(35);
        w.write_u8(PacketId::OpenConnectionReply2 as u8);
        w.write_magic();
        w.write_u64_be(self.server_guid);
        w.write_system_address(self.client_address);
        w.write_u16_be(self.mtu_size);
        w.write_bool(self.use_encryption);
        w.into_bytes()
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::OpenConnectionReply2 as u8 {
            bail!("expected ID_OPEN_CONNECTION_REPLY_2 (0x08), got 0x{id:02X}");
        }
        r.read_magic()?;
        let server_guid = r.read_u64_be()?;
        let client_address = r.read_system_address()?;
        let mtu_size = r.read_u16_be()?;
        let use_encryption = r.read_bool()?;
        Ok(Self {
            server_guid,
            client_address,
            mtu_size,
            use_encryption,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnconnectedPing {
    pub send_ping_time: u64,
    pub client_guid: u64,
}

impl UnconnectedPing {
    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::UnconnectedPing as u8
            && id != PacketId::UnconnectedPingOpenConnections as u8
        {
            bail!("expected ID_UNCONNECTED_PING, got 0x{id:02X}");
        }
        let send_ping_time = r.read_u64_be()?;
        r.read_magic()?;
        let client_guid = if r.remaining() >= 8 {
            r.read_u64_be()?
        } else {
            0
        };
        Ok(Self {
            send_ping_time,
            client_guid,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnconnectedPong {
    pub send_ping_time: u64,
    pub server_guid: u64,
    pub motd: String,
}

impl UnconnectedPong {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(35 + self.motd.len());
        w.write_u8(PacketId::UnconnectedPong as u8);
        w.write_u64_be(self.send_ping_time);
        w.write_u64_be(self.server_guid);
        w.write_magic();
        w.write_string_u16(&self.motd);
        w.into_bytes()
    }
}

// ============================================================================
// Connected RakNet Control Packets
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionRequest {
    pub client_guid: u64,
    pub request_timestamp: u64,
    pub use_security: bool,
}

impl ConnectionRequest {
    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::ConnectionRequest as u8 {
            bail!("expected ID_CONNECTION_REQUEST (0x09), got 0x{id:02X}");
        }
        let client_guid = r.read_u64_be()?;
        let request_timestamp = r.read_u64_be()?;
        let use_security = if r.remaining() >= 1 {
            r.read_bool()?
        } else {
            false
        };
        Ok(Self {
            client_guid,
            request_timestamp,
            use_security,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(18);
        w.write_u8(PacketId::ConnectionRequest as u8);
        w.write_u64_be(self.client_guid);
        w.write_u64_be(self.request_timestamp);
        w.write_bool(self.use_security);
        w.into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionRequestAccepted {
    pub client_address: SocketAddr,
    pub system_index: u16,
    pub request_timestamp: u64,
    pub accepted_timestamp: u64,
}

impl ConnectionRequestAccepted {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(96);
        w.write_u8(PacketId::ConnectionRequestAccepted as u8);
        w.write_system_address(self.client_address);
        w.write_u16_be(self.system_index);
        // RakNet sends 10 internal system addresses (standard 10 slots).
        let dummy = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
        for _ in 0..10 {
            w.write_system_address(dummy);
        }
        w.write_u64_be(self.request_timestamp);
        w.write_u64_be(self.accepted_timestamp);
        w.into_bytes()
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::ConnectionRequestAccepted as u8 {
            bail!("expected ID_CONNECTION_REQUEST_ACCEPTED (0x10), got 0x{id:02X}");
        }
        let client_address = r.read_system_address()?;
        let system_index = r.read_u16_be()?;
        for _ in 0..10 {
            let _ = r.read_system_address()?;
        }
        let request_timestamp = r.read_u64_be()?;
        let accepted_timestamp = r.read_u64_be()?;
        Ok(Self {
            client_address,
            system_index,
            request_timestamp,
            accepted_timestamp,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewIncomingConnection {
    pub server_address: SocketAddr,
    pub ping_timestamp: u64,
    pub pong_timestamp: u64,
}

impl NewIncomingConnection {
    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::NewIncomingConnection as u8 {
            bail!("expected ID_NEW_INCOMING_CONNECTION (0x13), got 0x{id:02X}");
        }
        let server_address = r.read_system_address()?;
        // Skip internal addresses (either 10 or 20 depending on RakNet build; read last 16 bytes for timestamps).
        let rest = r.rest();
        if rest.len() < 16 {
            bail!("ID_NEW_INCOMING_CONNECTION too short for timestamps");
        }
        let ts = &rest[rest.len() - 16..];
        let ping_timestamp = u64::from_be_bytes(ts[0..8].try_into().unwrap());
        let pong_timestamp = u64::from_be_bytes(ts[8..16].try_into().unwrap());
        Ok(Self {
            server_address,
            ping_timestamp,
            pong_timestamp,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(96);
        w.write_u8(PacketId::NewIncomingConnection as u8);
        w.write_system_address(self.server_address);
        let dummy = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0));
        for _ in 0..10 {
            w.write_system_address(dummy);
        }
        w.write_u64_be(self.ping_timestamp);
        w.write_u64_be(self.pong_timestamp);
        w.into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedPing {
    pub ping_time: u64,
}

impl ConnectedPing {
    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::ConnectedPing as u8 {
            bail!("expected ID_CONNECTED_PING (0x00), got 0x{id:02X}");
        }
        Ok(Self {
            ping_time: r.read_u64_be()?,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(9);
        w.write_u8(PacketId::ConnectedPing as u8);
        w.write_u64_be(self.ping_time);
        w.into_bytes()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectedPong {
    pub ping_time: u64,
    pub pong_time: u64,
}

impl ConnectedPong {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(17);
        w.write_u8(PacketId::ConnectedPong as u8);
        w.write_u64_be(self.ping_time);
        w.write_u64_be(self.pong_time);
        w.into_bytes()
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::ConnectedPong as u8 {
            bail!("expected ID_CONNECTED_PONG (0x03), got 0x{id:02X}");
        }
        Ok(Self {
            ping_time: r.read_u64_be()?,
            pong_time: r.read_u64_be()?,
        })
    }
}

// ============================================================================
// 2016 Roblox Application Layer Packets (0x81..0x97)
// ============================================================================

/// `ID_PROTOCOL_SYNC` (0x90): Negotiates the Roblox network schema/protocol version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolSync {
    pub protocol_version: u32,
    pub api_dictionary_compression: bool,
}

impl ProtocolSync {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(16);
        w.write_u8(PacketId::ProtocolSync as u8);
        w.write_u32_be(self.protocol_version);
        w.write_bool(self.api_dictionary_compression);
        w.into_bytes()
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::ProtocolSync as u8 {
            bail!("expected ID_PROTOCOL_SYNC (0x90), got 0x{id:02X}");
        }
        let protocol_version = if r.remaining() >= 4 {
            r.read_u32_be()?
        } else {
            DEFAULT_ROBLOX_2016_PROTOCOL_VERSION
        };
        let api_dictionary_compression = if r.remaining() >= 1 {
            r.read_bool()?
        } else {
            false
        };
        Ok(Self {
            protocol_version,
            api_dictionary_compression,
        })
    }
}

/// `ID_SET_GLOBALS` (0x81): Initial server global flags (e.g. StreamingEnabled, PlaceId).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetGlobals {
    pub streaming_enabled: bool,
    pub place_id: u64,
}

impl SetGlobals {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(16);
        w.write_u8(PacketId::SetGlobals as u8);
        w.write_bool(self.streaming_enabled);
        w.write_u64_be(self.place_id);
        w.into_bytes()
    }
}

/// `ID_PLACEID_VERIFICATION` (0x92): Confirms the PlaceId being joined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceIdVerification {
    pub place_id: u64,
}

impl PlaceIdVerification {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(9);
        w.write_u8(PacketId::PlaceIdVerification as u8);
        w.write_u64_be(self.place_id);
        w.into_bytes()
    }
}

/// `ID_SUBMIT_TICKET` (0x8A): Client submits join ticket (UserId, Username, ProtocolVersion).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitTicket {
    pub user_id: i32,
    pub protocol_version: u32,
    pub username: String,
    pub ticket: String,
}

impl SubmitTicket {
    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::SubmitTicket as u8 {
            bail!("expected ID_SUBMIT_TICKET (0x8A), got 0x{id:02X}");
        }
        let user_id = if r.remaining() >= 4 {
            r.read_i32_be()?
        } else {
            1
        };
        let protocol_version = if r.remaining() >= 4 {
            r.read_u32_be()?
        } else {
            DEFAULT_ROBLOX_2016_PROTOCOL_VERSION
        };
        let username = if r.remaining() >= 4 {
            r.read_string_u32().unwrap_or_else(|_| format!("Player{user_id}"))
        } else {
            format!("Player{user_id}")
        };
        let ticket = if r.remaining() >= 4 {
            r.read_string_u32().unwrap_or_default()
        } else {
            String::new()
        };
        Ok(Self {
            user_id,
            protocol_version,
            username,
            ticket,
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(32 + self.username.len() + self.ticket.len());
        w.write_u8(PacketId::SubmitTicket as u8);
        w.write_i32_be(self.user_id);
        w.write_u32_be(self.protocol_version);
        w.write_string_u32(&self.username);
        w.write_string_u32(&self.ticket);
        w.into_bytes()
    }
}

/// `ID_MARKER` (0x84): Replication barrier/checkpoint marker (signals join-data completion).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerPacket {
    pub marker_id: u32,
}

impl MarkerPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(5);
        w.write_u8(PacketId::Marker as u8);
        w.write_u32_be(self.marker_id);
        w.into_bytes()
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::Marker as u8 {
            bail!("expected ID_MARKER (0x84), got 0x{id:02X}");
        }
        Ok(Self {
            marker_id: r.read_u32_be()?,
        })
    }
}

/// `ID_CHAT_ALL` (0x87): Broadcast chat message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatAllPacket {
    pub sender: String,
    pub message: String,
}

impl ChatAllPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = WireWriter::with_capacity(9 + self.sender.len() + self.message.len());
        w.write_u8(PacketId::ChatAll as u8);
        w.write_string_u32(&self.sender);
        w.write_string_u32(&self.message);
        w.into_bytes()
    }

    pub fn decode(buf: &[u8]) -> Result<Self> {
        let mut r = WireReader::new(buf);
        let id = r.read_u8()?;
        if id != PacketId::ChatAll as u8 {
            bail!("expected ID_CHAT_ALL (0x87), got 0x{id:02X}");
        }
        let sender = r.read_string_u32()?;
        let message = r.read_string_u32()?;
        Ok(Self { sender, message })
    }
}

/// Subpacket types inside `ID_DATA` (0x83) replication stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DataSubpacketType {
    End = 0x00,
    DeleteInstance = 0x01,
    NewInstance = 0x02,
    ChangeProperty = 0x03,
    Marker = 0x04,
    Ping = 0x05,
    PingBack = 0x06,
    Event = 0x07,
}

/// Clean-room `ID_DATA` (0x83) snapshot encoder for replicating `DataModel` `Part`s.
pub fn encode_parts_replication_data(parts: &[Part]) -> Vec<u8> {
    let mut w = WireWriter::with_capacity(64 + parts.len() * 64);
    w.write_u8(PacketId::Data as u8);
    for part in parts {
        w.write_u8(DataSubpacketType::NewInstance as u8);
        w.write_u32_be(part.id.max(1) as u32); // Referent ID
        w.write_string_u16("Part");
        w.write_string_u16(&part.name);
        // Position (3x f32)
        w.write_f32_be(part.position.x);
        w.write_f32_be(part.position.y);
        w.write_f32_be(part.position.z);
        // Size (3x f32)
        w.write_f32_be(part.size.x);
        w.write_f32_be(part.size.y);
        w.write_f32_be(part.size.z);
        // Color RGB (3x f32)
        w.write_f32_be(part.color[0]);
        w.write_f32_be(part.color[1]);
        w.write_f32_be(part.color[2]);
        w.write_f32_be(part.transparency);
        w.write_bool(part.anchored);
    }
    w.write_u8(DataSubpacketType::End as u8);
    w.into_bytes()
}
