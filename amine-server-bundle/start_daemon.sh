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
