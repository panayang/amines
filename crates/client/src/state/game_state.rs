use gloo_net::http::Request;
use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;
use shared::ai_solver::{AiAction, AiSolver, BotTier};
use shared::board::{Board, BoardConfig, GameStatus};
use shared::protocol::*;
use shared::topology::Coord3D;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, MessageEvent, WebSocket};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    SinglePlayer,
    Multiplayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRankInfo {
    pub rank: usize,
    pub username: String,
    pub color: String,
    pub score: u32,
    pub is_eliminated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameOverModalData {
    pub winners: Vec<String>,
    pub is_all_eliminated: bool,
    pub is_board_cleared: bool,
    pub player_rankings: Vec<PlayerRankInfo>,
    pub elapsed_seconds: u64,
    pub revealed_count: usize,
    pub total_non_mines: usize,
}

#[derive(Clone, Copy)]
pub struct GameState {
    pub mode: ReadSignal<AppMode>,
    pub set_mode: WriteSignal<AppMode>,

    // Single-Player State
    pub sp_board: ReadSignal<Board>,
    pub set_sp_board: WriteSignal<Board>,
    pub sp_config: ReadSignal<BoardConfig>,
    pub set_sp_config: WriteSignal<BoardConfig>,
    pub sp_current_layer: ReadSignal<usize>,
    pub set_sp_current_layer: WriteSignal<usize>,
    pub sp_time: ReadSignal<u64>,
    pub set_sp_time: WriteSignal<u64>,
    pub sp_moves: ReadSignal<u32>,
    pub set_sp_moves: WriteSignal<u32>,
    pub sp_is_timer_running: ReadSignal<bool>,
    pub set_sp_is_timer_running: WriteSignal<bool>,
    pub custom_config: ReadSignal<BoardConfig>,
    pub set_custom_config: WriteSignal<BoardConfig>,
    pub hint_coord: ReadSignal<Option<Coord3D>>,
    pub set_hint_coord: WriteSignal<Option<Coord3D>>,

    // Multiplayer State
    pub mp_room: ReadSignal<Option<RoomSnapshot>>,
    pub set_mp_room: WriteSignal<Option<RoomSnapshot>>,
    pub mp_current_layer: ReadSignal<usize>,
    pub set_mp_current_layer: WriteSignal<usize>,
    pub mp_chat_logs: ReadSignal<Vec<ChatMessagePayload>>,
    pub set_mp_chat_logs: WriteSignal<Vec<ChatMessagePayload>>,
    pub mp_connected: ReadSignal<bool>,
    pub set_mp_connected: WriteSignal<bool>,
    pub mp_game_over_data: ReadSignal<Option<GameOverModalData>>,
    pub set_mp_game_over_data: WriteSignal<Option<GameOverModalData>>,
    pub mp_ws: StoredValue<Option<WebSocket>>,
}

impl GameState {
    pub fn new() -> Self {
        let initial_config = BoardConfig::easy();
        let default_custom = BoardConfig::custom(12, 12, 3, 40).unwrap();
        let (mode, set_mode) = signal(AppMode::SinglePlayer);

        let (sp_board, set_sp_board) = signal(Board::new(initial_config));
        let (sp_config, set_sp_config) = signal(initial_config);
        let (custom_config, set_custom_config) = signal(default_custom);
        let (hint_coord, set_hint_coord) = signal(None::<Coord3D>);
        let (sp_current_layer, set_sp_current_layer) = signal(0);
        let (sp_time, set_sp_time) = signal(0u64);
        let (sp_moves, set_sp_moves) = signal(0u32);
        let (sp_is_timer_running, set_sp_is_timer_running) = signal(false);

        let (mp_room, set_mp_room) = signal(None::<RoomSnapshot>);
        let (mp_current_layer, set_mp_current_layer) = signal(0);
        let (mp_chat_logs, set_mp_chat_logs) = signal(Vec::<ChatMessagePayload>::new());
        let (mp_connected, set_mp_connected) = signal(false);
        let (mp_game_over_data, set_mp_game_over_data) = signal(None::<GameOverModalData>);
        let mp_ws = StoredValue::new(None::<WebSocket>);

        let state = Self {
            mode,
            set_mode,
            sp_board,
            set_sp_board,
            sp_config,
            set_sp_config,
            sp_current_layer,
            set_sp_current_layer,
            sp_time,
            set_sp_time,
            sp_moves,
            set_sp_moves,
            sp_is_timer_running,
            set_sp_is_timer_running,
            custom_config,
            set_custom_config,
            hint_coord,
            set_hint_coord,
            mp_room,
            set_mp_room,
            mp_current_layer,
            set_mp_current_layer,
            mp_chat_logs,
            set_mp_chat_logs,
            mp_connected,
            set_mp_connected,
            mp_game_over_data,
            set_mp_game_over_data,
            mp_ws,
        };

        // Start SP timer loop
        let is_running_sig = state.sp_is_timer_running;
        let set_time_sig = state.set_sp_time;
        wasm_bindgen_futures::spawn_local(async move {
            loop {
                TimeoutFuture::new(1000).await;
                if is_running_sig.get() {
                    set_time_sig.update(|t| *t += 1);
                }
            }
        });

        state
    }

    pub fn set_app_mode(&self, new_mode: AppMode) {
        self.set_mode.set(new_mode);
        if new_mode == AppMode::Multiplayer {
            self.ensure_ws_connected();
        }
    }

    pub fn reset_sp_game(&self, config: BoardConfig) {
        self.set_sp_config.set(config);
        self.set_sp_board.set(Board::new(config));
        self.set_sp_current_layer.set(0);
        self.set_hint_coord.set(None);
        self.set_sp_time.set(0);
        self.set_sp_moves.set(0);
        self.set_sp_is_timer_running.set(false);
    }

    pub fn step_layer(&self, delta: i32) {
        match self.mode.get() {
            AppMode::SinglePlayer => {
                let max_d = self.sp_config.get().depth;
                if max_d == 0 {
                    return;
                }
                let cur = self.sp_current_layer.get() as i32;
                let next = (cur + delta).clamp(0, max_d as i32 - 1) as usize;
                self.set_sp_current_layer.set(next);
            }
            AppMode::Multiplayer => {
                if let Some(room) = self.mp_room.get() {
                    let max_d = room.config.depth;
                    if max_d == 0 {
                        return;
                    }
                    let cur = self.mp_current_layer.get() as i32;
                    let next = (cur + delta).clamp(0, max_d as i32 - 1) as usize;
                    self.set_mp_current_layer.set(next);
                }
            }
        }
    }

    pub fn set_layer(&self, layer: usize) {
        match self.mode.get() {
            AppMode::SinglePlayer => self.set_sp_current_layer.set(layer),
            AppMode::Multiplayer => self.set_mp_current_layer.set(layer),
        }
    }

    // --- Single-Player Logic ---
    pub fn sp_reveal(&self, coord: Coord3D, auth_token: Option<String>) {
        let mut board = self.sp_board.get();
        if board.status == GameStatus::Won || board.status == GameStatus::Lost {
            return;
        }

        if !self.sp_is_timer_running.get() && !board.is_generated {
            self.set_sp_is_timer_running.set(true);
        }

        let _ = board.reveal(coord, None, None);
        self.set_sp_moves.update(|m| *m += 1);

        if board.is_won() {
            self.set_sp_is_timer_running.set(false);
            let time_ms = self.sp_time.get() * 1000;
            let moves = self.sp_moves.get();
            let config = self.sp_config.get();

            if let Some(tok) = auth_token {
                let req_payload = PbRecordRequest {
                    difficulty: config.difficulty,
                    config_hash: config.config_hash(),
                    time_ms,
                    moves,
                };
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = Request::post("/api/pb/record")
                        .header("Authorization", &format!("Bearer {tok}"))
                        .json(&req_payload)
                        .unwrap()
                        .send()
                        .await;
                });
            }
        } else if board.is_lost() {
            self.set_sp_is_timer_running.set(false);
        }

        self.set_sp_board.set(board);
    }

    pub fn sp_toggle_flag(&self, coord: Coord3D) {
        let mut board = self.sp_board.get();
        if board.status == GameStatus::Won || board.status == GameStatus::Lost {
            return;
        }

        if board.toggle_flag(coord) {
            self.set_sp_board.set(board);
        }
    }

    pub fn sp_chord(&self, coord: Coord3D, auth_token: Option<String>) {
        let mut board = self.sp_board.get();
        if board.status == GameStatus::Won || board.status == GameStatus::Lost {
            return;
        }

        let _ = board.chord(coord, None, None);
        self.set_sp_moves.update(|m| *m += 1);

        if board.is_won() {
            self.set_sp_is_timer_running.set(false);
            let time_ms = self.sp_time.get() * 1000;
            let moves = self.sp_moves.get();
            let config = self.sp_config.get();

            if let Some(tok) = auth_token {
                let req_payload = PbRecordRequest {
                    difficulty: config.difficulty,
                    config_hash: config.config_hash(),
                    time_ms,
                    moves,
                };
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = Request::post("/api/pb/record")
                        .header("Authorization", &format!("Bearer {tok}"))
                        .json(&req_payload)
                        .unwrap()
                        .send()
                        .await;
                });
            }
        } else if board.is_lost() {
            self.set_sp_is_timer_running.set(false);
        }

        self.set_sp_board.set(board);
    }

    pub fn sp_ai_step(&self, tier: BotTier, auth_token: Option<String>) -> Option<String> {
        let board = self.sp_board.get();
        if board.status == GameStatus::Won || board.status == GameStatus::Lost {
            return None;
        }

        let snapshots: Vec<CellSnapshot> = board.cells.iter().map(CellSnapshot::from).collect();
        let action = AiSolver::decide_action(board.dims, &snapshots, tier, board.config.mines)?;

        match action {
            AiAction::Reveal(c) => {
                self.set_sp_current_layer.set(c.z);
                self.set_hint_coord.set(Some(c));
                self.sp_reveal(c, auth_token);
                Some(format!("Reveal @ ({},{},{})", c.x, c.y, c.z))
            }
            AiAction::Flag(c) => {
                self.set_sp_current_layer.set(c.z);
                self.set_hint_coord.set(Some(c));
                self.sp_toggle_flag(c);
                Some(format!("Flag @ ({},{},{})", c.x, c.y, c.z))
            }
            AiAction::Chord(c) => {
                self.set_sp_current_layer.set(c.z);
                self.set_hint_coord.set(Some(c));
                self.sp_chord(c, auth_token);
                Some(format!("Chord @ ({},{},{})", c.x, c.y, c.z))
            }
        }
    }

    pub fn sp_ai_hint(&self, tier: BotTier) -> Option<String> {
        let board = self.sp_board.get();
        if board.status == GameStatus::Won || board.status == GameStatus::Lost {
            return None;
        }

        let snapshots: Vec<CellSnapshot> = board.cells.iter().map(CellSnapshot::from).collect();
        let action = AiSolver::decide_action(board.dims, &snapshots, tier, board.config.mines)?;

        match action {
            AiAction::Reveal(c) => {
                self.set_sp_current_layer.set(c.z);
                self.set_hint_coord.set(Some(c));
                Some(format!("💡 REVEAL @ ({},{},{})", c.x, c.y, c.z))
            }
            AiAction::Flag(c) => {
                self.set_sp_current_layer.set(c.z);
                self.set_hint_coord.set(Some(c));
                Some(format!("🚩 FLAG @ ({},{},{})", c.x, c.y, c.z))
            }
            AiAction::Chord(c) => {
                self.set_sp_current_layer.set(c.z);
                self.set_hint_coord.set(Some(c));
                Some(format!("⚡ CHORD @ ({},{},{})", c.x, c.y, c.z))
            }
        }
    }

    // --- Multiplayer WebSocket Methods ---
    pub fn ensure_ws_connected(&self) {
        if self.mp_ws.with_value(|ws| ws.is_some()) {
            return;
        }

        let window = match web_sys::window() {
            Some(w) => w,
            None => return,
        };
        let location = window.location();
        let host = match location.host() {
            Ok(h) => h,
            Err(_) => "127.0.0.1:3000".to_string(),
        };
        let protocol = match location.protocol() {
            Ok(p) if p == "https:" => "wss:",
            _ => "ws:",
        };

        let ws_url = format!("{protocol}//{host}/ws");
        let ws = match WebSocket::new(&ws_url) {
            Ok(w) => w,
            Err(_) => return,
        };

        let set_conn = self.set_mp_connected;
        let set_room = self.set_mp_room;
        let set_chat = self.set_mp_chat_logs;
        let game_over_data_sig = self.mp_game_over_data;
        let set_game_over = self.set_mp_game_over_data;
        let ws_holder = self.mp_ws;

        // onopen
        let onopen = Closure::<dyn FnMut()>::new(move || {
            set_conn.set(true);
        });
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        onopen.forget();

        // onclose
        let onclose = Closure::<dyn FnMut(CloseEvent)>::new(move |_| {
            set_conn.set(false);
            ws_holder.set_value(None);
        });
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        onclose.forget();

        // onmessage
        let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
            if let Some(txt) = e.data().as_string() {
                if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&txt) {
                    match server_msg {
                        ServerMessage::RoomState(snap) => {
                            if (snap.status == GameStatus::Won || snap.status == GameStatus::Lost)
                                && game_over_data_sig.get_untracked().is_none()
                            {
                                let mut ranked: Vec<PlayerRankInfo> = snap
                                    .players
                                    .iter()
                                    .map(|p| PlayerRankInfo {
                                        rank: 0,
                                        username: p.username.clone(),
                                        color: p.color.clone(),
                                        score: p.score,
                                        is_eliminated: p.is_eliminated,
                                    })
                                    .collect();
                                ranked.sort_by_key(|b| std::cmp::Reverse(b.score));
                                for (i, p) in ranked.iter_mut().enumerate() {
                                    p.rank = i + 1;
                                }
                                let all_elim = !snap.players.is_empty()
                                    && snap.players.iter().all(|p| p.is_eliminated);
                                let board_cleared = snap.revealed_count >= snap.total_non_mines;

                                let winners: Vec<String> = if let Some(top) = ranked.first() {
                                    ranked
                                        .iter()
                                        .filter(|p| p.score == top.score && top.score > 0)
                                        .map(|p| p.username.clone())
                                        .collect()
                                } else {
                                    Vec::new()
                                };

                                set_game_over.set(Some(GameOverModalData {
                                    winners,
                                    is_all_eliminated: all_elim,
                                    is_board_cleared: board_cleared,
                                    player_rankings: ranked,
                                    elapsed_seconds: snap.elapsed_seconds,
                                    revealed_count: snap.revealed_count,
                                    total_non_mines: snap.total_non_mines,
                                }));
                            }
                            set_room.set(Some(snap));
                        }
                        ServerMessage::GameStarted { config } => {
                            set_game_over.set(None);
                            set_room.update(|r| {
                                if let Some(room) = r {
                                    room.status = GameStatus::Playing;
                                    room.config = config;
                                    room.revealed_count = 0;
                                    for c in &mut room.cells {
                                        c.is_revealed = false;
                                        c.is_flagged = false;
                                        c.is_mine = false;
                                    }
                                    for p in &mut room.players {
                                        p.score = 0;
                                        p.is_eliminated = false;
                                        p.is_spectator = false;
                                    }
                                }
                            });
                        }
                        ServerMessage::CellsRevealed {
                            revealed,
                            score_deltas,
                        } => {
                            set_room.update(|r| {
                                if let Some(room) = r {
                                    for cell_info in revealed {
                                        let idx = cell_info
                                            .coord
                                            .to_index(room.config.width, room.config.height);
                                        if idx < room.cells.len() {
                                            room.cells[idx].is_revealed = true;
                                            room.cells[idx].adjacent_mines =
                                                cell_info.adjacent_mines;
                                            room.cells[idx].is_mine = cell_info.is_mine;
                                            room.cells[idx].revealed_by = cell_info.revealed_by;
                                            room.cells[idx].player_color = cell_info.player_color;
                                        }
                                        room.revealed_count += 1;
                                    }
                                    for delta in score_deltas {
                                        if let Some(p) = room
                                            .players
                                            .iter_mut()
                                            .find(|p| p.id == delta.player_id)
                                        {
                                            p.score = delta.total_score;
                                        }
                                    }
                                }
                            });
                        }
                        ServerMessage::PlayerEliminated {
                            player_id,
                            hit_coord,
                            all_mines,
                            ..
                        } => {
                            set_room.update(|r| {
                                if let Some(room) = r {
                                    if let Some(p) =
                                        room.players.iter_mut().find(|p| p.id == player_id)
                                    {
                                        p.is_eliminated = true;
                                        p.is_spectator = true;
                                    }
                                    let hit_idx =
                                        hit_coord.to_index(room.config.width, room.config.height);
                                    if hit_idx < room.cells.len() {
                                        room.cells[hit_idx].is_revealed = true;
                                        room.cells[hit_idx].is_mine = true;
                                    }
                                    for mine_c in all_mines {
                                        let m_idx =
                                            mine_c.to_index(room.config.width, room.config.height);
                                        if m_idx < room.cells.len() {
                                            room.cells[m_idx].is_mine = true;
                                        }
                                    }
                                }
                            });
                        }
                        ServerMessage::PlayerFlagToggled {
                            coord, is_flagged, ..
                        } => {
                            set_room.update(|r| {
                                if let Some(room) = r {
                                    let idx = coord.to_index(room.config.width, room.config.height);
                                    if idx < room.cells.len() {
                                        room.cells[idx].is_flagged = is_flagged;
                                    }
                                }
                            });
                        }
                        ServerMessage::GameOver {
                            winners,
                            final_scores,
                        } => {
                            set_room.update(|r| {
                                if let Some(room) = r {
                                    for score in &final_scores {
                                        if let Some(p) = room
                                            .players
                                            .iter_mut()
                                            .find(|p| p.id == score.player_id)
                                        {
                                            p.score = score.total_score;
                                        }
                                    }

                                    let mut ranked: Vec<PlayerRankInfo> = room
                                        .players
                                        .iter()
                                        .map(|p| PlayerRankInfo {
                                            rank: 0,
                                            username: p.username.clone(),
                                            color: p.color.clone(),
                                            score: p.score,
                                            is_eliminated: p.is_eliminated,
                                        })
                                        .collect();
                                    ranked.sort_by_key(|b| std::cmp::Reverse(b.score));
                                    for (i, p) in ranked.iter_mut().enumerate() {
                                        p.rank = i + 1;
                                    }
                                    let all_elim = !room.players.is_empty()
                                        && room.players.iter().all(|p| p.is_eliminated);
                                    let board_cleared = room.revealed_count >= room.total_non_mines;

                                    room.status = if board_cleared {
                                        GameStatus::Won
                                    } else {
                                        GameStatus::Lost
                                    };

                                    set_game_over.set(Some(GameOverModalData {
                                        winners,
                                        is_all_eliminated: all_elim,
                                        is_board_cleared: board_cleared,
                                        player_rankings: ranked,
                                        elapsed_seconds: room.elapsed_seconds,
                                        revealed_count: room.revealed_count,
                                        total_non_mines: room.total_non_mines,
                                    }));
                                }
                            });
                        }
                        ServerMessage::ChatMessage(chat) => {
                            set_chat.update(|logs| logs.push(chat));
                        }
                        _ => {}
                    }
                }
            }
        });
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();

        self.mp_ws.set_value(Some(ws));
    }

    pub fn send_ws_msg(&self, msg: &ClientMessage) {
        self.ensure_ws_connected();
        self.mp_ws.with_value(|ws_opt| {
            if let Some(ws) = ws_opt.as_ref() {
                if let Ok(json) = serde_json::to_string(msg) {
                    let _ = ws.send_with_str(&json);
                }
            }
        });
    }

    pub fn mp_create_room(
        &self,
        name: String,
        config: BoardConfig,
        username: String,
        token: Option<String>,
    ) {
        self.set_mp_game_over_data.set(None);
        self.send_ws_msg(&ClientMessage::CreateRoom {
            name,
            config,
            username,
            token,
        });
    }

    pub fn mp_join_room(&self, room_id: String, username: String, token: Option<String>) {
        self.set_mp_game_over_data.set(None);
        self.send_ws_msg(&ClientMessage::JoinRoom {
            room_id,
            username,
            token,
        });
    }

    pub fn mp_set_ready(&self, ready: bool) {
        self.send_ws_msg(&ClientMessage::SetReady { ready });
    }

    pub fn mp_start_game(&self) {
        self.set_mp_game_over_data.set(None);
        self.send_ws_msg(&ClientMessage::StartGame);
    }

    pub fn mp_reveal(&self, coord: Coord3D) {
        self.send_ws_msg(&ClientMessage::RevealCell { coord });
    }

    pub fn mp_chord(&self, coord: Coord3D) {
        self.send_ws_msg(&ClientMessage::ChordCell { coord });
    }

    pub fn mp_toggle_flag(&self, coord: Coord3D) {
        self.send_ws_msg(&ClientMessage::ToggleFlag { coord });
    }

    pub fn mp_send_chat(&self, text: String) {
        self.send_ws_msg(&ClientMessage::SendChat { text });
    }

    pub fn mp_add_bot(&self, tier: shared::BotTier, speed_ms: Option<u64>) {
        self.send_ws_msg(&ClientMessage::AddBot { tier, speed_ms });
    }

    pub fn mp_remove_bot(&self, bot_id: String) {
        self.send_ws_msg(&ClientMessage::RemoveBot { bot_id });
    }

    pub fn mp_update_bot_speed(&self, speed_ms: u64) {
        self.send_ws_msg(&ClientMessage::UpdateBotSpeed { speed_ms });
    }

    pub fn mp_leave_room(&self) {
        self.set_mp_game_over_data.set(None);
        self.send_ws_msg(&ClientMessage::LeaveRoom);
        self.set_mp_room.set(None);
    }
}
