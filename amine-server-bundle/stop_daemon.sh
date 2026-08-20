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
