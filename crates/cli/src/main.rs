use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use shared::ai_solver::{AiAction, AiSolver, BotTier};
use shared::board::{Board, BoardConfig, GameStatus, RevealResult};
use shared::protocol::CellSnapshot;
use shared::topology::Coord3D;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "amine-cli")]
#[command(about = "Command-driven Headless Engine for 3D Möbius Minesweeper (LLM Fluid Intelligence Benchmark)", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new 3D Möbius Minesweeper game session and save to state file
    Init {
        #[arg(short = 'w', long, default_value_t = 9)]
        width: usize,

        #[arg(short = 'H', long, default_value_t = 9)]
        height: usize,

        #[arg(short = 'z', long, default_value_t = 3)]
        depth: usize,

        #[arg(short = 'm', long, default_value_t = 25)]
        mines: usize,

        #[arg(short = 'd', short_alias = 'D', long)]
        difficulty: Option<CliDifficulty>,

        #[arg(long)]
        seed: Option<u64>,

        #[arg(short = 's', long, default_value = "mobius_state.json")]
        state_file: PathBuf,
    },

    /// Apply an action (reveal, flag, chord) to the game and update state file
    Step {
        #[arg(short = 'a', long)]
        action: CliActionType,

        #[arg(short = 'x', long)]
        x: usize,

        #[arg(short = 'y', long)]
        y: usize,

        #[arg(short = 'z', long)]
        z: usize,

        #[arg(short = 's', long, default_value = "mobius_state.json")]
        state_file: PathBuf,

        #[arg(short = 'f', long, default_value = "both")]
        format: OutputFormat,
    },

    /// View the current board state from the state file
    View {
        #[arg(short = 'z', long)]
        layer: Option<usize>,

        #[arg(short = 's', long, default_value = "mobius_state.json")]
        state_file: PathBuf,

        #[arg(short = 'f', long, default_value = "both")]
        format: OutputFormat,

        #[arg(long)]
        show_hidden: bool,
    },

    /// Ask the mathematical solver for the optimal next move based on current visible state
    SolveStep {
        #[arg(short = 't', long, default_value = "master")]
        tier: CliBotTier,

        #[arg(short = 's', long, default_value = "mobius_state.json")]
        state_file: PathBuf,
    },

    /// Run an automated benchmark over N games to evaluate AI / LLM capability
    Benchmark {
        #[arg(short = 't', long, default_value = "master")]
        tier: CliBotTier,

        #[arg(short = 'd', short_alias = 'D', long, default_value = "medium")]
        difficulty: CliDifficulty,

        #[arg(short = 'w', long)]
        width: Option<usize>,

        #[arg(short = 'H', long)]
        height: Option<usize>,

        #[arg(short = 'z', long)]
        depth: Option<usize>,

        #[arg(short = 'm', long)]
        mines: Option<usize>,

        #[arg(short = 'n', long, default_value_t = 10)]
        games: usize,

        #[arg(long)]
        seed: Option<u64>,
    },

    /// View or clear local Personal Bests & high score records
    Records {
        #[arg(long)]
        clear: bool,
    },

    /// Start an interactive continuous CLI game session (terminal REPL mode)
    Play {
        #[arg(short = 'd', short_alias = 'D', long, default_value = "easy")]
        difficulty: CliDifficulty,

        #[arg(short = 'w', long)]
        width: Option<usize>,

        #[arg(short = 'H', long)]
        height: Option<usize>,

        #[arg(short = 'z', long)]
        depth: Option<usize>,

        #[arg(short = 'm', long)]
        mines: Option<usize>,

        #[arg(long)]
        seed: Option<u64>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize, PartialEq, Eq)]
enum CliDifficulty {
    Easy,
    Medium,
    Expert,
    Custom,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
enum CliActionType {
    Reveal,
    Flag,
    Chord,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
enum CliBotTier {
    Novice,
    Intermediate,
    Advanced,
    Master,
}

impl From<CliBotTier> for BotTier {
    fn from(t: CliBotTier) -> Self {
        match t {
            CliBotTier::Novice => BotTier::Novice,
            CliBotTier::Intermediate => BotTier::Intermediate,
            CliBotTier::Advanced => BotTier::Advanced,
            CliBotTier::Master => BotTier::Master,
        }
    }
}

#[derive(Clone, Copy, ValueEnum, Serialize, Deserialize, PartialEq, Eq)]
enum OutputFormat {
    Json,
    Text,
    Both,
}

#[derive(Serialize, Deserialize)]
struct GameStateFile {
    pub config: BoardConfig,
    pub board: Board,
    pub moves: Vec<MoveRecord>,
    pub seed: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone)]
struct MoveRecord {
    pub step: usize,
    pub action: String,
    pub coord: Coord3D,
    pub result: String,
    pub revealed_count: usize,
}

#[derive(Serialize)]
struct StepOutput {
    pub step: usize,
    pub action: String,
    pub coord: Coord3D,
    pub result: String,
    pub status: GameStatus,
    pub revealed_now: usize,
    pub total_revealed: usize,
    pub total_non_mines: usize,
    pub flag_count: usize,
    pub remaining_mines: usize,
    pub visible_grid: Vec<LayerSliceJson>,
}

#[derive(Serialize)]
struct LayerSliceJson {
    pub layer_z: usize,
    pub rows: Vec<String>,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init {
            width,
            height,
            depth,
            mines,
            difficulty,
            seed,
            state_file,
        } => {
            let config_res = if let Some(diff) = difficulty {
                match diff {
                    CliDifficulty::Easy => Ok(BoardConfig::easy()),
                    CliDifficulty::Medium => Ok(BoardConfig::medium()),
                    CliDifficulty::Expert => Ok(BoardConfig::expert()),
                    CliDifficulty::Custom => BoardConfig::custom(width, height, depth, mines),
                }
            } else {
                BoardConfig::custom(width, height, depth, mines)
            };

            let config = match config_res {
                Ok(c) => c,
                Err(err) => {
                    eprintln!(
                        "{}",
                        serde_json::json!({
                            "error": "INVALID_CONFIGURATION",
                            "message": err
                        })
                    );
                    std::process::exit(1);
                }
            };

            let board = Board::new(config);
            let state = GameStateFile {
                config,
                board,
                moves: Vec::new(),
                seed,
            };

            let json_str = serde_json::to_string_pretty(&state).expect("Failed to serialize state");
            fs::write(&state_file, json_str).expect("Failed to write state file");

            println!(
                "{}",
                serde_json::json!({
                    "status": "INITIALIZED",
                    "state_file": state_file.to_string_lossy(),
                    "dimensions": { "width": config.width, "height": config.height, "depth": config.depth },
                    "total_cells": config.total_cells(),
                    "mines": config.mines,
                    "topology": "3D_MOBIUS_STRIP",
                    "mobius_rules": {
                        "x_cross_right": "X'=0, Y'=(H-1)-Y, Z'=(D-1)-Z",
                        "x_cross_left": "X'=W-1, Y'=(H-1)-Y, Z'=(D-1)-Z"
                    }
                })
            );
        }

        Commands::Step {
            action,
            x,
            y,
            z,
            state_file,
            format,
        } => {
            let content = fs::read_to_string(&state_file).unwrap_or_else(|_| {
                panic!("Failed to read state file at {}", state_file.display())
            });
            let mut state: GameStateFile =
                serde_json::from_str(&content).expect("Invalid state file JSON");

            let coord = Coord3D::new(x, y, z);
            if !state.board.dims.is_valid_coord(coord) {
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "error": "OUT_OF_BOUNDS",
                        "message": format!("Coordinate ({x},{y},{z}) is outside dimensions {}x{}x{}", state.config.width, state.config.height, state.config.depth)
                    })
                );
                std::process::exit(1);
            }

            let res_str: String;
            let mut newly_revealed = 0;

            match action {
                CliActionType::Reveal => {
                    if !state.board.is_generated {
                        state.board.generate_mines_with_seed(coord, state.seed);
                    }
                    let res = state.board.reveal(coord, None, None);
                    match res {
                        RevealResult::Success { revealed }
                        | RevealResult::FirstClickGenerated { revealed } => {
                            newly_revealed = revealed.len();
                            res_str = format!("REVEALED_{newly_revealed}_CELLS");
                        }
                        RevealResult::HitMine { hit_coord, .. } => {
                            res_str = format!("HIT_MINE_AT_{hit_coord:?}");
                        }
                        RevealResult::AlreadyRevealed => res_str = "ALREADY_REVEALED".into(),
                        RevealResult::Flagged => res_str = "FLAGGED_CELL".into(),
                        RevealResult::NoOp => res_str = "NO_OP".into(),
                    }
                }
                CliActionType::Flag => {
                    let ok = state.board.toggle_flag(coord);
                    res_str = if ok {
                        let c = state.board.get_cell(coord);
                        if c.is_flagged {
                            "FLAG_SET".into()
                        } else {
                            "FLAG_REMOVED".into()
                        }
                    } else {
                        "FLAG_FAILED".into()
                    };
                }
                CliActionType::Chord => {
                    let res = state.board.chord(coord, None, None);
                    match res {
                        RevealResult::Success { revealed } => {
                            newly_revealed = revealed.len();
                            res_str = format!("CHORD_REVEALED_{newly_revealed}_CELLS");
                        }
                        RevealResult::HitMine { hit_coord, .. } => {
                            res_str = format!("CHORD_HIT_MINE_AT_{hit_coord:?}");
                        }
                        _ => res_str = "CHORD_NO_OP".into(),
                    }
                }
            }

            let move_rec = MoveRecord {
                step: state.moves.len() + 1,
                action: match action {
                    CliActionType::Reveal => "Reveal",
                    CliActionType::Flag => "Flag",
                    CliActionType::Chord => "Chord",
                }
                .to_string(),
                coord,
                result: res_str.clone(),
                revealed_count: state.board.revealed_count,
            };
            state.moves.push(move_rec);

            let json_str = serde_json::to_string_pretty(&state).expect("Failed to serialize state");
            fs::write(&state_file, json_str).expect("Failed to write state file");

            let visible_grid = render_visible_grid_json(&state.board, false);

            let output = StepOutput {
                step: state.moves.len(),
                action: format!("{:?}", action),
                coord,
                result: res_str,
                status: state.board.status,
                revealed_now: newly_revealed,
                total_revealed: state.board.revealed_count,
                total_non_mines: state.config.total_cells() - state.config.mines,
                flag_count: state.board.flag_count,
                remaining_mines: state.config.mines.saturating_sub(state.board.flag_count),
                visible_grid,
            };

            if format == OutputFormat::Json || format == OutputFormat::Both {
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            }

            if format == OutputFormat::Text || format == OutputFormat::Both {
                render_ascii_board(&state.board, None, false, false);
            }
        }

        Commands::View {
            layer,
            state_file,
            format,
            show_hidden,
        } => {
            let content = fs::read_to_string(&state_file).unwrap_or_else(|_| {
                panic!("Failed to read state file at {}", state_file.display())
            });
            let state: GameStateFile =
                serde_json::from_str(&content).expect("Invalid state file JSON");

            if format == OutputFormat::Json || format == OutputFormat::Both {
                let grid = render_visible_grid_json(&state.board, show_hidden);
                println!(
                    "{}",
                    serde_json::json!({
                        "status": state.board.status,
                        "revealed_count": state.board.revealed_count,
                        "total_non_mines": state.config.total_cells() - state.config.mines,
                        "flag_count": state.board.flag_count,
                        "mines_remaining": state.config.mines.saturating_sub(state.board.flag_count),
                        "grid": grid,
                        "move_count": state.moves.len()
                    })
                );
            }

            if format == OutputFormat::Text || format == OutputFormat::Both {
                render_ascii_board(&state.board, layer, show_hidden, false);
            }
        }

        Commands::SolveStep { tier, state_file } => {
            let content = fs::read_to_string(&state_file).unwrap_or_else(|_| {
                panic!("Failed to read state file at {}", state_file.display())
            });
            let state: GameStateFile =
                serde_json::from_str(&content).expect("Invalid state file JSON");

            let snapshots: Vec<CellSnapshot> =
                state.board.cells.iter().map(CellSnapshot::from).collect();
            let action = AiSolver::decide_action(
                state.config.dims(),
                &snapshots,
                tier.into(),
                state.config.mines,
            );

            match action {
                Some(act) => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "ACTION_RECOMMENDED",
                            "tier": format!("{:?}", tier),
                            "action": match act {
                                AiAction::Reveal(c) => serde_json::json!({ "type": "Reveal", "coord": c }),
                                AiAction::Flag(c) => serde_json::json!({ "type": "Flag", "coord": c }),
                                AiAction::Chord(c) => serde_json::json!({ "type": "Chord", "coord": c }),
                            }
                        })
                    );
                }
                None => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "NO_ACTION_AVAILABLE",
                            "message": "Game over or no valid moves remaining."
                        })
                    );
                }
            }
        }

        Commands::Benchmark {
            tier,
            difficulty,
            width,
            height,
            depth,
            mines,
            games,
            seed,
        } => {
            let config = if difficulty == CliDifficulty::Custom
                || width.is_some()
                || height.is_some()
                || depth.is_some()
                || mines.is_some()
            {
                let w = width.unwrap_or(16);
                let h = height.unwrap_or(16);
                let d = depth.unwrap_or(4);
                let m = mines.unwrap_or(160);
                BoardConfig::custom(w, h, d, m).expect("Invalid custom benchmark dimensions")
            } else {
                match difficulty {
                    CliDifficulty::Easy => BoardConfig::easy(),
                    CliDifficulty::Medium => BoardConfig::medium(),
                    CliDifficulty::Expert => BoardConfig::expert(),
                    CliDifficulty::Custom => unreachable!(),
                }
            };

            let bot_tier: BotTier = tier.into();
            let mut wins = 0;
            let mut losses = 0;
            let mut total_moves = 0;
            let mut total_revealed = 0;

            println!(
                "🚀 Running Benchmark: Tier={:?}, Diff={:?}, Games={}, Dims={}x{}x{} ({} mines)...",
                bot_tier,
                difficulty,
                games,
                config.width,
                config.height,
                config.depth,
                config.mines
            );

            for g in 1..=games {
                let game_seed = seed.map(|s| s + g as u64);
                let mut board = Board::new(config);
                let center = Coord3D::new(config.width / 2, config.height / 2, config.depth / 2);
                board.generate_mines_with_seed(center, game_seed);
                board.reveal(center, None, None);

                let mut moves = 0;
                while board.status == GameStatus::Playing && moves < 2000 {
                    moves += 1;
                    let snapshots: Vec<CellSnapshot> =
                        board.cells.iter().map(CellSnapshot::from).collect();
                    let action =
                        AiSolver::decide_action(config.dims(), &snapshots, bot_tier, config.mines);

                    match action {
                        Some(AiAction::Reveal(c)) => {
                            board.reveal(c, None, None);
                        }
                        Some(AiAction::Chord(c)) => {
                            board.chord(c, None, None);
                        }
                        Some(AiAction::Flag(c)) => {
                            board.toggle_flag(c);
                        }
                        None => break,
                    }
                }

                total_moves += moves;
                total_revealed += board.revealed_count;

                if board.status == GameStatus::Won {
                    wins += 1;
                    println!(
                        "  • Game #{g:02}: 🏆 WON in {moves} moves ({}/{} cleared)",
                        board.revealed_count,
                        config.total_cells() - config.mines
                    );
                } else {
                    losses += 1;
                    println!(
                        "  • Game #{g:02}: 💥 LOST at move {moves} ({}/{} cleared)",
                        board.revealed_count,
                        config.total_cells() - config.mines
                    );
                }
            }

            let win_rate = (wins as f64) / (games as f64) * 100.0;
            let avg_moves = (total_moves as f64) / (games as f64);
            let avg_revealed = (total_revealed as f64) / (games as f64);

            println!("\n================ BENCHMARK REPORT ================");
            println!(
                "{}",
                serde_json::json!({
                    "tier": format!("{:?}", bot_tier),
                    "difficulty": format!("{:?}", difficulty),
                    "total_games": games,
                    "wins": wins,
                    "losses": losses,
                    "win_rate_pct": format!("{:.2}%", win_rate),
                    "avg_moves_per_game": avg_moves,
                    "avg_revealed_cells": avg_revealed,
                    "total_target_cells": config.total_cells() - config.mines,
                })
            );
        }
        Commands::Records { clear } => {
            if clear {
                let empty_pb = shared::board::LocalPersonalBests::default();
                empty_pb.save();
                println!("🧹 Local Personal Best records cleared.");
            } else {
                let pb = shared::board::LocalPersonalBests::load_or_default();
                println!("\n🏆 3D MÖBIUS MINESWEEPER // PERSONAL BEST RECORDS");
                println!("==================================================");
                let diffs = [
                    (
                        shared::board::Difficulty::Easy,
                        "Beginner (9x9x3, 25 mines)",
                    ),
                    (
                        shared::board::Difficulty::Medium,
                        "Intermediate (16x16x4, 160 mines)",
                    ),
                    (
                        shared::board::Difficulty::Expert,
                        "Expert (30x16x6, 580 mines)",
                    ),
                    (shared::board::Difficulty::Custom, "Custom Configuration"),
                ];
                for (d, label) in diffs {
                    if let Some(r) = pb.get_pb(d) {
                        println!(
                            "• {:<36} : ⚡ {:>3}s ({:>3} moves) [{}]",
                            label, r.time_secs, r.moves, r.date
                        );
                    } else {
                        println!("• {:<36} : - (No Record)", label);
                    }
                }
                println!(
                    "\nJSON Format:\n{}",
                    serde_json::to_string_pretty(&pb).unwrap_or_default()
                );
            }
        }
        Commands::Play {
            difficulty,
            width,
            height,
            depth,
            mines,
            seed,
        } => {
            let mut current_diff =
                if width.is_some() || height.is_some() || depth.is_some() || mines.is_some() {
                    CliDifficulty::Custom
                } else {
                    difficulty
                };
            let mut custom_w = width.unwrap_or(12);
            let mut custom_h = height.unwrap_or(12);
            let mut custom_d = depth.unwrap_or(3);
            let mut custom_m = mines.unwrap_or(40);
            let current_seed = seed;

            let make_config = |diff: CliDifficulty,
                               w: usize,
                               h: usize,
                               d: usize,
                               m: usize|
             -> Result<BoardConfig, String> {
                match diff {
                    CliDifficulty::Easy => Ok(BoardConfig::easy()),
                    CliDifficulty::Medium => Ok(BoardConfig::medium()),
                    CliDifficulty::Expert => Ok(BoardConfig::expert()),
                    CliDifficulty::Custom => BoardConfig::custom(w, h, d, m),
                }
            };

            let mut config = match make_config(current_diff, custom_w, custom_h, custom_d, custom_m)
            {
                Ok(c) => c,
                Err(err) => {
                    eprintln!("❌ Invalid board configuration: {err}");
                    std::process::exit(1);
                }
            };
            let mut board = Board::new(config);
            let mut current_layer: usize = 0;
            let mut moves: u32 = 0;
            let mut accumulated_time = std::time::Duration::ZERO;
            let mut timer_start: Option<std::time::Instant> = None;
            let mut pause_start: Option<std::time::Instant> = None;
            let mut is_paused = false;
            let mut move_history: Vec<String> = Vec::new();
            let mut pb_records = shared::board::LocalPersonalBests::load_or_default();

            println!("\n╔═══════════════════════════════════════════════════════════════════════╗");
            println!("║  🎮 3D MÖBIUS MINESWEEPER // ENHANCED CONSOLE REPL ENGINE             ║");
            println!("╠═══════════════════════════════════════════════════════════════════════╣");
            println!("║ Core Actions:                                                         ║");
            println!("║   r <x> <y> [z]    : ⛏️  Reveal cell (or Chord if opened)            ║");
            println!("║   f <x> <y> [z]    : 🚩 Flag / Unflag cell                            ║");
            println!("║   c <x> <y> [z]    : ⚡ Chord cell (reveal unopened neighbors)        ║");
            println!("║ Navigation & View:                                                    ║");
            println!(
                "║   z <layer>        : 🎚️  Switch active view layer (0..{})              ║",
                config.depth.saturating_sub(1)
            );
            println!("║   u / d            : ⟲ Prev Layer / ⟳ Next Layer (Up/Down)            ║");
            println!("║   v / view         : 👁️  Render active layer                          ║");
            println!("║   all / slices     : 🌌 Render all 3D topological slices             ║");
            println!("║ Tactical & AI Tools:                                                  ║");
            println!("║   p / pause        : ⏸️  Pause / Resume timer (masks grid)            ║");
            println!("║   h / hint [tier]  : 💡 Ask mathematical AI solver for recommendation ║");
            println!("║   auto [n] / step  : 🤖 Let AI execute 1 or N optimal moves           ║");
            println!("║   solve / run      : 🚀 Let AI auto-solve until finish or no moves    ║");
            println!("║   radar <x> <y> [z]: 📡 Inspect Möbius inversion & coordinate topology║");
            println!("║ Session Management:                                                   ║");
            println!("║   diff <e|m|x|c>   : ⚙️  Switch difficulty (easy, medium, expert, c)   ║");
            println!("║   history / moves  : 📜 View move history log                         ║");
            println!("║   records / pb     : 🏆 Display Personal Best high score table        ║");
            println!("║   restart / new    : 🔄 Restart current board                         ║");
            println!("║   quit / exit / q  : 🚪 Exit game session                             ║");
            println!("╚═══════════════════════════════════════════════════════════════════════╝\n");

            render_ascii_board(&board, Some(current_layer), false, false);

            let stdin = std::io::stdin();
            let mut input_buf = String::new();

            loop {
                let current_elapsed = if let Some(st) = timer_start {
                    if is_paused {
                        accumulated_time.as_secs()
                    } else {
                        (accumulated_time + st.elapsed()).as_secs()
                    }
                } else {
                    0
                };

                let remaining_mines = config.mines.saturating_sub(board.flag_count);
                let pause_tag = if is_paused { " [⏸️ PAUSED]" } else { "" };
                print!(
                    "[Layer Z={}/{} | 💣 Rem: {:03} | ⏱️  {:03}s{} | 👟 Moves: {:03}] > ",
                    current_layer,
                    config.depth.saturating_sub(1),
                    remaining_mines,
                    current_elapsed,
                    pause_tag,
                    moves
                );
                std::io::Write::flush(&mut std::io::stdout()).unwrap();

                input_buf.clear();
                if stdin.read_line(&mut input_buf).is_err() || input_buf.trim().is_empty() {
                    continue;
                }

                let tokens: Vec<&str> = input_buf.split_whitespace().collect();
                if tokens.is_empty() {
                    continue;
                }

                let cmd = tokens[0].to_lowercase();

                // Handle Pause & Resume
                if cmd == "p" || cmd == "pause" || cmd == "resume" {
                    if is_paused {
                        is_paused = false;
                        if let Some(pst) = pause_start {
                            accumulated_time += pst.elapsed();
                            pause_start = None;
                            timer_start = Some(std::time::Instant::now());
                        }
                        println!("▶️ Game Resumed!");
                        render_ascii_board(&board, Some(current_layer), false, false);
                    } else {
                        if board.status == GameStatus::Playing {
                            is_paused = true;
                            if let Some(st) = timer_start {
                                accumulated_time += st.elapsed();
                                timer_start = None;
                            }
                            pause_start = Some(std::time::Instant::now());
                            println!("⏸️ Game Paused. Type 'p' or 'resume' to continue.");
                            render_ascii_board(&board, Some(current_layer), false, true);
                        } else {
                            println!("⚠️ Cannot pause when game is not active.");
                        }
                    }
                    continue;
                }

                if is_paused {
                    println!(
                        "⏸️ Game is currently paused. Type 'p' or 'resume' to resume playing."
                    );
                    continue;
                }

                match cmd.as_str() {
                    "q" | "quit" | "exit" => {
                        println!("👋 Thanks for playing 3D Möbius Minesweeper! Goodbye.");
                        break;
                    }
                    "restart" | "new" => {
                        if let Ok(new_cfg) =
                            make_config(current_diff, custom_w, custom_h, custom_d, custom_m)
                        {
                            config = new_cfg;
                            board = Board::new(config);
                            current_layer = 0;
                            moves = 0;
                            accumulated_time = std::time::Duration::ZERO;
                            timer_start = None;
                            pause_start = None;
                            is_paused = false;
                            move_history.clear();
                            println!("🔄 Game restarted! New board generated.");
                            render_ascii_board(&board, Some(current_layer), false, false);
                        }
                    }
                    "diff" => {
                        if tokens.len() < 2 {
                            println!("❌ Usage: diff <easy|medium|expert|custom> [w h z m]");
                            continue;
                        }
                        let (new_diff, new_w, new_h, new_d, new_m) = match tokens[1]
                            .to_lowercase()
                            .as_str()
                        {
                            "e" | "easy" => {
                                (CliDifficulty::Easy, custom_w, custom_h, custom_d, custom_m)
                            }
                            "m" | "medium" => (
                                CliDifficulty::Medium,
                                custom_w,
                                custom_h,
                                custom_d,
                                custom_m,
                            ),
                            "x" | "expert" => (
                                CliDifficulty::Expert,
                                custom_w,
                                custom_h,
                                custom_d,
                                custom_m,
                            ),
                            "c" | "custom" => {
                                let w = if tokens.len() >= 3 {
                                    tokens[2].parse().unwrap_or(custom_w)
                                } else {
                                    custom_w
                                };
                                let h = if tokens.len() >= 4 {
                                    tokens[3].parse().unwrap_or(custom_h)
                                } else {
                                    custom_h
                                };
                                let d = if tokens.len() >= 5 {
                                    tokens[4].parse().unwrap_or(custom_d)
                                } else {
                                    custom_d
                                };
                                let m = if tokens.len() >= 6 {
                                    tokens[5].parse().unwrap_or(custom_m)
                                } else {
                                    custom_m
                                };
                                (CliDifficulty::Custom, w, h, d, m)
                            }
                            _ => {
                                println!("❌ Unknown difficulty. Choose from easy, medium, expert, custom.");
                                continue;
                            }
                        };
                        match make_config(new_diff, new_w, new_h, new_d, new_m) {
                            Ok(new_cfg) => {
                                current_diff = new_diff;
                                custom_w = new_w;
                                custom_h = new_h;
                                custom_d = new_d;
                                custom_m = new_m;
                                config = new_cfg;
                                board = Board::new(config);
                                current_layer = 0;
                                moves = 0;
                                accumulated_time = std::time::Duration::ZERO;
                                timer_start = None;
                                pause_start = None;
                                move_history.clear();
                                println!(
                                    "⚙️ Difficulty switched to {:?} ({}x{}x{}, {} mines).",
                                    current_diff,
                                    config.width,
                                    config.height,
                                    config.depth,
                                    config.mines
                                );
                                render_ascii_board(&board, Some(current_layer), false, false);
                            }
                            Err(err) => {
                                println!("❌ Custom dimension error: {err}");
                            }
                        }
                    }
                    "records" | "pb" => {
                        println!("\n🏆 3D MÖBIUS MINESWEEPER // PERSONAL BESTS & HALL OF FAME");
                        println!("══════════════════════════════════════════════════════════");
                        for (d, label) in [
                            (
                                shared::board::Difficulty::Easy,
                                "🟢 Beginner (9x9x3, 25 mines)",
                            ),
                            (
                                shared::board::Difficulty::Medium,
                                "🟡 Intermediate (16x16x4, 160 mines)",
                            ),
                            (
                                shared::board::Difficulty::Expert,
                                "🔴 Expert (30x16x6, 580 mines)",
                            ),
                            (shared::board::Difficulty::Custom, "⚙️ Custom Configuration"),
                        ] {
                            if let Some(r) = pb_records.get_pb(d) {
                                println!(
                                    "  • {:<36}: ⚡ {:>3}s ({:>3} moves) [{}]",
                                    label, r.time_secs, r.moves, r.date
                                );
                            } else {
                                println!("  • {:<36}: - (No Record)", label);
                            }
                        }
                        println!();
                    }
                    "history" | "moves" => {
                        println!("\n📜 MOVE HISTORY LOG (Total {} moves):", moves);
                        for (idx, entry) in move_history.iter().rev().take(15).rev().enumerate() {
                            println!("  {}. {}", idx + 1, entry);
                        }
                        println!();
                    }
                    "radar" | "topo" => {
                        if tokens.len() < 3 {
                            println!("❌ Usage: radar <x> <y> [z]");
                            continue;
                        }
                        let x: usize = tokens[1].parse().unwrap_or(0);
                        let y: usize = tokens[2].parse().unwrap_or(0);
                        let z: usize = if tokens.len() > 3 {
                            tokens[3].parse().unwrap_or(current_layer)
                        } else {
                            current_layer
                        };
                        let coord = Coord3D::new(x, y, z);
                        if !config.dims().is_valid_coord(coord) {
                            println!("❌ Coordinate ({x},{y},{z}) is out of bounds.");
                            continue;
                        }
                        let inv_y = (config.height - 1) - y;
                        let inv_z = (config.depth - 1) - z;
                        let neighbors = config.dims().get_neighbors(coord);
                        let cell = board.get_cell(coord);
                        println!("\n📡 TOPOLOGICAL RADAR INSPECTION @ ({x}, {y}, {z}):");
                        println!(
                            "  • Cell Status     : {}",
                            if cell.is_revealed {
                                format!("Revealed ({})", cell.adjacent_mines)
                            } else if cell.is_flagged {
                                "Flagged 🚩".into()
                            } else {
                                "Hidden ⬛".into()
                            }
                        );
                        println!(
                            "  • Möbius Inversion: (X'=0, Y'={}, Z'={}) across left/right boundary",
                            inv_y, inv_z
                        );
                        println!(
                            "  • 3D Moore Degree : {} valid adjacent neighbors",
                            neighbors.len()
                        );
                        println!();
                    }
                    "v" | "view" => {
                        render_ascii_board(&board, Some(current_layer), false, false);
                    }
                    "all" | "slices" => {
                        render_ascii_board(&board, None, false, false);
                    }
                    "z" | "layer" => {
                        if tokens.len() > 1 {
                            if let Ok(lz) = tokens[1].parse::<usize>() {
                                if lz < config.depth {
                                    current_layer = lz;
                                    println!("🎚️ Switched to Layer Z = {lz}");
                                    render_ascii_board(&board, Some(current_layer), false, false);
                                } else {
                                    println!(
                                        "❌ Layer {lz} is out of bounds (0..{}).",
                                        config.depth.saturating_sub(1)
                                    );
                                }
                            }
                        }
                    }
                    "u" | "up" | "prev" => {
                        if current_layer > 0 {
                            current_layer -= 1;
                            println!("⟲ Switched to Layer Z = {current_layer}");
                            render_ascii_board(&board, Some(current_layer), false, false);
                        } else {
                            println!("ℹ️ Already at lowest layer Z = 0.");
                        }
                    }
                    "d" | "down" | "next" => {
                        if current_layer + 1 < config.depth {
                            current_layer += 1;
                            println!("⟳ Switched to Layer Z = {current_layer}");
                            render_ascii_board(&board, Some(current_layer), false, false);
                        } else {
                            println!(
                                "ℹ️ Already at highest layer Z = {}.",
                                config.depth.saturating_sub(1)
                            );
                        }
                    }
                    "h" | "hint" => {
                        let tier = if tokens.len() > 1 {
                            match tokens[1].to_lowercase().as_str() {
                                "novice" | "n" | "1" => BotTier::Novice,
                                "inter" | "intermediate" | "i" | "2" => BotTier::Intermediate,
                                "adv" | "advanced" | "a" | "3" => BotTier::Advanced,
                                _ => BotTier::Master,
                            }
                        } else {
                            BotTier::Master
                        };

                        let snapshots: Vec<CellSnapshot> =
                            board.cells.iter().map(CellSnapshot::from).collect();
                        if let Some(act) =
                            AiSolver::decide_action(config.dims(), &snapshots, tier, config.mines)
                        {
                            match act {
                                AiAction::Reveal(c) => println!(
                                    "💡 AI Solver ({:?}): REVEAL @ ({}, {}, {})",
                                    tier, c.x, c.y, c.z
                                ),
                                AiAction::Flag(c) => println!(
                                    "🚩 AI Solver ({:?}): FLAG @ ({}, {}, {})",
                                    tier, c.x, c.y, c.z
                                ),
                                AiAction::Chord(c) => println!(
                                    "⚡ AI Solver ({:?}): CHORD @ ({}, {}, {})",
                                    tier, c.x, c.y, c.z
                                ),
                            }
                        } else {
                            println!("💡 AI Solver ({:?}): No deterministic mathematical moves found (guess required).", tier);
                        }
                    }
                    "auto" | "step" => {
                        if board.status == GameStatus::Won || board.status == GameStatus::Lost {
                            println!("⚠️ Game is already over. Type 'restart' to play again.");
                            continue;
                        }
                        if timer_start.is_none() {
                            timer_start = Some(std::time::Instant::now());
                        }

                        let count: usize = if tokens.len() > 1 {
                            tokens[1].parse().unwrap_or(1).max(1)
                        } else {
                            1
                        };

                        for _ in 0..count {
                            if board.status != GameStatus::Playing && board.is_generated {
                                break;
                            }
                            let snapshots: Vec<CellSnapshot> =
                                board.cells.iter().map(CellSnapshot::from).collect();
                            if let Some(act) = AiSolver::decide_action(
                                config.dims(),
                                &snapshots,
                                BotTier::Master,
                                config.mines,
                            ) {
                                moves += 1;
                                match act {
                                    AiAction::Reveal(c) => {
                                        current_layer = c.z;
                                        let res = board.reveal(c, None, None);
                                        move_history.push(format!(
                                            "AI REVEAL ({},{},{}) -> {:?}",
                                            c.x, c.y, c.z, res
                                        ));
                                        println!("🤖 AI REVEAL @ ({}, {}, {})", c.x, c.y, c.z);
                                    }
                                    AiAction::Flag(c) => {
                                        current_layer = c.z;
                                        board.toggle_flag(c);
                                        move_history
                                            .push(format!("AI FLAG ({},{},{})", c.x, c.y, c.z));
                                        println!("🤖 AI FLAG @ ({}, {}, {})", c.x, c.y, c.z);
                                    }
                                    AiAction::Chord(c) => {
                                        current_layer = c.z;
                                        let res = board.chord(c, None, None);
                                        move_history.push(format!(
                                            "AI CHORD ({},{},{}) -> {:?}",
                                            c.x, c.y, c.z, res
                                        ));
                                        println!("🤖 AI CHORD @ ({}, {}, {})", c.x, c.y, c.z);
                                    }
                                }
                            } else {
                                println!("💡 AI: No deterministic mathematical moves remaining.");
                                break;
                            }
                        }
                        render_ascii_board(&board, Some(current_layer), false, false);
                    }
                    "solve" | "run" => {
                        if board.status == GameStatus::Won || board.status == GameStatus::Lost {
                            println!("⚠️ Game is already over. Type 'restart' to play again.");
                            continue;
                        }
                        if timer_start.is_none() {
                            timer_start = Some(std::time::Instant::now());
                        }

                        let mut step_count = 0;
                        while board.status == GameStatus::Playing || !board.is_generated {
                            let snapshots: Vec<CellSnapshot> =
                                board.cells.iter().map(CellSnapshot::from).collect();
                            if let Some(act) = AiSolver::decide_action(
                                config.dims(),
                                &snapshots,
                                BotTier::Master,
                                config.mines,
                            ) {
                                moves += 1;
                                step_count += 1;
                                match act {
                                    AiAction::Reveal(c) => {
                                        current_layer = c.z;
                                        let _ = board.reveal(c, None, None);
                                    }
                                    AiAction::Flag(c) => {
                                        current_layer = c.z;
                                        board.toggle_flag(c);
                                    }
                                    AiAction::Chord(c) => {
                                        current_layer = c.z;
                                        let _ = board.chord(c, None, None);
                                    }
                                }
                                if board.status != GameStatus::Playing {
                                    break;
                                }
                            } else {
                                println!("💡 AI Solver halted after {step_count} automated steps: No deterministic moves left.");
                                break;
                            }
                        }
                        println!("🚀 Executed {step_count} automated solver steps.");
                        render_ascii_board(&board, Some(current_layer), false, false);
                    }
                    "r" | "reveal" | "f" | "flag" | "c" | "chord" => {
                        if board.status == GameStatus::Won || board.status == GameStatus::Lost {
                            println!("⚠️ Game is already over. Type 'restart' to play again.");
                            continue;
                        }
                        if tokens.len() < 3 {
                            println!("❌ Usage: {} <x> <y> [z]", tokens[0]);
                            continue;
                        }
                        let x: usize = match tokens[1].parse() {
                            Ok(v) if v < config.width => v,
                            _ => {
                                println!("❌ Invalid X coordinate (0..{})", config.width);
                                continue;
                            }
                        };
                        let y: usize = match tokens[2].parse() {
                            Ok(v) if v < config.height => v,
                            _ => {
                                println!("❌ Invalid Y coordinate (0..{})", config.height);
                                continue;
                            }
                        };
                        let z: usize = if tokens.len() > 3 {
                            match tokens[3].parse() {
                                Ok(v) if v < config.depth => v,
                                _ => {
                                    println!("❌ Invalid Z coordinate (0..{})", config.depth);
                                    continue;
                                }
                            }
                        } else {
                            current_layer
                        };

                        if timer_start.is_none() {
                            timer_start = Some(std::time::Instant::now());
                        }

                        let coord = Coord3D::new(x, y, z);
                        moves += 1;

                        match tokens[0].to_lowercase().as_str() {
                            "r" | "reveal" => {
                                if !board.is_generated {
                                    board.generate_mines_with_seed(coord, current_seed);
                                }
                                let cell = board.get_cell(coord);
                                if cell.is_revealed {
                                    let res = board.chord(coord, None, None);
                                    move_history.push(format!("CHORD ({x},{y},{z}) -> {:?}", res));
                                } else {
                                    let res = board.reveal(coord, None, None);
                                    move_history.push(format!("REVEAL ({x},{y},{z}) -> {:?}", res));
                                }
                            }
                            "f" | "flag" => {
                                board.toggle_flag(coord);
                                move_history.push(format!("FLAG ({x},{y},{z})"));
                            }
                            "c" | "chord" => {
                                let res = board.chord(coord, None, None);
                                move_history.push(format!("CHORD ({x},{y},{z}) -> {:?}", res));
                            }
                            _ => {}
                        }

                        current_layer = z;
                        render_ascii_board(&board, Some(current_layer), false, false);
                    }
                    "help" | "?" => {
                        println!("📖 Commands: r <x> <y> [z], f <x> <y> [z], c <x> <y> [z], z <layer>, u/d, p/pause, h/hint, auto [n], solve, radar <x> <y> [z], diff <e|m|x|c>, records, history, restart, quit");
                    }
                    other => {
                        println!(
                            "❌ Unknown command: '{}'. Type 'help' for full command list.",
                            other
                        );
                    }
                }

                if board.status == GameStatus::Won {
                    let total_time = if let Some(st) = timer_start {
                        (accumulated_time + st.elapsed()).as_secs()
                    } else {
                        accumulated_time.as_secs()
                    };
                    println!(
                        "\n╔═══════════════════════════════════════════════════════════════════╗"
                    );
                    println!(
                        "║  🏆🎉 CONGRATULATIONS! YOU HAVE WON 3D MÖBIUS MINESWEEPER! 🎉🏆    ║"
                    );
                    println!(
                        "╠═══════════════════════════════════════════════════════════════════╣"
                    );
                    println!(
                        "║  ⏱️  Final Time : {:>4}s                                           ║",
                        total_time
                    );
                    println!(
                        "║  👟 Total Moves : {:>4}                                            ║",
                        moves
                    );
                    println!(
                        "║  💣 Cleared Grid: {} non-mine cells cleared                      ║",
                        config.total_cells() - config.mines
                    );
                    let is_new = pb_records.update_if_best(config.difficulty, total_time, moves);
                    if is_new {
                        println!(
                            "║  ✨ NEW ALL-TIME PERSONAL BEST ACHIEVED! ✨                       ║"
                        );
                    }
                    println!(
                        "╚═══════════════════════════════════════════════════════════════════╝"
                    );
                    println!("Type 'restart' to start a new match, or 'quit' to exit.\n");
                } else if board.status == GameStatus::Lost {
                    println!(
                        "\n╔═══════════════════════════════════════════════════════════════════╗"
                    );
                    println!(
                        "║  💥💀 BOOM! YOU HIT A MINE! SIMULATION TERMINATED! 💀💥           ║"
                    );
                    println!(
                        "╚═══════════════════════════════════════════════════════════════════╝"
                    );
                    render_ascii_board(&board, None, true, false);
                    println!("Type 'restart' to try again, or 'quit' to exit.\n");
                }
            }
        }
    }
}

fn render_visible_grid_json(board: &Board, show_hidden: bool) -> Vec<LayerSliceJson> {
    let mut slices = Vec::new();
    let dims = board.dims;

    for z in 0..dims.depth {
        let mut rows = Vec::new();
        for y in 0..dims.height {
            let mut row_str = String::new();
            for x in 0..dims.width {
                let cell = board.get_cell(Coord3D::new(x, y, z));
                let ch = if cell.is_flagged {
                    'F'
                } else if cell.is_revealed {
                    if cell.is_mine {
                        'M'
                    } else if cell.adjacent_mines == 0 {
                        '.'
                    } else {
                        char::from_digit(cell.adjacent_mines as u32, 10).unwrap_or('?')
                    }
                } else if show_hidden && cell.is_mine {
                    '*'
                } else {
                    '?'
                };
                row_str.push(ch);
            }
            rows.push(row_str);
        }
        slices.push(LayerSliceJson { layer_z: z, rows });
    }

    slices
}

fn render_ascii_board(board: &Board, layer: Option<usize>, show_hidden: bool, is_paused: bool) {
    let dims = board.dims;
    let layers_to_show: Vec<usize> = match layer {
        Some(z) if z < dims.depth => vec![z],
        _ => (0..dims.depth).collect(),
    };

    println!("\n╔════════════════════════════════════════════════════════════════════════════╗");
    println!(
        "║  3D MÖBIUS MINESWEEPER PROJECTION (Status: {:?})",
        board.status
    );
    println!(
        "║  Revealed: {}/{} | Flagged: {} | Target Mines: {}",
        board.revealed_count,
        dims.total_cells() - board.config.mines,
        board.flag_count,
        board.config.mines
    );
    println!("╚════════════════════════════════════════════════════════════════════════════╝");

    if is_paused {
        println!("\n  ┌────────────────────────────────────────────────────────┐");
        println!("  │  ⏸️  [ SIMULATION PAUSED // MÖBIUS PROJECTION MASKED ] │");
        println!("  │  Type 'p' or 'resume' to resume and unmask the grid.   │");
        println!("  └────────────────────────────────────────────────────────┘\n");
        return;
    }

    for z in layers_to_show {
        println!(
            "\n─── [ Layer Z = {z} / {} ] (Left Edge ⟲ Inverted Y'/Z' ⟲ Right Edge) ───",
            dims.depth.saturating_sub(1)
        );
        print!("  Y' │ Y │ ");
        for x in 0..dims.width {
            print!("{:2} ", x);
        }
        println!("│ Y │ Y'");

        for y in 0..dims.height {
            let inv_y = (dims.height - 1) - y;
            print!("{:02} │{:02} │ ", inv_y, y);
            for x in 0..dims.width {
                let cell = board.get_cell(Coord3D::new(x, y, z));
                let token = if cell.is_flagged {
                    "🚩 "
                } else if cell.is_revealed {
                    if cell.is_mine {
                        "💣 "
                    } else if cell.adjacent_mines == 0 {
                        " · "
                    } else {
                        &format!(" {} ", cell.adjacent_mines)
                    }
                } else if show_hidden && cell.is_mine {
                    " 💣"
                } else {
                    " ⬛"
                };
                print!("{token}");
            }
            println!("│{:02} │ {:02}", y, inv_y);
        }

        print!("  Y' │ Y │ ");
        for x in 0..dims.width {
            print!("{:2} ", x);
        }
        println!("│ Y │ Y'");
    }
    println!();
}
