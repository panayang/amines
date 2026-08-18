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
            let config = if let Some(diff) = difficulty {
                match diff {
                    CliDifficulty::Easy => BoardConfig::easy(),
                    CliDifficulty::Medium => BoardConfig::medium(),
                    CliDifficulty::Expert => BoardConfig::expert(),
                    CliDifficulty::Custom => BoardConfig::custom(width, height, depth, mines)
                        .expect("Invalid custom board dimensions"),
                }
            } else {
                BoardConfig::custom(width, height, depth, mines)
                    .expect("Invalid custom board dimensions")
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
                render_ascii_board(&state.board, None, false);
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
                render_ascii_board(&state.board, layer, show_hidden);
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

fn render_ascii_board(board: &Board, layer: Option<usize>, show_hidden: bool) {
    let dims = board.dims;
    let layers_to_show: Vec<usize> = match layer {
        Some(z) if z < dims.depth => vec![z],
        _ => (0..dims.depth).collect(),
    };

    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!(
        "║  3D MÖBIUS MINESWEEPER STATE (Status: {:?})",
        board.status
    );
    println!(
        "║  Revealed: {}/{} | Flagged: {} | Target Mines: {}",
        board.revealed_count,
        dims.total_cells() - board.config.mines,
        board.flag_count,
        board.config.mines
    );
    println!("╚════════════════════════════════════════════════════════════════╝");

    for z in layers_to_show {
        println!(
            "\n─── [ Layer Z = {z} / {} ] (Left: Invert Y/Z ⟲ Right: Invert Y/Z) ───",
            dims.depth - 1
        );
        print!("    ");
        for x in 0..dims.width {
            print!("{:2} ", x);
        }
        println!();

        for y in 0..dims.height {
            print!("{:2} |", y);
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
            println!("| Y={y}");
        }
    }
    println!();
}
