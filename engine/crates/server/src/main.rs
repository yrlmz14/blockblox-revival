use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use revival_core::{physics::PhysicsWorld, DataModel};
use revival_net::{ClientMessage, ServerInfo, ServerMessage};
use revival_raknet::{run_raknet_server, RakNetConfig, RakNetEvent, DEFAULT_RAKNET_PORT};
use tokio::{
    net::UdpSocket,
    sync::{broadcast, mpsc, Mutex, RwLock},
};
use tracing::{info, warn};

const PWA_HTML: &str = include_str!("../../../../launchers/web-pwa/index.html");

struct AppState {
    place_name: String,
    raknet_port: u16,
    data_model: RwLock<DataModel>,
    physics: Mutex<PhysicsWorld>,
    next_player_id: AtomicU64,
    player_count: AtomicU64,
    tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let (mut dm, place_name) = if let Ok(path) = std::env::var("PLACE_FILE") {
        let bytes = std::fs::read(&path)?;
        let name = std::path::Path::new(&path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Custom Place")
            .to_string();
        info!("loaded place file from {path}");
        (DataModel::from_place_bytes(&bytes)?, name)
    } else {
        info!("starting with default baseplate");
        (DataModel::new_baseplate(), "Default Baseplate".to_string())
    };

    for line in dm.run_luau_scripts_mut()? {
        info!("{line}");
    }

    let raknet_port: u16 = std::env::var("RAKNET_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_RAKNET_PORT);

    let initial_parts = dm.parts.clone();
    let physics = PhysicsWorld::from_data_model(&dm);
    let (tx, _) = broadcast::channel(128);
    let state = Arc::new(AppState {
        place_name: place_name.clone(),
        raknet_port,
        data_model: RwLock::new(dm),
        physics: Mutex::new(physics),
        next_player_id: AtomicU64::new(1),
        player_count: AtomicU64::new(0),
        tx,
    });

    // Shared snapshot of Parts for 2016 RakNet UDP replication
    let parts_snapshot = Arc::new(RwLock::new(initial_parts));

    // 30 Hz authoritative physics tick loop (lightweight for Pi Zero 2 W & x86_64)
    let tick_state = state.clone();
    let tick_parts = parts_snapshot.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(33));
        loop {
            interval.tick().await;
            let mut dm = tick_state.data_model.write().await;
            let mut phys = tick_state.physics.lock().await;
            phys.step(1.0 / 30.0, &mut dm);
            *tick_parts.write().await = dm.parts.clone();
        }
    });

    // Clean-room 2016 Legacy RCC (RakNet UDP) server on RAKNET_PORT (default 53640)
    if raknet_port != 0 {
        let raknet_addr = SocketAddr::from(([0, 0, 0, 0], raknet_port));
        match UdpSocket::bind(raknet_addr).await {
            Ok(udp_socket) => {
                let (ev_tx, mut ev_rx) = mpsc::unbounded_channel();
                let raknet_cfg = RakNetConfig::new(raknet_addr, place_name.clone());
                tokio::spawn(async move {
                    if let Err(e) =
                        run_raknet_server(Arc::new(udp_socket), raknet_cfg, parts_snapshot, ev_tx)
                            .await
                    {
                        warn!("2016 RakNet server exited: {e}");
                    }
                });

                // Bridge 2016 RakNet peer events into the shared WebSocket/server state
                let bridge_state = state.clone();
                tokio::spawn(async move {
                    while let Some(ev) = ev_rx.recv().await {
                        match ev {
                            RakNetEvent::PeerJoined {
                                user_id, username, ..
                            } => {
                                bridge_state.player_count.fetch_add(1, Ordering::Relaxed);
                                let msg = ServerMessage::PlayerJoined {
                                    player_id: user_id.max(0) as u64,
                                    username,
                                };
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    let _ = bridge_state.tx.send(json);
                                }
                            }
                            RakNetEvent::PeerChat {
                                username, message, ..
                            } => {
                                let msg = ServerMessage::Chat { username, message };
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    let _ = bridge_state.tx.send(json);
                                }
                            }
                            RakNetEvent::PeerPhysics {
                                user_id, position, ..
                            } => {
                                let msg = ServerMessage::PlayerMoved {
                                    player_id: user_id.max(0) as u64,
                                    position,
                                };
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    let _ = bridge_state.tx.send(json);
                                }
                            }
                            RakNetEvent::PeerLeft { .. } => {
                                bridge_state.player_count.fetch_sub(1, Ordering::Relaxed);
                            }
                        }
                    }
                });
            }
            Err(e) => {
                warn!("could not bind 2016 RakNet UDP port {raknet_port}: {e}");
            }
        }
    }

    let app = Router::new()
        .route("/", get(|| async { Html(PWA_HTML) }))
        .route("/health", get(|| async { "ok" }))
        .route("/api/servers", get(list_servers))
        .route("/Game/Join.ashx", get(legacy_2016_join_info))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("revival-server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn list_servers(State(state): State<Arc<AppState>>) -> Json<Vec<ServerInfo>> {
    Json(vec![ServerInfo {
        name: "Revival Server (WS + 2016 RakNet)".into(),
        players: state.player_count.load(Ordering::Relaxed) as usize,
        max_players: 16,
        place_name: state.place_name.clone(),
    }])
}

async fn legacy_2016_join_info(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ClientPort": 0,
        "MachineAddress": "127.0.0.1",
        "ServerPort": state.raknet_port,
        "PingUrl": "",
        "PingInterval": 20,
        "UserName": "Player",
        "UserId": 1,
        "PlaceId": 1818,
        "PlaceName": state.place_name,
        "ProtocolVersion": revival_raknet::DEFAULT_ROBLOX_2016_PROTOCOL_VERSION,
        "UseR15": false,
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let player_id = state.next_player_id.fetch_add(1, Ordering::Relaxed);
    state.player_count.fetch_add(1, Ordering::Relaxed);

    let parts = state.data_model.read().await.parts.clone();
    let welcome = ServerMessage::Welcome { player_id, parts };
    if let Ok(json) = serde_json::to_string(&welcome) {
        let _ = socket.send(Message::Text(json.into())).await;
    }

    let mut rx = state.tx.subscribe();
    let mut username = format!("Player{player_id}");

    loop {
        tokio::select! {
            Some(Ok(msg)) = socket.recv() => {
                if let Message::Text(text) = msg {
                    if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                        let out = match client_msg {
                            ClientMessage::Join { username: u } => {
                                username = u.clone();
                                Some(ServerMessage::PlayerJoined { player_id, username: u })
                            }
                            ClientMessage::Move { position } => {
                                Some(ServerMessage::PlayerMoved { player_id, position })
                            }
                            ClientMessage::Chat { message } => {
                                Some(ServerMessage::Chat { username: username.clone(), message })
                            }
                        };
                        if let Some(ev) = out {
                            if let Ok(json) = serde_json::to_string(&ev) {
                                let _ = state.tx.send(json);
                            }
                        }
                    }
                } else if matches!(msg, Message::Close(_)) {
                    break;
                }
            }
            Ok(broadcast_json) = rx.recv() => {
                if socket.send(Message::Text(broadcast_json.into())).await.is_err() {
                    break;
                }
            }
            else => break,
        }
    }

    state.player_count.fetch_sub(1, Ordering::Relaxed);
    if let Ok(json) = serde_json::to_string(&ServerMessage::PlayerLeft { player_id }) {
        let _ = state.tx.send(json);
    }
}
