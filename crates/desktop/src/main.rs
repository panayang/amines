use eframe::egui::{self, Color32, Rect, Stroke, Vec2};
use futures_util::{SinkExt, StreamExt};
use shared::ai_solver::{AiAction, AiSolver, BotTier};
use shared::board::{Board, BoardConfig, Difficulty, GameStatus, LocalPersonalBests, RevealResult};
use shared::i18n::Language;
use shared::protocol::{CellSnapshot, ClientMessage, PlayerInfo, ScoreDelta, ServerMessage};
use shared::topology::Coord3D;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

enum NetToGuiEvent {
    Connected,
    Disconnected(String),
    ServerMsg(ServerMessage),
}

struct DesktopApp {
    mode: AppMode,
    board_config: BoardConfig,
    board: Board,
    current_layer: usize,
    game_start_time: Option<Instant>,
    elapsed_secs: u64,
    moves_count: u32,
    face_state: FaceState,
    is_mouse_down: bool,

    // Host local server
    server_port: u16,
    is_server_running: bool,
    server_handle: Option<tokio::task::JoinHandle<()>>,

    // Online multiplayer
    server_url: String,
    room_code: String,
    player_name: String,
    is_connected: bool,
    in_room: bool,
    is_room_host: bool,
    room_players: Vec<PlayerInfo>,
    multiplayer_scores: Vec<(String, u32)>,
    chat_input: String,
    chat_messages: Vec<String>,
    net_tx: Option<Sender<ClientMessage>>,
    net_rx: Receiver<NetToGuiEvent>,
    mp_ready: bool,
    bot_speed_ms: u64,
    create_room_name: String,
    create_room_diff: Difficulty,
    game_over_settlement: Option<(Vec<String>, Vec<ScoreDelta>)>,

    // Solver & Analysis
    solver_tier: BotTier,
    last_hint: Option<String>,
    hint_coord: Option<Coord3D>,
    hovered_cell: Option<Coord3D>,
    is_paused: bool,

    // Records & Personal Best
    pb_records: LocalPersonalBests,
    show_pb_modal: bool,
    sp_victory_modal: bool,
    is_new_pb_achieved: bool,
    lang: Language,

    // Custom Mode
    is_custom_modal_open: bool,
    custom_w: usize,
    custom_h: usize,
    custom_d: usize,
    custom_m: usize,
    custom_error: Option<String>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum AppMode {
    SinglePlayer,
    Multiplayer,
    HostServer,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum FaceState {
    Normal,
    Won,
    Dead,
}

impl DesktopApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load system fonts for full Unicode, Emoji, and CJK rendering
        setup_system_fonts(&cc.egui_ctx);

        // Setup dark cyber-retro theme visuals in egui
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(12, 8, 24);
        visuals.window_fill = Color32::from_rgb(18, 12, 34);
        visuals.override_text_color = Some(Color32::from_rgb(226, 232, 240));
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(18, 12, 34);
        visuals.widgets.noninteractive.bg_stroke =
            Stroke::new(1.0_f32, Color32::from_rgb(45, 30, 75));
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(28, 18, 52);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(70, 45, 110));
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 28, 88);
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(167, 139, 250));
        visuals.widgets.active.bg_fill = Color32::from_rgb(68, 38, 128);
        visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, Color32::from_rgb(192, 132, 252));
        cc.egui_ctx.set_visuals(visuals);

        let config = BoardConfig::medium();
        let board = Board::new(config);
        let (_tx, net_rx) = channel();

        Self {
            mode: AppMode::SinglePlayer,
            board_config: config,
            board,
            current_layer: config.depth / 2,
            game_start_time: None,
            elapsed_secs: 0,
            moves_count: 0,
            face_state: FaceState::Normal,
            is_mouse_down: false,

            server_port: 3000,
            is_server_running: false,
            server_handle: None,

            server_url: "ws://127.0.0.1:3000/ws".into(),
            room_code: "".into(),
            player_name: format!("RetroOperative_{}", rand::random::<u16>() % 1000),
            is_connected: false,
            in_room: false,
            is_room_host: false,
            room_players: Vec::new(),
            multiplayer_scores: Vec::new(),
            chat_input: "".into(),
            chat_messages: Vec::new(),
            net_tx: None,
            net_rx,
            mp_ready: false,
            bot_speed_ms: 800,
            create_room_name: "Retro Match".into(),
            create_room_diff: Difficulty::Medium,
            game_over_settlement: None,

            solver_tier: BotTier::Master,
            last_hint: None,
            hint_coord: None,
            hovered_cell: None,
            is_paused: false,

            pb_records: LocalPersonalBests::load_or_default(),
            show_pb_modal: false,
            sp_victory_modal: false,
            is_new_pb_achieved: false,
            lang: Language::En,

            is_custom_modal_open: false,
            custom_w: 16,
            custom_h: 16,
            custom_d: 4,
            custom_m: 160,
            custom_error: None,
        }
    }

    fn restart_single_player(&mut self) {
        self.board = Board::new(self.board_config);
        self.game_start_time = None;
        self.elapsed_secs = 0;
        self.moves_count = 0;
        self.face_state = FaceState::Normal;
        self.last_hint = None;
        self.is_paused = false;
        self.sp_victory_modal = false;
        self.is_new_pb_achieved = false;
    }

    fn connect_ws(&mut self) {
        let url_str = self.server_url.clone();
        let (gui_tx, gui_rx) = channel::<NetToGuiEvent>();
        let (net_tx, mut net_rx_cmd) = tokio::sync::mpsc::unbounded_channel::<ClientMessage>();
        let (app_tx, app_rx) = channel::<ClientMessage>();

        std::thread::spawn(move || {
            while let Ok(msg) = app_rx.recv() {
                if net_tx.send(msg).is_err() {
                    break;
                }
            }
        });

        self.net_tx = Some(app_tx);
        self.net_rx = gui_rx;

        tokio::spawn(async move {
            let url = match url::Url::parse(&url_str) {
                Ok(u) => u,
                Err(e) => {
                    let _ = gui_tx.send(NetToGuiEvent::Disconnected(format!("Invalid URL: {e}")));
                    return;
                }
            };

            match connect_async(url).await {
                Ok((ws_stream, _)) => {
                    let _ = gui_tx.send(NetToGuiEvent::Connected);
                    let (mut write, mut read) = ws_stream.split();

                    let gui_tx_read = gui_tx.clone();
                    let read_task = tokio::spawn(async move {
                        while let Some(Ok(msg)) = read.next().await {
                            if let Message::Text(txt) = msg {
                                if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&txt)
                                {
                                    let _ = gui_tx_read.send(NetToGuiEvent::ServerMsg(server_msg));
                                }
                            }
                        }
                    });

                    let write_task = tokio::spawn(async move {
                        while let Some(client_msg) = net_rx_cmd.recv().await {
                            if let Ok(json) = serde_json::to_string(&client_msg) {
                                if write.send(Message::Text(json)).await.is_err() {
                                    break;
                                }
                            }
                        }
                    });

                    tokio::select! {
                        _ = read_task => {},
                        _ = write_task => {},
                    }
                    let _ = gui_tx.send(NetToGuiEvent::Disconnected("Connection closed".into()));
                }
                Err(e) => {
                    let _ =
                        gui_tx.send(NetToGuiEvent::Disconnected(format!("Connect failed: {e}")));
                }
            }
        });
    }

    fn poll_network(&mut self) {
        while let Ok(event) = self.net_rx.try_recv() {
            match event {
                NetToGuiEvent::Connected => {
                    self.is_connected = true;
                    self.chat_messages.push("🟢 Connected to server!".into());
                }
                NetToGuiEvent::Disconnected(reason) => {
                    self.is_connected = false;
                    self.in_room = false;
                    self.chat_messages
                        .push(format!("🔴 Disconnected: {reason}"));
                }
                NetToGuiEvent::ServerMsg(msg) => match msg {
                    ServerMessage::RoomState(snap) => {
                        self.room_code = snap.room_id.clone();
                        self.in_room = true;
                        self.board_config = snap.config;
                        self.board = Board::new(snap.config);
                        self.bot_speed_ms = snap.bot_speed_ms;
                        if let Some(me) =
                            snap.players.iter().find(|p| p.username == self.player_name)
                        {
                            self.mp_ready = me.is_ready;
                            self.is_room_host = me.is_host;
                        }
                        self.room_players = snap.players;
                        self.chat_messages.push(format!(
                            "🏠 Room: {} ({} players)",
                            snap.name,
                            self.room_players.len()
                        ));
                    }
                    ServerMessage::GameStarted { config } => {
                        self.board_config = config;
                        self.board = Board::new(config);
                        self.face_state = FaceState::Normal;
                        self.game_over_settlement = None;
                        self.chat_messages
                            .push("🚀 Multiplayer match started!".into());
                    }
                    ServerMessage::CellsRevealed {
                        revealed,
                        score_deltas,
                    } => {
                        for rev in revealed {
                            if let Some(cell) =
                                self.board.cells.iter_mut().find(|c| c.coord == rev.coord)
                            {
                                cell.is_revealed = true;
                                cell.adjacent_mines = rev.adjacent_mines;
                                cell.is_mine = rev.is_mine;
                            }
                            self.board.revealed_count += 1;
                        }
                        for delta in score_deltas {
                            if let Some(p) = self
                                .multiplayer_scores
                                .iter_mut()
                                .find(|(id, _)| *id == delta.player_id)
                            {
                                p.1 = delta.total_score;
                            } else {
                                self.multiplayer_scores
                                    .push((delta.player_id, delta.total_score));
                            }
                        }
                    }
                    ServerMessage::PlayerEliminated {
                        username,
                        hit_coord,
                        ..
                    } => {
                        self.chat_messages.push(format!(
                            "💥 {username} hit mine at ({},{},{})",
                            hit_coord.x, hit_coord.y, hit_coord.z
                        ));
                    }
                    ServerMessage::PlayerFlagToggled {
                        coord, is_flagged, ..
                    } => {
                        if let Some(cell) = self.board.cells.iter_mut().find(|c| c.coord == coord) {
                            cell.is_flagged = is_flagged;
                            if is_flagged {
                                self.board.flag_count += 1;
                            } else {
                                self.board.flag_count = self.board.flag_count.saturating_sub(1);
                            }
                        }
                    }
                    ServerMessage::ChatMessage(chat) => {
                        self.chat_messages
                            .push(format!("[{}]: {}", chat.username, chat.text));
                    }
                    ServerMessage::GameOver {
                        winners,
                        final_scores,
                    } => {
                        self.face_state = FaceState::Dead;
                        self.game_over_settlement = Some((winners.clone(), final_scores.clone()));
                        self.chat_messages
                            .push(format!("🏁 Game Over! Winner(s): {:?}", winners));
                    }
                    _ => {}
                },
            }
        }
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_network();

        // Global Keyboard Shortcuts in Single Player
        if self.mode == AppMode::SinglePlayer {
            if ctx.input(|i| i.key_pressed(egui::Key::P)) {
                self.is_paused = !self.is_paused;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::R) || i.key_pressed(egui::Key::F2)) {
                self.restart_single_player();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::F3) || i.key_pressed(egui::Key::G)) {
                self.show_pb_modal = !self.show_pb_modal;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::B)) {
                self.trigger_ai_step(self.solver_tier);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Slash)) {
                self.trigger_ai_hint(self.solver_tier);
            }
            if ctx.input(|i| {
                i.key_pressed(egui::Key::PageUp) || i.key_pressed(egui::Key::OpenBracket)
            }) {
                self.current_layer = self.current_layer.saturating_sub(1);
            }
            if (ctx.input(|i| {
                i.key_pressed(egui::Key::PageDown) || i.key_pressed(egui::Key::CloseBracket)
            })) && self.current_layer + 1 < self.board.dims.depth
            {
                self.current_layer += 1;
            }
        } else if self.mode == AppMode::Multiplayer && ctx.input(|i| i.key_pressed(egui::Key::S)) {
            if let Some(tx) = &self.net_tx {
                let _ = tx.send(ClientMessage::StartGame);
            }
        }

        // Update timer and auto-trigger Victory Modal
        if self.mode == AppMode::SinglePlayer {
            if let Some(st) = self.game_start_time {
                if self.board.status == GameStatus::Playing && !self.is_paused {
                    self.elapsed_secs = st.elapsed().as_secs();
                }
            }
            if self.board.status == GameStatus::Won {
                self.face_state = FaceState::Won;
                if !self.sp_victory_modal {
                    self.is_new_pb_achieved = self.pb_records.update_if_best(
                        self.board_config.difficulty,
                        self.elapsed_secs,
                        self.moves_count,
                    );
                    self.sp_victory_modal = true;
                }
            }
        }

        // Top Classic Menu Bar
        egui::TopBottomPanel::top("menu_bar")
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(15, 10, 30))
                    .inner_margin(6.0),
            )
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    let is_zh = self.lang == Language::Zh;
                    ui.menu_button(
                        egui::RichText::new(if is_zh {
                            " 🎮 游戏 "
                        } else {
                            " 🎮 Game "
                        })
                        .strong(),
                        |ui| {
                            if ui
                                .button(if is_zh {
                                    "⚡ 新游戏 (F2)"
                                } else {
                                    "⚡ New Game (F2)"
                                })
                                .clicked()
                            {
                                self.restart_single_player();
                                ui.close_menu();
                            }
                            if ui
                                .button(if is_zh {
                                    "🏆 个人最佳纪录 (F3)"
                                } else {
                                    "🏆 Personal Bests & Records (F3)"
                                })
                                .clicked()
                            {
                                self.show_pb_modal = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui
                                .radio_value(
                                    &mut self.board_config.difficulty,
                                    Difficulty::Easy,
                                    if is_zh {
                                        "🟢 初级 (9x9x3, 25雷)"
                                    } else {
                                        "🟢 Beginner (9x9x3, 25 mines)"
                                    },
                                )
                                .clicked()
                            {
                                self.board_config = BoardConfig::easy();
                                self.restart_single_player();
                                ui.close_menu();
                            }
                            if ui
                                .radio_value(
                                    &mut self.board_config.difficulty,
                                    Difficulty::Medium,
                                    if is_zh {
                                        "🟡 中级 (16x16x4, 160雷)"
                                    } else {
                                        "🟡 Intermediate (16x16x4, 160 mines)"
                                    },
                                )
                                .clicked()
                            {
                                self.board_config = BoardConfig::medium();
                                self.restart_single_player();
                                ui.close_menu();
                            }
                            if ui
                                .radio_value(
                                    &mut self.board_config.difficulty,
                                    Difficulty::Expert,
                                    if is_zh {
                                        "🔴 高级 (30x16x6, 580雷)"
                                    } else {
                                        "🔴 Expert (30x16x6, 580 mines)"
                                    },
                                )
                                .clicked()
                            {
                                self.board_config = BoardConfig::expert();
                                self.restart_single_player();
                                ui.close_menu();
                            }
                            if ui
                                .button(if is_zh {
                                    "⚙️ 自定义网格..."
                                } else {
                                    "⚙️ Custom Grid..."
                                })
                                .clicked()
                            {
                                self.is_custom_modal_open = true;
                                ui.close_menu();
                            }
                            ui.separator();
                            if ui
                                .button(if is_zh { "❌ 退出" } else { "❌ Exit" })
                                .clicked()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        },
                    );

                    ui.menu_button(
                        egui::RichText::new(if is_zh {
                            " 🏆 纪录 "
                        } else {
                            " 🏆 Records "
                        })
                        .strong(),
                        |ui| {
                            if ui
                                .button(if is_zh {
                                    "📊 查看个人最佳排行榜 (F3)"
                                } else {
                                    "📊 View Personal Best High Scores (F3)"
                                })
                                .clicked()
                            {
                                self.show_pb_modal = true;
                                ui.close_menu();
                            }
                        },
                    );

                    ui.menu_button(
                        egui::RichText::new(if is_zh {
                            " 🌐 语言 (Language) "
                        } else {
                            " 🌐 Language "
                        })
                        .strong(),
                        |ui| {
                            if ui
                                .selectable_label(self.lang == Language::En, "🇺🇸 English")
                                .clicked()
                            {
                                self.lang = Language::En;
                                ui.close_menu();
                            }
                            if ui
                                .selectable_label(self.lang == Language::Zh, "🇨🇳 简体中文")
                                .clicked()
                            {
                                self.lang = Language::Zh;
                                ui.close_menu();
                            }
                        },
                    );

                    ui.menu_button(
                        egui::RichText::new(if is_zh {
                            " 🧠 AI 解算器 "
                        } else {
                            " 🧠 AI Solver "
                        })
                        .strong(),
                        |ui| {
                            if ui
                                .button(if is_zh {
                                    "💡 单步提示 / 执行"
                                } else {
                                    "💡 Step Hint / Move"
                                })
                                .clicked()
                            {
                                let snapshots: Vec<CellSnapshot> =
                                    self.board.cells.iter().map(CellSnapshot::from).collect();
                                if let Some(act) = AiSolver::decide_action(
                                    self.board.dims,
                                    &snapshots,
                                    self.solver_tier,
                                    self.board_config.mines,
                                ) {
                                    match act {
                                        AiAction::Reveal(c) => {
                                            self.last_hint = Some(format!(
                                                "Reveal at ({},{},{})",
                                                c.x, c.y, c.z
                                            ));
                                            self.current_layer = c.z;
                                            self.board.reveal(c, None, None);
                                        }
                                        AiAction::Flag(c) => {
                                            self.last_hint = Some(format!(
                                                "Flag mine at ({},{},{})",
                                                c.x, c.y, c.z
                                            ));
                                            self.current_layer = c.z;
                                            self.board.toggle_flag(c);
                                        }
                                        AiAction::Chord(c) => {
                                            self.last_hint = Some(format!(
                                                "Chord safe neighbors at ({},{},{})",
                                                c.x, c.y, c.z
                                            ));
                                            self.current_layer = c.z;
                                            self.board.chord(c, None, None);
                                        }
                                    }
                                } else {
                                    self.last_hint = Some(
                                        if is_zh {
                                            "未找到确定性解"
                                        } else {
                                            "No deterministic moves found"
                                        }
                                        .into(),
                                    );
                                }
                                ui.close_menu();
                            }
                            ui.separator();
                            ui.radio_value(
                                &mut self.solver_tier,
                                BotTier::Novice,
                                "Pascal (Novice Diophantine)",
                            );
                            ui.radio_value(
                                &mut self.solver_tier,
                                BotTier::Intermediate,
                                "Boole (Overlap Bounds)",
                            );
                            ui.radio_value(
                                &mut self.solver_tier,
                                BotTier::Advanced,
                                "Lovelace (Bounded RREF)",
                            );
                            ui.radio_value(
                                &mut self.solver_tier,
                                BotTier::Master,
                                "Turing (RREF + Shannon Entropy)",
                            );
                        },
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} Z = {}/{}",
                                if is_zh {
                                    "3D 莫比乌斯切片"
                                } else {
                                    "3D Möbius Slice"
                                },
                                self.current_layer,
                                self.board.dims.depth - 1
                            ))
                            .color(Color32::from_rgb(167, 139, 250))
                            .strong(),
                        );
                    });
                });
            });

        // Top Navigation Header with Luminescent Glass Tabs
        egui::TopBottomPanel::top("mode_header")
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(18, 12, 34))
                    .inner_margin(8.0),
            )
            .show(ctx, |ui| {
                let is_zh = self.lang == Language::Zh;
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.mode,
                        AppMode::SinglePlayer,
                        if is_zh {
                            " 🕹️ 单人模式 "
                        } else {
                            " 🕹️ Single Player "
                        },
                    );
                    ui.selectable_value(
                        &mut self.mode,
                        AppMode::Multiplayer,
                        if is_zh {
                            " 🌐 多人联机 "
                        } else {
                            " 🌐 Multiplayer Online "
                        },
                    );
                    ui.selectable_value(
                        &mut self.mode,
                        AppMode::HostServer,
                        if is_zh {
                            " 🖥️ 本地独立主机 "
                        } else {
                            " 🖥️ Host Dedicated Server "
                        },
                    );

                    if let Some(hint) = &self.last_hint {
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!(
                                "💡 {}: {hint}",
                                if is_zh { "AI 提示" } else { "AI Hint" }
                            ))
                            .color(Color32::from_rgb(251, 191, 36))
                            .strong(),
                        );
                    }
                });
            });

        // Main Panel
        egui::CentralPanel::default().show(ctx, |ui| match self.mode {
            AppMode::SinglePlayer => {
                self.render_classical_hud(ui);
                ui.add_space(8.0);
                self.render_sp_ai_bot_bar(ui);
                ui.add_space(8.0);
                self.render_layer_bar(ui);
                ui.add_space(8.0);
                self.render_3d_beveled_board(ui);
            }
            AppMode::Multiplayer => {
                self.render_multiplayer_view(ui);
            }
            AppMode::HostServer => {
                self.render_host_server_view(ui);
            }
        });

        // Custom Configuration Modal Dialog
        if self.is_custom_modal_open {
            let is_zh = self.lang == Language::Zh;
            egui::Window::new(if is_zh {
                "⚙️ 自定义 3D 莫比乌斯雷区参数"
            } else {
                "⚙️ Custom 3D Möbius Setup"
            })
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.heading(if is_zh {
                    "配置 3D 莫比乌斯网格"
                } else {
                    "Customize 3D Möbius Grid"
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(if is_zh {
                        "宽度 (X, 4-60):"
                    } else {
                        "Width (X, 4-60):"
                    });
                    ui.add(egui::DragValue::new(&mut self.custom_w).range(4..=60));
                });
                ui.horizontal(|ui| {
                    ui.label(if is_zh {
                        "高度 (Y, 4-40):"
                    } else {
                        "Height (Y, 4-40):"
                    });
                    ui.add(egui::DragValue::new(&mut self.custom_h).range(4..=40));
                });
                ui.horizontal(|ui| {
                    ui.label(if is_zh {
                        "深度 (Z, 1-16):"
                    } else {
                        "Depth (Z, 1-16):"
                    });
                    ui.add(egui::DragValue::new(&mut self.custom_d).range(1..=16));
                });
                ui.horizontal(|ui| {
                    ui.label(if is_zh {
                        "地雷总数 (1-5000):"
                    } else {
                        "Mines Count:"
                    });
                    ui.add(egui::DragValue::new(&mut self.custom_m).range(1..=5000));
                });

                let total = self.custom_w * self.custom_h * self.custom_d;
                let density = if total > 0 {
                    (self.custom_m as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {total} | {}: {density:.1}%",
                        if is_zh { "总格子数" } else { "Total Cells" },
                        if is_zh { "雷区密度" } else { "Density" }
                    ))
                    .color(Color32::from_rgb(167, 139, 250)),
                );

                if let Some(err) = &self.custom_error {
                    ui.colored_label(Color32::RED, err);
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(if is_zh {
                            " ✅ 开始自定义游戏 "
                        } else {
                            " ✅ Start Custom Game "
                        })
                        .clicked()
                    {
                        match BoardConfig::custom(
                            self.custom_w,
                            self.custom_h,
                            self.custom_d,
                            self.custom_m,
                        ) {
                            Ok(cfg) => {
                                self.board_config = cfg;
                                self.current_layer = cfg.depth / 2;
                                self.restart_single_player();
                                self.is_custom_modal_open = false;
                                self.custom_error = None;
                            }
                            Err(e) => {
                                self.custom_error = Some(e);
                            }
                        }
                    }
                    if ui
                        .button(if is_zh { " 取消 " } else { " Cancel " })
                        .clicked()
                    {
                        self.is_custom_modal_open = false;
                        self.custom_error = None;
                    }
                });
            });
        }

        // Personal Bests High Scores Modal Dialog
        if self.show_pb_modal {
            egui::Window::new(if self.lang == Language::Zh {
                "🏆 个人最佳纪录与排行榜 (Personal Bests)"
            } else {
                "🏆 Personal Bests & High Scores"
            })
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.heading(if self.lang == Language::Zh {
                    "📊 3D 莫比乌斯扫雷 - 本地纪录榜"
                } else {
                    "📊 3D Möbius Minesweeper Records"
                });
                ui.add_space(8.0);

                egui::Grid::new("pb_view_grid")
                    .striped(true)
                    .min_col_width(120.0)
                    .show(ui, |ui| {
                        ui.strong(if self.lang == Language::Zh {
                            "难度"
                        } else {
                            "Difficulty"
                        });
                        ui.strong(if self.lang == Language::Zh {
                            "最快耗时"
                        } else {
                            "Best Time"
                        });
                        ui.strong(if self.lang == Language::Zh {
                            "步数"
                        } else {
                            "Moves"
                        });
                        ui.strong(if self.lang == Language::Zh {
                            "达成日期"
                        } else {
                            "Date"
                        });
                        ui.end_row();

                        let diffs = [
                            (
                                Difficulty::Easy,
                                if self.lang == Language::Zh {
                                    "🟢 初级 (9x9x3)"
                                } else {
                                    "🟢 Beginner (9x9x3)"
                                },
                            ),
                            (
                                Difficulty::Medium,
                                if self.lang == Language::Zh {
                                    "🟡 中级 (16x16x4)"
                                } else {
                                    "🟡 Intermediate (16x16x4)"
                                },
                            ),
                            (
                                Difficulty::Expert,
                                if self.lang == Language::Zh {
                                    "🔴 高级 (30x16x6)"
                                } else {
                                    "🔴 Expert (30x16x6)"
                                },
                            ),
                            (
                                Difficulty::Custom,
                                if self.lang == Language::Zh {
                                    "⚙️ 自定义 (Custom)"
                                } else {
                                    "⚙️ Custom"
                                },
                            ),
                        ];

                        for (diff, label) in diffs {
                            ui.label(label);
                            if let Some(pb) = self.pb_records.get_pb(diff) {
                                ui.colored_label(
                                    Color32::from_rgb(34, 197, 94),
                                    format!("{}s", pb.time_secs),
                                );
                                ui.label(format!("{} moves", pb.moves));
                                ui.label(&pb.date);
                            } else {
                                ui.label(if self.lang == Language::Zh {
                                    "暂无纪录"
                                } else {
                                    "No Record"
                                });
                                ui.label("-");
                                ui.label("-");
                            }
                            ui.end_row();
                        }
                    });

                ui.add_space(10.0);
                if ui
                    .button(if self.lang == Language::Zh {
                        " 关闭 (Close) "
                    } else {
                        " Close "
                    })
                    .clicked()
                {
                    self.show_pb_modal = false;
                }
            });
        }

        // Single Player Victory Modal Dialog
        if self.sp_victory_modal {
            egui::Window::new(if self.lang == Language::Zh {
                "🏆 通关胜利！(Mission Complete)"
            } else {
                "🏆 Mission Complete // Victory!"
            })
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading(
                        egui::RichText::new(if self.lang == Language::Zh {
                            "🎉 3D 莫比乌斯全图扫空！🎉"
                        } else {
                            "🎉 3D MÖBIUS CLEARED! 🎉"
                        })
                        .size(22.0)
                        .color(Color32::from_rgb(251, 191, 36)),
                    );
                    if self.is_new_pb_achieved {
                        ui.label(
                            egui::RichText::new(if self.lang == Language::Zh {
                                "⭐ 创造了新的个人最好成绩 (NEW PB)! ⭐"
                            } else {
                                "⭐ NEW PERSONAL BEST ACHIEVED! ⭐"
                            })
                            .color(Color32::from_rgb(34, 197, 94))
                            .strong(),
                        );
                    }
                });
                ui.separator();
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    ui.label(if self.lang == Language::Zh {
                        "⏱️ 最终耗时: "
                    } else {
                        "⏱️ Final Time: "
                    });
                    ui.strong(format!("{}s", self.elapsed_secs));
                    ui.add_space(12.0);
                    ui.label(if self.lang == Language::Zh {
                        "🎯 操作步数: "
                    } else {
                        "🎯 Moves Count: "
                    });
                    ui.strong(format!("{}", self.moves_count));
                });

                ui.add_space(8.0);
                ui.strong(if self.lang == Language::Zh {
                    "📊 个人最佳成绩榜 (Personal Bests):"
                } else {
                    "📊 All-Time Personal Bests:"
                });
                egui::Grid::new("sp_victory_pb_grid")
                    .striped(true)
                    .show(ui, |ui| {
                        let diffs = [
                            (Difficulty::Easy, "Beginner (9x9x3):"),
                            (Difficulty::Medium, "Intermediate (16x16x4):"),
                            (Difficulty::Expert, "Expert (30x16x6):"),
                            (Difficulty::Custom, "Custom:"),
                        ];
                        for (d, label) in diffs {
                            ui.label(label);
                            if let Some(r) = self.pb_records.get_pb(d) {
                                ui.colored_label(
                                    Color32::from_rgb(34, 197, 94),
                                    format!("{}s ({} moves) [{}]", r.time_secs, r.moves, r.date),
                                );
                            } else {
                                ui.label("-");
                            }
                            ui.end_row();
                        }
                    });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(if self.lang == Language::Zh {
                            " 🔄 再来一局 (F2 / R) "
                        } else {
                            " 🔄 Play Again (F2 / R) "
                        })
                        .clicked()
                    {
                        self.restart_single_player();
                    }
                    if ui
                        .button(if self.lang == Language::Zh {
                            " 关闭 "
                        } else {
                            " Dismiss "
                        })
                        .clicked()
                    {
                        self.sp_victory_modal = false;
                    }
                });
            });
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(40));
    }
}

impl DesktopApp {
    /// Renders an authentic retro-modern Minesweeper header with 7-segment digital displays & spring smiley button
    fn render_classical_hud(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(Color32::from_rgb(20, 14, 38))
            .stroke(Stroke::new(1.5_f32, Color32::from_rgb(80, 50, 130)))
            .rounding(6.0)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Left: Mines remaining LED 7-segment display & PB Badge
                    let mines_left = self
                        .board_config
                        .mines
                        .saturating_sub(self.board.flag_count);
                    draw_seven_segment_display(ui, mines_left.min(999) as i32);

                    ui.add_space(8.0);
                    let pb_str =
                        if let Some(rec) = self.pb_records.get_pb(self.board_config.difficulty) {
                            format!("🏆 PB: {}s", rec.time_secs)
                        } else {
                            "🏆 PB: --".to_string()
                        };
                    if ui
                        .button(
                            egui::RichText::new(pb_str)
                                .size(13.0)
                                .color(Color32::from_rgb(251, 191, 36))
                                .strong(),
                        )
                        .clicked()
                    {
                        self.show_pb_modal = true;
                    }

                    // Centering Spacer
                    let remaining_w = ui.available_width() - 150.0;
                    if remaining_w > 0.0 {
                        ui.add_space((remaining_w / 2.0 - 55.0).max(8.0));
                    }

                    // Center: Vector Smiley Button & Pause Button
                    if draw_vector_smiley_button(ui, self.face_state, self.is_mouse_down).clicked()
                    {
                        self.restart_single_player();
                    }

                    ui.add_space(8.0);
                    let is_zh = self.lang == Language::Zh;
                    let pause_label = if self.is_paused {
                        if is_zh {
                            "▶️ 继续 (P)"
                        } else {
                            "▶️ Resume (P)"
                        }
                    } else {
                        if is_zh {
                            "⏸️ 暂停 (P)"
                        } else {
                            "⏸️ Pause (P)"
                        }
                    };
                    if ui.button(pause_label).clicked() {
                        self.is_paused = !self.is_paused;
                    }

                    // Right: Elapsed Time LED 7-segment display
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        draw_seven_segment_display(ui, self.elapsed_secs.min(999) as i32);
                    });
                });
            });
    }

    fn render_sp_ai_bot_bar(&mut self, ui: &mut egui::Ui) {
        let is_zh = self.lang == Language::Zh;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(if is_zh {
                    "💡 AI 单步提示 (高亮):"
                } else {
                    "💡 AI Hint (Highlight):"
                })
                .color(Color32::from_rgb(251, 191, 36))
                .strong(),
            );

            if ui.button("Pascal").clicked() {
                self.trigger_ai_hint(BotTier::Novice);
            }
            if ui.button("Boole").clicked() {
                self.trigger_ai_hint(BotTier::Intermediate);
            }
            if ui.button("Lovelace").clicked() {
                self.trigger_ai_hint(BotTier::Advanced);
            }
            if ui.button("Turing").clicked() {
                self.trigger_ai_hint(BotTier::Master);
            }

            ui.separator();

            ui.label(
                egui::RichText::new(if is_zh {
                    "🤖 AI 自动走子:"
                } else {
                    "🤖 AI Auto-Move:"
                })
                .color(Color32::from_rgb(167, 139, 250))
                .strong(),
            );

            if ui
                .button(if is_zh {
                    "Pascal (初级)"
                } else {
                    "Pascal (Novice)"
                })
                .clicked()
            {
                self.trigger_ai_step(BotTier::Novice);
            }
            if ui
                .button(if is_zh {
                    "Boole (中级)"
                } else {
                    "Boole (Inter)"
                })
                .clicked()
            {
                self.trigger_ai_step(BotTier::Intermediate);
            }
            if ui
                .button(if is_zh {
                    "Lovelace (高级)"
                } else {
                    "Lovelace (Adv)"
                })
                .clicked()
            {
                self.trigger_ai_step(BotTier::Advanced);
            }
            if ui
                .button(if is_zh {
                    "Turing (大师)"
                } else {
                    "Turing (Master)"
                })
                .clicked()
            {
                self.trigger_ai_step(BotTier::Master);
            }
        });
    }

    fn trigger_ai_hint(&mut self, tier: BotTier) {
        let is_zh = self.lang == Language::Zh;
        let snapshots: Vec<CellSnapshot> =
            self.board.cells.iter().map(CellSnapshot::from).collect();
        if let Some(act) =
            AiSolver::decide_action(self.board.dims, &snapshots, tier, self.board_config.mines)
        {
            let c = match act {
                AiAction::Reveal(c) => {
                    self.last_hint = Some(if is_zh {
                        format!("💡 解算: 揭开坐标 ({},{},{})", c.x, c.y, c.z)
                    } else {
                        format!("💡 Solver: REVEAL @ ({},{},{})", c.x, c.y, c.z)
                    });
                    c
                }
                AiAction::Flag(c) => {
                    self.last_hint = Some(if is_zh {
                        format!("🚩 解算: 标旗坐标 ({},{},{})", c.x, c.y, c.z)
                    } else {
                        format!("🚩 Solver: FLAG @ ({},{},{})", c.x, c.y, c.z)
                    });
                    c
                }
                AiAction::Chord(c) => {
                    self.last_hint = Some(if is_zh {
                        format!("⚡ 解算: 双击连开 ({},{},{})", c.x, c.y, c.z)
                    } else {
                        format!("⚡ Solver: CHORD @ ({},{},{})", c.x, c.y, c.z)
                    });
                    c
                }
            };
            self.current_layer = c.z;
            self.hint_coord = Some(c);
        } else {
            self.last_hint = Some(
                if is_zh {
                    "💡 未找到确定性数学解。"
                } else {
                    "💡 No deterministic mathematical moves found."
                }
                .into(),
            );
        }
    }

    fn trigger_ai_step(&mut self, tier: BotTier) {
        let is_zh = self.lang == Language::Zh;
        if self.board.status == GameStatus::Lost || self.board.status == GameStatus::Won {
            return;
        }
        if self.game_start_time.is_none() {
            self.game_start_time = Some(Instant::now());
        }
        let snapshots: Vec<CellSnapshot> =
            self.board.cells.iter().map(CellSnapshot::from).collect();
        if let Some(act) =
            AiSolver::decide_action(self.board.dims, &snapshots, tier, self.board_config.mines)
        {
            match act {
                AiAction::Reveal(c) => {
                    self.last_hint = Some(if is_zh {
                        format!("💡 解算: 揭开坐标 ({},{},{})", c.x, c.y, c.z)
                    } else {
                        format!("💡 Solver: REVEAL @ ({},{},{})", c.x, c.y, c.z)
                    });
                    self.current_layer = c.z;
                    self.hint_coord = Some(c);
                    let res = self.board.reveal(c, None, None);
                    if let RevealResult::HitMine { .. } = res {
                        self.face_state = FaceState::Dead;
                    }
                }
                AiAction::Flag(c) => {
                    self.last_hint = Some(if is_zh {
                        format!("🚩 解算: 标旗坐标 ({},{},{})", c.x, c.y, c.z)
                    } else {
                        format!("🚩 Solver: FLAG @ ({},{},{})", c.x, c.y, c.z)
                    });
                    self.current_layer = c.z;
                    self.hint_coord = Some(c);
                    self.board.toggle_flag(c);
                }
                AiAction::Chord(c) => {
                    self.last_hint = Some(if is_zh {
                        format!("⚡ 解算: 双击连开 ({},{},{})", c.x, c.y, c.z)
                    } else {
                        format!("⚡ Solver: CHORD @ ({},{},{})", c.x, c.y, c.z)
                    });
                    self.current_layer = c.z;
                    self.hint_coord = Some(c);
                    let res = self.board.chord(c, None, None);
                    if let RevealResult::HitMine { .. } = res {
                        self.face_state = FaceState::Dead;
                    }
                }
            }
            if self.board.status == GameStatus::Won {
                self.face_state = FaceState::Won;
            }
        } else {
            self.last_hint = Some(
                if is_zh {
                    "💡 未找到确定性数学解。"
                } else {
                    "💡 No deterministic mathematical moves found."
                }
                .into(),
            );
        }
    }

    fn render_layer_bar(&mut self, ui: &mut egui::Ui) {
        let is_zh = self.lang == Language::Zh;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(if is_zh {
                    "3D 深度切片 (Z):"
                } else {
                    "3D Depth Layer (Z):"
                })
                .color(Color32::from_rgb(167, 139, 250))
                .strong(),
            );

            for z in 0..self.board.dims.depth {
                let is_sel = self.current_layer == z;
                let text = if is_zh {
                    format!(" 第 {} 层 (Z) ", z)
                } else {
                    format!(" Layer Z = {} ", z)
                };
                let btn = egui::Button::new(egui::RichText::new(text).strong())
                    .fill(if is_sel {
                        Color32::from_rgb(124, 58, 237)
                    } else {
                        Color32::from_rgb(25, 15, 45)
                    })
                    .stroke(Stroke::new(
                        1.0_f32,
                        if is_sel {
                            Color32::from_rgb(192, 132, 252)
                        } else {
                            Color32::from_rgb(60, 35, 95)
                        },
                    ))
                    .rounding(4.0);

                if ui.add(btn).clicked() {
                    self.current_layer = z;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(if is_zh {
                        " ⟲ 上一层 (PgUp) "
                    } else {
                        " ⟲ Prev (PgUp) "
                    })
                    .clicked()
                    && self.current_layer > 0
                {
                    self.current_layer -= 1;
                }
                if ui
                    .button(if is_zh {
                        " 下一层 (PgDn) ⟳ "
                    } else {
                        " Next (PgDn) ⟳ "
                    })
                    .clicked()
                    && self.current_layer + 1 < self.board.dims.depth
                {
                    self.current_layer += 1;
                }
            });
        });
    }

    /// Renders genuine 3D beveled tiles with directional lighting and authentic physical relief
    fn render_3d_beveled_board(&mut self, ui: &mut egui::Ui) {
        let dims = self.board.dims;
        let z = self.current_layer;
        let cell_size = 32.0;

        self.is_mouse_down = ui.input(|i| i.pointer.primary_down());

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Left Möbius Guide Ribbon
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("◄ MÖBIUS")
                                .size(10.0)
                                .color(Color32::from_rgb(167, 139, 250))
                                .strong(),
                        );
                        for y in 0..dims.height {
                            let inv_y = (dims.height - 1) - y;
                            ui.label(
                                egui::RichText::new(format!("Y'={inv_y:02}"))
                                    .size(9.0)
                                    .color(Color32::from_rgb(130, 110, 160))
                                    .monospace(),
                            );
                        }
                    });

                    // Left Y-Coordinates Sidebar
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Y")
                                .size(11.0)
                                .color(Color32::from_rgb(192, 132, 252))
                                .strong(),
                        );
                        for y in 0..dims.height {
                            let (rect, _) = ui.allocate_exact_size(
                                Vec2::new(20.0, cell_size),
                                egui::Sense::hover(),
                            );
                            let painter = ui.painter_at(rect);
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                format!("{y:02}"),
                                egui::FontId::monospace(10.0),
                                Color32::from_rgb(148, 163, 184),
                            );
                        }
                    });

                    // Central 3D Minesweeper Matrix (with X coordinate header)
                    ui.vertical(|ui| {
                        // Top X-Coordinates Header Row
                        ui.horizontal(|ui| {
                            for x in 0..dims.width {
                                let (rect, _) = ui.allocate_exact_size(
                                    Vec2::new(cell_size, 18.0),
                                    egui::Sense::hover(),
                                );
                                let painter = ui.painter_at(rect);
                                painter.text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    format!("{x}"),
                                    egui::FontId::monospace(10.0),
                                    Color32::from_rgb(148, 163, 184),
                                );
                            }
                        });

                        // Grid Cells or Anti-Cheat Pause Overlay
                        let is_zh = self.lang == Language::Zh;
                        if self.mode == AppMode::SinglePlayer && self.is_paused {
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(
                                    dims.width as f32 * cell_size,
                                    dims.height as f32 * cell_size,
                                ),
                                egui::Sense::click(),
                            );
                            if response.clicked() {
                                self.is_paused = false;
                            }
                            let painter = ui.painter_at(rect);
                            painter.rect_filled(
                                rect,
                                8.0,
                                Color32::from_rgba_premultiplied(10, 5, 25, 250),
                            );
                            painter.rect_stroke(
                                rect,
                                8.0,
                                Stroke::new(2.0_f32, Color32::from_rgb(167, 139, 250)),
                            );
                            painter.text(
                                rect.center() - Vec2::new(0.0, 16.0),
                                egui::Align2::CENTER_CENTER,
                                if is_zh {
                                    "⏸️ 游戏已暂停"
                                } else {
                                    "⏸️ SIMULATION PAUSED"
                                },
                                egui::FontId::proportional(22.0),
                                Color32::from_rgb(251, 191, 36),
                            );
                            painter.text(
                                rect.center() + Vec2::new(0.0, 16.0),
                                egui::Align2::CENTER_CENTER,
                                if is_zh {
                                    "点击此处或按 [P] 继续游戏"
                                } else {
                                    "Press [P] or Click to Resume"
                                },
                                egui::FontId::monospace(13.0),
                                Color32::from_rgb(226, 232, 240),
                            );
                        } else {
                            for y in 0..dims.height {
                                ui.horizontal(|ui| {
                                    for x in 0..dims.width {
                                        let coord = Coord3D::new(x, y, z);
                                        let is_rev = self.board.get_cell(coord).is_revealed;
                                        let is_hinted = self.hint_coord == Some(coord)
                                            && self.current_layer == z;

                                        let (rect, response) = ui.allocate_exact_size(
                                            Vec2::new(cell_size, cell_size),
                                            egui::Sense::click_and_drag(),
                                        );

                                        if response.hovered() {
                                            self.hovered_cell = Some(coord);
                                        }

                                        let is_game_over = self.board.status == GameStatus::Lost
                                            || self.board.status == GameStatus::Won;

                                        let is_flag_action = response.secondary_clicked()
                                            || response.clicked_by(egui::PointerButton::Secondary)
                                            || (response.hovered()
                                                && ui.input(|i| {
                                                    i.key_pressed(egui::Key::F)
                                                        || i.key_pressed(egui::Key::X)
                                                        || i.pointer.button_clicked(
                                                            egui::PointerButton::Secondary,
                                                        )
                                                }));

                                        let is_reveal_action = response
                                            .clicked_by(egui::PointerButton::Primary)
                                            || (response.hovered()
                                                && ui.input(|i| {
                                                    i.key_pressed(egui::Key::Space)
                                                        || i.key_pressed(egui::Key::Enter)
                                                }))
                                            || (response.clicked() && !is_flag_action);

                                        if !is_game_over {
                                            if is_flag_action {
                                                if self.mode == AppMode::Multiplayer {
                                                    if let Some(tx) = &self.net_tx {
                                                        let _ =
                                                            tx.send(ClientMessage::ToggleFlag {
                                                                coord,
                                                            });
                                                    }
                                                } else {
                                                    self.moves_count += 1;
                                                    self.board.toggle_flag(coord);
                                                }
                                            } else if is_reveal_action {
                                                if self.mode == AppMode::Multiplayer {
                                                    if let Some(tx) = &self.net_tx {
                                                        if is_rev {
                                                            let _ =
                                                                tx.send(ClientMessage::ChordCell {
                                                                    coord,
                                                                });
                                                        } else {
                                                            let _ = tx.send(
                                                                ClientMessage::RevealCell { coord },
                                                            );
                                                        }
                                                    }
                                                } else {
                                                    self.moves_count += 1;
                                                    if self.game_start_time.is_none() {
                                                        self.game_start_time = Some(Instant::now());
                                                    }
                                                    if is_rev {
                                                        self.board.chord(coord, None, None);
                                                    } else {
                                                        let res =
                                                            self.board.reveal(coord, None, None);
                                                        if let RevealResult::HitMine { .. } = res {
                                                            self.face_state = FaceState::Dead;
                                                        }
                                                    }
                                                    if self.board.status == GameStatus::Won {
                                                        self.face_state = FaceState::Won;
                                                        if !self.sp_victory_modal {
                                                            self.is_new_pb_achieved =
                                                                self.pb_records.update_if_best(
                                                                    self.board_config.difficulty,
                                                                    self.elapsed_secs,
                                                                    self.moves_count,
                                                                );
                                                            self.sp_victory_modal = true;
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        let cell = self.board.get_cell(coord);
                                        let painter = ui.painter_at(rect);
                                        draw_beveled_cell(
                                            &painter,
                                            rect,
                                            cell,
                                            response.hovered(),
                                            response.is_pointer_button_down_on(),
                                            is_hinted,
                                        );
                                    }
                                });
                            }
                        }
                    });

                    // Right Möbius Guide Ribbon
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("MÖBIUS ►")
                                .size(10.0)
                                .color(Color32::from_rgb(167, 139, 250))
                                .strong(),
                        );
                        for y in 0..dims.height {
                            let inv_y = (dims.height - 1) - y;
                            ui.label(
                                egui::RichText::new(format!("Y'={inv_y:02}"))
                                    .size(9.0)
                                    .color(Color32::from_rgb(130, 110, 160))
                                    .monospace(),
                            );
                        }
                    });
                });
            });
    }

    fn render_multiplayer_view(&mut self, ui: &mut egui::Ui) {
        let is_zh = self.lang == Language::Zh;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(if is_zh { "服务器地址:" } else { "Server:" }).strong());
            ui.text_edit_singleline(&mut self.server_url);

            ui.label(egui::RichText::new(if is_zh { "代号/昵称:" } else { "Name:" }).strong());
            ui.text_edit_singleline(&mut self.player_name);

            if !self.is_connected {
                if ui
                    .button(if is_zh {
                        " 🔌 连接 WebSocket "
                    } else {
                        " 🔌 Connect WebSocket "
                    })
                    .clicked()
                {
                    self.connect_ws();
                }
            } else {
                ui.label(
                    egui::RichText::new(if is_zh {
                        "🟢 已连接"
                    } else {
                        "🟢 Connected"
                    })
                    .color(Color32::GREEN)
                    .strong(),
                );
            }
        });

        ui.separator();

        if self.is_connected && !self.in_room {
            ui.group(|ui| {
                ui.heading(if is_zh {
                    "➕ 创建新对战房间"
                } else {
                    "➕ Create New Match Room"
                });
                ui.horizontal(|ui| {
                    ui.label(if is_zh { "房间名称:" } else { "Room Name:" });
                    ui.text_edit_singleline(&mut self.create_room_name);

                    egui::ComboBox::from_label(if is_zh { "难度预设" } else { "Difficulty" })
                        .selected_text(match self.create_room_diff {
                            Difficulty::Easy => {
                                if is_zh {
                                    "🟢 初级 (9x9x3, 25雷)"
                                } else {
                                    "Easy (9x9x3, 25 mines)"
                                }
                            }
                            Difficulty::Medium => {
                                if is_zh {
                                    "🟡 中级 (16x16x4, 160雷)"
                                } else {
                                    "Medium (16x16x4, 160 mines)"
                                }
                            }
                            Difficulty::Expert => {
                                if is_zh {
                                    "🔴 高级 (30x16x6, 580雷)"
                                } else {
                                    "Expert (30x16x6, 580 mines)"
                                }
                            }
                            Difficulty::Custom => {
                                if is_zh {
                                    "⚙️ 自定义参数 (Custom)"
                                } else {
                                    "Custom Setup"
                                }
                            }
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.create_room_diff,
                                Difficulty::Easy,
                                if is_zh {
                                    "🟢 初级 (9x9x3, 25雷)"
                                } else {
                                    "Easy (9x9x3, 25)"
                                },
                            );
                            ui.selectable_value(
                                &mut self.create_room_diff,
                                Difficulty::Medium,
                                if is_zh {
                                    "🟡 中级 (16x16x4, 160雷)"
                                } else {
                                    "Medium (16x16x4, 160)"
                                },
                            );
                            ui.selectable_value(
                                &mut self.create_room_diff,
                                Difficulty::Expert,
                                if is_zh {
                                    "🔴 高级 (30x16x6, 580雷)"
                                } else {
                                    "Expert (30x16x6, 580)"
                                },
                            );
                            ui.selectable_value(
                                &mut self.create_room_diff,
                                Difficulty::Custom,
                                if is_zh {
                                    "⚙️ 自定义网格 (Custom)"
                                } else {
                                    "Custom Setup"
                                },
                            );
                        });

                    if ui
                        .button(if is_zh {
                            " 🚀 创建房间 "
                        } else {
                            " 🚀 Create Room "
                        })
                        .clicked()
                    {
                        let config_res = match self.create_room_diff {
                            Difficulty::Easy => Ok(BoardConfig::easy()),
                            Difficulty::Medium => Ok(BoardConfig::medium()),
                            Difficulty::Expert => Ok(BoardConfig::expert()),
                            Difficulty::Custom => BoardConfig::custom(
                                self.custom_w,
                                self.custom_h,
                                self.custom_d,
                                self.custom_m,
                            ),
                        };
                        match config_res {
                            Ok(config) => {
                                self.custom_error = None;
                                if let Some(tx) = &self.net_tx {
                                    let _ = tx.send(ClientMessage::CreateRoom {
                                        name: self.create_room_name.clone(),
                                        config,
                                        username: self.player_name.clone(),
                                        token: None,
                                    });
                                }
                            }
                            Err(e) => {
                                self.custom_error = Some(e);
                            }
                        }
                    }
                });

                if self.create_room_diff == Difficulty::Custom {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(if is_zh { "宽度(X):" } else { "Width(X):" });
                        ui.add(egui::DragValue::new(&mut self.custom_w).range(4..=60));
                        ui.label(if is_zh { "高度(Y):" } else { "Height(Y):" });
                        ui.add(egui::DragValue::new(&mut self.custom_h).range(4..=40));
                        ui.label(if is_zh { "层数(Z):" } else { "Depth(Z):" });
                        ui.add(egui::DragValue::new(&mut self.custom_d).range(1..=16));
                        ui.label(if is_zh { "地雷数:" } else { "Mines:" });
                        ui.add(egui::DragValue::new(&mut self.custom_m).range(1..=5000));
                    });
                    if let Some(err) = &self.custom_error {
                        ui.colored_label(Color32::RED, err);
                    }
                }
            });

            ui.add_space(8.0);

            ui.group(|ui| {
                ui.heading(if is_zh {
                    "🚪 加入已有房间"
                } else {
                    "🚪 Join Existing Room"
                });
                ui.horizontal(|ui| {
                    ui.label(if is_zh { "房间代码:" } else { "Room Code:" });
                    ui.text_edit_singleline(&mut self.room_code);
                    if ui
                        .button(if is_zh {
                            " 🚪 加入房间 "
                        } else {
                            " 🚪 Join Room "
                        })
                        .clicked()
                        && !self.room_code.is_empty()
                    {
                        if let Some(tx) = &self.net_tx {
                            let _ = tx.send(ClientMessage::JoinRoom {
                                room_id: self.room_code.clone(),
                                username: self.player_name.clone(),
                                token: None,
                            });
                        }
                    }
                });
            });
        }

        if self.in_room {
            ui.horizontal(|ui| {
                ui.heading(format!(
                    "{}: {}",
                    if is_zh {
                        "🏠 房间代码"
                    } else {
                        "🏠 Room Code"
                    },
                    self.room_code
                ));
                ui.add_space(8.0);

                let launch_btn = egui::Button::new(
                    egui::RichText::new(if is_zh {
                        " 🚀 发射开局 [S] "
                    } else {
                        " 🚀 LAUNCH MATCH [S] "
                    })
                    .color(Color32::from_rgb(16, 24, 16))
                    .strong(),
                )
                .fill(Color32::from_rgb(34, 197, 94));
                if ui.add(launch_btn).clicked() {
                    if let Some(tx) = &self.net_tx {
                        let _ = tx.send(ClientMessage::StartGame);
                    }
                }

                let ready_text = if self.mp_ready {
                    if is_zh {
                        "⏳ 取消准备"
                    } else {
                        "⏳ Set Unready"
                    }
                } else {
                    if is_zh {
                        "✅ 设为准备"
                    } else {
                        "✅ Set Ready"
                    }
                };
                if ui.button(ready_text).clicked() {
                    self.mp_ready = !self.mp_ready;
                    if let Some(tx) = &self.net_tx {
                        let _ = tx.send(ClientMessage::SetReady {
                            ready: self.mp_ready,
                        });
                    }
                }
                if ui
                    .button(if is_zh {
                        "🚪 离开房间"
                    } else {
                        "🚪 Leave Room"
                    })
                    .clicked()
                {
                    if let Some(tx) = &self.net_tx {
                        let _ = tx.send(ClientMessage::LeaveRoom);
                    }
                    self.in_room = false;
                }
            });

            ui.columns(2, |cols| {
                cols[0].group(|ui| {
                    self.render_layer_bar(ui);
                    ui.add_space(6.0);
                    self.render_3d_beveled_board(ui);
                });

                cols[1].vertical(|ui| {
                    // AI NPC Manager Panel
                    ui.group(|ui| {
                        ui.heading(if is_zh {
                            "🤖 AI 机器人管理"
                        } else {
                            "🤖 AI NPC Manager"
                        });
                        ui.label(
                            egui::RichText::new(if is_zh {
                                "向多人对战房间添加 AI 智能体:"
                            } else {
                                "Add AI bots to this multiplayer match:"
                            })
                            .size(11.0)
                            .color(Color32::GRAY),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("+ Pascal").clicked() {
                                if let Some(tx) = &self.net_tx {
                                    let _ = tx.send(ClientMessage::AddBot {
                                        tier: BotTier::Novice,
                                        speed_ms: Some(self.bot_speed_ms),
                                    });
                                }
                            }
                            if ui.button("+ Boole").clicked() {
                                if let Some(tx) = &self.net_tx {
                                    let _ = tx.send(ClientMessage::AddBot {
                                        tier: BotTier::Intermediate,
                                        speed_ms: Some(self.bot_speed_ms),
                                    });
                                }
                            }
                            if ui.button("+ Lovelace").clicked() {
                                if let Some(tx) = &self.net_tx {
                                    let _ = tx.send(ClientMessage::AddBot {
                                        tier: BotTier::Advanced,
                                        speed_ms: Some(self.bot_speed_ms),
                                    });
                                }
                            }
                            if ui.button("+ Turing").clicked() {
                                if let Some(tx) = &self.net_tx {
                                    let _ = tx.send(ClientMessage::AddBot {
                                        tier: BotTier::Master,
                                        speed_ms: Some(self.bot_speed_ms),
                                    });
                                }
                            }
                        });

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(if is_zh {
                                "思考延迟 (ms):"
                            } else {
                                "Bot Speed (ms):"
                            });
                            if ui
                                .add(egui::Slider::new(&mut self.bot_speed_ms, 200..=5000))
                                .changed()
                            {
                                if let Some(tx) = &self.net_tx {
                                    let _ = tx.send(ClientMessage::UpdateBotSpeed {
                                        speed_ms: self.bot_speed_ms,
                                    });
                                }
                            }
                        });

                        // List active bots with kick button
                        let bots: Vec<PlayerInfo> = self
                            .room_players
                            .iter()
                            .filter(|p| p.is_bot)
                            .cloned()
                            .collect();
                        if !bots.is_empty() {
                            ui.separator();
                            ui.label(if is_zh {
                                "房间内活跃 AI 机器人:"
                            } else {
                                "Active Bots in Room:"
                            });
                            for bot in bots {
                                ui.horizontal(|ui| {
                                    ui.label(format!("🤖 {} ({:?})", bot.username, bot.bot_tier));
                                    if ui
                                        .button(if is_zh { "❌ 踢出" } else { "❌ Kick" })
                                        .clicked()
                                    {
                                        if let Some(tx) = &self.net_tx {
                                            let _ = tx.send(ClientMessage::RemoveBot {
                                                bot_id: bot.id.clone(),
                                            });
                                        }
                                    }
                                });
                            }
                        }
                    });

                    ui.add_space(6.0);
                    ui.heading(if is_zh {
                        "🏆 实时积分排行榜"
                    } else {
                        "🏆 Live Leaderboard"
                    });
                    for (player_id, score) in &self.multiplayer_scores {
                        ui.label(
                            egui::RichText::new(format!("👤 {player_id}: {score} pts"))
                                .color(Color32::from_rgb(251, 191, 36))
                                .strong(),
                        );
                    }

                    ui.separator();
                    ui.heading(if is_zh {
                        "💬 房间战术聊天"
                    } else {
                        "💬 Room Chat"
                    });
                    egui::ScrollArea::vertical()
                        .max_height(160.0)
                        .show(ui, |ui| {
                            for msg in &self.chat_messages {
                                ui.label(egui::RichText::new(msg).size(12.0));
                            }
                        });

                    ui.horizontal(|ui| {
                        let text_resp = ui.text_edit_singleline(&mut self.chat_input);
                        let is_enter =
                            text_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if (ui.button(if is_zh { "发送" } else { "Send" }).clicked() || is_enter)
                            && !self.chat_input.is_empty()
                        {
                            if let Some(tx) = &self.net_tx {
                                let _ = tx.send(ClientMessage::SendChat {
                                    text: self.chat_input.clone(),
                                });
                            }
                            self.chat_input.clear();
                        }
                    });
                });
            });

            // Settlement Leaderboard Modal
            let mut close_settlement = false;
            if let Some((winners, final_scores)) = &self.game_over_settlement {
                egui::Window::new(if self.lang == Language::Zh {
                    "🏁 对战结算排行榜 (Match Settlement)"
                } else {
                    "🏁 Match Settlement & Leaderboard"
                })
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading(
                            egui::RichText::new(format!(
                                "🏆 {}: {}",
                                if is_zh { "优胜者" } else { "Winner(s)" },
                                winners.join(", ")
                            ))
                            .size(18.0)
                            .color(Color32::from_rgb(251, 191, 36)),
                        );
                    });
                    ui.separator();
                    ui.add_space(4.0);

                    egui::Grid::new("mp_settlement_grid")
                        .striped(true)
                        .min_col_width(90.0)
                        .show(ui, |ui| {
                            ui.strong(if self.lang == Language::Zh {
                                "排名"
                            } else {
                                "Rank"
                            });
                            ui.strong(if self.lang == Language::Zh {
                                "玩家 / 智能体"
                            } else {
                                "Operative"
                            });
                            ui.strong(if self.lang == Language::Zh {
                                "总积分"
                            } else {
                                "Total Score"
                            });
                            ui.strong(if self.lang == Language::Zh {
                                "本局增减"
                            } else {
                                "Points Delta"
                            });
                            ui.end_row();

                            for (idx, delta) in final_scores.iter().enumerate() {
                                let rank_badge = match idx {
                                    0 => "🥇 1st",
                                    1 => "🥈 2nd",
                                    2 => "🥉 3rd",
                                    _ => "  -  ",
                                };
                                ui.label(rank_badge);
                                ui.label(&delta.player_id);
                                ui.label(format!("{} pts", delta.total_score));
                                ui.colored_label(
                                    Color32::from_rgb(34, 197, 94),
                                    format!("+{} pts", delta.points),
                                );
                                ui.end_row();
                            }
                        });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let launch_next_btn = egui::Button::new(
                            egui::RichText::new(if self.lang == Language::Zh {
                                " 🚀 开启下一局 [S] "
                            } else {
                                " 🚀 Launch Next Round [S] "
                            })
                            .color(Color32::from_rgb(16, 24, 16))
                            .strong(),
                        )
                        .fill(Color32::from_rgb(34, 197, 94));
                        if ui.add(launch_next_btn).clicked() {
                            if let Some(tx) = &self.net_tx {
                                let _ = tx.send(ClientMessage::StartGame);
                            }
                            close_settlement = true;
                        }
                        if ui
                            .button(if self.lang == Language::Zh {
                                " 关闭结算 "
                            } else {
                                " Close Settlement "
                            })
                            .clicked()
                        {
                            close_settlement = true;
                        }
                    });
                });
            }
            if close_settlement {
                self.game_over_settlement = None;
            }
        }
    }

    fn render_host_server_view(&mut self, ui: &mut egui::Ui) {
        let is_zh = self.lang == Language::Zh;
        ui.heading(if is_zh {
            "🖥️ 内置 3D 莫比乌斯扫雷服务端"
        } else {
            "🖥️ Embedded 3D Möbius Minesweeper Server"
        });
        ui.label(if is_zh {
            "直接在此桌面应用程序内托管专用的本地多人对战服务器。"
        } else {
            "Host a dedicated local multiplayer server directly from this desktop application."
        });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(if is_zh {
                    "服务端端口:"
                } else {
                    "Server Port:"
                })
                .strong(),
            );
            ui.add(egui::DragValue::new(&mut self.server_port));

            if !self.is_server_running {
                if ui
                    .button(if is_zh {
                        " 🚀 启动本地服务端 "
                    } else {
                        " 🚀 Start Local Server "
                    })
                    .clicked()
                {
                    let port = self.server_port;
                    self.is_server_running = true;
                    self.server_handle = Some(tokio::spawn(async move {
                        let _ = server::run_server(port, "minesweeper_desktop.db", None).await;
                    }));
                }
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "{} 0.0.0.0:{}",
                        if is_zh {
                            "🟢 服务端运行在"
                        } else {
                            "🟢 Server running on"
                        },
                        self.server_port
                    ))
                    .color(Color32::GREEN)
                    .strong(),
                );
                if ui
                    .button(if is_zh {
                        " 🛑 停止服务端 "
                    } else {
                        " 🛑 Stop Server "
                    })
                    .clicked()
                {
                    if let Some(h) = self.server_handle.take() {
                        h.abort();
                    }
                    self.is_server_running = false;
                }
            }
        });
    }
}

/// Helper function to draw authentic 7-segment digital display in egui
fn draw_seven_segment_display(ui: &mut egui::Ui, value: i32) {
    let text = format!("{:03}", value.clamp(0, 999));
    ui.label(
        egui::RichText::new(text)
            .font(egui::FontId::monospace(26.0))
            .color(Color32::from_rgb(255, 45, 60))
            .background_color(Color32::from_rgb(8, 4, 14)),
    );
}

/// Custom painter for genuine 3D beveled Minesweeper tile with specular highlights
fn draw_vector_smiley_button(
    ui: &mut egui::Ui,
    face_state: FaceState,
    is_mouse_down: bool,
) -> egui::Response {
    let desired_size = Vec2::new(48.0, 48.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    let painter = ui.painter_at(rect);

    let is_pressed = response.is_pointer_button_down_on() || (is_mouse_down && response.hovered());

    // Cyber classic button bevel & backdrop
    let bg_color = if is_pressed {
        Color32::from_rgb(35, 20, 60)
    } else if response.hovered() {
        Color32::from_rgb(55, 30, 95)
    } else {
        Color32::from_rgb(45, 25, 75)
    };
    painter.rect_filled(rect, 8.0, bg_color);
    painter.rect_stroke(
        rect,
        8.0,
        Stroke::new(
            2.0_f32,
            if response.hovered() {
                Color32::from_rgb(251, 191, 36)
            } else {
                Color32::from_rgb(167, 139, 250)
            },
        ),
    );

    let center = rect.center();
    let face_r = 15.0;

    // Iconic Yellow Smiley Face Base
    painter.circle_filled(center, face_r, Color32::from_rgb(255, 215, 0));
    painter.circle_stroke(
        center,
        face_r,
        Stroke::new(1.5_f32, Color32::from_rgb(40, 20, 0)),
    );

    match face_state {
        FaceState::Normal => {
            if is_mouse_down {
                // Surprised 😮: Big round eyes and round open mouth
                painter.circle_filled(center + Vec2::new(-5.0, -3.5), 2.2, Color32::BLACK);
                painter.circle_filled(center + Vec2::new(5.0, -3.5), 2.2, Color32::BLACK);
                painter.circle_filled(center + Vec2::new(0.0, 5.0), 3.2, Color32::BLACK);
            } else {
                // Happy 🙂: Two eye dots and upward curved smile
                painter.circle_filled(center + Vec2::new(-4.5, -3.5), 1.8, Color32::BLACK);
                painter.circle_filled(center + Vec2::new(4.5, -3.5), 1.8, Color32::BLACK);

                let mut path = Vec::new();
                for i in 0..=8 {
                    let t = i as f32 / 8.0;
                    let x = -6.0 + 12.0 * t;
                    let y = 3.0 + 4.0 * (1.0 - (2.0 * t - 1.0).powi(2));
                    path.push(center + Vec2::new(x, y));
                }
                for i in 0..path.len() - 1 {
                    painter
                        .line_segment([path[i], path[i + 1]], Stroke::new(2.0_f32, Color32::BLACK));
                }
            }
        }
        FaceState::Won => {
            // Cool with Sunglasses 😎
            painter.rect_filled(
                egui::Rect::from_min_size(center + Vec2::new(-9.0, -6.0), Vec2::new(7.5, 6.0)),
                2.0,
                Color32::BLACK,
            );
            painter.rect_filled(
                egui::Rect::from_min_size(center + Vec2::new(1.5, -6.0), Vec2::new(7.5, 6.0)),
                2.0,
                Color32::BLACK,
            );
            painter.line_segment(
                [
                    center + Vec2::new(-1.5, -4.0),
                    center + Vec2::new(1.5, -4.0),
                ],
                Stroke::new(2.0_f32, Color32::BLACK),
            );

            let mut path = Vec::new();
            for i in 0..=8 {
                let t = i as f32 / 8.0;
                let x = -6.0 + 12.0 * t;
                let y = 4.0 + 3.0 * (1.0 - (2.0 * t - 1.0).powi(2));
                path.push(center + Vec2::new(x, y));
            }
            for i in 0..path.len() - 1 {
                painter.line_segment([path[i], path[i + 1]], Stroke::new(2.0_f32, Color32::BLACK));
            }
        }
        FaceState::Dead => {
            // Dead 😵: X eyes and sad frowning mouth
            let eye_l = center + Vec2::new(-4.5, -4.0);
            painter.line_segment(
                [eye_l + Vec2::new(-2.5, -2.5), eye_l + Vec2::new(2.5, 2.5)],
                Stroke::new(1.8_f32, Color32::BLACK),
            );
            painter.line_segment(
                [eye_l + Vec2::new(-2.5, 2.5), eye_l + Vec2::new(2.5, -2.5)],
                Stroke::new(1.8_f32, Color32::BLACK),
            );

            let eye_r = center + Vec2::new(4.5, -4.0);
            painter.line_segment(
                [eye_r + Vec2::new(-2.5, -2.5), eye_r + Vec2::new(2.5, 2.5)],
                Stroke::new(1.8_f32, Color32::BLACK),
            );
            painter.line_segment(
                [eye_r + Vec2::new(-2.5, 2.5), eye_r + Vec2::new(2.5, -2.5)],
                Stroke::new(1.8_f32, Color32::BLACK),
            );

            let mut path = Vec::new();
            for i in 0..=8 {
                let t = i as f32 / 8.0;
                let x = -6.0 + 12.0 * t;
                let y = 7.0 - 4.0 * (1.0 - (2.0 * t - 1.0).powi(2));
                path.push(center + Vec2::new(x, y));
            }
            for i in 0..path.len() - 1 {
                painter.line_segment([path[i], path[i + 1]], Stroke::new(2.0_f32, Color32::BLACK));
            }
        }
    }

    response
}

fn draw_beveled_cell(
    painter: &egui::Painter,
    rect: Rect,
    cell: &shared::board::Cell,
    is_hovered: bool,
    is_pressed: bool,
    is_hinted: bool,
) {
    let bevel_w = 2.5_f32;

    if !cell.is_revealed {
        // Raised 3D Beveled Tile
        let bg_color = if is_pressed {
            Color32::from_rgb(35, 20, 60)
        } else if is_hovered {
            Color32::from_rgb(55, 32, 95)
        } else {
            Color32::from_rgb(45, 25, 80)
        };

        // Base plate
        painter.rect_filled(rect, 0.0, bg_color);

        if !is_pressed {
            // Light top and left edges (highlight)
            let highlight = Color32::from_rgb(140, 100, 220);
            painter.line_segment(
                [rect.left_top(), rect.right_top()],
                Stroke::new(bevel_w, highlight),
            );
            painter.line_segment(
                [rect.left_top(), rect.left_bottom()],
                Stroke::new(bevel_w, highlight),
            );

            // Dark bottom and right edges (shadow)
            let shadow = Color32::from_rgb(20, 10, 40);
            painter.line_segment(
                [rect.right_top(), rect.right_bottom()],
                Stroke::new(bevel_w, shadow),
            );
            painter.line_segment(
                [rect.left_bottom(), rect.right_bottom()],
                Stroke::new(bevel_w, shadow),
            );
        }

        if cell.is_flagged {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "🚩",
                egui::FontId::proportional(17.0),
                Color32::from_rgb(255, 50, 50),
            );
        }
    } else {
        // Sunken Revealed Cell Plate
        let bg_color = if cell.is_mine {
            Color32::from_rgb(180, 20, 30)
        } else {
            Color32::from_rgb(22, 14, 40)
        };

        painter.rect_filled(rect, 0.0, bg_color);

        // Inset border shadow
        let inset_stroke = Stroke::new(1.0_f32, Color32::from_rgb(12, 8, 25));
        painter.rect_stroke(rect, 0.0, inset_stroke);

        if cell.is_mine {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "💣",
                egui::FontId::proportional(17.0),
                Color32::WHITE,
            );
        } else if cell.adjacent_mines > 0 {
            let (num_str, color) = match cell.adjacent_mines {
                1 => ("1", Color32::from_rgb(96, 165, 250)), // Neon Blue
                2 => ("2", Color32::from_rgb(52, 211, 153)), // Emerald
                3 => ("3", Color32::from_rgb(248, 113, 113)), // Coral Red
                4 => ("4", Color32::from_rgb(192, 132, 252)), // Purple
                5 => ("5", Color32::from_rgb(251, 146, 60)), // Orange
                6 => ("6", Color32::from_rgb(45, 212, 191)), // Teal
                7 => ("7", Color32::WHITE),                  // White
                _ => ("8", Color32::from_rgb(244, 114, 182)), // Pink
            };

            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                num_str,
                egui::FontId::monospace(18.0),
                color,
            );
        }
    }

    if is_hinted {
        painter.rect_stroke(
            rect.expand(1.5),
            2.0,
            Stroke::new(2.5_f32, Color32::from_rgb(251, 191, 36)),
        );
    }
}

/// Automatically detects and mounts system Unicode, Emoji, Math symbols, and CJK fonts into egui
fn setup_system_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let font_candidates = [
        // High Quality Standalone TTF & OTF Fonts
        "/usr/share/fonts/google-droid-sans-fonts/DroidSansFallbackFull.ttf",
        "/usr/share/fonts/fandol/FandolHei-Regular.otf",
        "/usr/share/fonts/fandol/FandolSong-Regular.otf",
        "/usr/share/fonts/google-noto/NotoSansMath-Regular.ttf",
        "/usr/share/fonts/google-noto/NotoSansSymbols-Regular.ttf",
        "/usr/share/fonts/google-noto/NotoSansSymbols2-Regular.ttf",
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        // Windows & macOS system fonts (TTF/OTF)
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    ];

    let mut loaded_idx = 0;
    for path in font_candidates {
        if let Ok(data) = std::fs::read(path) {
            let font_name = format!("sys_font_{loaded_idx}");
            fonts
                .font_data
                .insert(font_name.clone(), egui::FontData::from_owned(data));
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                family.push(font_name.clone());
            }
            if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
                family.push(font_name);
            }
            loaded_idx += 1;
        }
    }

    ctx.set_fonts(fonts);
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 780.0])
            .with_title("3D Möbius Minesweeper - Cyber-Classic Desktop Edition"),
        ..Default::default()
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();

    eframe::run_native(
        "3D Möbius Minesweeper",
        native_options,
        Box::new(|cc| Ok(Box::new(DesktopApp::new(cc)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desktop_font_and_layout_render() {
        let ctx = egui::Context::default();
        setup_system_fonts(&ctx);

        let (net_tx, _net_rx) = std::sync::mpsc::channel();
        let (_gui_tx, gui_rx) = std::sync::mpsc::channel();

        let mut app = DesktopApp {
            mode: AppMode::SinglePlayer,
            board: Board::new(BoardConfig::easy()),
            board_config: BoardConfig::easy(),
            custom_w: 12,
            custom_h: 12,
            custom_d: 3,
            custom_m: 40,
            custom_error: None,
            is_custom_modal_open: false,
            current_layer: 0,
            elapsed_secs: 0,
            is_paused: false,
            face_state: FaceState::Normal,
            is_mouse_down: false,
            moves_count: 0,
            last_hint: None,
            hint_coord: None,
            solver_tier: BotTier::Master,
            net_tx: Some(net_tx),
            net_rx: gui_rx,
            is_connected: false,
            server_port: 8080,
            is_server_running: false,
            server_handle: None,
            server_url: "ws://127.0.0.1:8080".to_string(),
            in_room: false,
            room_code: "".to_string(),
            player_name: "TestUser".to_string(),
            mp_ready: false,
            is_room_host: false,
            room_players: Vec::new(),
            create_room_name: "Test Room".to_string(),
            create_room_diff: Difficulty::Easy,
            chat_input: "".to_string(),
            chat_messages: Vec::new(),
            multiplayer_scores: Vec::new(),
            game_over_settlement: None,
            bot_speed_ms: 1000,
            hovered_cell: None,
            pb_records: LocalPersonalBests::default(),
            show_pb_modal: false,
            sp_victory_modal: false,
            is_new_pb_achieved: false,
            lang: Language::Zh,
            game_start_time: None,
        };

        // Run a test frame
        let _output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.render_classical_hud(ui);
                app.render_sp_ai_bot_bar(ui);
                app.render_layer_bar(ui);
                app.render_3d_beveled_board(ui);
            });
        });
    }
}
