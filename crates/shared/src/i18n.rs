use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    En,
    Zh,
}

impl Language {
    pub fn code(&self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Zh => "zh",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code {
            "zh" => Language::Zh,
            _ => Language::En,
        }
    }

    pub fn toggle(&self) -> Self {
        match self {
            Language::En => Language::Zh,
            Language::Zh => Language::En,
        }
    }
}

pub fn t(lang: Language, key: &str) -> &str {
    match lang {
        Language::En => match key {
            // App Title & Header
            "app_title" => "3D Möbius Minesweeper",
            "app_subtitle" => "Topological Non-Euclidean Multiplayer Minesweeper",
            "nav_single" => "Single-Player",
            "nav_multi" => "Multiplayer Battle",
            "nav_stats" => "Leaderboard & Stats",
            "nav_login" => "Sign In",
            "nav_register" => "Register",
            "nav_logout" => "Sign Out",
            "nav_guest" => "Guest Player",

            // Difficulties
            "diff_easy" => "Easy",
            "diff_medium" => "Medium",
            "diff_expert" => "Expert",
            "diff_custom" => "Custom",

            // Game Board & HUD
            "hud_time" => "Time",
            "hud_mines" => "Mines",
            "hud_layer" => "Depth Layer (Z)",
            "hud_pb" => "Personal Best",
            "hud_no_pb" => "No Record",
            "hud_restart" => "Restart Game",
            "hud_room_code" => "Room Code",
            "hud_copy_code" => "Copy Code",
            "hud_copied" => "Copied!",
            "hud_score" => "Score",
            "tool_dig" => "⛏️ Dig Mode",
            "tool_flag" => "🚩 Flag Mode",
            "hud_pause" => "Pause",
            "hud_resume" => "Resume",
            "status_paused" => "Game Paused // Click or Press [P] to Resume",
            "mobile_prev_layer" => "◄ Prev (Z-)",
            "mobile_next_layer" => "Next (Z+) ►",
            "mp_leave_room" => "Leave Room",
            "mp_create_match" => "Create Match",

            // Mobius & Slice Guides
            "guide_mobius_left" => "Möbius Seam Left (Inverts Y & Z)",
            "guide_mobius_right" => "Möbius Seam Right (Inverts Y & Z)",
            "guide_controls" => "Q/E or Scroll: Switch Layer | Left Click: Reveal | Right Click: Flag | Chord Click: Quick Reveal",
            "global_banner_text" => "3D Möbius Topology: Crossing X=0 / X=W-1 double-inverts Y & Z axes. 26-neighborhood 3D Flood Fill active.",

            // Guide Card
            "guide_title" => "Topology Guide",
            "guide_item_1" => "• Slice Projection: The viewport displays the 2D layer Z. Navigate depth with Q/E or mouse wheel.",
            "guide_item_2" => "• Möbius Seam: Crossing left/right inverts Y: (H-1)-Y and Z: (D-1)-Z.",
            "guide_item_3" => "• 26-Neighborhood: Numbers range from 0 to 26 based on all adjacent cells across 3D space.",
            "guide_item_4" => "• Safe Opening: First click guarantees a 27-cell clean opening.",

            // Game Results
            "status_ready" => "Click any cell to start (Safe 27-cell opening guaranteed)",
            "status_playing" => "Game in progress...",
            "status_won" => "Victory! Board Cleared!",
            "status_lost" => "Game Over! You hit a mine.",
            "status_spectating" => "Spectating Mode (Eliminated)",

            // Multiplayer Lobby
            "lobby_title" => "Multiplayer Lobby",
            "lobby_create_room" => "Create Room",
            "lobby_join_room" => "Join Room",
            "lobby_room_name" => "Room Name",
            "lobby_enter_code" => "Enter 6-char Room Code",
            "lobby_nickname" => "Player Nickname",
            "lobby_ready" => "Ready",
            "lobby_unready" => "Not Ready",
            "lobby_start" => "Start Match",
            "lobby_waiting_host" => "Waiting for host to start...",
            "lobby_players" => "Players in Room",
            "lobby_public_rooms" => "Active Public Rooms",
            "lobby_no_rooms" => "No active rooms found. Create one to begin!",

            // AI NPC Controls
            "bot_manager_title" => "AI NPC Bots",
            "bot_add_novice" => "+ Novice",
            "bot_add_inter" => "+ Intermediate",
            "bot_add_adv" => "+ Advanced",
            "bot_add_master" => "+ Master",
            "bot_speed_label" => "AI Decision Delay",
            "bot_speed_unit" => "ms",
            "bot_remove" => "Kick",

            // Chat & Logs
            "tab_chat" => "Room Chat",
            "tab_events" => "Event Logs",
            "chat_placeholder" => "Type a message or emoji...",
            "chat_send" => "Send",

            // Custom Dialog
            "custom_title" => "Custom Board Configuration",
            "custom_width" => "Width (X)",
            "custom_height" => "Height (Y)",
            "custom_depth" => "Depth (Z)",
            "custom_mines" => "Mine Count",
            "custom_max_mines" => "Max Allowed Mines",
            "custom_confirm" => "Confirm & Apply",
            "custom_cancel" => "Cancel",
            "custom_total_cells" => "Total Cells: ",
            "custom_safe_opening" => "Safe Opening: ",
            "custom_cells_unit" => "27 cells",

            // Common Buttons & Statuses
            "lobby_status_ready" => "READY",
            "lobby_status_waiting" => "WAITING",
            "bot_speed_hint" => "Fast (0.2s) ──── Slow (20.0s)",
            "btn_edit" => "Edit",
            "btn_join" => "Join",
            "bot_panel_desc" => "Trigger mathematical 3D topological solver to reveal safe cells or place flags:",
            "bot_hint_btn" => "Hint (Highlight)",
            "bot_step_btn" => "Step (Execute)",

            // Auth Modal
            "auth_login_title" => "Account Login",
            "auth_register_title" => "Create Account",
            "auth_username" => "Username",
            "auth_password" => "Password",
            "auth_submit_login" => "Log In",
            "auth_submit_register" => "Sign Up",
            "auth_switch_to_reg" => "Don't have an account? Sign up",
            "auth_switch_to_login" => "Already have an account? Log in",

            // Game Over & Settlement Modal
            "game_over_title_lost" => "💥 ALL PLAYERS ELIMINATED",
            "game_over_title_won" => "🏆 SECTOR CLEARED! VICTORY",
            "game_over_subtitle_lost" => "All squad operatives hit mines in this 3D Möbius space.",
            "game_over_subtitle_won" => "All non-mine cells successfully neutralized!",
            "game_over_leaderboard" => "Match Final Leaderboard",
            "game_over_rank" => "Rank",
            "game_over_player" => "Player",
            "game_over_score" => "Score",
            "game_over_status_survived" => "Survived",
            "game_over_status_eliminated" => "Eliminated",
            "game_over_play_again" => "Play Again",
            "game_over_leave_room" => "Leave Room",
            "game_over_close" => "Inspect Board",
            "game_over_revealed" => "Revealed Cells",
            "game_over_time" => "Time Elapsed",

            _ => key,
        },
        Language::Zh => match key {
            // App Title & Header
            "app_title" => "3D 莫比乌斯环拓扑扫雷",
            "app_subtitle" => "非欧几里得空间立体多人即时扫雷",
            "nav_single" => "单人模式",
            "nav_multi" => "多人即时竞速",
            "nav_stats" => "排行榜与战绩",
            "nav_login" => "登录账号",
            "nav_register" => "注册账号",
            "nav_logout" => "退出登录",
            "nav_guest" => "游客玩家",

            // Difficulties
            "diff_easy" => "初级",
            "diff_medium" => "中级",
            "diff_expert" => "高级",
            "diff_custom" => "自定义",

            // Game Board & HUD
            "hud_time" => "用时",
            "hud_mines" => "剩余雷数",
            "hud_layer" => "当前深度切片 (Z)",
            "hud_pb" => "历史最佳 (PB)",
            "hud_no_pb" => "暂无记录",
            "hud_restart" => "重新开始",
            "hud_room_code" => "房间号",
            "hud_copy_code" => "复制房间号",
            "hud_copied" => "已复制!",
            "hud_score" => "得分",
            "tool_dig" => "⛏️ 翻开模式",
            "tool_flag" => "🚩 插旗模式",
            "hud_pause" => "暂停",
            "hud_resume" => "继续",
            "status_paused" => "游戏已暂停 // 点击或按 [P] 键恢复",
            "mobile_prev_layer" => "◄ 上一层 (Z-)",
            "mobile_next_layer" => "下一层 (Z+) ►",
            "mp_leave_room" => "离开房间",
            "mp_create_match" => "创建对局",

            // Mobius & Slice Guides
            "guide_mobius_left" => "莫比乌斯环左边界 (Y/Z 双重反转)",
            "guide_mobius_right" => "莫比乌斯环右边界 (Y/Z 双重反转)",
            "guide_controls" => "Q/E 或滚轮: 切层 | 左键: 翻格 | 右键: 插旗 | 双击/双键: 快速排雷",
            "global_banner_text" => "3D 莫比乌斯环拓扑：左右跨界执行 Y 与 Z 轴双重反转，立体 26 邻域无缝连锁扩散。",

            // Guide Card
            "guide_title" => "拓扑映射指南",
            "guide_item_1" => "• 纯 2D 切片：视图仅渲染深度 Z 平面，使用 Q/E 或滚轮自由切层。",
            "guide_item_2" => "• 莫比乌斯缝合：穿越左右边界时，Y' = (H-1)-Y 且 Z' = (D-1)-Z。",
            "guide_item_3" => "• 26 邻域雷数：数字范围 0~26，统计立体三维所有有效相邻地雷。",
            "guide_item_4" => "• 首击安全区：首击必出 27 格无雷立体连锁展开。",

            // Game Results
            "status_ready" => "点击任意格子开局（保证首击 27 邻域绝对安全）",
            "status_playing" => "正在扫雷中...",
            "status_won" => "排雷成功！恭喜通关！",
            "status_lost" => "触雷爆炸！本局结束。",
            "status_spectating" => "观战模式（您已触雷出局）",

            // Multiplayer Lobby
            "lobby_title" => "多人即时对战大厅",
            "lobby_create_room" => "创建对战房间",
            "lobby_join_room" => "加入对战房间",
            "lobby_room_name" => "房间名称",
            "lobby_enter_code" => "输入 6 位房间号码",
            "lobby_nickname" => "玩家昵称",
            "lobby_ready" => "准备就绪",
            "lobby_unready" => "取消准备",
            "lobby_start" => "开始对战",
            "lobby_waiting_host" => "等待房主开启对局...",
            "lobby_players" => "房间内玩家",
            "lobby_public_rooms" => "当前活跃房间",
            "lobby_no_rooms" => "暂无活跃房间，立即创建一个吧！",

            // AI NPC Controls
            "bot_manager_title" => "AI NPC 机器人管理",
            "bot_add_novice" => "+ 初级",
            "bot_add_inter" => "+ 中级",
            "bot_add_adv" => "+ 高级",
            "bot_add_master" => "+ 大师",
            "bot_speed_label" => "AI 反应延迟",
            "bot_speed_unit" => "毫秒",
            "bot_remove" => "请出",

            // Chat & Logs
            "tab_chat" => "房间聊天",
            "tab_events" => "对局动态",
            "chat_placeholder" => "输入消息或表情...",
            "chat_send" => "发送",

            // Custom Dialog
            "custom_title" => "自定义棋盘参数",
            "custom_width" => "宽度 (X)",
            "custom_height" => "高度 (Y)",
            "custom_depth" => "深度 (Z)",
            "custom_mines" => "地雷总数",
            "custom_max_mines" => "最大允许雷数",
            "custom_confirm" => "确认并应用",
            "custom_cancel" => "取消",
            "custom_total_cells" => "总格数: ",
            "custom_safe_opening" => "首击安全区: ",
            "custom_cells_unit" => "27 格",

            // Common Buttons & Statuses
            "lobby_status_ready" => "已就绪",
            "lobby_status_waiting" => "等待中",
            "bot_speed_hint" => "极速 (0.2s) ──── 缓慢 (20.0s)",
            "btn_edit" => "修改",
            "btn_join" => "加入",
            "bot_panel_desc" => "呼叫三维莫比乌斯拓扑求解器进行推理走棋或排雷：",
            "bot_hint_btn" => "提示 (仅高亮)",
            "bot_step_btn" => "走棋 (自动执行)",

            // Auth Modal
            "auth_login_title" => "玩家账号登录",
            "auth_register_title" => "新玩家账号注册",
            "auth_username" => "用户名",
            "auth_password" => "密码",
            "auth_submit_login" => "立即登录",
            "auth_submit_register" => "完成注册",
            "auth_switch_to_reg" => "还没有账号？立即注册",
            "auth_switch_to_login" => "已有账号？直接登录",

            // Game Over & Settlement Modal
            "game_over_title_lost" => "💥 全员触雷出局！本局结束",
            "game_over_title_won" => "🏆 雷区已全部排净！胜利结算",
            "game_over_subtitle_lost" => "所有排雷队员均在莫比乌斯环空间内触雷出局。",
            "game_over_subtitle_won" => "已成功完成该莫比乌斯三维空间的全部非雷区翻开！",
            "game_over_leaderboard" => "本局积分排行榜",
            "game_over_rank" => "排名",
            "game_over_player" => "玩家",
            "game_over_score" => "得分",
            "game_over_status_survived" => "存活",
            "game_over_status_eliminated" => "触雷出局",
            "game_over_play_again" => "再来一局",
            "game_over_leave_room" => "离开房间",
            "game_over_close" => "查看棋盘",
            "game_over_revealed" => "已揭开格子",
            "game_over_time" => "对局用时",

            _ => key,
        },
    }
}

pub fn render_system_event(lang: Language, event_key: &str, params: &[String]) -> String {
    match event_key {
        "player_joined" => {
            let u = params.first().map(|s| s.as_str()).unwrap_or("Player");
            format_player_joined_msg(lang, u)
        }
        "player_left" => {
            let u = params.first().map(|s| s.as_str()).unwrap_or("Player");
            format_player_left_msg(lang, u)
        }
        "player_eliminated" => {
            let u = params.first().map(|s| s.as_str()).unwrap_or("Player");
            format_eliminated_msg(lang, u)
        }
        "game_started" => format_game_started_msg(lang),
        "game_over" => {
            let u = params.first().map(|s| s.as_str()).unwrap_or("Winner");
            let score = params
                .get(1)
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            format_game_over_msg(lang, u, score)
        }
        _ => params.first().cloned().unwrap_or_default(),
    }
}

pub fn format_eliminated_msg(lang: Language, username: &str) -> String {
    match lang {
        Language::En => format!("Player [{username}] hit a mine and was eliminated!"),
        Language::Zh => format!("玩家 [{username}] 触雷出局！"),
    }
}

pub fn format_player_joined_msg(lang: Language, username: &str) -> String {
    match lang {
        Language::En => format!("Player [{username}] joined the room."),
        Language::Zh => format!("玩家 [{username}] 加入了房间。"),
    }
}

pub fn format_player_left_msg(lang: Language, username: &str) -> String {
    match lang {
        Language::En => format!("Player [{username}] left the room."),
        Language::Zh => format!("玩家 [{username}] 离开了房间。"),
    }
}

pub fn format_game_started_msg(lang: Language) -> String {
    match lang {
        Language::En => {
            "The battle has begun! Fast-click non-mine cells to earn points!".to_string()
        }
        Language::Zh => "战斗开始！快速翻开非雷格争夺积分！".to_string(),
    }
}

pub fn format_game_over_msg(lang: Language, winner_name: &str, score: u32) -> String {
    match lang {
        Language::En => format!("Game Over! Champion: [{winner_name}] with {score} points!"),
        Language::Zh => format!("对局结束！恭喜获胜者: [{winner_name}]，获得 {score} 分！"),
    }
}
