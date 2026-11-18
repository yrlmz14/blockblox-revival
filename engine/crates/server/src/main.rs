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
use tokio::sync::{broadcast, Mutex, RwLock};
use tracing::info;

const PWA_HTML: &str = include_str!("../../../../launchers/web-pwa/index.html");

struct AppState {
    place_name: String,
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

    let physics = PhysicsWorld::from_data_model(&dm);
    let (tx, _) = broadcast::channel(128);
    let state = Arc::new(AppState {
        place_name,
        data_model: RwLock::new(dm),
        physics: Mutex::new(physics),
        next_player_id: AtomicU64::new(1),
        player_count: AtomicU64::new(0),
        tx,
    });

    // 30 Hz authoritative physics tick loop (lightweight for Pi Zero 2 W & x86_64)
    let tick_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(33));
        loop {
            interval.tick().await;
            let mut dm = tick_state.data_model.write().await;
            let mut phys = tick_state.physics.lock().await;
            phys.step(1.0 / 30.0, &mut dm);
        }
    });

    let app = Router::new()
        .route("/", get(|| async { Html(PWA_HTML) }))
        .route("/health", get(|| async { "ok" }))
        .route("/api/servers", get(list_servers))
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
        name: "Revival Server".into(),
        players: state.player_count.load(Ordering::Relaxed) as usize,
        max_players: 16,
        place_name: state.place_name.clone(),
    }])
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
