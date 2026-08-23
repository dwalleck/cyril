#!/usr/bin/env python3
"""Independent Python oracle for the Rust platform probe."""

from __future__ import annotations

import hmac
import json
import os
from pathlib import Path
import signal
import struct
import subprocess
import sys
import tempfile
import time

MAX_FRAME_LENGTH = 1024 * 1024
CREDENTIAL = bytes([0x5A] * 32)


def frame(value: dict[str, object]) -> bytes:
    payload = json.dumps(value, separators=(",", ":")).encode()
    return struct.pack(">I", len(payload)) + payload


def evaluate(data: bytes) -> str:
    if len(data) < 4:
        return "malformed_frame"
    (announced,) = struct.unpack(">I", data[:4])
    if announced > MAX_FRAME_LENGTH:
        return "frame_too_large"
    payload = data[4:]
    if len(payload) != announced:
        return "malformed_frame"
    try:
        request = json.loads(payload)
    except (json.JSONDecodeError, UnicodeDecodeError):
        return "malformed_frame"
    provided = request.get("auth")
    if not isinstance(provided, list):
        return "unauthorized"
    try:
        auth = bytes(provided)
    except (TypeError, ValueError):
        return "unauthorized"
    if len(auth) != len(CREDENTIAL) or not hmac.compare_digest(auth, CREDENTIAL):
        return "unauthorized"
    if request.get("version") != 1:
        return "unsupported_version"
    operation = request.get("operation")
    if operation in ("health", "shutdown"):
        return "ok"
    return "unknown_operation"


def child(marker: Path) -> None:
    grandchild = subprocess.Popen(["sleep", "30"])
    marker.write_text(str(grandchild.pid))
    grandchild.wait()


def wait_for(path: Path) -> None:
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if path.exists():
            return
        time.sleep(0.01)
    raise RuntimeError(f"timed out waiting for {path}")


def process_tree_oracle() -> dict[str, object]:
    with tempfile.TemporaryDirectory(prefix="cyril-j7um-oracle-") as root:
        marker = Path(root) / "grandchild.pid"
        holder = subprocess.Popen(
            [sys.executable, str(Path(__file__).resolve()), "--tree-child", str(marker)],
            start_new_session=True,
        )
        wait_for(marker)
        grandchild_pid = int(marker.read_text())
        started = time.monotonic()
        os.killpg(holder.pid, signal.SIGKILL)
        holder.wait(timeout=2)
        process_path = Path(f"/proc/{grandchild_pid}")
        deadline = time.monotonic() + 2
        while process_path.exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        return {
            "grandchild_reaped": not process_path.exists(),
            "kill_completed_within_two_seconds": time.monotonic() - started < 2,
            "mechanism": "python_start_new_session_killpg",
        }


def main() -> None:
    if len(sys.argv) == 3 and sys.argv[1] == "--tree-child":
        child(Path(sys.argv[2]))
        return
    invalid = frame({"auth": [0x11] * 32, "operation": "health", "version": 1})
    valid = frame({"auth": list(CREDENTIAL), "operation": "health", "version": 1})
    unknown = frame({"auth": list(CREDENTIAL), "operation": "future", "version": 1})
    unsupported = frame(
        {"auth": list(CREDENTIAL), "operation": "health", "version": 2}
    )
    result = {
        "framing": {
            "invalid_auth": evaluate(invalid),
            "malformed": evaluate(b"\x00\x00\x00\x02{"),
            "missing_auth": evaluate(frame({"operation": "health", "version": 1})),
            "oversized": evaluate(struct.pack(">I", MAX_FRAME_LENGTH + 1)),
            "unknown_operation": evaluate(unknown),
            "unsupported_version": evaluate(unsupported),
            "valid_health": evaluate(valid),
        },
        "process_tree": process_tree_oracle(),
    }
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
