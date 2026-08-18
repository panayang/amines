use crate::auth::AuthService;
use crate::room::RoomManager;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use parking_lot::RwLock;
use shared::protocol::{ClientMessage, ServerMessage};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

pub struct AppState {
    pub auth: AuthService,
    pub rooms: RoomManager,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let session_id = Uuid::new_v4().to_string();

    // Channel for sending messages to this client WS
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

    // Spawn task to forward outgoing ServerMessages to WebSocket
    let outgoing_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if ws_sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    let current_room_id = Arc::new(RwLock::new(None::<String>));
    let mut broadcast_forward_task: Option<JoinHandle<()>> = None;

    while let Some(result) = ws_receiver.next().await {
        let msg = match result {
            Ok(Message::Text(text)) => text,
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_)) => {
                let _ = tx.send(ServerMessage::Pong);
                continue;
            }
            Err(_) => break,
            _ => continue,
        };

        let client_msg: ClientMessage = match serde_json::from_str(&msg) {
            Ok(m) => m,
            Err(e) => {
                let _ = tx.send(ServerMessage::Error {
                    message: format!("Invalid message format: {e}"),
                });
                continue;
            }
        };

        match client_msg {
            ClientMessage::Ping => {
                let _ = tx.send(ServerMessage::Pong);
            }
            ClientMessage::CreateRoom {
                name,
                config,
                username,
                token,
            } => {
                if let Some(h) = broadcast_forward_task.take() {
                    h.abort();
                }

                let user_id = token
                    .as_deref()
                    .and_then(|t| state.auth.get_user_id_by_token(t));
                let (room_id, room_arc) = state.rooms.create_room(name, session_id.clone(), config);

                let mut sub_rx = {
                    let mut room = room_arc.write();
                    room.add_player(&session_id, &username, user_id);
                    room.broadcast_tx.subscribe()
                };

                *current_room_id.write() = Some(room_id.clone());

                let tx_clone = tx.clone();
                broadcast_forward_task = Some(tokio::spawn(async move {
                    while let Ok(bc_msg) = sub_rx.recv().await {
                        if tx_clone.send(bc_msg).is_err() {
                            break;
                        }
                    }
                }));

                let room = room_arc.read();
                let _ = tx.send(ServerMessage::RoomState(room.snapshot()));
            }
            ClientMessage::JoinRoom {
                room_id,
                username,
                token,
            } => {
                let room_arc = match state.rooms.get_room(&room_id) {
                    Some(r) => r,
                    None => {
                        let _ = tx.send(ServerMessage::Error {
                            message: "Room not found".to_string(),
                        });
                        continue;
                    }
                };

                if let Some(h) = broadcast_forward_task.take() {
                    h.abort();
                }

                let user_id = token
                    .as_deref()
                    .and_then(|t| state.auth.get_user_id_by_token(t));

                let mut sub_rx = {
                    let mut room = room_arc.write();
                    room.add_player(&session_id, &username, user_id);
                    room.broadcast_tx.subscribe()
                };

                *current_room_id.write() = Some(room_id.clone());

                let tx_clone = tx.clone();
                broadcast_forward_task = Some(tokio::spawn(async move {
                    while let Ok(bc_msg) = sub_rx.recv().await {
                        if tx_clone.send(bc_msg).is_err() {
                            break;
                        }
                    }
                }));

                let room = room_arc.read();
                let _ = tx.send(ServerMessage::RoomState(room.snapshot()));
            }
            ClientMessage::SetReady { ready } => {
                if let Some(r_id) = current_room_id.read().as_ref() {
                    if let Some(room_arc) = state.rooms.get_room(r_id) {
                        room_arc.write().set_ready(&session_id, ready);
                    }
                }
            }
            ClientMessage::StartGame => {
                if let Some(r_id) = current_room_id.read().as_ref() {
                    if let Some(room_arc) = state.rooms.get_room(r_id) {
                        if let Err(e) = room_arc.write().start_game(&session_id) {
                            let _ = tx.send(ServerMessage::Error { message: e });
                        }
                    }
                }
            }
            ClientMessage::RevealCell { coord } => {
                if let Some(r_id) = current_room_id.read().as_ref() {
                    if let Some(room_arc) = state.rooms.get_room(r_id) {
                        room_arc.write().handle_reveal(&session_id, coord);
                    }
                }
            }
            ClientMessage::ChordCell { coord } => {
                if let Some(r_id) = current_room_id.read().as_ref() {
                    if let Some(room_arc) = state.rooms.get_room(r_id) {
                        room_arc.write().handle_chord(&session_id, coord);
                    }
                }
            }
            ClientMessage::ToggleFlag { coord } => {
                if let Some(r_id) = current_room_id.read().as_ref() {
                    if let Some(room_arc) = state.rooms.get_room(r_id) {
                        room_arc.write().handle_toggle_flag(&session_id, coord);
                    }
                }
            }
            ClientMessage::SendChat { text } => {
                if let Some(r_id) = current_room_id.read().as_ref() {
                    if let Some(room_arc) = state.rooms.get_room(r_id) {
                        room_arc.write().handle_chat(&session_id, &text);
                    }
                }
            }
            ClientMessage::AddBot { tier, speed_ms } => {
                if let Some(r_id) = current_room_id.read().as_ref() {
                    if let Some(room_arc) = state.rooms.get_room(r_id) {
                        let room_arc_weak = Arc::downgrade(&room_arc);
                        let mut room = room_arc.write();
                        if room.host_id == session_id {
                            room.add_bot(tier, speed_ms, room_arc_weak);
                        }
                    }
                }
            }
            ClientMessage::RemoveBot { bot_id } => {
                if let Some(r_id) = current_room_id.read().as_ref() {
                    if let Some(room_arc) = state.rooms.get_room(r_id) {
                        let mut room = room_arc.write();
                        if room.host_id == session_id {
                            room.remove_bot(&bot_id);
                        }
                    }
                }
            }
            ClientMessage::UpdateBotSpeed { speed_ms } => {
                if let Some(r_id) = current_room_id.read().as_ref() {
                    if let Some(room_arc) = state.rooms.get_room(r_id) {
                        let mut room = room_arc.write();
                        if room.host_id == session_id {
                            room.update_bot_speed(speed_ms);
                        }
                    }
                }
            }
            ClientMessage::LeaveRoom => {
                if let Some(h) = broadcast_forward_task.take() {
                    h.abort();
                }
                if let Some(r_id) = current_room_id.write().take() {
                    if let Some(room_arc) = state.rooms.get_room(&r_id) {
                        room_arc.write().remove_player(&session_id);
                        state.rooms.clean_empty_rooms();
                    }
                }
            }
        }
    }

    if let Some(h) = broadcast_forward_task.take() {
        h.abort();
    }

    // Cleanup on disconnect
    if let Some(r_id) = current_room_id.read().as_ref() {
        if let Some(room_arc) = state.rooms.get_room(r_id) {
            room_arc.write().remove_player(&session_id);
            state.rooms.clean_empty_rooms();
        }
    }

    outgoing_task.abort();
}
