#!/usr/bin/env python3
"""
Probe: end-to-end verification of the cargo PATH shim (experiments/agent-shims/)
through a live kiro-cli-chat 2.5.1 ACP session.

Setup: prepend the shim dir to PATH in kiro's environment (what cyril would do
when spawning the bridge), then prompt the agent to run a plain
`cargo test -p cyril-core`. The agent does not know about the shim.

Questions:
  Q1. Interception — does the agent's ordinary `cargo` command resolve to the
      shim? (rawOutput stdout starts with "[cargo-shim]")
  Q2. Context economy — does the model-visible stdout exclude the per-test
      noise ("... ok" lines) that the display channel carries?
  Q3. Display channel — does the FULL cargo output stream to the wire as
      tool_call_update content (user-visible in cyril)?
  Q4. agent_notes — does the shim's note (full-log pointer) arrive?
  Q5. Behavior — does the agent correctly report the test outcome from the
      summary alone?

Writes wire log to /tmp/cyril-probe-cargo-shim.log and prints findings.
"""

import json
import os
import subprocess
import sys
import threading
import time

KIRO = os.path.expanduser(
    "~/.local/share/kiro-research/binaries/2.5.1/kiro-cli-chat"
)
CWD = "/home/dwalleck/repos/cyril"
SHIM_DIR = os.path.join(CWD, "experiments", "agent-shims")
LOG_PATH = "/tmp/cyril-probe-cargo-shim.log"


def main() -> int:
    log_file = open(LOG_PATH, "w")
    env = dict(os.environ)
    env["PATH"] = SHIM_DIR + ":" + env["PATH"]
    print(f"[setup] shim dir prepended to PATH: {SHIM_DIR}")
    print(f"[setup] wire log = {LOG_PATH}")
    print()

    proc = subprocess.Popen(
        [KIRO, "acp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        cwd=CWD,
        env=env,
    )

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

    def auto_approve_loop():
        seen_ids = set()
        while True:
            with incoming_lock:
                frames = list(incoming)
            for f in frames:
                if (
                    f.get("method") == "session/request_permission"
                    and f.get("id") not in seen_ids
                ):
                    seen_ids.add(f["id"])
                    options = f.get("params", {}).get("options", [])
                    allow = next(
                        (o for o in options if o.get("kind") == "allow_once"),
                        options[0] if options else None,
                    )
                    if allow:
                        send_response(
                            f["id"],
                            {
                                "outcome": {
                                    "outcome": "selected",
                                    "optionId": allow["optionId"],
                                }
                            },
                        )
                        print(f"[perm] auto-approved {allow['optionId']!r}")
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
    if not wait_for_response(init_id, timeout=10):
        print("[ERROR] no response to initialize")
        proc.terminate()
        return 1
    print("[1] initialize OK")

    # ─── 2. session/new ────────────────────────────────────────────────
    new_id = send("session/new", {"cwd": CWD, "mcpServers": []})
    new_resp = wait_for_response(new_id, timeout=15)
    if not new_resp or "error" in new_resp:
        print(f"[ERROR] session/new failed: {new_resp}")
        proc.terminate()
        return 1
    session_id = new_resp["result"]["sessionId"]
    print(f"[2] session/new OK → {session_id}")
    time.sleep(2)

    # ─── 3. prompt: plain cargo test ───────────────────────────────────
    prompt_id = send(
        "session/prompt",
        {
            "sessionId": session_id,
            "prompt": [
                {
                    "type": "text",
                    "text": (
                        "Run `cargo test -p cyril-core` and tell me whether "
                        "the tests passed."
                    ),
                }
            ],
        },
    )
    print("[3] prompt sent — waiting for turn to complete (up to 540s)…")
    prompt_resp = wait_for_response(prompt_id, timeout=540)
    if not prompt_resp:
        print("[ERROR] turn did not complete within 540s")
        proc.terminate()
        return 1
    print(f"[3] turn complete: {prompt_resp.get('result')}")
    time.sleep(2)
    proc.terminate()

    # ─── 4. analysis ───────────────────────────────────────────────────
    with incoming_lock:
        frames = list(incoming)

    # Find the execute tool_call_update carrying the final Json result.
    exec_result = None
    for f in frames:
        upd = f.get("params", {}).get("update", {})
        if upd.get("sessionUpdate") != "tool_call_update":
            continue
        for item in (upd.get("rawOutput") or {}).get("items", []):
            if isinstance(item, dict) and "Json" in item:
                j = item["Json"]
                if "stdout" in j:
                    exec_result = j
    # Concatenate all streamed tool_call_update content text.
    streamed = ""
    for f in frames:
        upd = f.get("params", {}).get("update", {})
        if upd.get("sessionUpdate") != "tool_call_update":
            continue
        for c in upd.get("content") or []:
            inner = c.get("content", {})
            if isinstance(inner, dict):
                streamed += inner.get("text", "")
    agent_text = "".join(
        f["params"]["update"]["content"].get("text", "")
        for f in frames
        if f.get("method") == "session/update"
        and f["params"].get("update", {}).get("sessionUpdate")
        == "agent_message_chunk"
        and isinstance(f["params"]["update"].get("content"), dict)
    )

    print()
    print("=" * 70)
    print("FINDINGS")
    print("=" * 70)
    if not exec_result:
        print("[ERROR] no execute tool result with stdout found on the wire")
        return 1

    stdout = exec_result.get("stdout", "")
    notes = exec_result.get("agent_notes", "")
    ok_lines_stdout = stdout.count("... ok")
    ok_lines_streamed = streamed.count("... ok")

    print(f"Q1 shim intercepted plain `cargo`? "
          f"{'YES' if '[cargo-shim]' in stdout else 'NO'}")
    print(f"Q2 context economy:")
    print(f"   model-visible stdout: {len(stdout)} chars, "
          f"{ok_lines_stdout} '... ok' lines")
    print(f"   streamed display:     {len(streamed)} chars, "
          f"{ok_lines_streamed} '... ok' lines")
    print(f"Q3 full output streamed to wire (display)? "
          f"{'YES' if ok_lines_streamed > 0 and len(streamed) > len(stdout) else 'NO'}")
    print(f"Q4 agent_notes delivered? "
          f"{'YES' if 'Full log:' in notes else 'NO'} → {notes[:160]!r}")
    print(f"Q5 agent reply: {agent_text[:400]!r}")
    print()
    print(f"model-visible stdout was:\n{stdout}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
