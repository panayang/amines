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
