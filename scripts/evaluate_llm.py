#!/usr/bin/env python3
"""
LLM Evaluation Benchmark for 3D Möbius Minesweeper (Fluid Intelligence Test)
=============================================================================
Evaluates Large Language Models via Ollama API on 3D Möbius Minesweeper games.
Tracks step-by-step reasoning, move validity, reveal progress, flag precision/recall,
and outputs detailed per-round logs and summary statistics into CSV.
"""

import argparse
import csv
import json
import os
import re
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


@dataclass
class RoundStats:
    round_idx: int
    model: str
    difficulty: str
    dimensions: str
    total_mines: int
    total_non_mines: int
    status: str  # WON, HIT_MINE, INVALID_ACTION, MAX_MOVES, API_ERROR
    moves: int
    revealed_cells: int
    revealed_pct: float
    correct_flags: int
    false_flags: int
    total_flags: int
    flag_precision: float
    flag_recall: float
    duration_secs: float
    end_reason: str
    history_log: List[str] = field(default_factory=list)


def build_release_cli(workspace_dir: Path) -> Path:
    """Build the release profile of amine-cli and return binary path."""
    cli_bin = workspace_dir / "target" / "release" / "amine-cli"
    print(f"📦 Compiling release binary: cargo build --release -p amine-cli ...")
    start = time.time()
    res = subprocess.run(
        ["cargo", "build", "--release", "-p", "amine-cli"],
        cwd=str(workspace_dir),
        capture_output=True,
        text=True,
    )
    if res.returncode != 0:
        print(f"❌ Build failed:\n{res.stderr}", file=sys.stderr)
        sys.exit(1)
    print(f"✅ amine-cli ready in {time.time() - start:.2f}s ({cli_bin})\n")
    return cli_bin


def call_ollama_chat(
    endpoint: str,
    api_key: Optional[str],
    model: str,
    messages: List[Dict[str, str]],
    temperature: float = 0.0,
    timeout: int = 120,
) -> str:
    """Call Ollama chat API with Bearer token authentication."""
    headers = {
        "Content-Type": "application/json",
        "User-Agent": "Amine-Möbius-Benchmark/1.0",
    }
    if api_key:
        headers["Authorization"] = f"Bearer {api_key.strip()}"

    payload = {
        "model": model,
        "messages": messages,
        "stream": False,
        "options": {
            "temperature": temperature,
        },
    }

    req = urllib.request.Request(
        endpoint,
        data=json.dumps(payload).encode("utf-8"),
        headers=headers,
        method="POST",
    )

    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = json.loads(resp.read().decode("utf-8"))
            if "message" in data and "content" in data["message"]:
                return data["message"]["content"].strip()
            elif "response" in data:
                return data["response"].strip()
            else:
                return json.dumps(data)
    except urllib.error.HTTPError as e:
        err_body = e.read().decode("utf-8", errors="ignore")
        raise RuntimeError(f"HTTP {e.code} ({e.reason}): {err_body}")
    except urllib.error.URLError as e:
        raise RuntimeError(f"Network error connecting to {endpoint}: {e.reason}")


def parse_llm_action(text: str) -> Optional[Tuple[str, int, int, int]]:
    """
    Parse LLM response into action (reveal|flag|chord) and coordinates (x, y, z).
    Supports multiple concise formats:
      - reveal x y z
      - flag x y z
      - chord x y z
      - {"action": "reveal", "x": 1, "y": 2, "z": 0}
    """
    # 1. Check for standard command line format
    match = re.search(r"\b(reveal|flag|chord|r|f|c)\s+(\d+)\s+(\d+)(?:\s+(\d+))?\b", text, re.IGNORECASE)
    if match:
        act_raw = match.group(1).lower()
        act = "reveal" if act_raw in ("r", "reveal") else ("flag" if act_raw in ("f", "flag") else "chord")
        x = int(match.group(2))
        y = int(match.group(3))
        z = int(match.group(4)) if match.group(4) is not None else 0
        return act, x, y, z

    # 2. Check for JSON format
    json_match = re.search(r"\{[^{}]*\}", text)
    if json_match:
        try:
            d = json.loads(json_match.group(0))
            action = str(d.get("action", d.get("type", ""))).lower()
            if action in ("reveal", "flag", "chord", "r", "f", "c"):
                act = "reveal" if action in ("r", "reveal") else ("flag" if action in ("f", "flag") else "chord")
                x = int(d.get("x", 0))
                y = int(d.get("y", 0))
                z = int(d.get("z", 0))
                return act, x, y, z
        except Exception:
            pass

    return None


def format_grid_for_llm(visible_grid: List[Dict[str, Any]]) -> str:
    """Format 3D layer grid slices concisely for LLM prompt."""
    lines = []
    for slice_info in visible_grid:
        z = slice_info["layer_z"]
        rows = slice_info["rows"]
        w = len(rows[0]) if rows else 0
        x_header = "    " + " ".join(f"{x:1}" for x in range(w))
        lines.append(f"Layer Z={z}:")
        lines.append(x_header)
        for y, row_str in enumerate(rows):
            formatted_row = " ".join(row_str)
            lines.append(f"{y:2} | {formatted_row}")
        lines.append("")
    return "\n".join(lines).strip()


def run_llm_game(
    cli_bin: Path,
    endpoint: str,
    api_key: Optional[str],
    model: str,
    difficulty: str,
    round_idx: int,
    seed: Optional[int],
    max_moves: int,
    temperature: float,
    verbose: bool,
) -> RoundStats:
    """Run a single benchmark game session with the LLM."""
    with tempfile.NamedTemporaryFile(suffix=".json", delete=False) as tmp:
        state_file = Path(tmp.name)

    # 1. Initialize Game via CLI
    diff_arg = "easy" if difficulty in ("easy", "e", "1") else ("medium" if difficulty in ("mid", "medium", "m", "2") else "expert")
    init_cmd = [str(cli_bin), "init", "-d", diff_arg, "-s", str(state_file)]
    if seed is not None:
        init_cmd.extend(["--seed", str(seed)])

    res = subprocess.run(init_cmd, capture_output=True, text=True)
    if res.returncode != 0:
        raise RuntimeError(f"CLI init failed: {res.stderr}")

    init_info = json.loads(res.stdout)
    dims = init_info["dimensions"]
    w, h, d = dims["width"], dims["height"], dims["depth"]
    total_mines = init_info["mines"]
    total_cells = init_info["total_cells"]
    total_non_mines = total_cells - total_mines

    # Concise System Prompt
    system_prompt = (
        f"You are playing 3D Möbius Minesweeper.\n"
        f"Board dimensions: Width={w}, Height={h}, Depth={d}. Total mines={total_mines}.\n"
        f"Topology (Möbius Strip in 3D):\n"
        f"- X-axis wraps with inverted Y and Z across seams:\n"
        f"  * Crossing right (X={w}): wraps to (X'=0, Y'={h-1}-Y, Z'={d-1}-Z)\n"
        f"  * Crossing left (X=-1): wraps to (X'={w-1}, Y'={h-1}-Y, Z'={d-1}-Z)\n"
        f"- Y and Z boundaries do NOT wrap (they truncate).\n"
        f"Symbols: '?'=Unrevealed, 'F'=Flagged, '.'=0 adjacent mines, '1'-'26'=adjacent mine count.\n"
        f"Rule: Output ONLY YOUR NEXT ACTION in format: reveal <x> <y> <z>, flag <x> <y> <z>, or chord <x> <y> <z>.\n"
        f"Do NOT output markdown explanations. Just output the single action command."
    )

    # Initial safe first click at board center
    center_x, center_y, center_z = w // 2, h // 2, d // 2
    first_step_cmd = [
        str(cli_bin), "step",
        "-a", "reveal",
        "-x", str(center_x),
        "-y", str(center_y),
        "-z", str(center_z),
        "-s", str(state_file),
        "-f", "json",
    ]
    step_res = subprocess.run(first_step_cmd, capture_output=True, text=True)
    if step_res.returncode != 0:
        raise RuntimeError(f"CLI first step failed: {step_res.stderr}")

    first_output = json.loads(step_res.stdout)
    grid_text = format_grid_for_llm(first_output["visible_grid"])

    # Multi-turn conversation history
    messages = [
        {"role": "system", "content": system_prompt},
        {
            "role": "user",
            "content": (
                f"Initial game state after opening center ({center_x},{center_y},{center_z}):\n"
                f"Status: {first_output['status']} | Revealed: {first_output['total_revealed']}/{total_non_mines} | Remaining Mines: {first_output['remaining_mines']}\n\n"
                f"{grid_text}\n\n"
                f"What is your next move?"
            ),
        },
    ]

    history_log = [f"Init center reveal @ ({center_x},{center_y},{center_z}) -> {first_output['result']}"]
    move_count = 1
    game_status = first_output["status"]
    end_reason = "Playing"
    start_time = time.time()

    if verbose:
        print(f"\n--- [Round {round_idx:02}] {model} on {diff_arg.upper()} ({w}x{h}x{d}, {total_mines} mines) ---")
        print(f"Turn 01: Center safe open -> Revealed {first_output['revealed_now']} cells")

    while game_status == "Playing" and move_count < max_moves:
        move_count += 1
        # Call LLM
        try:
            llm_reply = call_ollama_chat(endpoint, api_key, model, messages, temperature=temperature)
        except Exception as e:
            game_status = "API_ERROR"
            end_reason = f"API Error: {e}"
            if verbose:
                print(f"❌ {end_reason}")
            break

        # Parse action
        parsed = parse_llm_action(llm_reply)
        if not parsed:
            game_status = "INVALID_ACTION"
            end_reason = f"Unparseable LLM output: '{llm_reply[:80]}'"
            if verbose:
                print(f"Turn {move_count:02}: ❌ {end_reason}")
            break

        act, ax, ay, az = parsed

        # Execute CLI step
        step_cmd = [
            str(cli_bin), "step",
            "-a", act,
            "-x", str(ax),
            "-y", str(ay),
            "-z", str(az),
            "-s", str(state_file),
            "-f", "json",
        ]
        step_proc = subprocess.run(step_cmd, capture_output=True, text=True)
        if step_proc.returncode != 0:
            game_status = "INVALID_ACTION"
            end_reason = f"Out of bounds or invalid move @ ({ax},{ay},{az})"
            if verbose:
                print(f"Turn {move_count:02}: ❌ {end_reason}")
            break

        step_data = json.loads(step_proc.stdout)
        game_status = step_data["status"]
        res_str = step_data["result"]
        history_log.append(f"Move #{move_count}: {act.upper()} ({ax},{ay},{az}) -> {res_str}")

        if verbose:
            tag = "💥" if "HIT_MINE" in res_str else ("🚩" if act == "flag" else "⛏️")
            print(f"Turn {move_count:02}: {tag} {act.upper()} ({ax},{ay},{az}) | Status: {game_status} | Cleared: {step_data['total_revealed']}/{total_non_mines}")

        if "HIT_MINE" in res_str:
            game_status = "LOST"
            end_reason = f"Hit mine @ ({ax},{ay},{az})"
            break
        elif game_status == "Won":
            end_reason = "Victory! Cleared all non-mine cells"
            break

        # Append messages for multi-turn context
        messages.append({"role": "assistant", "content": f"{act} {ax} {ay} {az}"})
        new_grid_text = format_grid_for_llm(step_data["visible_grid"])
        messages.append({
            "role": "user",
            "content": (
                f"Result: {res_str} (Status: {game_status}, Revealed: {step_data['total_revealed']}/{total_non_mines}, Remaining Mines: {step_data['remaining_mines']})\n\n"
                f"{new_grid_text}\n\n"
                f"Next move?"
            ),
        })

    if move_count >= max_moves and game_status == "Playing":
        game_status = "MAX_MOVES_REACHED"
        end_reason = f"Reached move limit ({max_moves})"

    duration = time.time() - start_time

    # Read final state file to compute ground-truth flag precision & recall
    try:
        with open(state_file, "r") as f:
            final_state = json.load(f)
        cells = final_state["board"]["cells"]
        revealed_cells = sum(1 for c in cells if c.get("is_revealed", False) and not c.get("is_mine", False))
        correct_flags = sum(1 for c in cells if c.get("is_flagged", False) and c.get("is_mine", False))
        false_flags = sum(1 for c in cells if c.get("is_flagged", False) and not c.get("is_mine", False))
        total_flags = correct_flags + false_flags
    except Exception:
        revealed_cells = 0
        correct_flags = 0
        false_flags = 0
        total_flags = 0

    # Cleanup temp state file
    try:
        os.remove(state_file)
    except Exception:
        pass

    revealed_pct = (revealed_cells / total_non_mines * 100.0) if total_non_mines > 0 else 0.0
    flag_precision = (correct_flags / total_flags * 100.0) if total_flags > 0 else 100.0
    flag_recall = (correct_flags / total_mines * 100.0) if total_mines > 0 else 0.0

    return RoundStats(
        round_idx=round_idx,
        model=model,
        difficulty=diff_arg,
        dimensions=f"{w}x{h}x{d}",
        total_mines=total_mines,
        total_non_mines=total_non_mines,
        status="WON" if game_status == "Won" else ("LOST" if game_status in ("LOST", "Lost") else game_status),
        moves=move_count,
        revealed_cells=revealed_cells,
        revealed_pct=revealed_pct,
        correct_flags=correct_flags,
        false_flags=false_flags,
        total_flags=total_flags,
        flag_precision=flag_precision,
        flag_recall=flag_recall,
        duration_secs=duration,
        end_reason=end_reason,
        history_log=history_log,
    )


def save_csv_results(output_csv: Path, stats_list: List[RoundStats]):
    """Save all evaluation round results to CSV file."""
    fieldnames = [
        "round",
        "model",
        "difficulty",
        "dimensions",
        "total_mines",
        "total_non_mines",
        "status",
        "moves",
        "revealed_cells",
        "revealed_pct",
        "correct_flags",
        "false_flags",
        "total_flags",
        "flag_precision_pct",
        "flag_recall_pct",
        "duration_secs",
        "end_reason",
    ]
    with open(output_csv, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        for s in stats_list:
            writer.writerow({
                "round": s.round_idx,
                "model": s.model,
                "difficulty": s.difficulty,
                "dimensions": s.dimensions,
                "total_mines": s.total_mines,
                "total_non_mines": s.total_non_mines,
                "status": s.status,
                "moves": s.moves,
                "revealed_cells": s.revealed_cells,
                "revealed_pct": f"{s.revealed_pct:.2f}",
                "correct_flags": s.correct_flags,
                "false_flags": s.false_flags,
                "total_flags": s.total_flags,
                "flag_precision_pct": f"{s.flag_precision:.2f}",
                "flag_recall_pct": f"{s.flag_recall:.2f}",
                "duration_secs": f"{s.duration_secs:.2f}",
                "end_reason": s.end_reason,
            })


def print_summary_report(stats_list: List[RoundStats], output_csv: Path):
    """Print a clean ASCII summary report of LLM performance."""
    total = len(stats_list)
    if total == 0:
        return

    wins = sum(1 for s in stats_list if s.status == "WON")
    losses = sum(1 for s in stats_list if s.status == "LOST")
    invalid = sum(1 for s in stats_list if s.status == "INVALID_ACTION")
    errors = sum(1 for s in stats_list if s.status == "API_ERROR")

    win_rate = (wins / total) * 100.0
    avg_moves = sum(s.moves for s in stats_list) / total
    avg_revealed_pct = sum(s.revealed_pct for s in stats_list) / total
    total_correct_flags = sum(s.correct_flags for s in stats_list)
    total_false_flags = sum(s.false_flags for s in stats_list)
    total_flags = total_correct_flags + total_false_flags
    overall_flag_prec = (total_correct_flags / total_flags * 100.0) if total_flags > 0 else 100.0
    avg_duration = sum(s.duration_secs for s in stats_list) / total

    print("\n" + "═" * 78)
    print(f"📊 3D MÖBIUS MINESWEEPER // LLM FLUID INTELLIGENCE BENCHMARK REPORT")
    print("═" * 78)
    print(f"  • Model Under Test   : {stats_list[0].model}")
    print(f"  • Evaluation Diff    : {stats_list[0].difficulty.upper()} ({stats_list[0].dimensions}, {stats_list[0].total_mines} mines)")
    print(f"  • Total Test Rounds  : {total}")
    print(f"  • Match Outcomes     : 🏆 Wins: {wins} | 💥 Losses: {losses} | ⚠️ Invalid/Err: {invalid + errors}")
    print(f"  • Win Rate           : {win_rate:.2f}%")
    print(f"  • Avg Cleared Non-Mine: {avg_revealed_pct:.2f}%")
    print(f"  • Avg Moves / Match  : {avg_moves:.2f}")
    print(f"  • Flags Placed       : Correct: {total_correct_flags} | False: {total_false_flags} (Precision: {overall_flag_prec:.2f}%)")
    print(f"  • Avg Match Duration : {avg_duration:.2f}s")
    print(f"  • CSV Results Saved  : {output_csv.resolve()}")
    print("═" * 78 + "\n")


def main():
    parser = argparse.ArgumentParser(
        description="LLM Fluid Intelligence Benchmark for 3D Möbius Minesweeper",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter,
    )
    parser.add_argument("--model", "-m", type=str, required=True, help="Ollama model name (e.g. gpt-oss:120b, llama3.3:70b)")
    parser.add_argument("--api-key", "-k", type=str, default=os.getenv("OLLAMA_API_KEY", ""), help="Ollama API key (defaults to $OLLAMA_API_KEY)")
    parser.add_argument("--endpoint", "-e", type=str, default=os.getenv("OLLAMA_ENDPOINT", "https://ollama.com/api/chat"), help="Ollama chat endpoint URL")
    parser.add_argument("--difficulty", "-d", choices=["easy", "mid", "medium", "hard", "expert"], default="easy", help="Game difficulty (easy, mid/medium, hard/expert)")
    parser.add_argument("--rounds", "-n", type=int, default=5, help="Number of benchmark games to evaluate")
    parser.add_argument("--output", "-o", type=str, default=None, help="Output CSV path (default: benchmark_<model>_<diff>_<timestamp>.csv)")
    parser.add_argument("--max-moves", type=int, default=150, help="Maximum moves per game session")
    parser.add_argument("--seed", type=int, default=None, help="Base random seed for reproducibility")
    parser.add_argument("--temperature", type=float, default=0.0, help="LLM sampling temperature (0.0 for deterministic)")
    parser.add_argument("--no-build", action="store_true", help="Skip cargo build release step")
    parser.add_argument("--verbose", "-v", action="store_true", help="Print detailed turn-by-turn log during evaluation")

    args = parser.parse_args()

    workspace_dir = Path(__file__).resolve().parent.parent

    # 1. Compile release binary if needed
    if not args.no_build:
        cli_bin = build_release_cli(workspace_dir)
    else:
        cli_bin = workspace_dir / "target" / "release" / "amine-cli"
        if not cli_bin.exists():
            print(f"Binary {cli_bin} not found. Building release binary...")
            cli_bin = build_release_cli(workspace_dir)

    # 2. Setup output CSV path
    clean_model_name = re.sub(r"[^\w\-\.]", "_", args.model)
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    if args.output:
        output_csv = Path(args.output)
    else:
        output_csv = workspace_dir / f"benchmark_{clean_model_name}_{args.difficulty}_{timestamp}.csv"

    print(f"🚀 Starting Benchmark on {args.model} ({args.difficulty.upper()}) for {args.rounds} rounds...")
    print(f"🌐 Endpoint: {args.endpoint} | Auth: {'Bearer Key Provided' if args.api_key else 'None'}\n")

    stats_list = []
    for r in range(1, args.rounds + 1):
        round_seed = (args.seed + r) if args.seed is not None else None
        print(f"▶️ [Round {r:02}/{args.rounds:02}] Running simulation...", end="", flush=True)
        stats = run_llm_game(
            cli_bin=cli_bin,
            endpoint=args.endpoint,
            api_key=args.api_key,
            model=args.model,
            difficulty=args.difficulty,
            round_idx=r,
            seed=round_seed,
            max_moves=args.max_moves,
            temperature=args.temperature,
            verbose=args.verbose,
        )
        stats_list.append(stats)

        tag = "🏆 WON" if stats.status == "WON" else f"💥 {stats.status}"
        print(f"\r▶️ [Round {r:02}/{args.rounds:02}] {tag} in {stats.moves} moves | Cleared: {stats.revealed_pct:.1f}% | Correct Flags: {stats.correct_flags}/{stats.total_mines} | ⏱️ {stats.duration_secs:.1f}s ({stats.end_reason})")

    # 3. Save to CSV and Print Summary
    save_csv_results(output_csv, stats_list)
    print_summary_report(stats_list, output_csv)


if __name__ == "__main__":
    main()
