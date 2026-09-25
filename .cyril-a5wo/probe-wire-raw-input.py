#!/usr/bin/env python3
"""Audit rawInput representability in a pinned extracted @kiro/agent bundle."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

EXPECTED_SHA256 = "965ae084945a48eb73fe2049feed7e3deb6fb8d8a9cf49aa4713b172ed3fb70a"
EXPECTED_VERSION = "0.38.7"
EXPECTED_SITES = {
    "tool_call": [488290, 488444, 488823, 488917],
    "tool_call_update": [
        487977,
        488299,
        488315,
        488489,
        488499,
        488544,
        488550,
        488558,
        488578,
        488659,
        488667,
        488727,
        488837,
        488934,
        488959,
    ],
}


def call_snippet(lines: list[str], start: int) -> str:
    """Return one pretty-printed emitter call using its balanced parentheses."""
    collected: list[str] = []
    depth = 0
    started = False
    for line in lines[start - 1 :]:
        collected.append(line)
        depth += line.count("(") - line.count(")")
        started = started or "(" in line
        if started and depth == 0:
            return "\n".join(collected)
    raise ValueError(f"unterminated emitter call at line {start}")


def has_raw_input(snippet: str) -> bool:
    return re.search(r"(?m)^\s*rawInput(?::|,)", snippet) is not None


def require_line(lines: list[str], line_number: int, fragment: str) -> None:
    actual = lines[line_number - 1]
    if fragment not in actual:
        raise ValueError(f"line {line_number} missing {fragment!r}: {actual!r}")


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {Path(sys.argv[0]).name} <acp-server.js> <package.json>", file=sys.stderr)
        return 2

    source_path = Path(sys.argv[1])
    package_path = Path(sys.argv[2])
    source_bytes = source_path.read_bytes()
    digest = hashlib.sha256(source_bytes).hexdigest()
    if digest != EXPECTED_SHA256:
        print(f"unexpected acp-server.js sha256: {digest}", file=sys.stderr)
        return 1

    package = json.loads(package_path.read_text())
    if package.get("name") != "@kiro/agent" or package.get("version") != EXPECTED_VERSION:
        print(f"unexpected package identity: {package.get('name')} {package.get('version')}", file=sys.stderr)
        return 1

    lines = source_bytes.decode().split("\n")
    found: dict[str, list[int]] = {"tool_call": [], "tool_call_update": []}
    for line_number, line in enumerate(lines, 1):
        if "this.toolCallEmitter.toolCallUpdate(" in line:
            found["tool_call_update"].append(line_number)
        elif "this.toolCallEmitter.toolCall(" in line:
            found["tool_call"].append(line_number)

    if found != EXPECTED_SITES:
        print(f"emitter site drift: {json.dumps(found, sort_keys=True)}", file=sys.stderr)
        return 1

    # The emitter boundary makes rawInput optional on initial calls and passes
    # update patches through unchanged.
    require_line(lines, 459243, "input.rawInput !== void 0")
    require_line(lines, 459249, "buildToolCallUpdateSessionUpdate")
    require_line(lines, 459250, 'sessionUpdate: "tool_call_update"')

    # SyncTool explicitly emits partial rawInput as AgentExecutionAction events.
    require_line(lines, 429386, "partial `rawInput`")
    require_line(lines, 429390, "emitStreamingAction(state2, event)")
    require_line(lines, 429393, 'type: "AgentExecutionAction"')
    require_line(lines, 444399, "rawInput: { path: chunk.path }")
    require_line(lines, 444615, "rawInput: { path: chunk.path }")
    require_line(lines, 445884, "rawInput: { path: chunk.path }")

    # ACPEventAdapter forwards those event values to both initial and update frames.
    require_line(lines, 488874, "event.rawInput ?? event.input")
    require_line(lines, 488923, "rawInput,")
    require_line(lines, 488937, "rawInput,")

    rows = []
    for kind, sites in found.items():
        for line_number in sites:
            rows.append(
                {
                    "kind": kind,
                    "line": line_number,
                    "raw_input": "present" if has_raw_input(call_snippet(lines, line_number)) else "absent",
                }
            )

    print(f"package=@kiro/agent@{EXPECTED_VERSION}")
    print(f"sha256={digest}")
    for row in rows:
        print(f"{row['kind']} line={row['line']} rawInput={row['raw_input']}")
    print("partial_producers=append:444399,write:444615,replace:445884")
    print("verdict=REPRESENTABLE absent=true partial=true")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
