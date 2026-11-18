use revival_core::Part;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub players: usize,
    pub max_players: usize,
    pub place_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Join { username: String },
    Move { position: [f32; 3] },
    Chat { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        player_id: u64,
        parts: Vec<Part>,
    },
    PlayerJoined {
        player_id: u64,
        username: String,
    },
    PlayerMoved {
        player_id: u64,
        position: [f32; 3],
    },
    PlayerLeft {
        player_id: u64,
    },
    PartsSync {
        parts: Vec<Part>,
    },
    Chat {
        username: String,
        message: String,
    },
}
