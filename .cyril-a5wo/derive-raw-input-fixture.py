#!/usr/bin/env python3
"""Derive the cyril-a5wo rawInput fixture from pinned KAS source sites."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

EXPECTED_SOURCE_SHA256 = "965ae084945a48eb73fe2049feed7e3deb6fb8d8a9cf49aa4713b172ed3fb70a"
EXPECTED_PACKAGE_VERSION = "0.38.7"
SESSION_ID = "source-derived-raw-input"
PARTIAL_PATH = "/workspace/src/space ü.rs"


def require_line(lines: list[str], line_number: int, fragment: str) -> None:
    actual = lines[line_number - 1]
    if fragment not in actual:
        raise ValueError(f"line {line_number} missing {fragment!r}: {actual!r}")


def source_gate(source: Path) -> None:
    source_bytes = source.read_bytes()
    digest = hashlib.sha256(source_bytes).hexdigest()
    if digest != EXPECTED_SOURCE_SHA256:
        raise ValueError(f"unexpected acp-server.js sha256: {digest}")

    package_path = source.parents[2] / "package.json"
    package = json.loads(package_path.read_text())
    if package.get("name") != "@kiro/agent" or package.get("version") != EXPECTED_PACKAGE_VERSION:
        raise ValueError(f"unexpected package identity: {package.get('name')} {package.get('version')}")

    lines = source_bytes.decode().split("\n")
    # user_input initial tool_call and completion both omit rawInput.
    require_line(lines, 488444, "this.toolCallEmitter.toolCall({")
    require_line(lines, 488448, 'status: "pending"')
    require_line(lines, 488489, "this.toolCallEmitter.toolCallUpdate({")
    require_line(lines, 488491, 'status: "completed"')
    # Streaming replace first emits path only, then expands the object as
    # newStr/oldStr chunks arrive.
    require_line(lines, 445879, "this.emitStreamingAction(state2, {")
    require_line(lines, 445884, "rawInput: { path: chunk.path }")
    require_line(lines, 445891, "const rawInput = { path: chunk.path, newStr: chunk.newStr }")
    require_line(lines, 445892, "rawInput.oldStr = chunk.oldStr")
    # ACPEventAdapter forwards the streamed rawInput to initial and update frames.
    require_line(lines, 488917, "this.toolCallEmitter.toolCall(")
    require_line(lines, 488923, "rawInput,")
    require_line(lines, 488934, "this.toolCallEmitter.toolCallUpdate({")
    require_line(lines, 488937, "rawInput,")


def frame(update: dict[str, object]) -> dict[str, object]:
    return {
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {"sessionId": SESSION_ID, "update": update},
    }


def fixture_bytes() -> bytes:
    frames = [
        frame(
            {
                "sessionUpdate": "tool_call",
                "toolCallId": "source-absent",
                "title": "Choose an option",
                "kind": "other",
                "status": "pending",
                "_meta": {"kiro": {"toolId": "user_input"}},
            }
        ),
        frame(
            {
                "sessionUpdate": "tool_call_update",
                "toolCallId": "source-absent",
                "status": "completed",
            }
        ),
        frame(
            {
                "sessionUpdate": "tool_call",
                "toolCallId": "source-partial",
                "title": "Replacing text",
                "kind": "edit",
                "status": "in_progress",
                "rawInput": {"path": PARTIAL_PATH},
                "locations": [{"path": PARTIAL_PATH}],
            }
        ),
        frame(
            {
                "sessionUpdate": "tool_call_update",
                "toolCallId": "source-partial",
                "status": "in_progress",
                "rawInput": {"path": PARTIAL_PATH, "newStr": "partial replacement"},
                "locations": [{"path": PARTIAL_PATH}],
            }
        ),
    ]
    text = "".join(
        json.dumps(item, ensure_ascii=False, separators=(",", ":")) + "\n" for item in frames
    )
    return text.encode()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", required=True, type=Path)
    output = parser.add_mutually_exclusive_group(required=True)
    output.add_argument("--write", type=Path)
    output.add_argument("--check", type=Path)
    args = parser.parse_args()

    try:
        source_gate(args.source)
        expected = fixture_bytes()
        if args.write is not None:
            args.write.parent.mkdir(parents=True, exist_ok=True)
            args.write.write_bytes(expected)
            action = "wrote"
            path = args.write
        else:
            actual = args.check.read_bytes()
            if actual != expected:
                raise ValueError(f"fixture differs from source derivation: {args.check}")
            action = "matched"
            path = args.check
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(error, file=sys.stderr)
        return 1

    digest = hashlib.sha256(expected).hexdigest()
    print(f"{action} {path}")
    print(f"frames=4 source_sha256={EXPECTED_SOURCE_SHA256} fixture_sha256={digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
