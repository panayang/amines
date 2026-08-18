use crate::ai_solver::BotTier;
use crate::board::{BoardConfig, Cell, Difficulty, GameStatus, RevealedCellInfo};
use crate::topology::Coord3D;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: String,
    pub username: String,
    pub color: String,
    pub score: u32,
    pub is_eliminated: bool,
    pub is_host: bool,
    pub is_ready: bool,
    pub is_spectator: bool,
    #[serde(default)]
    pub is_bot: bool,
    #[serde(default)]
    pub bot_tier: Option<BotTier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSnapshot {
    pub room_id: String,
    pub name: String,
    pub host_id: String,
    pub config: BoardConfig,
    pub status: GameStatus,
    pub players: Vec<PlayerInfo>,
    pub revealed_count: usize,
    pub total_non_mines: usize,
    pub elapsed_seconds: u64,
    pub cells: Vec<CellSnapshot>,
    #[serde(default = "default_bot_speed")]
    pub bot_speed_ms: u64,
}

fn default_bot_speed() -> u64 {
    800
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellSnapshot {
    pub coord: Coord3D,
    pub is_revealed: bool,
    pub adjacent_mines: u8,
    pub is_flagged: bool,
    pub is_mine: bool,
    pub revealed_by: Option<String>,
    pub player_color: Option<String>,
}

impl From<&Cell> for CellSnapshot {
    fn from(c: &Cell) -> Self {
        Self {
            coord: c.coord,
            is_revealed: c.is_revealed,
            adjacent_mines: if c.is_revealed { c.adjacent_mines } else { 0 },
            is_flagged: c.is_flagged,
            is_mine: if c.is_revealed { c.is_mine } else { false },
            revealed_by: c.revealed_by.clone(),
            player_color: c.player_color.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessagePayload {
    pub id: String,
    pub player_id: Option<String>,
    pub username: String,
    pub color: Option<String>,
    pub text: String,
    pub is_system: bool,
    pub timestamp: i64,
    #[serde(default)]
    pub event_key: Option<String>,
    #[serde(default)]
    pub event_params: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreDelta {
    pub player_id: String,
    pub points: u32,
    pub total_score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    CreateRoom {
        name: String,
        config: BoardConfig,
        username: String,
        token: Option<String>,
    },
    JoinRoom {
        room_id: String,
        username: String,
        token: Option<String>,
    },
    SetReady {
        ready: bool,
    },
    StartGame,
    RevealCell {
        coord: Coord3D,
    },
    ChordCell {
        coord: Coord3D,
    },
    ToggleFlag {
        coord: Coord3D,
    },
    SendChat {
        text: String,
    },
    AddBot {
        tier: BotTier,
        speed_ms: Option<u64>,
    },
    RemoveBot {
        bot_id: String,
    },
    UpdateBotSpeed {
        speed_ms: u64,
    },
    LeaveRoom,
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    RoomState(RoomSnapshot),
    GameStarted {
        config: BoardConfig,
    },
    CellsRevealed {
        revealed: Vec<RevealedCellInfo>,
        score_deltas: Vec<ScoreDelta>,
    },
    PlayerEliminated {
        player_id: String,
        username: String,
        hit_coord: Coord3D,
        all_mines: Vec<Coord3D>,
    },
    PlayerFlagToggled {
        coord: Coord3D,
        is_flagged: bool,
        player_id: String,
    },
    GameOver {
        winners: Vec<String>,
        final_scores: Vec<ScoreDelta>,
    },
    ChatMessage(ChatMessagePayload),
    GlobalNotification {
        title: String,
        message: String,
    },
    Pong,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSummary {
    pub id: String,
    pub name: String,
    pub host_name: String,
    pub player_count: usize,
    pub difficulty: Difficulty,
    pub status: GameStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub username: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbRecordRequest {
    pub difficulty: Difficulty,
    pub config_hash: String,
    pub time_ms: u64,
    pub moves: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbRecordResponse {
    pub id: String,
    pub is_new_best: bool,
    pub current_best_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStatsResponse {
    pub username: String,
    pub easy_pb_ms: Option<u64>,
    pub medium_pb_ms: Option<u64>,
    pub expert_pb_ms: Option<u64>,
    pub sp_games_played: u32,
    pub sp_games_won: u32,
    pub mp_games_played: u32,
    pub mp_games_won: u32,
    pub mp_total_score: u32,
}
