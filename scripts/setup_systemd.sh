#!/usr/bin/env bash
# ==============================================================================
# 3D Möbius Minesweeper - Systemd Service Installer
# ==============================================================================
# Registers the standalone amine-server as a systemd system service for
# automatic startup on boot.
#
# Usage:
#   sudo ./setup_systemd.sh [--port 3500] [--user <username>] [--service-name amine-server]
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUNDLE_DIR="${SCRIPT_DIR}"

# Check if script is in scripts/ directory or bundle directory
if [ -f "${SCRIPT_DIR}/amine-server" ]; then
    BUNDLE_DIR="${SCRIPT_DIR}"
elif [ -f "${SCRIPT_DIR}/../amine-server-bundle/amine-server" ]; then
    BUNDLE_DIR="$(cd "${SCRIPT_DIR}/../amine-server-bundle" && pwd)"
elif [ -f "${SCRIPT_DIR}/../target/release/server" ]; then
    BUNDLE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
fi

# Detect actual non-root user if run with sudo
ACTUAL_USER="${SUDO_USER:-"$(id -un)"}"
ACTUAL_GROUP="$(id -gn "${ACTUAL_USER}" 2>/dev/null || echo "${ACTUAL_USER}")"

PORT="3500"
SERVICE_NAME="amine-server"
RUN_USER="${ACTUAL_USER}"
RUN_GROUP="${ACTUAL_GROUP}"

# Parse command line options
while [[ $# -gt 0 ]]; do
    case "$1" in
        -p|--port)
            PORT="$2"
            shift 2
            ;;
        -u|--user)
            RUN_USER="$2"
            RUN_GROUP="$(id -gn "${RUN_USER}" 2>/dev/null || echo "${RUN_USER}")"
            shift 2
            ;;
        -s|--service-name)
            SERVICE_NAME="$2"
            shift 2
            ;;
        -d|--dir)
            BUNDLE_DIR="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: sudo $0 [OPTIONS]"
            echo "Options:"
            echo "  -p, --port <PORT>          Port to listen on (default: 3500)"
            echo "  -u, --user <USER>          User to run service as (default: ${ACTUAL_USER})"
            echo "  -s, --service-name <NAME>  Systemd service name (default: amine-server)"
            echo "  -d, --dir <PATH>           Directory containing amine-server (default: ${BUNDLE_DIR})"
            echo "  -h, --help                 Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

# Ensure root privileges
if [ "$(id -u)" -ne 0 ]; then
    echo "❌ Error: Root privileges are required to configure systemd services." >&2
    echo "👉 Please run with sudo: sudo $0 $@" >&2
    exit 1
fi

BINARY_PATH="${BUNDLE_DIR}/amine-server"
if [ ! -f "${BINARY_PATH}" ]; then
    # Check if target/release/server exists
    if [ -f "${BUNDLE_DIR}/target/release/server" ]; then
        BINARY_PATH="${BUNDLE_DIR}/target/release/server"
    else
        echo "❌ Error: Could not find server binary at ${BINARY_PATH}" >&2
        echo "Please build or package the server first." >&2
        exit 1
    fi
fi

# Ensure binary is executable
chmod +x "${BINARY_PATH}"

DIST_DIR="${BUNDLE_DIR}/dist"
DB_PATH="${BUNDLE_DIR}/minesweeper.db"

# Ensure directory permissions are accessible by service user
chown -R "${RUN_USER}:${RUN_GROUP}" "${BUNDLE_DIR}" 2>/dev/null || true

SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"

echo "======================================================================"
echo "⚙️  Installing Systemd Service: ${SERVICE_NAME}"
echo "======================================================================"
echo "📂 Working Directory : ${BUNDLE_DIR}"
echo "🏃 Executable        : ${BINARY_PATH}"
echo "👤 Run as User       : ${RUN_USER}:${RUN_GROUP}"
echo "🌐 Port              : ${PORT}"
echo "📄 Service Unit File : ${SERVICE_FILE}"
echo "======================================================================"

cat << EOF > "${SERVICE_FILE}"
[Unit]
Description=3D Mobius Minesweeper Server
After=network.target network-online.target
Wants=network-online.target

[Service]
Type=simple
User=${RUN_USER}
Group=${RUN_GROUP}
WorkingDirectory=${BUNDLE_DIR}
ExecStart=${BINARY_PATH} --port ${PORT} --host 0.0.0.0 --db ${DB_PATH} --dist ${DIST_DIR}
Restart=always
RestartSec=5
KillSignal=SIGTERM
TimeoutStopSec=10

# Environment variables
Environment=PORT=${PORT}
Environment=HOST=0.0.0.0
Environment=DATABASE_PATH=${DB_PATH}
Environment=CLIENT_DIST=${DIST_DIR}
Environment=RUST_LOG=server=info,tower_http=info

# Security & limits
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
EOF

chmod 644 "${SERVICE_FILE}"

echo "🔄 Reloading systemd daemon..."
systemctl daemon-reload

echo "⚡ Enabling ${SERVICE_NAME}.service for auto-start on boot..."
systemctl enable "${SERVICE_NAME}.service"

echo "▶️  Starting ${SERVICE_NAME}.service..."
systemctl restart "${SERVICE_NAME}.service"

sleep 1

echo ""
echo "======================================================================"
echo "🎉 Systemd service installed and started successfully!"
echo "======================================================================"
echo "🌐 Service URL     : http://0.0.0.0:${PORT}"
echo "📊 Check Status    : sudo systemctl status ${SERVICE_NAME}"
echo "📄 View Live Logs  : sudo journalctl -u ${SERVICE_NAME} -f"
echo "⏹️  Stop Service    : sudo systemctl stop ${SERVICE_NAME}"
echo "🔄 Restart Service : sudo systemctl restart ${SERVICE_NAME}"
echo "❌ Uninstall       : sudo ${BUNDLE_DIR}/uninstall_systemd.sh"
echo "======================================================================"
echo ""
systemctl status "${SERVICE_NAME}.service" --no-pager || true
