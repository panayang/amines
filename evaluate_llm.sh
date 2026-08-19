#!/usr/bin/env bash
# ==============================================================================
# 3D Möbius Minesweeper - LLM Fluid Intelligence Benchmark Runner
# ==============================================================================
# Usage:
#   ./evaluate_llm.sh --model "gpt-oss:120b" --api-key "YOUR_KEY" --difficulty easy -n 5
# Or via environment variables:
#   export OLLAMA_API_KEY="YOUR_KEY"
#   ./evaluate_llm.sh -m "gpt-oss:120b" -d mid -n 10
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYTHON_SCRIPT="${SCRIPT_DIR}/scripts/evaluate_llm.py"

chmod +x "${PYTHON_SCRIPT}"

# Make sure python3 is available
if ! command -v python3 &> /dev/null; then
    echo "❌ Error: python3 is required but not found in PATH." >&2
    exit 1
fi

# Run the python benchmark runner
exec python3 "${PYTHON_SCRIPT}" "$@"
