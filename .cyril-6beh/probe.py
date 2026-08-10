#!/usr/bin/env python3
"""Produce fresh Kiro 2.16.2 failed/aborted workflow evidence."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SPIKE = ROOT / "experiments" / "conductor-spike"
OUT = Path(__file__).resolve().parent
if len(sys.argv) < 2:
    print(f"usage: {sys.argv[0]} KIRO_CLI_CHAT_BINARY [REPS]", file=sys.stderr)
    raise SystemExit(2)
KIRO = Path(sys.argv[1]).resolve()
REPS = sys.argv[2] if len(sys.argv) > 2 else "3"


def variant(source: Path, old: str, new: str, target: Path) -> None:
    text = source.read_text()
    if text.count(old) != 1:
        raise SystemExit(f"expected one replacement in {source}: {old!r}")
    target.write_text(text.replace(old, new))


def statuses(path: Path) -> list[str]:
    found = []
    for line in path.read_text().splitlines():
        frame = json.loads(line)
        frame = frame.get("parsed", frame)
        if frame.get("method") == "_kiro/workflow/run_complete":
            found.append(frame.get("params", {}).get("status"))
    return found


with tempfile.TemporaryDirectory(prefix="cyril-6beh-probe-") as tmp:
    tmp = Path(tmp)
    completion = SPIKE / "probe-kas-completion-signal-2.16.2.py"
    failure = tmp / "probe-failure.py"
    valid_token = "TOK = read_token()"
    expired_token = 'TOK = read_token()\nTOK["expiresAt"] = "1970-01-01T00:00:00Z"'
    variant(completion, valid_token, expired_token, failure)
    fail_log = OUT / "terminal-failed-2.16.2.jsonl"
    subprocess.run([sys.executable, failure, KIRO, fail_log, "neutral", REPS], check=True)

    sweep = SPIKE / "probe-kas-rpc-sweep-2.16.0.py"
    abort = tmp / "probe-abort.py"
    gated = 'nid = req("session/new", {"cwd": CWD, "mcpServers": [],\n                          "_meta": {"kiro": {"settings": {"workflows": {"enabled": True}}}}})'
    ungated = 'nid = req("session/new", {"cwd": CWD, "mcpServers": []})'
    variant(sweep, gated, ungated, abort)
    abort_log = OUT / "terminal-aborted-2.16.2.jsonl"
    subprocess.run([sys.executable, abort, KIRO, abort_log], check=True)

print(json.dumps({"failed-arm": statuses(fail_log), "aborted-arm": statuses(abort_log)}, sort_keys=True))
