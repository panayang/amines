# 3D Möbius Minesweeper (`amine`)

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-15%20passing-brightgreen.svg)]()

An implementation of Minesweeper set in a 3D non-Euclidean Möbius strip manifold. Written entirely in Rust, featuring a modular multi-crate architecture with Web (WASM), Desktop (egui), Terminal (Ratatui), and CLI benchmarking interfaces, backed by an Axum WebSocket server and a multi-tiered constraint satisfaction AI solver.

---

## 🌌 Mathematical Topology

Traditional Minesweeper operates on a flat 2D Euclidean grid with 8 adjacent neighbors. **3D Möbius Minesweeper** extends the grid across $W \times H \times D$ coordinates with **26-neighbor 3D Moore connectivity**:

1. **Volume & Neighbors**:
   Each interior cell $(x, y, z)$ has up to 26 neighbors $(\Delta x, \Delta y, \Delta z) \in \{-1, 0, 1\}^3 \setminus \{(0, 0, 0)\}$.
2. **Möbius Seam Mapping**:
   The $X$-axis wraps across boundaries, while inverting both the $Y$ and $Z$ axes at the topological seam:
   $$\text{Neighbor crossing } X = W \implies (0,\, H - 1 - y,\, D - 1 - z)$$
   $$\text{Neighbor crossing } X = -1 \implies (W - 1,\, H - 1 - y,\, D - 1 - z)$$
3. **Bounded Dimensions**:
   Height ($Y$) and Depth ($Z$) boundaries remain bounded and do not wrap, preserving topological non-orientability without degenerating into a 3-torus.
4. **First-Click Safety Guarantee**:
   The first revealed tile and all its 26 topological neighbors are guaranteed free of mines, ensuring opening cascades.

---

## 📦 Workspace Architecture

The project is structured as a cargo workspace with 6 specialized crates:

```
.
├── Cargo.toml
└── crates/
    ├── shared/       # Topology math, board engine, 4-tier AI solver, i18n, and wire protocol
    ├── server/       # Axum + Tokio WebSocket server, room manager, AI NPC daemon, SQLite DB
    ├── client/       # Leptos 0.7 WebAssembly SPA frontend (HTML5/Vanilla CSS)
    ├── desktop/      # Egui (eframe) native desktop client with 3D beveled tiles & embedded server
    ├── tui/          # Ratatui terminal UI with 2D slicing, keybinding manual & embedded host
    └── cli/          # CLI interface for LLM fluid intelligence benchmarking & step execution
```

### Crate Descriptions

- **`shared`**: Core engine shared across all binaries. Contains deterministic 3D Möbius neighbor resolution, safe minefield generation, localized internationalization (`en` / `zh`), and a 4-tier deduction AI solver:
  - **Tier 1 (Pascal - Novice)**: Single-cell trivial deduction (remaining unrevealed == remaining mines).
  - **Tier 2 (Boole - Intermediate)**: Overlap and subset interval difference deduction ($A \subset B$).
  - **Tier 3 (Lovelace - Advanced)**: Gaussian elimination over binary mine probability matrices.
  - **Tier 4 (Turing - Master)**: Global mine count constraint search and probabilistic inference.
- **`server`**: Asynchronous game daemon built with Axum, Tokio, and SQLite (`sqlx`/`rusqlite`). Manages room lobbies, real-time board state synchronization over WebSockets, concurrent AI bot players with configurable tick speeds (200ms – 20s), and personal best score tracking.
- **`client`**: WebAssembly single-page application built using Leptos 0.7. Features fluid 2D slice rendering, aligned $X$/$Y$ coordinate indexing, glowing golden AI hint focus highlighting, responsive mobile viewports, full English/Chinese localization, and a custom board geometry designer.
- **`desktop`**: Native GUI application powered by `eframe`/`egui`. Includes 3D beveled relief tile rendering, 7-segment digital LED timer/mine counters, embedded dedicated server hosting, and real-time multiplayer sparring.
- **`tui`**: Terminal user interface powered by `ratatui` and `crossterm`. Features layer navigation, an interactive command cheatsheet modal (`[F1]` / `[?]`), multiplayer room lobbies, and local server orchestration.
- **`cli`**: Headless command-line tool designed for automation and evaluating LLM fluid intelligence. Supports initializing games, executing single actions (reveal/flag/chord/solve-step), outputting structured JSON state dumps, and batch benchmarking solver win rates.

---

## 🎮 Preset Difficulties

| Difficulty | Dimensions ($W \times H \times D$) | Total Cells | Mines | Mine Density |
| :--- | :--- | :--- | :--- | :--- |
| **Beginner (Easy)** | $9 \times 9 \times 3$ | 243 | 25 | $\approx 10.29\%$ |
| **Intermediate (Medium)** | $16 \times 16 \times 4$ | 1,024 | 160 | $\approx 15.63\%$ |
| **Expert** | $30 \times 16 \times 6$ | 2,880 | 580 | $\approx 20.14\%$ |
| **Custom** | $W \in [4, 60], H \in [4, 40], D \in [1, 16]$ | Dynamic | Validated ($< 85\%$) | User Defined |

---

## 🚀 Quick Start

### Prerequisites
- [Rust](https://www.rust-lang.org/) (version 1.80+ or latest stable)
- [Trunk](https://trunkrs.dev/) (only required for building the Web client: `cargo install trunk`)

### 1. Run the Dedicated Server
```bash
cargo run --bin server -- --port 3000 --db-path minesweeper.db
```

### 2. Run the Web Application
```bash
cd crates/client
trunk serve --port 8080
```
Open `http://localhost:8080` in your browser.

### 3. Run the Native Desktop Application
```bash
cargo run --bin amine-desktop
```

### 4. Run the Terminal UI (TUI)
```bash
cargo run --bin amine-tui
```

### 5. Run CLI Automation & LLM Evaluation
```bash
# Initialize a new game state to a JSON file
cargo run --bin amine-cli -- init -D medium -o game.json

# View board representation
cargo run --bin amine-cli -- view -i game.json

# Execute a reveal action at (X=8, Y=8, Z=2)
cargo run --bin amine-cli -- step -i game.json -a reveal -x 8 -y 8 -z 2 -o game.json

# Have Turing Master AI execute the next logical deduction
cargo run --bin amine-cli -- step -i game.json -a solve-step --tier master -o game.json

# Run a headless AI benchmark across 50 games
cargo run --bin amine-cli -- benchmark -D easy --tier master -n 50
```

---

## 🧪 Testing & Verification

Run the full workspace automated test suite:
```bash
cargo test --workspace
```

Run workspace linter checks (enforcing zero Clippy warnings):
```bash
cargo clippy --workspace --all-targets
```

---

## ⌨️ TUI Keybindings Reference

- **`[WASD]` / `[Arrow Keys]`**: Move cursor across current 2D layer.
- **`[PgUp]` / `[PgDn]`** or **`[` / `]`**: Ascend / descend 3D depth layers ($Z$).
- **`[Space]` / `[Enter]`**: Reveal cell (or Chord if already opened and flags match).
- **`[F]`**: Toggle flag 🚩 on unrevealed cell.
- **`[C]`**: Chord adjacent safe cells.
- **`[1]` / `[2]` / `[3]` / `[4]`**: Select Easy, Medium, Expert, or Custom mode.
- **`[R]`**: Restart game with first-click safety guarantee.
- **`[B]`**: Trigger AI auto-move (Turing Master).
- **`[/]`**: Calculate AI hint without modifying the board.
- **`[M]`**: Cycle client mode (Single-Player ⇄ Multiplayer ⇄ Host Server).
- **`[F1]` / `[?]` / `[K]`**: Toggle in-app command manual and controls modal.
- **`[Q]` / `[Esc]`**: Quit application.

---

## 📄 License

Dual-licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
