#!/usr/bin/env python3
"""Derive the KAS 0.38.7 pause ordering from cyril-6beh's wire fixture."""
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent / "crates/cyril-core/tests/fixtures/kas/workflow/oracle-replay-events.jsonl"
TARGET = HERE / "source-derived-new-ordering.jsonl"
frames = [json.loads(line) for line in SOURCE.read_text().splitlines() if line]
paused_at = next(i for i, frame in enumerate(frames) if frame.get("method") == "_kiro/workflow/paused")
paused = frames.pop(paused_at)
paused["params"].update(
    initiator="user",
    initiatorReason="operator requested pause",
)
complete_at = next(i for i, frame in enumerate(frames) if frame.get("method") == "_kiro/workflow/run_complete")
frames[complete_at]["params"].update(
    initiator="user",
    initiatorReason="operator requested pause",
)
frames.insert(complete_at, paused)
TARGET.write_text(
    "\n".join(json.dumps(frame, separators=(",", ":"), ensure_ascii=False) for frame in frames) + "\n"
)
