#!/usr/bin/env python3
"""
Probe: does Kiro emit THINKING over the ACP wire, and did 2.5.0 change it?

2.5.0 changelog headline: "Thinking display -- see the agent's reasoning process
in real time. Enabled by default, toggle via /settings > Display > Show thinking."

Question for cyril (an ACP client): does the reasoning surface as ACP session
updates (agent_thought_chunk, or a _kiro.dev/* variant), or is it purely v2-TUI-side?

Baseline: 2.4.1 captures show thinking stayed BELOW the ACP wire (only
agent_message_chunk / tool_call_chunk variants, no agent_thought_chunk).

Run the SAME script against both binaries (same day, same backend) to isolate the
binary axis:
    KIRO_BIN=~/.local/bin/kiro-cli-chat python3 probe-thinking-2.5.0.py   # 2.5.0
    KIRO_BIN=~/.local/share/kiro-research/binaries/2.4.1/kiro-cli-chat python3 probe-thinking-2.5.0.py
"""

import json
import os
import subprocess
import sys
import threading
import time
from collections import Counter

KIRO = os.path.expanduser(os.environ.get("KIRO_BIN", "~/.local/bin/kiro-cli-chat"))
CWD = "/home/dwalleck/repos/cyril"
TAG = os.environ.get("PROBE_TAG", os.path.basename(os.path.dirname(KIRO)) or "live")
LOG_PATH = f"/tmp/cyril-probe-thinking-{TAG}.log"

# A prompt that strongly induces multi-step reasoning without needing tools.
PROMPT = (
    "Think carefully step by step before answering. "
    "I have 3 boxes. Box A has twice as many marbles as Box B. "
    "Box C has 5 fewer than Box A. Together they have 39 marbles. "
    "Reason through it and tell me how many marbles are in each box. "
    "Do NOT use any tools or read any files; just reason it out."
)


def main() -> int:
    log_file = open(LOG_PATH, "w")
    print(f"[setup] binary   = {KIRO}")
    print(f"[setup] wire log = {LOG_PATH}")
    print()

    proc = subprocess.Popen(
        [KIRO, "acp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        cwd=CWD,
    )

    incoming: list[dict] = []
    lock = threading.Lock()

    def reader():
        while True:
            line = proc.stdout.readline()
            if not line:
                return
            text = line.decode("utf-8", errors="replace").rstrip("\n")
            log_file.write(f"S->C {text}\n")
            log_file.flush()
            try:
                frame = json.loads(text)
                with lock:
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
        log_file.write(f"C->S {line}\n")
        log_file.flush()
        proc.stdin.write((line + "\n").encode())
        proc.stdin.flush()
        return msg.get("id")

    def send_response(req_id, result):
        msg = {"jsonrpc": "2.0", "id": req_id, "result": result}
        proc.stdin.write((json.dumps(msg) + "\n").encode())
        proc.stdin.flush()
        log_file.write(f"C->S {json.dumps(msg)}\n")
        log_file.flush()

    def auto_approve():
        seen = set()
        while True:
            with lock:
                frames = list(incoming)
            for f in frames:
                if f.get("method") == "session/request_permission" and f.get("id") not in seen:
                    seen.add(f["id"])
                    opts = f.get("params", {}).get("options", [])
                    allow = next((o for o in opts if o.get("kind") == "allow_once"), opts[0] if opts else None)
                    if allow:
                        send_response(f["id"], {"outcome": {"outcome": "selected", "optionId": allow["optionId"]}})
            time.sleep(0.15)

    threading.Thread(target=auto_approve, daemon=True).start()

    def wait_for(req_id, timeout=20.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            with lock:
                for f in incoming:
                    if f.get("id") == req_id and ("result" in f or "error" in f):
                        return f
            time.sleep(0.1)
        return None

    # 1. initialize
    rid = send("initialize", {
        "protocolVersion": 1,
        "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}, "terminal": False},
        "clientInfo": {"name": os.environ.get("CLIENT_NAME", "cyril-probe"), "version": "0.0.1"},
    })
    r = wait_for(rid, 10)
    if not r or "error" in r:
        print(f"[ERROR] initialize failed: {r}")
        proc.terminate(); return 1
    print("[1] initialize OK")

    # 2. session/new — capture advertised models
    rid = send("session/new", {"cwd": CWD, "mcpServers": []})
    r = wait_for(rid, 20)
    if not r or "error" in r:
        print(f"[ERROR] session/new failed: {r}")
        proc.terminate(); return 1
    result = r["result"]
    session_id = result.get("sessionId")
    models = result.get("models") or result.get("modelState") or {}
    print(f"[2] session/new OK  sessionId={session_id}")
    # dump model option list if present
    avail = []
    if isinstance(models, dict):
        avail = models.get("availableModels") or models.get("available_models") or []
    print(f"    advertised models: {json.dumps(models)[:600]}")

    # 3. pick a thinking-capable model (prefer opus/claude-4) via commands/execute
    chosen = None
    for m in avail:
        name = (m.get("modelId") or m.get("name") or m.get("value") or m.get("label") or "").lower()
        if "opus" in name:
            chosen = m
            break
    if not chosen and avail:
        chosen = avail[0]
    if chosen:
        val = chosen.get("modelId") or chosen.get("value") or chosen.get("name")
        print(f"[3] selecting model: {val}")
        send("_kiro.dev/commands/execute",
             {"command": {"command": "model", "args": {"value": val}}}, want_response=False)
        time.sleep(1.5)
    else:
        print("[3] no model list to choose from; using session default")

    # 3b. optionally force extended thinking via /effort (model-conditional)
    effort = os.environ.get("PROBE_EFFORT")
    if effort:
        print(f"[3b] setting effort: {effort}")
        send("_kiro.dev/commands/execute",
             {"command": {"command": "effort", "args": {"value": effort}}}, want_response=False)
        time.sleep(1.5)

    # mark cutoff so we only categorize updates from the prompt turn onward
    with lock:
        cutoff = len(incoming)

    # 4. send the reasoning prompt
    print("[4] sending reasoning prompt ...")
    rid = send("session/prompt", {
        "sessionId": session_id,
        "prompt": [{"type": "text", "text": PROMPT}],
    })
    r = wait_for(rid, 90)
    print(f"[4] session/prompt returned: stop_reason={ (r or {}).get('result',{}).get('stopReason') }")
    time.sleep(1.0)

    # 5. categorize everything received during the turn
    with lock:
        turn = incoming[cutoff:]

    method_counts = Counter()
    update_variants = Counter()
    thought_hits = []
    for f in turn:
        m = f.get("method")
        if m:
            method_counts[m] += 1
        params = f.get("params") or {}
        upd = params.get("update") or {}
        variant = upd.get("sessionUpdate")
        if variant:
            update_variants[variant] += 1
        # scan the whole frame text for thought/thinking signal
        blob = json.dumps(f).lower()
        if "thought" in blob or "thinking" in blob or "reasoning" in blob:
            thought_hits.append(f)

    print("\n========== RESULTS (" + TAG + ") ==========")
    print("methods during turn:")
    for k, v in method_counts.most_common():
        print(f"   {v:5d}  {k}")
    print("sessionUpdate variants during turn:")
    for k, v in update_variants.most_common():
        print(f"   {v:5d}  {k}")
    print(f"\nframes containing thought/thinking/reasoning: {len(thought_hits)}")
    for f in thought_hits[:6]:
        print("   " + json.dumps(f)[:400])
    print("\nfull wire log: " + LOG_PATH)

    proc.terminate()
    return 0


if __name__ == "__main__":
    sys.exit(main())
