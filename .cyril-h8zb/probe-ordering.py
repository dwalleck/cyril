#!/usr/bin/env python3
"""Probe: in real captured v2 sessions, do `_kiro.dev/metadata` frames arrive
BEFORE the `session/prompt` response (the frame carrying `stopReason`)?

Decides where a refusal alert can render: if metadata (with refusal) lands
before TurnCompleted, the system message can commit inside the turn.

Probe capture and oracle capture are different sessions, different days,
different binary versions — independently collected data, same question.
"""

import json
import sys
from pathlib import Path

def scan(path: Path) -> None:
    meta_lines, prompt_resp_lines = [], []
    for i, line in enumerate(path.read_text().splitlines(), 1):
        try:
            frame = json.loads(line)
        except json.JSONDecodeError:
            continue
        # KIRO_ACP_RECORD_PATH traces wrap each frame as {ts, dir, msg}.
        if "msg" in frame:
            frame = frame["msg"]
        if frame.get("method") == "_kiro.dev/metadata":
            meta_lines.append(i)
        result = frame.get("result")
        if isinstance(result, dict) and "stopReason" in result:
            prompt_resp_lines.append((i, result["stopReason"]))
    print(f"{path.name}:")
    print(f"  metadata frames at lines: {meta_lines}")
    print(f"  prompt responses at lines: {prompt_resp_lines}")
    for resp_line, reason in prompt_resp_lines:
        before = [m for m in meta_lines if m < resp_line]
        print(f"  response@{resp_line} ({reason}): {len(before)} metadata frame(s) precede it")

for arg in sys.argv[1:]:
    scan(Path(arg))
    print()
