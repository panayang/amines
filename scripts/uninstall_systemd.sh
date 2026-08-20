#!/usr/bin/env bash
# ==============================================================================
# 3D Möbius Minesweeper - Systemd Service Uninstaller
# ==============================================================================
# Removes and disables the amine-server systemd service.
#
# Usage:
#   sudo ./uninstall_systemd.sh [--service-name amine-server]
# ==============================================================================

set -euo pipefail

SERVICE_NAME="${1:-"amine-server"}"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"

if [ "$(id -u)" -ne 0 ]; then
    echo "❌ Error: Root privileges are required to uninstall systemd services." >&2
    echo "👉 Please run with sudo: sudo $0 $@" >&2
    exit 1
fi

echo "🛑 Stopping and disabling ${SERVICE_NAME}.service..."
systemctl stop "${SERVICE_NAME}.service" 2>/dev/null || true
systemctl disable "${SERVICE_NAME}.service" 2>/dev/null || true

if [ -f "${SERVICE_FILE}" ]; then
    rm -f "${SERVICE_FILE}"
    echo "🗑️  Removed ${SERVICE_FILE}"
fi

systemctl daemon-reload
systemctl reset-failed 2>/dev/null || true

echo "✅ ${SERVICE_NAME}.service has been completely uninstalled."
