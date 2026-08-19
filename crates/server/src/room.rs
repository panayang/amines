use crate::db::Database;
use parking_lot::RwLock;
use rand::Rng;
use shared::ai_solver::BotTier;
use shared::board::{Board, BoardConfig, GameStatus, RevealResult};
use shared::i18n::{
    format_eliminated_msg, format_game_over_msg, format_player_joined_msg, Language,
};
use shared::protocol::*;
use shared::topology::Coord3D;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

const PLAYER_PALETTE: &[&str] = &[
    "#8b5cf6", // Violet
    "#38bdf8", // Sky Blue
    "#ec4899", // Pink
    "#10b981", // Emerald
    "#f59e0b", // Amber
    "#a855f7", // Purple
    "#06b6d4", // Cyan
    "#ef4444", // Red
];

pub struct RoomPlayer {
    pub id: String,
    pub username: String,
    pub user_id: Option<String>,
    pub color: String,
    pub score: u32,
    pub is_eliminated: bool,
    pub is_host: bool,
    pub is_ready: bool,
    pub is_spectator: bool,
    pub is_bot: bool,
    pub bot_tier: Option<BotTier>,
}

impl RoomPlayer {
    pub fn to_info(&self) -> PlayerInfo {
        PlayerInfo {
            id: self.id.clone(),
            username: self.username.clone(),
            color: self.color.clone(),
            score: self.score,
            is_eliminated: self.is_eliminated,
            is_host: self.is_host,
            is_ready: self.is_ready,
            is_spectator: self.is_spectator,
            is_bot: self.is_bot,
            bot_tier: self.bot_tier,
        }
    }
}

pub struct Room {
    pub id: String,
    pub name: String,
    pub host_id: String,
    pub config: BoardConfig,
    pub board: Board,
    pub status: GameStatus,
    pub players: HashMap<String, RoomPlayer>,
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub broadcast_tx: broadcast::Sender<ServerMessage>,
    pub bot_speed_ms: u64,
    pub bot_cancels: HashMap<String, tokio_util::sync::CancellationToken>,
    pub db: Database,
}

impl Room {
    pub fn new(
        id: String,
        name: String,
        host_id: String,
        config: BoardConfig,
        db: Database,
    ) -> Self {
        let (broadcast_tx, _) = broadcast::channel(256);
        let board = Board::new(config);

        Self {
            id,
            name,
            host_id,
            config,
            board,
            status: GameStatus::Waiting,
            players: HashMap::new(),
            start_time: None,
            broadcast_tx,
            bot_speed_ms: 800,
            bot_cancels: HashMap::new(),
            db,
        }
    }

    pub fn snapshot(&self) -> RoomSnapshot {
        let elapsed = match self.start_time {
            Some(st) => (chrono::Utc::now() - st).num_seconds().max(0) as u64,
            None => 0,
        };

        let cells = self.board.cells.iter().map(CellSnapshot::from).collect();

        RoomSnapshot {
            room_id: self.id.clone(),
            name: self.name.clone(),
            host_id: self.host_id.clone(),
            config: self.config,
            status: self.status,
            players: self.players.values().map(|p| p.to_info()).collect(),
            revealed_count: self.board.revealed_count,
            total_non_mines: self.board.dims.total_cells() - self.config.mines,
            elapsed_seconds: elapsed,
            cells,
            bot_speed_ms: self.bot_speed_ms,
        }
    }

    pub fn add_player(
        &mut self,
        session_id: &str,
        username: &str,
        user_id: Option<String>,
    ) -> PlayerInfo {
        let is_host = self.players.is_empty();
        let color_index = self.players.len() % PLAYER_PALETTE.len();
        let color = PLAYER_PALETTE[color_index].to_string();

        let is_spectator = self.status == GameStatus::Playing;

        let player = RoomPlayer {
            id: session_id.to_string(),
            username: username.to_string(),
            user_id,
            color,
            score: 0,
            is_eliminated: is_spectator,
            is_host,
            is_ready: is_host,
            is_spectator,
            is_bot: false,
            bot_tier: None,
        };

        let info = player.to_info();
        self.players.insert(session_id.to_string(), player);

        if is_host {
            self.host_id = session_id.to_string();
        }

        // Broadcast join system chat with event_key
        let join_text = format_player_joined_msg(Language::En, username);
        let _ = self
            .broadcast_tx
            .send(ServerMessage::ChatMessage(ChatMessagePayload {
                id: Uuid::new_v4().to_string(),
                player_id: None,
                username: "SYSTEM".to_string(),
                color: Some("#a855f7".to_string()),
                text: join_text,
                is_system: true,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_key: Some("player_joined".to_string()),
                event_params: vec![username.to_string()],
            }));

        self.broadcast_state();
        info
    }

    pub fn add_bot(
        &mut self,
        tier: BotTier,
        speed_ms: Option<u64>,
        room_arc: std::sync::Weak<RwLock<Room>>,
    ) -> String {
        let bot_id = format!("bot_{}", &Uuid::new_v4().to_string()[..8]);
        let bot_name = match tier {
            BotTier::Novice => format!("[AI] Pascal ({})", tier.name_en()),
            BotTier::Intermediate => format!("[AI] Boole ({})", tier.name_en()),
            BotTier::Advanced => format!("[AI] Lovelace ({})", tier.name_en()),
            BotTier::Master => format!("[AI] Turing ({})", tier.name_en()),
        };

        if let Some(spd) = speed_ms {
            self.bot_speed_ms = spd;
        }

        let color_index = self.players.len() % PLAYER_PALETTE.len();
        let color = PLAYER_PALETTE[color_index].to_string();

        let player = RoomPlayer {
            id: bot_id.clone(),
            username: bot_name.clone(),
            user_id: None,
            color,
            score: 0,
            is_eliminated: false,
            is_host: false,
            is_ready: true,
            is_spectator: false,
            is_bot: true,
            bot_tier: Some(tier),
        };

        self.players.insert(bot_id.clone(), player);

        let cancel_token = tokio_util::sync::CancellationToken::new();
        let rx = self.broadcast_tx.subscribe();
        crate::bot::spawn_bot_worker(bot_id.clone(), tier, rx, room_arc, cancel_token.clone());
        self.bot_cancels.insert(bot_id.clone(), cancel_token);

        let _ = self
            .broadcast_tx
            .send(ServerMessage::ChatMessage(ChatMessagePayload {
                id: Uuid::new_v4().to_string(),
                player_id: None,
                username: "SYSTEM".to_string(),
                color: Some("#a855f7".to_string()),
                text: format!("AI [{bot_name}] joined the room."),
                is_system: true,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_key: Some("player_joined".to_string()),
                event_params: vec![bot_name],
            }));

        self.broadcast_state();
        bot_id
    }

    pub fn remove_bot(&mut self, bot_id: &str) {
        if let Some(cancel) = self.bot_cancels.remove(bot_id) {
            cancel.cancel();
        }
        self.remove_player(bot_id);
    }

    pub fn update_bot_speed(&mut self, speed_ms: u64) {
        self.bot_speed_ms = speed_ms;
        self.broadcast_state();
    }

    pub fn remove_player(&mut self, session_id: &str) {
        if let Some(_player) = self.players.remove(session_id) {
            if let Some(cancel) = self.bot_cancels.remove(session_id) {
                cancel.cancel();
            }

            // If host left, migrate host to next human player if available
            if self.host_id == session_id && !self.players.is_empty() {
                if let Some(new_host) = self.players.values_mut().find(|p| !p.is_bot) {
                    new_host.is_host = true;
                    self.host_id = new_host.id.clone();
                } else if let Some(first) = self.players.values_mut().next() {
                    first.is_host = true;
                    self.host_id = first.id.clone();
                }
            }

            self.broadcast_state();
        }
    }

    pub fn set_ready(&mut self, session_id: &str, ready: bool) {
        if let Some(player) = self.players.get_mut(session_id) {
            player.is_ready = ready;
            self.broadcast_state();
        }
    }

    pub fn start_game(&mut self, session_id: &str) -> Result<(), String> {
        if self.host_id != session_id {
            return Err("Only the room host can start the game".into());
        }
        if self.status == GameStatus::Playing {
            return Err("Game is already in progress".into());
        }

        self.status = GameStatus::Playing;
        self.start_time = Some(chrono::Utc::now());
        self.board = Board::new(self.config);

        for player in self.players.values_mut() {
            player.score = 0;
            player.is_eliminated = false;
            player.is_spectator = false;
        }

        let _ = self.broadcast_tx.send(ServerMessage::GameStarted {
            config: self.config,
        });

        // Broadcast game started system message
        let _ = self
            .broadcast_tx
            .send(ServerMessage::ChatMessage(ChatMessagePayload {
                id: Uuid::new_v4().to_string(),
                player_id: None,
                username: "SYSTEM".to_string(),
                color: Some("#38bdf8".to_string()),
                text: "Game started!".to_string(),
                is_system: true,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_key: Some("game_started".to_string()),
                event_params: vec![],
            }));

        self.broadcast_state();
        Ok(())
    }

    pub fn handle_reveal(&mut self, session_id: &str, coord: Coord3D) {
        if self.status != GameStatus::Playing {
            return;
        }

        let (player_name, player_color, is_eliminated) = {
            match self.players.get(session_id) {
                Some(p) => (p.username.clone(), p.color.clone(), p.is_eliminated),
                None => return,
            }
        };

        if is_eliminated {
            return;
        }

        let result = self.board.reveal(
            coord,
            Some(session_id.to_string()),
            Some(player_color.clone()),
        );

        match result {
            RevealResult::FirstClickGenerated { revealed } => {
                let pts = 1;
                if let Some(p) = self.players.get_mut(session_id) {
                    p.score += pts;
                }

                let score_deltas = vec![ScoreDelta {
                    player_id: session_id.to_string(),
                    points: pts,
                    total_score: self.players.get(session_id).map(|p| p.score).unwrap_or(0),
                }];

                let _ = self.broadcast_tx.send(ServerMessage::CellsRevealed {
                    revealed,
                    score_deltas,
                });

                self.check_game_over();
            }
            RevealResult::Success { revealed } => {
                let pts = revealed.len() as u32;
                if let Some(p) = self.players.get_mut(session_id) {
                    p.score += pts;
                }

                let score_deltas = vec![ScoreDelta {
                    player_id: session_id.to_string(),
                    points: pts,
                    total_score: self.players.get(session_id).map(|p| p.score).unwrap_or(0),
                }];

                let _ = self.broadcast_tx.send(ServerMessage::CellsRevealed {
                    revealed,
                    score_deltas,
                });

                self.check_game_over();
            }
            RevealResult::HitMine {
                hit_coord,
                all_mines,
                ..
            } => {
                if let Some(p) = self.players.get_mut(session_id) {
                    p.is_eliminated = true;
                    p.is_spectator = true;
                }

                // Broadcast elimination
                let _ = self.broadcast_tx.send(ServerMessage::PlayerEliminated {
                    player_id: session_id.to_string(),
                    username: player_name.clone(),
                    hit_coord,
                    all_mines,
                });

                let elim_text = format_eliminated_msg(Language::En, &player_name);
                let _ = self
                    .broadcast_tx
                    .send(ServerMessage::ChatMessage(ChatMessagePayload {
                        id: Uuid::new_v4().to_string(),
                        player_id: None,
                        username: "SYSTEM".to_string(),
                        color: Some("#ef4444".to_string()),
                        text: elim_text,
                        is_system: true,
                        timestamp: chrono::Utc::now().timestamp_millis(),
                        event_key: Some("player_eliminated".to_string()),
                        event_params: vec![player_name],
                    }));

                self.check_game_over();
            }
            _ => {}
        }
    }

    pub fn handle_chord(&mut self, session_id: &str, coord: Coord3D) {
        if self.status != GameStatus::Playing {
            return;
        }

        let (player_name, player_color, is_eliminated) = {
            match self.players.get(session_id) {
                Some(p) => (p.username.clone(), p.color.clone(), p.is_eliminated),
                None => return,
            }
        };

        if is_eliminated {
            return;
        }

        let result = self.board.chord(
            coord,
            Some(session_id.to_string()),
            Some(player_color.clone()),
        );

        match result {
            RevealResult::Success { revealed } => {
                if !revealed.is_empty() {
                    let pts = revealed.len() as u32;
                    if let Some(p) = self.players.get_mut(session_id) {
                        p.score += pts;
                    }

                    let score_deltas = vec![ScoreDelta {
                        player_id: session_id.to_string(),
                        points: pts,
                        total_score: self.players.get(session_id).map(|p| p.score).unwrap_or(0),
                    }];

                    let _ = self.broadcast_tx.send(ServerMessage::CellsRevealed {
                        revealed,
                        score_deltas,
                    });

                    self.check_game_over();
                }
            }
            RevealResult::HitMine {
                hit_coord,
                all_mines,
                ..
            } => {
                if let Some(p) = self.players.get_mut(session_id) {
                    p.is_eliminated = true;
                    p.is_spectator = true;
                }

                let _ = self.broadcast_tx.send(ServerMessage::PlayerEliminated {
                    player_id: session_id.to_string(),
                    username: player_name.clone(),
                    hit_coord,
                    all_mines,
                });

                let elim_text = format_eliminated_msg(Language::En, &player_name);
                let _ = self
                    .broadcast_tx
                    .send(ServerMessage::ChatMessage(ChatMessagePayload {
                        id: Uuid::new_v4().to_string(),
                        player_id: None,
                        username: "SYSTEM".to_string(),
                        color: Some("#ef4444".to_string()),
                        text: elim_text,
                        is_system: true,
                        timestamp: chrono::Utc::now().timestamp_millis(),
                        event_key: Some("player_eliminated".to_string()),
                        event_params: vec![player_name],
                    }));

                self.check_game_over();
            }
            _ => {}
        }
    }

    pub fn handle_toggle_flag(&mut self, session_id: &str, coord: Coord3D) {
        if self.status != GameStatus::Playing {
            return;
        }

        let is_eliminated = self
            .players
            .get(session_id)
            .map(|p| p.is_eliminated)
            .unwrap_or(true);
        if is_eliminated {
            return;
        }

        let cell = self.board.get_cell(coord);
        let will_flag = !cell.is_flagged;
        if self.board.toggle_flag(coord) {
            let _ = self.broadcast_tx.send(ServerMessage::PlayerFlagToggled {
                coord,
                is_flagged: will_flag,
                player_id: session_id.to_string(),
            });
        }
    }

    pub fn handle_chat(&mut self, session_id: &str, text: &str) {
        let (username, color) = match self.players.get(session_id) {
            Some(p) => (p.username.clone(), p.color.clone()),
            None => return,
        };

        if text.trim().is_empty() {
            return;
        }

        let _ = self
            .broadcast_tx
            .send(ServerMessage::ChatMessage(ChatMessagePayload {
                id: Uuid::new_v4().to_string(),
                player_id: Some(session_id.to_string()),
                username,
                color: Some(color),
                text: text.to_string(),
                is_system: false,
                timestamp: chrono::Utc::now().timestamp_millis(),
                event_key: None,
                event_params: vec![],
            }));
    }

    fn check_game_over(&mut self) {
        let total_non_mines = self.board.dims.total_cells() - self.config.mines;
        let all_revealed = self.board.revealed_count >= total_non_mines;
        let all_eliminated =
            !self.players.is_empty() && self.players.values().all(|p| p.is_eliminated);

        if all_revealed || all_eliminated {
            self.status = if all_revealed {
                for c in self.board.cells.iter_mut() {
                    if c.is_mine {
                        c.is_flagged = true;
                    }
                }
                self.board.flag_count = self.config.mines;
                GameStatus::Won
            } else {
                GameStatus::Lost
            };

            let mut sorted_players: Vec<&RoomPlayer> = self.players.values().collect();
            sorted_players.sort_by_key(|b| std::cmp::Reverse(b.score));

            let final_scores: Vec<ScoreDelta> = sorted_players
                .iter()
                .map(|p| ScoreDelta {
                    player_id: p.id.clone(),
                    points: p.score,
                    total_score: p.score,
                })
                .collect();

            let winners: Vec<String> = if let Some(top) = sorted_players.first() {
                sorted_players
                    .iter()
                    .filter(|p| p.score == top.score && top.score > 0)
                    .map(|p| p.username.clone())
                    .collect()
            } else {
                Vec::new()
            };

            let _ = self.broadcast_tx.send(ServerMessage::GameOver {
                winners: winners.clone(),
                final_scores,
            });

            if let Some(top) = sorted_players.first() {
                let winner_name = &top.username;
                let over_msg = format_game_over_msg(Language::En, winner_name, top.score);
                let _ = self
                    .broadcast_tx
                    .send(ServerMessage::ChatMessage(ChatMessagePayload {
                        id: Uuid::new_v4().to_string(),
                        player_id: None,
                        username: "SYSTEM".to_string(),
                        color: Some("#fbbf24".to_string()),
                        text: over_msg,
                        is_system: true,
                        timestamp: chrono::Utc::now().timestamp_millis(),
                        event_key: Some("game_over".to_string()),
                        event_params: vec![winner_name.clone(), top.score.to_string()],
                    }));

                // Record match stats in DB (only for human registered users)
                let scores: Vec<(String, u32)> = self
                    .players
                    .values()
                    .filter_map(|p| p.user_id.clone().map(|uid| (uid, p.score)))
                    .collect();

                let winner_uid = top.user_id.as_deref();
                self.db.record_mp_match(&scores, winner_uid);
            }

            self.broadcast_state();
        }
    }

    pub fn broadcast_state(&self) {
        let snapshot = self.snapshot();
        let _ = self.broadcast_tx.send(ServerMessage::RoomState(snapshot));
    }
}

#[derive(Clone)]
pub struct RoomManager {
    rooms: Arc<RwLock<HashMap<String, Arc<RwLock<Room>>>>>,
    db: Database,
}

impl RoomManager {
    pub fn new(db: Database) -> Self {
        Self {
            rooms: Arc::new(RwLock::new(HashMap::new())),
            db,
        }
    }

    pub fn generate_room_code() -> String {
        const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        let mut rng = rand::thread_rng();
        (0..6)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    pub fn create_room(
        &self,
        name: String,
        host_id: String,
        config: BoardConfig,
    ) -> (String, Arc<RwLock<Room>>) {
        let room_id = Self::generate_room_code();
        let room = Arc::new(RwLock::new(Room::new(
            room_id.clone(),
            name,
            host_id,
            config,
            self.db.clone(),
        )));

        self.rooms.write().insert(room_id.clone(), room.clone());
        (room_id, room)
    }

    pub fn get_room(&self, room_id: &str) -> Option<Arc<RwLock<Room>>> {
        self.rooms.read().get(room_id).cloned()
    }

    pub fn list_rooms(&self) -> Vec<RoomSummary> {
        let rooms = self.rooms.read();
        rooms
            .values()
            .map(|r| {
                let room = r.read();
                let host_name = room
                    .players
                    .get(&room.host_id)
                    .map(|p| p.username.clone())
                    .unwrap_or_else(|| "Host".to_string());

                RoomSummary {
                    id: room.id.clone(),
                    name: room.name.clone(),
                    host_name,
                    player_count: room.players.len(),
                    difficulty: room.config.difficulty,
                    status: room.status,
                }
            })
            .collect()
    }

    pub fn clean_empty_rooms(&self) {
        self.rooms
            .write()
            .retain(|_, r| !r.read().players.is_empty());
    }
}
