#!/usr/bin/env python3
"""
Probe: _meta round-trip AND additional_directories runtime behavior on Kiro 2.4.1.

Two questions:
  Q1. Does Kiro echo client-supplied _meta back in responses?
  Q2. Does declaring additionalDirectories on session/new actually let the
      agent fs_read paths outside cwd?

Spawns kiro-cli-chat directly (no conductor needed; we log everything ourselves).
Writes wire log to /tmp/cyril-probe-meta-addl.log and prints findings.
"""

import json
import os
import pathlib
import subprocess
import sys
import tempfile
import threading
import time

KIRO = os.path.expanduser(
    "~/.local/share/kiro-research/binaries/2.4.1/kiro-cli-chat"
)
CWD = "/home/dwalleck/repos/cyril"
LOG_PATH = "/tmp/cyril-probe-meta-addl.log"

probe_dir = pathlib.Path(tempfile.mkdtemp(prefix="cyril-probe-addl-"))
marker = probe_dir / "MARKER.txt"
MAGIC = "ZUCCHINI_LIGHTHOUSE_42"
marker.write_text(MAGIC)

# Auto-discovery test: place steering-style files in probe_dir to see if
# additionalDirectories causes Kiro to auto-include them in /context.
AGENTS_MAGIC = "TRIANGLE_BICYCLE_AGENTS"
STEERING_MAGIC = "PURPLE_TYPEWRITER_STEERING"
(probe_dir / "AGENTS.md").write_text(
    f"# Project Agents\n\nMarker: {AGENTS_MAGIC}\n"
)
(probe_dir / ".kiro" / "steering").mkdir(parents=True)
(probe_dir / ".kiro" / "steering" / "marker.md").write_text(
    f"# Steering\n\nMarker: {STEERING_MAGIC}\n"
)
print(f"[setup] probe_dir AGENTS.md      → {AGENTS_MAGIC}")
print(f"[setup] probe_dir steering/marker → {STEERING_MAGIC}")

META_MARKER = "META_ROUNDTRIP_PROBE_777"


def main() -> int:
    log_file = open(LOG_PATH, "w")

    print(f"[setup] probe_dir = {probe_dir}")
    print(f"[setup] marker    = {marker} → {MAGIC!r}")
    print(f"[setup] wire log  = {LOG_PATH}")
    print()

    proc = subprocess.Popen(
        [KIRO, "acp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        cwd=CWD,
    )

    # Background reader collects every line from agent stdout.
    incoming: list[dict] = []
    incoming_lock = threading.Lock()

    def reader():
        while True:
            line = proc.stdout.readline()
            if not line:
                return
            text = line.decode("utf-8", errors="replace").rstrip("\n")
            log_file.write(f"S→C {text}\n")
            log_file.flush()
            try:
                frame = json.loads(text)
                with incoming_lock:
                    incoming.append(frame)
            except json.JSONDecodeError:
                pass

    threading.Thread(target=reader, daemon=True).start()

    next_id = [1]

    def send(method, params, want_response=True):
        msg = {"jsonrpc": "2.0", "method": method, "params": params}
        if want_response:
            msg["id"] = next_id[0]
            next_id[0] += 1
        line = json.dumps(msg)
        log_file.write(f"C→S {line}\n")
        log_file.flush()
        assert proc.stdin is not None
        proc.stdin.write((line + "\n").encode("utf-8"))
        proc.stdin.flush()
        return msg.get("id")

    def send_response(req_id, result):
        msg = {"jsonrpc": "2.0", "id": req_id, "result": result}
        line = json.dumps(msg)
        log_file.write(f"C→S {line}\n")
        log_file.flush()
        assert proc.stdin is not None
        proc.stdin.write((line + "\n").encode("utf-8"))
        proc.stdin.flush()

    # Auto-approval thread for session/request_permission
    permission_seen = [False]

    def auto_approve_loop():
        seen_ids = set()
        while True:
            with incoming_lock:
                for f in incoming:
                    if (
                        f.get("method") == "session/request_permission"
                        and f.get("id") not in seen_ids
                    ):
                        seen_ids.add(f["id"])
                        # Pick the first "allow_once"-like option
                        options = f.get("params", {}).get("options", [])
                        allow = next(
                            (o for o in options if o.get("kind") == "allow_once"),
                            options[0] if options else None,
                        )
                        if allow:
                            send_response(
                                f["id"],
                                {"outcome": {"outcome": "selected", "optionId": allow["optionId"]}},
                            )
                            permission_seen[0] = True
            time.sleep(0.2)

    threading.Thread(target=auto_approve_loop, daemon=True).start()

    def wait_for_response(req_id, timeout=15.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            with incoming_lock:
                for frame in incoming:
                    if frame.get("id") == req_id and (
                        "result" in frame or "error" in frame
                    ):
                        return frame
            time.sleep(0.1)
        return None

    def drain(seconds):
        time.sleep(seconds)

    # ─── 1. initialize ─────────────────────────────────────────────────
    init_id = send(
        "initialize",
        {
            "protocolVersion": 1,
            "clientCapabilities": {
                "fs": {"readTextFile": False, "writeTextFile": False},
                "terminal": False,
            },
            "clientInfo": {"name": "cyril-probe", "version": "0.0.1"},
        },
    )
    init_resp = wait_for_response(init_id, timeout=10)
    if not init_resp:
        print("[ERROR] no response to initialize within 10s")
        proc.terminate()
        return 1
    if "error" in init_resp:
        print(f"[ERROR] initialize failed: {init_resp['error']}")
        proc.terminate()
        return 1
    print("[1] initialize OK")

    # ─── 2. session/new with _meta + additionalDirectories ─────────────
    # Use env var SKIP_ADDL_DIRS=1 to run the control case (no additionalDirectories).
    new_params = {
        "cwd": CWD,
        "mcpServers": [],
        "_meta": {
            "cyril.probe.id": META_MARKER,
            "cyril.probe.timestamp": "2026-05-23T18:00:00Z",
        },
    }
    if not os.environ.get("SKIP_ADDL_DIRS"):
        new_params["additionalDirectories"] = [str(probe_dir)]
        print("[2] (including additionalDirectories in session/new)")
    else:
        print("[2] (CONTROL: NOT including additionalDirectories)")
    new_id = send("session/new", new_params)
    new_resp = wait_for_response(new_id, timeout=15)
    if not new_resp:
        print("[ERROR] no response to session/new within 15s")
        proc.terminate()
        return 1
    if "error" in new_resp:
        print(f"[ERROR] session/new failed: {new_resp['error']}")
        proc.terminate()
        return 1

    # Inspect response for _meta echo
    result = new_resp["result"]
    session_id = result.get("sessionId")
    top_meta = result.get("_meta")
    print(f"[2] session/new OK → sessionId = {session_id}")
    print(f"    response top-level _meta = {top_meta!r}")

    found_marker = META_MARKER in json.dumps(result)
    print(
        f"    META_MARKER ('{META_MARKER}') anywhere in response? "
        f"{'YES (echoed!)' if found_marker else 'NO (silently dropped)'}"
    )

    # Drain any post-session/new notifications (commands/available, etc.)
    drain(3)

    # ─── 3. prompt the agent to read the marker file ───────────────────
    prompt_id = send(
        "session/prompt",
        {
            "sessionId": session_id,
            "prompt": [
                {
                    "type": "text",
                    "text": (
                        f"Use fs_read to read the file {marker} and report "
                        "the exact content. Be brief — one sentence."
                    ),
                }
            ],
        },
    )

    # The prompt response only comes back at turn-end. Meanwhile we get
    # streaming notifications. Wait up to 60s for the turn to complete.
    prompt_resp = wait_for_response(prompt_id, timeout=60)
    if not prompt_resp:
        print("[ERROR] no response to session/prompt within 60s")
    elif "error" in prompt_resp:
        print(f"[ERROR] session/prompt failed: {prompt_resp['error']}")
    else:
        print(f"[3] session/prompt OK — stopReason: {prompt_resp['result'].get('stopReason')}")

    # ─── 3b. invoke /context to see breakdown ──────────────────────────
    ctx_id = send(
        "_kiro.dev/commands/execute",
        {
            "sessionId": session_id,
            "command": {"command": "context", "args": {}},
        },
    )
    ctx_resp = wait_for_response(ctx_id, timeout=10)
    if ctx_resp and "result" in ctx_resp:
        breakdown = ctx_resp["result"].get("data", {}).get("breakdown", {})
        ctx_files = breakdown.get("contextFiles", {})
        items = ctx_files.get("items", [])
        print()
        print(f"[3b] /context breakdown:")
        print(f"     contextFiles count: {len(items)}")
        for item in items:
            print(f"       - {item.get('name')}  ({item.get('tokens')} tok, "
                  f"auto_included={item.get('auto_included', False)})")
        # Check if probe_dir paths appear (auto-discovery test)
        probe_in_ctx = any(str(probe_dir) in item.get("name", "") for item in items)
        print(f"     probe_dir referenced in contextFiles?   {'YES' if probe_in_ctx else 'NO'}")
        # Check if the probe_dir AGENTS.md or steering file was auto-loaded
        full_ctx = json.dumps(ctx_resp["result"])
        print(f"     probe_dir AGENTS.md auto-loaded?         "
              f"{'YES' if AGENTS_MAGIC in full_ctx else 'NO'}")
        print(f"     probe_dir steering/marker auto-loaded?   "
              f"{'YES' if STEERING_MAGIC in full_ctx else 'NO'}")
    else:
        print("[3b] /context returned no result")

    # ─── 4. analyze captured notifications ─────────────────────────────
    print()
    print("=== Analysis ===")

    with incoming_lock:
        frames = list(incoming)

    # Did the agent execute fs_read on our marker file?
    fs_read_attempted = False
    fs_read_path = None
    fs_read_completed = False
    fs_read_output = None
    permission_request = None

    for f in frames:
        if f.get("method") == "session/update":
            update = f.get("params", {}).get("update", {})
            kind = update.get("kind")
            raw_input = update.get("rawInput", {})
            raw_output = update.get("rawOutput", {})
            session_update = update.get("sessionUpdate")

            if kind == "read" and "operations" in raw_input:
                for op in raw_input["operations"]:
                    if op.get("path", "").startswith(str(probe_dir)):
                        fs_read_attempted = True
                        fs_read_path = op["path"]
                        if session_update == "tool_call_update":
                            fs_read_completed = update.get("status") == "completed"
                            items = raw_output.get("items", [])
                            if items:
                                fs_read_output = items[0].get("Text", "")
        elif f.get("method") == "session/request_permission":
            params = f.get("params", {})
            permission_request = params

    print(f"  fs_read attempted on probe dir?  {'YES' if fs_read_attempted else 'NO'}")
    if fs_read_attempted:
        print(f"  fs_read path                    : {fs_read_path}")
        print(f"  fs_read completed?              : {'YES' if fs_read_completed else 'NO'}")
        if fs_read_output:
            contains_magic = MAGIC in fs_read_output
            print(f"  fs_read output had magic word?  : {'YES' if contains_magic else 'NO'}")
            print(f"  fs_read raw output (first 100ch): {fs_read_output[:100]!r}")

    if permission_request:
        print(f"  permission requested?           : YES")
        print(f"    options: {[o.get('name') for o in permission_request.get('options', [])]}")

    # Check final agent reply text for the magic word
    agent_text = ""
    for f in frames:
        if f.get("method") == "session/update":
            update = f.get("params", {}).get("update", {})
            if update.get("sessionUpdate") == "agent_message_chunk":
                content = update.get("content", {})
                if content.get("type") == "text":
                    agent_text += content.get("text", "")
    print()
    print(f"  Agent's final reply (first 300 chars):")
    print(f"  {agent_text[:300]!r}")
    print(f"  Magic word in reply? {'YES' if MAGIC in agent_text else 'NO'}")

    # ─── 5. teardown ───────────────────────────────────────────────────
    proc.stdin.close()
    try:
        proc.wait(timeout=3)
    except subprocess.TimeoutExpired:
        proc.terminate()
        proc.wait()

    log_file.close()
    print()
    print(f"Wire log saved to {LOG_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
