#!/usr/bin/env python3
"""Extract genuine turn_end shapes and compare pinned sanitized fixtures."""
import copy
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TRACE = ROOT / "experiments/conductor-spike/kas-live-session-trace-2.11.0.jsonl"
FIX = Path(__file__).parent / "fixtures"


def leaves(value, prefix=""):
    if isinstance(value, dict):
        for key, child in value.items():
            yield from leaves(child, f"{prefix}.{key}" if prefix else key)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from leaves(child, f"{prefix}[{index}]")
    else:
        yield prefix, value


def main():
    try:
        captures = []
        for line_no, raw in enumerate(TRACE.read_text(encoding="utf-8").splitlines(), 1):
            record = json.loads(raw)
            msg = record.get("msg", {})
            kiro = msg.get("params", {}).get("update", {}).get("_meta", {}).get("kiro", {})
            if kiro.get("kind") == "turn_end":
                captures.append((line_no, record["ts"], msg))
        fixtures = [
            json.loads(path.read_text(encoding="utf-8")) for path in sorted(FIX.glob("*.json"))
        ]
    except (OSError, json.JSONDecodeError, KeyError) as error:
        raise SystemExit(f"fixture extraction failed: {error}") from error
    print(f"capture_count={len(captures)} fixture_count={len(fixtures)}")
    for index, (line_no, ts, msg) in enumerate(captures):
        observed = list(leaves(msg))
        sanitized = copy.deepcopy(msg)
        sanitized["params"]["sessionId"] = "sess_<sanitized>"
        print(f"capture[{index}].line={line_no} ts={ts}")
        for path, value in observed:
            print(f"  {path}={json.dumps(value, separators=(',', ':'))}")
        print(f"  pinned_exact={sanitized in fixtures}")
        ids = [p for p, _ in observed if "id" in p.lower() and p != "params.sessionId"]
        print(f"  native_turn_id_candidates={ids}")


if __name__ == "__main__":
    main()
