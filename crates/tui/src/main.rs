use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::{SinkExt, StreamExt};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use shared::ai_solver::{AiAction, AiSolver, BotTier};
use shared::board::{Board, BoardConfig, GameStatus, RevealResult};
use shared::protocol::{CellSnapshot, ClientMessage, PlayerInfo, ServerMessage};
use shared::topology::Coord3D;
use std::io::stdout;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

enum TuiNetEvent {
    Connected,
    Disconnected(String),
    ServerMsg(ServerMessage),
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum TuiMode {
    SinglePlayer,
    Multiplayer,
    HostServer,
}

struct TuiApp {
    mode: TuiMode,
    board_config: BoardConfig,
    board: Board,
    cursor: Coord3D,
    current_layer: usize,
    game_start_time: Option<Instant>,
    elapsed_secs: u64,
    status_msg: String,

    // Host Server
    server_running: bool,
    server_port: u16,
    server_handle: Option<tokio::task::JoinHandle<()>>,

    // Multiplayer Online
    server_url: String,
    room_code: String,
    player_name: String,
    is_connected: bool,
    in_room: bool,
    room_players: Vec<PlayerInfo>,
    multiplayer_scores: Vec<(String, u32)>,
    logs: Vec<String>,
    net_tx: Option<tokio::sync::mpsc::UnboundedSender<ClientMessage>>,
    net_rx: Receiver<TuiNetEvent>,

    // Solver Recommendation
    last_hint: Option<String>,

    // Custom Mode
    is_custom_modal: bool,
    custom_w: usize,
    custom_h: usize,
    custom_d: usize,
    custom_m: usize,
    custom_field: usize, // 0: W, 1: H, 2: D, 3: M
    custom_error: Option<String>,

    // Help & Keybindings Manual Modal
    show_help_modal: bool,
}

impl TuiApp {
    fn new(net_rx: Receiver<TuiNetEvent>) -> Self {
        let config = BoardConfig::medium();
        let board = Board::new(config);
        Self {
            mode: TuiMode::SinglePlayer,
            board_config: config,
            board,
            cursor: Coord3D::new(config.width / 2, config.height / 2, config.depth / 2),
            current_layer: config.depth / 2,
            game_start_time: None,
            elapsed_secs: 0,
            status_msg: "SYSTEM READY // Press [F1] or [?] for Full Keymap Guide.".into(),

            server_running: false,
            server_port: 3000,
            server_handle: None,

            server_url: "ws://127.0.0.1:3000/ws".into(),
            room_code: "".into(),
            player_name: format!("TUI_{}", rand::random::<u16>() % 1000),
            is_connected: false,
            in_room: false,
            room_players: Vec::new(),
            multiplayer_scores: Vec::new(),
            logs: vec![
                "[SYS] Terminal initialized.".into(),
                "[SYS] 3D Möbius topology active.".into(),
            ],
            net_tx: None,
            net_rx,
            last_hint: None,

            is_custom_modal: false,
            custom_w: 16,
            custom_h: 16,
            custom_d: 4,
            custom_m: 160,
            custom_field: 0,
            custom_error: None,

            show_help_modal: false,
        }
    }

    fn restart_game(&mut self) {
        self.board = Board::new(self.board_config);
        self.game_start_time = None;
        self.elapsed_secs = 0;
        self.status_msg = "Game re-initialized. Ready.".into();
        self.last_hint = None;
    }

    fn connect_ws(&mut self) {
        let url_str = self.server_url.clone();
        let (gui_tx, gui_rx) = channel::<TuiNetEvent>();
        let (net_tx, mut net_rx_cmd) = tokio::sync::mpsc::unbounded_channel::<ClientMessage>();

        self.net_tx = Some(net_tx);
        self.net_rx = gui_rx;

        tokio::spawn(async move {
            let url = match url::Url::parse(&url_str) {
                Ok(u) => u,
                Err(e) => {
                    let _ = gui_tx.send(TuiNetEvent::Disconnected(format!("Invalid URL: {e}")));
                    return;
                }
            };

            match connect_async(url).await {
                Ok((ws_stream, _)) => {
                    let _ = gui_tx.send(TuiNetEvent::Connected);
                    let (mut write, mut read) = ws_stream.split();

                    let gui_tx_read = gui_tx.clone();
                    let read_task = tokio::spawn(async move {
                        while let Some(Ok(msg)) = read.next().await {
                            if let Message::Text(txt) = msg {
                                if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&txt)
                                {
                                    let _ = gui_tx_read.send(TuiNetEvent::ServerMsg(server_msg));
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
                    let _ = gui_tx.send(TuiNetEvent::Disconnected("Connection closed".into()));
                }
                Err(e) => {
                    let _ = gui_tx.send(TuiNetEvent::Disconnected(format!("Connect failed: {e}")));
                }
            }
        });
    }

    fn poll_network(&mut self) {
        while let Ok(event) = self.net_rx.try_recv() {
            match event {
                TuiNetEvent::Connected => {
                    self.is_connected = true;
                    self.logs
                        .push("🟢 [NET] Connected to WebSocket Server!".into());
                }
                TuiNetEvent::Disconnected(reason) => {
                    self.is_connected = false;
                    self.in_room = false;
                    self.logs.push(format!("🔴 [NET] Disconnected: {reason}"));
                }
                TuiNetEvent::ServerMsg(msg) => match msg {
                    ServerMessage::RoomState(snap) => {
                        self.room_code = snap.room_id.clone();
                        self.in_room = true;
                        self.board_config = snap.config;
                        self.board = Board::new(snap.config);
                        self.room_players = snap.players;
                        self.logs.push(format!(
                            "🏠 [ROOM] Room: {} ({} players)",
                            snap.name,
                            self.room_players.len()
                        ));
                    }
                    ServerMessage::GameStarted { config } => {
                        self.board_config = config;
                        self.board = Board::new(config);
                        self.logs
                            .push("🚀 [MATCH] Multiplayer game started!".into());
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
                        self.logs.push(format!(
                            "💥 [ELIM] {username} hit mine at ({},{},{})",
                            hit_coord.x, hit_coord.y, hit_coord.z
                        ));
                    }
                    ServerMessage::GameOver { winners, .. } => {
                        self.logs
                            .push(format!("🏁 [END] Game Over! Winner(s): {:?}", winners));
                    }
                    ServerMessage::ChatMessage(chat) => {
                        self.logs
                            .push(format!("💬 [CHAT] {}: {}", chat.username, chat.text));
                    }
                    _ => {}
                },
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (_tx, net_rx) = channel();
    let mut app = TuiApp::new(net_rx);

    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(40);

    loop {
        app.poll_network();

        if let Some(st) = app.game_start_time {
            if app.board.status == GameStatus::Playing {
                app.elapsed_secs = st.elapsed().as_secs();
            }
        }

        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.show_help_modal {
                        match key.code {
                            KeyCode::Esc
                            | KeyCode::F(1)
                            | KeyCode::Char('?')
                            | KeyCode::Char('k')
                            | KeyCode::Char('K')
                            | KeyCode::Char('h')
                            | KeyCode::Char('H')
                            | KeyCode::Char('q')
                            | KeyCode::Char('Q')
                            | KeyCode::Enter
                            | KeyCode::Char(' ') => {
                                app.show_help_modal = false;
                            }
                            _ => {
                                app.show_help_modal = false;
                            }
                        }
                    } else if app.is_custom_modal {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                                app.is_custom_modal = false;
                                app.custom_error = None;
                            }
                            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => {
                                if app.custom_field > 0 {
                                    app.custom_field -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') => {
                                if app.custom_field < 3 {
                                    app.custom_field += 1;
                                }
                            }
                            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('-') => {
                                match app.custom_field {
                                    0 => app.custom_w = app.custom_w.saturating_sub(1).max(4),
                                    1 => app.custom_h = app.custom_h.saturating_sub(1).max(4),
                                    2 => app.custom_d = app.custom_d.saturating_sub(1).max(1),
                                    _ => app.custom_m = app.custom_m.saturating_sub(5).max(1),
                                }
                            }
                            KeyCode::Right
                            | KeyCode::Char('d')
                            | KeyCode::Char('+')
                            | KeyCode::Char('=') => match app.custom_field {
                                0 => app.custom_w = (app.custom_w + 1).min(60),
                                1 => app.custom_h = (app.custom_h + 1).min(40),
                                2 => app.custom_d = (app.custom_d + 1).min(16),
                                _ => app.custom_m = (app.custom_m + 5).min(5000),
                            },
                            KeyCode::Enter => {
                                match BoardConfig::custom(
                                    app.custom_w,
                                    app.custom_h,
                                    app.custom_d,
                                    app.custom_m,
                                ) {
                                    Ok(cfg) => {
                                        app.board_config = cfg;
                                        app.current_layer = cfg.depth / 2;
                                        app.restart_game();
                                        app.is_custom_modal = false;
                                        app.custom_error = None;
                                        app.status_msg = format!(
                                            "Custom 3D Game ({:?}) Ready.",
                                            (cfg.width, cfg.height, cfg.depth, cfg.mines)
                                        );
                                    }
                                    Err(e) => {
                                        app.custom_error = Some(e);
                                    }
                                }
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => break,
                            KeyCode::F(1)
                            | KeyCode::Char('?')
                            | KeyCode::Char('k')
                            | KeyCode::Char('K') => {
                                app.show_help_modal = true;
                            }
                            KeyCode::Char('m') | KeyCode::Char('M') => {
                                app.mode = match app.mode {
                                    TuiMode::SinglePlayer => TuiMode::Multiplayer,
                                    TuiMode::Multiplayer => TuiMode::HostServer,
                                    TuiMode::HostServer => TuiMode::SinglePlayer,
                                };
                            }
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                app.restart_game();
                            }
                            KeyCode::Char('1') => {
                                app.board_config = BoardConfig::easy();
                                app.restart_game();
                                app.status_msg =
                                    "Switched to [1] Beginner (9x9x3, 25 mines).".into();
                            }
                            KeyCode::Char('2') => {
                                app.board_config = BoardConfig::medium();
                                app.restart_game();
                                app.status_msg =
                                    "Switched to [2] Intermediate (16x16x4, 160 mines).".into();
                            }
                            KeyCode::Char('3') => {
                                app.board_config = BoardConfig::expert();
                                app.restart_game();
                                app.status_msg =
                                    "Switched to [3] Expert (30x16x6, 580 mines).".into();
                            }
                            KeyCode::Char('4') | KeyCode::Char('u') | KeyCode::Char('U') => {
                                app.is_custom_modal = true;
                                app.status_msg = "CUSTOM SETUP // [W/S] Field, [A/D] Adjust, [Enter] Apply, [Esc] Cancel".into();
                            }
                            KeyCode::Up | KeyCode::Char('w') => {
                                if app.cursor.y > 0 {
                                    app.cursor.y -= 1;
                                }
                            }
                            KeyCode::Down | KeyCode::Char('s') => {
                                if app.cursor.y + 1 < app.board.dims.height {
                                    app.cursor.y += 1;
                                }
                            }
                            KeyCode::Left | KeyCode::Char('a') => {
                                if app.cursor.x > 0 {
                                    app.cursor.x -= 1;
                                }
                            }
                            KeyCode::Right | KeyCode::Char('d') => {
                                if app.cursor.x + 1 < app.board.dims.width {
                                    app.cursor.x += 1;
                                }
                            }
                            KeyCode::PageUp | KeyCode::Char('[') | KeyCode::Char('<') => {
                                if app.current_layer > 0 {
                                    app.current_layer -= 1;
                                    app.cursor.z = app.current_layer;
                                }
                            }
                            KeyCode::PageDown | KeyCode::Char(']') | KeyCode::Char('>') => {
                                if app.current_layer + 1 < app.board.dims.depth {
                                    app.current_layer += 1;
                                    app.cursor.z = app.current_layer;
                                }
                            }
                            KeyCode::Enter | KeyCode::Char(' ') => {
                                if app.board.status == GameStatus::Lost
                                    || app.board.status == GameStatus::Won
                                {
                                    app.status_msg =
                                        "Game Over! Press [R] to start a new game.".into();
                                } else {
                                    if app.game_start_time.is_none() {
                                        app.game_start_time = Some(Instant::now());
                                    }
                                    let c = app.cursor;
                                    let cell = app.board.get_cell(c);
                                    if cell.is_revealed {
                                        app.board.chord(c, None, None);
                                    } else {
                                        let res = app.board.reveal(c, None, None);
                                        if let RevealResult::HitMine { .. } = res {
                                            app.status_msg =
                                                "💥 [DETONATION] Mine hit! Press [R] to restart."
                                                    .into();
                                        }
                                    }
                                    if app.board.status == GameStatus::Won {
                                        app.status_msg =
                                            "🏆 [VICTORY] All non-mine cells cleared!".into();
                                    }
                                }
                            }
                            KeyCode::Char('f') | KeyCode::Char('F') => {
                                if app.board.status == GameStatus::Lost
                                    || app.board.status == GameStatus::Won
                                {
                                    app.status_msg =
                                        "Game Over! Press [R] to start a new game.".into();
                                } else {
                                    app.board.toggle_flag(app.cursor);
                                }
                            }
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                if app.board.status == GameStatus::Lost
                                    || app.board.status == GameStatus::Won
                                {
                                    app.status_msg =
                                        "Game Over! Press [R] to start a new game.".into();
                                } else {
                                    app.board.chord(app.cursor, None, None);
                                }
                            }
                            KeyCode::Char('/') => {
                                let snapshots: Vec<CellSnapshot> =
                                    app.board.cells.iter().map(CellSnapshot::from).collect();
                                if let Some(act) = AiSolver::decide_action(
                                    app.board.dims,
                                    &snapshots,
                                    BotTier::Master,
                                    app.board_config.mines,
                                ) {
                                    match act {
                                        AiAction::Reveal(coord) => {
                                            app.last_hint = Some(format!(
                                                "REVEAL @ ({},{},{})",
                                                coord.x, coord.y, coord.z
                                            ));
                                            app.status_msg = format!(
                                                "💡 Solver: REVEAL @ ({},{},{})",
                                                coord.x, coord.y, coord.z
                                            );
                                            app.cursor = coord;
                                            app.current_layer = coord.z;
                                        }
                                        AiAction::Flag(coord) => {
                                            app.last_hint = Some(format!(
                                                "FLAG @ ({},{},{})",
                                                coord.x, coord.y, coord.z
                                            ));
                                            app.status_msg = format!(
                                                "💡 Solver: FLAG @ ({},{},{})",
                                                coord.x, coord.y, coord.z
                                            );
                                            app.cursor = coord;
                                            app.current_layer = coord.z;
                                        }
                                        AiAction::Chord(coord) => {
                                            app.last_hint = Some(format!(
                                                "CHORD @ ({},{},{})",
                                                coord.x, coord.y, coord.z
                                            ));
                                            app.status_msg = format!(
                                                "💡 Solver: CHORD @ ({},{},{})",
                                                coord.x, coord.y, coord.z
                                            );
                                            app.cursor = coord;
                                            app.current_layer = coord.z;
                                        }
                                    }
                                } else {
                                    app.status_msg =
                                        "💡 No deterministic mathematical moves found.".into();
                                }
                            }
                            // Host Server trigger in Host Mode or Help modal in other modes
                            KeyCode::Char('h') | KeyCode::Char('H') => {
                                if app.mode == TuiMode::HostServer {
                                    if !app.server_running {
                                        let port = app.server_port;
                                        app.server_running = true;
                                        app.server_handle = Some(tokio::spawn(async move {
                                            let _ = server::run_server(
                                                port,
                                                "minesweeper_tui.db",
                                                None,
                                            )
                                            .await;
                                        }));
                                        app.status_msg =
                                            format!("Local Server ONLINE -> 0.0.0.0:{port}");
                                    } else {
                                        if let Some(h) = app.server_handle.take() {
                                            h.abort();
                                        }
                                        app.server_running = false;
                                        app.status_msg = "Local Server STOPPED.".into();
                                    }
                                } else {
                                    app.show_help_modal = true;
                                }
                            }
                            // Connect in MP mode
                            KeyCode::Char('n') | KeyCode::Char('N') => {
                                if app.mode == TuiMode::Multiplayer && !app.is_connected {
                                    app.connect_ws();
                                    app.status_msg = "Connecting to WebSocket server...".into();
                                }
                            }
                            // Create room in MP mode
                            KeyCode::Char('p') | KeyCode::Char('P') => {
                                if app.mode == TuiMode::Multiplayer
                                    && app.is_connected
                                    && !app.in_room
                                {
                                    if let Some(tx) = &app.net_tx {
                                        let _ = tx.send(ClientMessage::CreateRoom {
                                            name: format!("{}'s TUI Room", app.player_name),
                                            config: app.board_config,
                                            username: app.player_name.clone(),
                                            token: None,
                                        });
                                    }
                                }
                            }
                            // Add AI Bot (Novice / Master) in MP or execute AI move in SP
                            KeyCode::Char('b') | KeyCode::Char('B') => {
                                if app.mode == TuiMode::Multiplayer
                                    && app.is_connected
                                    && app.in_room
                                {
                                    if let Some(tx) = &app.net_tx {
                                        let _ = tx.send(ClientMessage::AddBot {
                                            tier: BotTier::Master,
                                            speed_ms: None,
                                        });
                                        app.status_msg =
                                            "🤖 Added Master AI Bot (Turing) to room!".into();
                                    }
                                } else if app.mode == TuiMode::SinglePlayer {
                                    if app.board.status == GameStatus::Lost
                                        || app.board.status == GameStatus::Won
                                    {
                                        app.status_msg =
                                            "Game Over! Press [R] to start a new game.".into();
                                    } else {
                                        let snapshots: Vec<CellSnapshot> = app
                                            .board
                                            .cells
                                            .iter()
                                            .map(CellSnapshot::from)
                                            .collect();
                                        if let Some(act) = AiSolver::decide_action(
                                            app.board.dims,
                                            &snapshots,
                                            BotTier::Master,
                                            app.board_config.mines,
                                        ) {
                                            match act {
                                                AiAction::Reveal(c) => {
                                                    app.board.reveal(c, None, None);
                                                    app.status_msg = format!(
                                                        "🤖 Bot executed REVEAL @ ({},{},{})",
                                                        c.x, c.y, c.z
                                                    );
                                                }
                                                AiAction::Flag(c) => {
                                                    app.board.toggle_flag(c);
                                                    app.status_msg = format!(
                                                        "🤖 Bot placed FLAG @ ({},{},{})",
                                                        c.x, c.y, c.z
                                                    );
                                                }
                                                AiAction::Chord(c) => {
                                                    app.board.chord(c, None, None);
                                                    app.status_msg = format!(
                                                        "🤖 Bot executed CHORD @ ({},{},{})",
                                                        c.x, c.y, c.z
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut Frame, app: &TuiApp) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(5),
        ])
        .split(f.area());

    // Top Cyberpunk HUD Header
    render_cyber_header(f, app, root[0]);

    // Body: Left (3D Board Viewport) & Right (Tactical Cockpit & Mini Layer Map)
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(66), Constraint::Percentage(34)])
        .split(root[1]);

    render_cyber_board(f, app, body[0]);
    render_cyber_cockpit(f, app, body[1]);

    // Bottom Navigation Bar
    render_cyber_footer(f, app, root[2]);

    // Fullscreen Help Cheatsheet Modal if requested
    if app.show_help_modal {
        render_help_modal(f, app, f.area());
    }
}

fn render_cyber_header(f: &mut Frame, app: &TuiApp, area: Rect) {
    let mode_badge = match app.mode {
        TuiMode::SinglePlayer => Span::styled(
            " [ 🕹️ SINGLE PLAYER ] ",
            Style::default()
                .fg(Color::Cyan)
                .bg(Color::Rgb(20, 10, 45))
                .add_modifier(Modifier::BOLD),
        ),
        TuiMode::Multiplayer => Span::styled(
            " [ 🌐 MULTIPLAYER ] ",
            Style::default()
                .fg(Color::Green)
                .bg(Color::Rgb(10, 30, 20))
                .add_modifier(Modifier::BOLD),
        ),
        TuiMode::HostServer => Span::styled(
            " [ 🖥️ EMBEDDED HOST ] ",
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::Rgb(40, 30, 10))
                .add_modifier(Modifier::BOLD),
        ),
    };

    let mines_left = app.board_config.mines.saturating_sub(app.board.flag_count);
    let status_color = match app.board.status {
        GameStatus::Playing => Color::Green,
        GameStatus::Won => Color::Yellow,
        GameStatus::Lost => Color::Red,
        _ => Color::White,
    };

    let header_line = Line::from(vec![
        Span::styled(
            "⚡ 3D MÖBIUS MINESWEEPER ",
            Style::default()
                .fg(Color::Rgb(167, 139, 250))
                .add_modifier(Modifier::BOLD),
        ),
        mode_badge,
        Span::raw("  "),
        Span::styled(
            format!("[ 💣 {:03} ] ", mines_left),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("[ ⏱️ {:03}s ] ", app.elapsed_secs.min(999)),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "[ 🎚️ LAYER Z: {}/{} ] ",
                app.current_layer,
                app.board.dims.depth - 1
            ),
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("[ {:?} ]", app.board.status),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Rgb(124, 58, 237)))
        .title(Span::styled(
            " TACTICAL MAINFRAME HUD ",
            Style::default()
                .fg(Color::Rgb(192, 132, 252))
                .add_modifier(Modifier::BOLD),
        ));

    let widget = Paragraph::new(header_line)
        .block(block)
        .alignment(Alignment::Center);
    f.render_widget(widget, area);
}

fn render_cyber_board(f: &mut Frame, app: &TuiApp, area: Rect) {
    let dims = app.board.dims;
    let z = app.current_layer;
    let mut lines = Vec::new();

    // Compute visible viewport bounds
    let max_visible_x = ((area.width.saturating_sub(20) as usize) / 3)
        .max(1)
        .min(dims.width);
    let max_visible_y = (area.height.saturating_sub(4) as usize)
        .max(1)
        .min(dims.height);

    let start_x = if dims.width <= max_visible_x {
        0
    } else {
        app.cursor
            .x
            .saturating_sub(max_visible_x / 2)
            .min(dims.width - max_visible_x)
    };
    let end_x = (start_x + max_visible_x).min(dims.width);

    let start_y = if dims.height <= max_visible_y {
        0
    } else {
        app.cursor
            .y
            .saturating_sub(max_visible_y / 2)
            .min(dims.height - max_visible_y)
    };
    let end_y = (start_y + max_visible_y).min(dims.height);

    // Top X Coordinates
    let mut top_spans = vec![Span::styled(
        " ◄ MÖB │ ",
        Style::default()
            .fg(Color::Rgb(167, 139, 250))
            .add_modifier(Modifier::BOLD),
    )];
    for x in start_x..end_x {
        top_spans.push(Span::styled(
            format!("{:2} ", x),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ));
    }
    top_spans.push(Span::styled(
        "│ MÖB ►",
        Style::default()
            .fg(Color::Rgb(167, 139, 250))
            .add_modifier(Modifier::BOLD),
    ));
    lines.push(Line::from(top_spans));

    for y in start_y..end_y {
        let inv_y = (dims.height - 1) - y;
        let mut row_spans = vec![Span::styled(
            format!(" Y'={:02} │ ", inv_y),
            Style::default().fg(Color::Rgb(140, 100, 220)),
        )];

        for x in start_x..end_x {
            let coord = Coord3D::new(x, y, z);
            let cell = app.board.get_cell(coord);
            let is_cursor = coord == app.cursor;

            let (symbol, fg_color, bg_color) = if cell.is_flagged {
                ("🚩 ", Color::Red, Color::Rgb(40, 15, 25))
            } else if cell.is_revealed {
                if cell.is_mine {
                    ("💣 ", Color::White, Color::Rgb(160, 20, 30))
                } else if cell.adjacent_mines == 0 {
                    (" · ", Color::DarkGray, Color::Rgb(15, 10, 28))
                } else {
                    let num = cell.adjacent_mines;
                    let num_fg = match num {
                        1 => Color::Rgb(96, 165, 250),  // Cyan Blue
                        2 => Color::Rgb(52, 211, 153),  // Emerald
                        3 => Color::Rgb(248, 113, 113), // Coral
                        4 => Color::Rgb(192, 132, 252), // Purple
                        5 => Color::Rgb(251, 146, 60),  // Orange
                        6 => Color::Rgb(45, 212, 191),  // Teal
                        7 => Color::White,
                        _ => Color::Rgb(244, 114, 182), // Pink
                    };
                    (
                        match num {
                            1 => " 1 ",
                            2 => " 2 ",
                            3 => " 3 ",
                            4 => " 4 ",
                            5 => " 5 ",
                            6 => " 6 ",
                            7 => " 7 ",
                            8 => " 8 ",
                            _ => " 9 ",
                        },
                        num_fg,
                        Color::Rgb(22, 14, 40),
                    )
                }
            } else {
                (" ■ ", Color::Rgb(100, 75, 150), Color::Rgb(28, 18, 52))
            };

            let mut style = Style::default().fg(fg_color).bg(bg_color);
            if is_cursor {
                style = style
                    .bg(Color::Rgb(124, 58, 237))
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
            }

            row_spans.push(Span::styled(symbol, style));
        }

        row_spans.push(Span::styled(
            format!("│ Y'={:02}", inv_y),
            Style::default().fg(Color::Rgb(140, 100, 220)),
        ));
        lines.push(Line::from(row_spans));
    }

    let scroll_info = if dims.width > max_visible_x || dims.height > max_visible_y {
        format!(
            " (X: {}..{}/{}, Y: {}..{}/{})",
            start_x, end_x, dims.width, start_y, end_y, dims.height
        )
    } else {
        "".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(167, 139, 250)))
        .title(Span::styled(
            format!(
                " 🌌 3D MÖBIUS TOPOLOGY SLICE [Z = {z} / {}]{scroll_info} ",
                dims.depth - 1
            ),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));

    let widget = Paragraph::new(lines).block(block);
    f.render_widget(widget, area);

    // If Custom Setup Dialog is active, render centered popup
    if app.is_custom_modal {
        render_custom_setup_modal(f, app, area);
    }
}

fn render_custom_setup_modal(f: &mut Frame, app: &TuiApp, area: Rect) {
    let popup_w = 48.min(area.width.saturating_sub(4));
    let popup_h = 14.min(area.height.saturating_sub(4));
    let popup_area = Rect::new(
        area.x + (area.width.saturating_sub(popup_w)) / 2,
        area.y + (area.height.saturating_sub(popup_h)) / 2,
        popup_w,
        popup_h,
    );

    let fields = [
        ("Width  (X, 4-60):", app.custom_w),
        ("Height (Y, 4-40):", app.custom_h),
        ("Depth  (Z, 1-16):", app.custom_d),
        ("Mines  (Count)  :", app.custom_m),
    ];

    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        "⚡ CONFIGURE 3D MÖBIUS DIMENSIONS ⚡",
        Style::default()
            .fg(Color::Rgb(192, 132, 252))
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(vec![Span::raw("")]));

    for (idx, (label, val)) in fields.iter().enumerate() {
        let is_sel = idx == app.custom_field;
        let prefix = if is_sel { " ▶ " } else { "   " };
        let style = if is_sel {
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::Rgb(50, 25, 90))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{prefix}{label} "), style),
            Span::styled(
                format!("[ {:4} ] ", val),
                style.fg(if is_sel { Color::Cyan } else { Color::Green }),
            ),
            Span::styled(
                if is_sel { "◄ [A/D] ►" } else { "" },
                Style::default().fg(Color::Rgb(167, 139, 250)),
            ),
        ]));
    }

    let total = app.custom_w * app.custom_h * app.custom_d;
    let density = if total > 0 {
        (app.custom_m as f64 / total as f64) * 100.0
    } else {
        0.0
    };
    lines.push(Line::from(vec![Span::raw("")]));
    lines.push(Line::from(vec![Span::styled(
        format!("Total: {total} cells | Density: {density:.1}%"),
        Style::default().fg(Color::DarkGray),
    )]));

    if let Some(err) = &app.custom_error {
        lines.push(Line::from(vec![Span::styled(
            format!("❌ {err}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]));
    } else {
        lines.push(Line::from(vec![Span::styled(
            "Press [Enter] to Apply, [Esc] to Cancel",
            Style::default().fg(Color::Green),
        )]));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" ⚙️ CUSTOM SETUP ");

    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Center),
        popup_area,
    );
}

fn render_cyber_cockpit(f: &mut Frame, app: &TuiApp, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // 3D Layer Stack Mini-Radar
            Constraint::Length(5), // Live Topological Radar
            Constraint::Length(7), // Tactical Cheatsheet & Keybinding Reference
            Constraint::Min(4),    // Logs & Status
        ])
        .split(area);

    // 1. 3D Layer Stack Visualizer
    let mut layer_lines = Vec::new();
    for z in 0..app.board.dims.depth {
        let is_active = z == app.current_layer;
        let prefix = if is_active { " ▶ " } else { "   " };
        let style = if is_active {
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::Rgb(50, 25, 90))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        layer_lines.push(Line::from(vec![Span::styled(
            format!(
                "{prefix}Layer Z={z}: [================] (Active: {})",
                if is_active { "YES" } else { "NO" }
            ),
            style,
        )]));
    }
    let layer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(140, 100, 220)))
        .title(" 🧬 3D Layer Stack Visualizer ");
    f.render_widget(Paragraph::new(layer_lines).block(layer_block), chunks[0]);

    // 2. Topological Radar & AI Assistant
    let mut radar_lines = Vec::new();
    radar_lines.push(Line::from(vec![
        Span::styled("🎯 TARGET: ", Style::default().fg(Color::Yellow)),
        Span::styled(
            format!("({}, {}, {}) ", app.cursor.x, app.cursor.y, app.cursor.z),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ INVERSION: ", Style::default().fg(Color::Magenta)),
        Span::styled(
            format!(
                "(X'={}, Y'={}, Z'={})",
                if app.cursor.x == 0 {
                    app.board.dims.width - 1
                } else {
                    0
                },
                (app.board.dims.height - 1) - app.cursor.y,
                (app.board.dims.depth - 1) - app.cursor.z
            ),
            Style::default().fg(Color::Magenta),
        ),
    ]));

    if let Some(hint) = &app.last_hint {
        radar_lines.push(Line::from(vec![
            Span::styled(
                "💡 AI RECOMMENDATION: ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                hint,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
    } else {
        radar_lines.push(Line::from(vec![
            Span::styled("💡 AI ASSISTANT: ", Style::default().fg(Color::DarkGray)),
            Span::raw("Press [/] to calculate optimal move"),
        ]));
    }

    let radar_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(124, 58, 237)))
        .title(" 📡 Topological Radar ");
    f.render_widget(Paragraph::new(radar_lines).block(radar_block), chunks[1]);

    // 3. Tactical Command Cheatsheet Card
    let cheatsheet_lines = vec![
        Line::from(vec![
            Span::styled(
                "• [1/2/3/4] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Easy / Med / Exp / Custom"),
        ]),
        Line::from(vec![
            Span::styled(
                "• [WASD]    ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Move 2D Cursor Across Grid"),
        ]),
        Line::from(vec![
            Span::styled(
                "• [PgUp/Dn] ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Switch Z Layer Slice (or [ / ])"),
        ]),
        Line::from(vec![
            Span::styled(
                "• [Space/F] ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Reveal Cell / Toggle Flag 🚩"),
        ]),
        Line::from(vec![
            Span::styled(
                "• [B] / [/] ",
                Style::default()
                    .fg(Color::Rgb(192, 132, 252))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Run AI Bot Move / Hint Solver"),
        ]),
    ];
    let cheatsheet_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(96, 165, 250)))
        .title(Span::styled(
            " ⌨️ COMMAND REFERENCE ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    f.render_widget(
        Paragraph::new(cheatsheet_lines).block(cheatsheet_block),
        chunks[2],
    );

    // 4. System Event Log
    let mut log_lines = Vec::new();
    log_lines.push(Line::from(vec![
        Span::styled("STATUS: ", Style::default().fg(Color::Yellow)),
        Span::raw(&app.status_msg),
    ]));
    log_lines.push(Line::from(vec![Span::raw("")]));
    for l in app.logs.iter().rev().take(6) {
        log_lines.push(Line::from(vec![Span::styled(
            l,
            Style::default().fg(Color::Rgb(180, 180, 200)),
        )]));
    }

    let log_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" 📜 Event Stream ");
    f.render_widget(
        Paragraph::new(log_lines)
            .block(log_block)
            .wrap(Wrap { trim: true }),
        chunks[3],
    );
}

fn render_cyber_footer(f: &mut Frame, _app: &TuiApp, area: Rect) {
    let line1 = Line::from(vec![
        Span::styled(
            " 🎮 DIFFICULTY: ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "[1] ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Easy (9x9x3, 25m)  "),
        Span::styled(
            "[2] ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Medium (16x16x4, 160m)  "),
        Span::styled(
            "[3] ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Expert (30x16x6, 580m)  "),
        Span::styled(
            "[4/U] ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Custom Setup  "),
        Span::styled(
            "[R] ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("New Game"),
    ]);

    let line2 = Line::from(vec![
        Span::styled(
            " 🕹️ CONTROLS:   ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "[WASD/Arrows] ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Move Cursor  "),
        Span::styled(
            "[Space/Enter] ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Reveal  "),
        Span::styled(
            "[F] ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("Flag 🚩  "),
        Span::styled(
            "[C] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Chord  "),
        Span::styled(
            "[PgUp/PgDn / [ ]] ",
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Z-Layer  "),
        Span::styled(
            "[B] ",
            Style::default()
                .fg(Color::Rgb(192, 132, 252))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("AI Bot Step"),
    ]);

    let line3 = Line::from(vec![
        Span::styled(
            " ⚡ MAINFRAME:  ",
            Style::default()
                .fg(Color::Rgb(192, 132, 252))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "[M] ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Cycle Mode (SP ⇄ MP ⇄ Host)  "),
        Span::styled(
            "[/] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("AI Move Hint  "),
        Span::styled(
            "[F1 / ? / H] ",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("Full Keymap Cheatsheet  "),
        Span::styled(
            "[Q] ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw("Quit"),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Rgb(124, 58, 237)))
        .title(Span::styled(
            " 🕹️ TACTICAL CONTROL BAR ",
            Style::default()
                .fg(Color::Rgb(192, 132, 252))
                .add_modifier(Modifier::BOLD),
        ));

    let widget = Paragraph::new(vec![line1, line2, line3]).block(block);
    f.render_widget(widget, area);
}

fn render_help_modal(f: &mut Frame, _app: &TuiApp, area: Rect) {
    let popup_w = 72.min(area.width.saturating_sub(4));
    let popup_h = 24.min(area.height.saturating_sub(4));
    let popup_area = Rect::new(
        area.x + (area.width.saturating_sub(popup_w)) / 2,
        area.y + (area.height.saturating_sub(popup_h)) / 2,
        popup_w,
        popup_h,
    );

    let lines = vec![
        Line::from(vec![Span::styled(
            "═══ 🌌 3D MÖBIUS MINESWEEPER: COMPLETE COMMAND MANUAL ═══",
            Style::default()
                .fg(Color::Rgb(192, 132, 252))
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled(
            "【 1. DIFFICULTY & BOARD GENERATION 】",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                "  • [1] : ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Beginner Preset (9 x 9 x 3 grid, 25 mines, ~10.3% density)"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [2] : ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Intermediate Preset (16 x 16 x 4 grid, 160 mines, ~15.6% density)"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [3] : ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Expert Preset (30 x 16 x 6 grid, 580 mines, ~20.1% density)"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [4] / [U] : ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Custom Setup (Configure W [4-60], H [4-40], D [1-16], Mines)"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [R] : ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Re-initialize and start a new game with first-click safety"),
        ]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled(
            "【 2. NAVIGATION & TACTICAL ACTIONS 】",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                "  • [WASD / Arrows] : ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Move cursor across current 2D layer"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [PgUp/PgDn / [ / ]] : ",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Ascend or descend through 3D Z layers"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [Space / Enter] : ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Reveal target cell (or Chord if already opened)"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [F] : ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("Toggle flag 🚩 on unrevealed tile"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [C] : ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Chord / clear adjacent safe tiles when flags match number"),
        ]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled(
            "【 3. AI SPARRING & NETWORK MODES 】",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled(
                "  • [B] : ",
                Style::default()
                    .fg(Color::Rgb(192, 132, 252))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("(SP) Auto-execute Turing Master AI move / (MP) Add Bot to Room"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [/] : ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Calculate AI hint & mathematical deduction without clicking"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [M] : ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("Cycle mode: Single Player ⇄ Online Multiplayer ⇄ Embedded Host"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [H] : ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("(Host Mode) Toggle start/stop local WebSocket daemon (:3000)"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [N] / [P] : ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("(Multiplayer Mode) Connect to WebSocket / Create Game Room"),
        ]),
        Line::from(vec![
            Span::styled(
                "  • [Q] : ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw("Quit Application"),
        ]),
        Line::from(vec![Span::raw("")]),
        Line::from(vec![Span::styled(
            "Press [Esc], [F1], [Enter], or [Space] to close manual",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Rgb(192, 132, 252)))
        .title(Span::styled(
            " 📖 TACTICAL MAINFRAME MANUAL [F1] ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));

    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(Paragraph::new(lines).block(block), popup_area);
}
