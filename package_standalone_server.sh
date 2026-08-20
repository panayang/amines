#!/usr/bin/env bash
# ==============================================================================
# 3D Möbius Minesweeper - Standalone Server Packager
# ==============================================================================
# Builds the Web client and Rust backend into a completely standalone directory
# that can be deployed and executed anywhere independently.
#
# Default Port: 3500 (Can be overridden via $PORT or --port / -p)
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="${1:-"${SCRIPT_DIR}/amine-server-bundle"}"

echo "======================================================================"
echo "📦 Packaging Standalone 3D Möbius Minesweeper Server"
echo "📂 Target Directory: ${TARGET_DIR}"
echo "======================================================================"

# 1. Build Web Client (Trunk Release)
echo ""
echo "🌐 [1/3] Building Web Frontend (Wasm/Trunk Release)..."
if ! command -v trunk &> /dev/null; then
    echo "❌ Error: 'trunk' tool is required to compile the web client." >&2
    exit 1
fi

(
    cd "${SCRIPT_DIR}/crates/client"
    trunk build --release
)

# 2. Build Server Backend (Cargo Release)
echo ""
echo "🦀 [2/3] Building Rust Backend Server (Cargo Release)..."
(
    cd "${SCRIPT_DIR}"
    cargo build --release -p server
)

# 3. Assemble Standalone Bundle
echo ""
echo "📁 [3/3] Assembling standalone bundle into: ${TARGET_DIR} ..."
rm -rf "${TARGET_DIR}"
mkdir -p "${TARGET_DIR}"

# Copy binary
cp "${SCRIPT_DIR}/target/release/server" "${TARGET_DIR}/amine-server"
chmod +x "${TARGET_DIR}/amine-server"

# Copy web dist
cp -r "${SCRIPT_DIR}/crates/client/dist" "${TARGET_DIR}/dist"

# Create run.sh
cat << 'EOF' > "${TARGET_DIR}/run.sh"
#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Default to 3500 unless overridden by PORT env
export PORT="${PORT:-3500}"
export HOST="${HOST:-0.0.0.0}"
export DATABASE_PATH="${DATABASE_PATH:-${DIR}/minesweeper.db}"
export CLIENT_DIST="${CLIENT_DIST:-${DIR}/dist}"

echo "🚀 Starting 3D Möbius Minesweeper on http://${HOST}:${PORT} ..."
exec "${DIR}/amine-server" "$@"
EOF
chmod +x "${TARGET_DIR}/run.sh"

# Create start_daemon.sh
cat << 'EOF' > "${TARGET_DIR}/start_daemon.sh"
#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PID_FILE="${DIR}/server.pid"
LOG_FILE="${DIR}/server.log"

export PORT="${PORT:-3500}"
export HOST="${HOST:-0.0.0.0}"
export DATABASE_PATH="${DATABASE_PATH:-${DIR}/minesweeper.db}"
export CLIENT_DIST="${CLIENT_DIST:-${DIR}/dist}"

if [ -f "${PID_FILE}" ] && kill -0 "$(cat "${PID_FILE}")" 2>/dev/null; then
    echo "⚠️  Server is already running with PID $(cat "${PID_FILE}") on http://${HOST}:${PORT}"
    exit 0
fi

nohup "${DIR}/amine-server" "$@" > "${LOG_FILE}" 2>&1 &
SERVER_PID=$!
echo "${SERVER_PID}" > "${PID_FILE}"

echo "✅ 3D Möbius Minesweeper server started in background (PID: ${SERVER_PID})"
echo "🌐 URL: http://${HOST}:${PORT}"
echo "📄 Logs: ${LOG_FILE}"
EOF
chmod +x "${TARGET_DIR}/start_daemon.sh"

# Create stop_daemon.sh
cat << 'EOF' > "${TARGET_DIR}/stop_daemon.sh"
#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PID_FILE="${DIR}/server.pid"

if [ -f "${PID_FILE}" ]; then
    PID="$(cat "${PID_FILE}")"
    if kill -0 "${PID}" 2>/dev/null; then
        echo "🛑 Stopping server (PID: ${PID})..."
        kill "${PID}"
        rm -f "${PID_FILE}"
        echo "✅ Server stopped."
    else
        echo "⚠️  Process ${PID} not running. Removing stale PID file."
        rm -f "${PID_FILE}"
    fi
else
    echo "ℹ️  No PID file found. Server is not running."
fi
EOF
chmod +x "${TARGET_DIR}/stop_daemon.sh"

# Copy systemd scripts
cp "${SCRIPT_DIR}/scripts/setup_systemd.sh" "${TARGET_DIR}/setup_systemd.sh"
cp "${SCRIPT_DIR}/scripts/uninstall_systemd.sh" "${TARGET_DIR}/uninstall_systemd.sh"
chmod +x "${TARGET_DIR}/setup_systemd.sh" "${TARGET_DIR}/uninstall_systemd.sh"

# Create README.md
cat << 'EOF' > "${TARGET_DIR}/README.md"
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

### 2. Run as a Systemd Service (Auto-Start on Boot) ⭐️ Recommended for Production
To install and register as a systemd service that starts automatically on system boot:
```bash
sudo ./setup_systemd.sh
```
*(Options: `sudo ./setup_systemd.sh --port 3500 --user myuser`)*

Manage the service:
```bash
sudo systemctl status amine-server
sudo journalctl -u amine-server -f
sudo systemctl restart amine-server
sudo systemctl stop amine-server
```

Uninstall systemd service:
```bash
sudo ./uninstall_systemd.sh
```

### 3. Run in Background (Without Systemd)
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
EOF

# Create tar.gz archive
ARCHIVE_NAME="amine-server-linux-$(uname -m).tar.gz"
echo "📦 Creating archive: ${ARCHIVE_NAME} ..."
(
    cd "$(dirname "${TARGET_DIR}")"
    tar -czf "${ARCHIVE_NAME}" "$(basename "${TARGET_DIR}")"
)

echo ""
echo "======================================================================"
echo "🎉 Standalone server packaging complete!"
echo "📂 Output Directory : ${TARGET_DIR}"
echo "📦 Tar Archive      : ${SCRIPT_DIR}/${ARCHIVE_NAME}"
echo "🚀 Default Port     : 3500 (Overridable via -p / \$PORT)"
echo "▶️  To start        : cd ${TARGET_DIR} && ./run.sh"
echo "======================================================================"
