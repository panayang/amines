# 3D Möbius Minesweeper - Standalone Server

This is a self-contained, standalone distribution of 3D Möbius Minesweeper containing:
- High-performance Rust async backend (`amine-server`) with WebSocket multiplayer rooms and SQLite leaderboard database.
- Pre-compiled WebAssembly Web Client in `dist/`.

## Quick Start (Default Port: 3500)

### 1. Run in Foreground
```bash
./run.sh
```
Or directly with the binary:
```bash
./amine-server
```

### 2. Run in Background (Daemon)
```bash
./start_daemon.sh
```
Stop the background daemon:
```bash
./stop_daemon.sh
```
View live logs:
```bash
tail -f server.log
```

## Custom Port & Configuration

### Option A: Via Environment Variables
```bash
PORT=3500 HOST=0.0.0.0 ./run.sh
```

### Option B: Via Command-Line Flags
```bash
./amine-server -p 3500 --host 0.0.0.0 --db ./minesweeper.db --dist ./dist
```

### Options Reference
- `-p, --port <PORT>`: Listening port (Default: `3500` or `$PORT`)
- `--host <HOST>`: Bind interface (Default: `0.0.0.0` or `$HOST`)
- `--db <PATH>`: SQLite database file (Default: `minesweeper.db` or `$DATABASE_PATH`)
- `-d, --dist <DIR>`: Web static assets directory (Default: `./dist` or `$CLIENT_DIST`)
