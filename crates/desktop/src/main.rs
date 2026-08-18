use eframe::egui::{self, Color32, Rect, Stroke, Vec2};
use futures_util::{SinkExt, StreamExt};
use shared::ai_solver::{AiAction, AiSolver, BotTier};
use shared::board::{Board, BoardConfig, Difficulty, GameStatus, RevealResult};
use shared::protocol::{CellSnapshot, ClientMessage, PlayerInfo, ServerMessage};
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

    // Solver & Analysis
    solver_tier: BotTier,
    last_hint: Option<String>,
    hint_coord: Option<Coord3D>,
    hovered_cell: Option<Coord3D>,

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

            solver_tier: BotTier::Master,
            last_hint: None,
            hint_coord: None,
            hovered_cell: None,

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
        self.face_state = FaceState::Normal;
        self.last_hint = None;
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
                    ServerMessage::ChatMessage(chat) => {
                        self.chat_messages
                            .push(format!("[{}]: {}", chat.username, chat.text));
                    }
                    ServerMessage::GameOver { winners, .. } => {
                        self.face_state = FaceState::Dead;
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

        // Update timer
        if let Some(st) = self.game_start_time {
            if self.board.status == GameStatus::Playing {
                self.elapsed_secs = st.elapsed().as_secs();
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
                    ui.menu_button(egui::RichText::new(" 🎮 Game ").strong(), |ui| {
                        if ui.button("⚡ New Game (F2)").clicked() {
                            self.restart_single_player();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui
                            .radio_value(
                                &mut self.board_config.difficulty,
                                Difficulty::Easy,
                                "🟢 Beginner (9x9x3, 25 mines)",
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
                                "🟡 Intermediate (16x16x4, 160 mines)",
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
                                "🔴 Expert (30x16x6, 580 mines)",
                            )
                            .clicked()
                        {
                            self.board_config = BoardConfig::expert();
                            self.restart_single_player();
                            ui.close_menu();
                        }
                        if ui.button("⚙️ Custom Grid (自定义)...").clicked() {
                            self.is_custom_modal_open = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("❌ Exit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });

                    ui.menu_button(egui::RichText::new(" 🧠 AI Solver ").strong(), |ui| {
                        if ui.button("💡 Step Hint / Move").clicked() {
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
                                        self.last_hint =
                                            Some(format!("Reveal at ({},{},{})", c.x, c.y, c.z));
                                        self.current_layer = c.z;
                                        self.board.reveal(c, None, None);
                                    }
                                    AiAction::Flag(c) => {
                                        self.last_hint =
                                            Some(format!("Flag mine at ({},{},{})", c.x, c.y, c.z));
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
                                self.last_hint = Some("No deterministic moves found".into());
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
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "3D Möbius Slice Z = {}/{}",
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
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.mode,
                        AppMode::SinglePlayer,
                        " 🕹️ Single Player ",
                    );
                    ui.selectable_value(
                        &mut self.mode,
                        AppMode::Multiplayer,
                        " 🌐 Multiplayer Online ",
                    );
                    ui.selectable_value(
                        &mut self.mode,
                        AppMode::HostServer,
                        " 🖥️ Host Dedicated Server ",
                    );

                    if let Some(hint) = &self.last_hint {
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!("💡 AI Hint: {hint}"))
                                .color(Color32::from_rgb(251, 191, 36))
                                .strong(),
                        );
                    }
                });
            });

        // Main Panel
        egui::CentralPanel::default().show(ctx, |ui| match self.mode {
            AppMode::SinglePlayer => {
                self.render_classical_hud_frame(ui);
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
            egui::Window::new("⚙️ Custom 3D Möbius Setup")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.heading("Customize 3D Möbius Grid");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Width (X, 4-60):");
                        ui.add(egui::DragValue::new(&mut self.custom_w).range(4..=60));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Height (Y, 4-40):");
                        ui.add(egui::DragValue::new(&mut self.custom_h).range(4..=40));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Depth (Z, 1-16):");
                        ui.add(egui::DragValue::new(&mut self.custom_d).range(1..=16));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Mines Count:");
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
                            "Total Cells: {total} | Density: {density:.1}%"
                        ))
                        .color(Color32::from_rgb(167, 139, 250)),
                    );

                    if let Some(err) = &self.custom_error {
                        ui.colored_label(Color32::RED, err);
                    }

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button(" ✅ Start Custom Game ").clicked() {
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
                        if ui.button(" Cancel ").clicked() {
                            self.is_custom_modal_open = false;
                            self.custom_error = None;
                        }
                    });
                });
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(40));
    }
}

impl DesktopApp {
    /// Renders an authentic retro-modern Minesweeper header with 7-segment digital displays & spring smiley button
    fn render_classical_hud_frame(&mut self, ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(Color32::from_rgb(20, 14, 38))
            .stroke(Stroke::new(1.5_f32, Color32::from_rgb(80, 50, 130)))
            .rounding(6.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Left: Mines remaining LED 7-segment display
                    let mines_left = self
                        .board_config
                        .mines
                        .saturating_sub(self.board.flag_count);
                    draw_seven_segment_display(ui, mines_left.min(999) as i32);

                    ui.add_space(ui.available_width() / 2.0 - 32.0);

                    // Center: High-relief Smiley button with spring physical click
                    let face_icon = match self.face_state {
                        FaceState::Normal => {
                            if self.is_mouse_down {
                                "😮"
                            } else {
                                "🙂"
                            }
                        }
                        FaceState::Won => "😎",
                        FaceState::Dead => "😵",
                    };

                    let smiley_btn = egui::Button::new(
                        egui::RichText::new(face_icon)
                            .size(28.0)
                            .color(Color32::YELLOW),
                    )
                    .fill(Color32::from_rgb(45, 25, 75))
                    .stroke(Stroke::new(2.0_f32, Color32::from_rgb(167, 139, 250)))
                    .min_size(Vec2::new(48.0, 48.0))
                    .rounding(24.0);

                    if ui.add(smiley_btn).clicked() {
                        self.restart_single_player();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Right: Elapsed Time LED 7-segment display
                        draw_seven_segment_display(ui, self.elapsed_secs.min(999) as i32);
                    });
                });
            });
    }

    fn render_sp_ai_bot_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("💡 AI Hint (Highlight):")
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
                egui::RichText::new("🤖 AI Auto-Move:")
                    .color(Color32::from_rgb(167, 139, 250))
                    .strong(),
            );

            if ui.button("Pascal (Novice)").clicked() {
                self.trigger_ai_step(BotTier::Novice);
            }
            if ui.button("Boole (Inter)").clicked() {
                self.trigger_ai_step(BotTier::Intermediate);
            }
            if ui.button("Lovelace (Adv)").clicked() {
                self.trigger_ai_step(BotTier::Advanced);
            }
            if ui.button("Turing (Master)").clicked() {
                self.trigger_ai_step(BotTier::Master);
            }
        });
    }

    fn trigger_ai_hint(&mut self, tier: BotTier) {
        let snapshots: Vec<CellSnapshot> =
            self.board.cells.iter().map(CellSnapshot::from).collect();
        if let Some(act) =
            AiSolver::decide_action(self.board.dims, &snapshots, tier, self.board_config.mines)
        {
            let c = match act {
                AiAction::Reveal(c) => {
                    self.last_hint = Some(format!("💡 Solver: REVEAL @ ({},{},{})", c.x, c.y, c.z));
                    c
                }
                AiAction::Flag(c) => {
                    self.last_hint = Some(format!("🚩 Solver: FLAG @ ({},{},{})", c.x, c.y, c.z));
                    c
                }
                AiAction::Chord(c) => {
                    self.last_hint = Some(format!("⚡ Solver: CHORD @ ({},{},{})", c.x, c.y, c.z));
                    c
                }
            };
            self.current_layer = c.z;
            self.hint_coord = Some(c);
        } else {
            self.last_hint = Some("💡 No deterministic mathematical moves found.".into());
        }
    }

    fn trigger_ai_step(&mut self, tier: BotTier) {
        let snapshots: Vec<CellSnapshot> =
            self.board.cells.iter().map(CellSnapshot::from).collect();
        if let Some(act) =
            AiSolver::decide_action(self.board.dims, &snapshots, tier, self.board_config.mines)
        {
            match act {
                AiAction::Reveal(c) => {
                    self.last_hint = Some(format!("Reveal @ ({},{},{})", c.x, c.y, c.z));
                    self.current_layer = c.z;
                    self.hint_coord = Some(c);
                    self.board.reveal(c, None, None);
                }
                AiAction::Flag(c) => {
                    self.last_hint = Some(format!("Flag @ ({},{},{})", c.x, c.y, c.z));
                    self.current_layer = c.z;
                    self.hint_coord = Some(c);
                    self.board.toggle_flag(c);
                }
                AiAction::Chord(c) => {
                    self.last_hint = Some(format!("Chord @ ({},{},{})", c.x, c.y, c.z));
                    self.current_layer = c.z;
                    self.hint_coord = Some(c);
                    self.board.chord(c, None, None);
                }
            }
        }
    }

    fn render_layer_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("3D Depth Layer (Z):")
                    .color(Color32::from_rgb(167, 139, 250))
                    .strong(),
            );

            for z in 0..self.board.dims.depth {
                let is_sel = self.current_layer == z;
                let text = format!(" Layer Z = {} ", z);
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
                if ui.button(" ⟲ Prev (PgUp) ").clicked() && self.current_layer > 0 {
                    self.current_layer -= 1;
                }
                if ui.button(" Next (PgDn) ⟳ ").clicked()
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

                        // Grid Cells
                        for y in 0..dims.height {
                            ui.horizontal(|ui| {
                                for x in 0..dims.width {
                                    let coord = Coord3D::new(x, y, z);
                                    let is_rev = self.board.get_cell(coord).is_revealed;
                                    let is_hinted =
                                        self.hint_coord == Some(coord) && self.current_layer == z;

                                    let (rect, response) = ui.allocate_exact_size(
                                        Vec2::new(cell_size, cell_size),
                                        egui::Sense::click(),
                                    );

                                    if response.hovered() {
                                        self.hovered_cell = Some(coord);
                                    }

                                    let is_game_over = self.board.status == GameStatus::Lost
                                        || self.board.status == GameStatus::Won;

                                    if !is_game_over {
                                        if response.clicked() {
                                            if self.game_start_time.is_none() {
                                                self.game_start_time = Some(Instant::now());
                                            }
                                            if is_rev {
                                                self.board.chord(coord, None, None);
                                            } else {
                                                let res = self.board.reveal(coord, None, None);
                                                if let RevealResult::HitMine { .. } = res {
                                                    self.face_state = FaceState::Dead;
                                                }
                                            }
                                            if self.board.status == GameStatus::Won {
                                                self.face_state = FaceState::Won;
                                            }
                                        } else if response.secondary_clicked() {
                                            self.board.toggle_flag(coord);
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
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Server:").strong());
            ui.text_edit_singleline(&mut self.server_url);

            ui.label(egui::RichText::new("Name:").strong());
            ui.text_edit_singleline(&mut self.player_name);

            if !self.is_connected {
                if ui.button(" 🔌 Connect WebSocket ").clicked() {
                    self.connect_ws();
                }
            } else {
                ui.label(
                    egui::RichText::new("🟢 Connected")
                        .color(Color32::GREEN)
                        .strong(),
                );
            }
        });

        ui.separator();

        if self.is_connected && !self.in_room {
            ui.horizontal(|ui| {
                if ui.button(" ➕ Create Match Room ").clicked() {
                    if let Some(tx) = &self.net_tx {
                        let _ = tx.send(ClientMessage::CreateRoom {
                            name: format!("{}'s Room", self.player_name),
                            config: self.board_config,
                            username: self.player_name.clone(),
                            token: None,
                        });
                    }
                }

                ui.separator();

                ui.label("Room Code:");
                ui.text_edit_singleline(&mut self.room_code);
                if ui.button(" 🚪 Join Room ").clicked() && !self.room_code.is_empty() {
                    if let Some(tx) = &self.net_tx {
                        let _ = tx.send(ClientMessage::JoinRoom {
                            room_id: self.room_code.clone(),
                            username: self.player_name.clone(),
                            token: None,
                        });
                    }
                }
            });
        }

        if self.in_room {
            ui.horizontal(|ui| {
                ui.heading(format!("🏠 Room Code: {}", self.room_code));
                if self.is_room_host && ui.button(" ▶️ Start Game ").clicked() {
                    if let Some(tx) = &self.net_tx {
                        let _ = tx.send(ClientMessage::StartGame);
                    }
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
                        ui.heading("🤖 AI NPC Manager");
                        ui.label(
                            egui::RichText::new("Add AI bots to this multiplayer match:")
                                .size(11.0)
                                .color(Color32::GRAY),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("+ Pascal (Novice)").clicked() {
                                if let Some(tx) = &self.net_tx {
                                    let _ = tx.send(ClientMessage::AddBot {
                                        tier: BotTier::Novice,
                                        speed_ms: None,
                                    });
                                }
                            }
                            if ui.button("+ Boole (Inter)").clicked() {
                                if let Some(tx) = &self.net_tx {
                                    let _ = tx.send(ClientMessage::AddBot {
                                        tier: BotTier::Intermediate,
                                        speed_ms: None,
                                    });
                                }
                            }
                            if ui.button("+ Turing (Master)").clicked() {
                                if let Some(tx) = &self.net_tx {
                                    let _ = tx.send(ClientMessage::AddBot {
                                        tier: BotTier::Master,
                                        speed_ms: None,
                                    });
                                }
                            }
                        });
                    });

                    ui.add_space(6.0);
                    ui.heading("🏆 Live Leaderboard");
                    for (player_id, score) in &self.multiplayer_scores {
                        ui.label(
                            egui::RichText::new(format!("👤 {player_id}: {score} pts"))
                                .color(Color32::from_rgb(251, 191, 36))
                                .strong(),
                        );
                    }

                    ui.separator();
                    ui.heading("💬 Room Chat");
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for msg in &self.chat_messages {
                                ui.label(egui::RichText::new(msg).size(12.0));
                            }
                        });

                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.chat_input);
                        if ui.button("Send").clicked() && !self.chat_input.is_empty() {
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
        }
    }

    fn render_host_server_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("🖥️ Embedded 3D Möbius Minesweeper Server");
        ui.label(
            "Host a dedicated local multiplayer server directly from this desktop application.",
        );

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Server Port:").strong());
            ui.add(egui::DragValue::new(&mut self.server_port));

            if !self.is_server_running {
                if ui.button(" 🚀 Start Local Server ").clicked() {
                    let port = self.server_port;
                    self.is_server_running = true;
                    self.server_handle = Some(tokio::spawn(async move {
                        let _ = server::run_server(port, "minesweeper_desktop.db", None).await;
                    }));
                }
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "🟢 Server running on 0.0.0.0:{}",
                        self.server_port
                    ))
                    .color(Color32::GREEN)
                    .strong(),
                );
                if ui.button(" 🛑 Stop Server ").clicked() {
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
