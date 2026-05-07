#!/usr/bin/env python3
"""
Watches ~/.kiro/sessions/cli/ for write activity while a Kiro turn is in flight.

Goal: determine whether Kiro writes the .json sidecar (and .jsonl transcript)
incrementally during a turn, or only on turn completion.

Output (stdout + watch.log):
  T+0.000ms  EVENT       MODIFY        ce09a750-....json
  T+0.001ms  SNAPSHOT    json size=4128 turns=3 last_end=UserTurnEnd updated_at=...
  T+1.234ms  EVENT       CREATE        ce09a750-....jsonl
  ...

Run:
  python3 watch.py [session_id]

If session_id is omitted, watches the whole sessions/cli/ directory and labels
every event with the file basename — useful for "start cyril, send a prompt"
flows where the session_id isn't known up front.
"""
from __future__ import annotations

import json
import signal
import subprocess
import sys
import time
from pathlib import Path

SESSIONS_DIR = Path.home() / ".kiro" / "sessions" / "cli"
EVENTS = "modify,close_write,moved_to,moved_from,create,delete,attrib"


def now_ms(start: float) -> str:
    return f"T+{(time.monotonic() - start) * 1000:9.3f}ms"


def snapshot(path: Path) -> str:
    if not path.exists():
        return "missing"
    try:
        st = path.stat()
    except OSError as e:
        return f"stat-err={e}"
    parts = [f"size={st.st_size}", f"mtime={st.st_mtime:.3f}"]
    if path.suffix == ".json":
        try:
            with path.open() as f:
                obj = json.load(f)
            md = obj.get("session_state", {}).get("conversation_metadata", {})
            turns = md.get("user_turn_metadatas") or []
            last = turns[-1] if turns else {}
            parts.append(f"turns={len(turns)}")
            parts.append(f"last_end={last.get('end_reason', '-')}")
            parts.append(f"input_tok={last.get('input_token_count', '-')}")
            parts.append(f"output_tok={last.get('output_token_count', '-')}")
            mu = last.get("metering_usage") or []
            credit_total = sum(m.get("value", 0) for m in mu if m.get("unit") == "credit")
            parts.append(f"credits={credit_total:.4f}")
            parts.append(f"updated_at={obj.get('updated_at', '-')}")
            parts.append(f"last_request_null={md.get('last_request') is None}")
        except json.JSONDecodeError:
            parts.append("json=PARTIAL")
        except Exception as e:
            parts.append(f"json-err={type(e).__name__}")
    elif path.suffix == ".jsonl":
        try:
            with path.open() as f:
                lines = [ln for ln in f if ln.strip()]
            parts.append(f"records={len(lines)}")
            if lines:
                last = json.loads(lines[-1])
                parts.append(f"last_kind={last.get('kind', '-')}")
        except Exception as e:
            parts.append(f"jsonl-err={type(e).__name__}")
    return " ".join(parts)


def main() -> int:
    target_id = sys.argv[1] if len(sys.argv) > 1 else None
    if not SESSIONS_DIR.exists():
        print(f"[fatal] {SESSIONS_DIR} does not exist", file=sys.stderr)
        return 2

    log_path = Path(__file__).parent / "watch.log"
    log = log_path.open("w", buffering=1)
    start = time.monotonic()

    def emit(line: str) -> None:
        print(line, flush=True)
        log.write(line + "\n")

    emit(f"# watching {SESSIONS_DIR} (filter={target_id or 'ALL'})")
    emit(f"# events: {EVENTS}")
    emit(f"# start_wallclock={time.time():.3f}")

    # initial snapshot of any relevant files
    if target_id:
        for ext in (".json", ".jsonl"):
            p = SESSIONS_DIR / f"{target_id}{ext}"
            emit(f"{now_ms(start)}  BASELINE    {ext[1:]:<6}        {p.name}  {snapshot(p)}")

    cmd = [
        "inotifywait", "--monitor", "--quiet",
        "--format", "%T|%e|%f",
        "--timefmt", "%s",
        "-e", EVENTS,
        str(SESSIONS_DIR),
    ]
    proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)

    def handle_sigint(*_args: object) -> None:
        emit(f"{now_ms(start)}  SHUTDOWN")
        proc.terminate()

    signal.signal(signal.SIGINT, handle_sigint)
    signal.signal(signal.SIGTERM, handle_sigint)

    try:
        assert proc.stdout is not None
        for raw in proc.stdout:
            raw = raw.strip()
            if not raw:
                continue
            try:
                _epoch, events, fname = raw.split("|", 2)
            except ValueError:
                emit(f"{now_ms(start)}  PARSE_FAIL  {raw}")
                continue
            if target_id and not fname.startswith(target_id):
                continue
            full = SESSIONS_DIR / fname
            ext = full.suffix.lstrip(".") or "??"
            emit(f"{now_ms(start)}  EVENT       {events:<13} {ext:<6} {fname}")
            # snapshot only for files we care about (.json holds metering, .jsonl is transcript)
            if full.suffix in (".json", ".jsonl"):
                emit(f"{now_ms(start)}  SNAPSHOT    {ext:<13} {ext:<6} {fname}  {snapshot(full)}")
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=2)
        except subprocess.TimeoutExpired:
            proc.kill()
        log.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
