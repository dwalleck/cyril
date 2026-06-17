#!/usr/bin/env python3
"""Probe Kiro 2.7.0 queue steering + /goal + _kiro/session/context on the ACP wire.

Run against BOTH binaries (same-day isolation):
  KIRO_BIN=~/.local/share/kiro-research/binaries/2.7.0/kiro-cli-chat PROBE_TAG=2.7.0 \
      python3 probe-steer-goal-2.7.0.py
  KIRO_BIN=~/.local/share/kiro-research/binaries/2.6.1/kiro-cli-chat PROBE_TAG=2.6.1 \
      python3 probe-steer-goal-2.7.0.py

Tests:
  A. commands/available — does `goal` (or steer-related) command appear? input spec?
  B. _session/steer mid-turn — agent redirects at tool boundary? what notifications echo back?
  C. _session/steer/clear — queued steer is dropped, turn proceeds unredirected
  D. goal via _kiro.dev/commands/execute — goal/status notification stream
  E. _kiro/session/context — new method, shape of response
"""
import json, os, subprocess, threading, time, sys

KIRO = os.path.expanduser(os.environ.get("KIRO_BIN", "~/.local/bin/kiro-cli-chat"))
CWD = os.getcwd()
TAG = os.environ.get("PROBE_TAG", "x")
LOG_PATH = f"/tmp/cyril-probe-steer-goal-{TAG}.log"
MODEL = os.environ.get("PROBE_MODEL", "claude-haiku-4.5")

log_file = open(LOG_PATH, "w")
proc = subprocess.Popen([KIRO, "acp"], stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, cwd=CWD)
assert proc.stdin is not None and proc.stdout is not None
incoming: list[dict] = []
lock = threading.Lock()
next_id = [1]


def reader():
    while True:
        line = proc.stdout.readline()
        if not line:
            return
        text = line.decode("utf-8", errors="replace").rstrip("\n")
        log_file.write(f"S->C {text}\n"); log_file.flush()
        try:
            with lock:
                incoming.append(json.loads(text))
        except json.JSONDecodeError:
            pass


def send(method, params, want_response=True):
    msg = {"jsonrpc": "2.0", "method": method, "params": params}
    if want_response:
        msg["id"] = next_id[0]; next_id[0] += 1
    log_file.write(f"C->S {json.dumps(msg)}\n"); log_file.flush()
    proc.stdin.write((json.dumps(msg) + "\n").encode()); proc.stdin.flush()
    return msg.get("id")


def send_response(req_id, result):
    msg = {"jsonrpc": "2.0", "id": req_id, "result": result}
    proc.stdin.write((json.dumps(msg) + "\n").encode()); proc.stdin.flush()
    log_file.write(f"C->S {json.dumps(msg)}\n"); log_file.flush()


approved = []


def auto_approve():
    seen = set()
    while True:
        with lock:
            frames = list(incoming)
        for f in frames:
            if f.get("method") == "session/request_permission" and f.get("id") not in seen:
                seen.add(f["id"])
                opts = f.get("params", {}).get("options", [])
                allow = next((o for o in opts if o.get("kind") == "allow_once"),
                             opts[0] if opts else None)
                if allow:
                    send_response(f["id"], {"outcome": {"outcome": "selected",
                                                        "optionId": allow["optionId"]}})
                    approved.append(time.time())
        time.sleep(0.05)


def wait_for(req_id, timeout=300.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        with lock:
            for f in incoming:
                if f.get("id") == req_id and ("result" in f or "error" in f):
                    return f
        time.sleep(0.05)
    return None


def frames_since(idx):
    with lock:
        return list(incoming[idx:]), len(incoming)


def mark():
    with lock:
        return len(incoming)


def summarize_turn(frames):
    variants, exts, tool_calls, agent_text = {}, {}, 0, []
    for f in frames:
        m = f.get("method")
        if m == "session/update":
            u = f.get("params", {}).get("update", {})
            v = u.get("sessionUpdate")
            variants[v] = variants.get(v, 0) + 1
            if v == "tool_call":
                tool_calls += 1
            if v == "agent_message_chunk" and u.get("content", {}).get("type") == "text":
                agent_text.append(u["content"]["text"])
        elif m and m not in ("session/request_permission",):
            exts[m] = exts.get(m, 0) + 1
    return variants, exts, tool_calls, "".join(agent_text)


threading.Thread(target=reader, daemon=True).start()
threading.Thread(target=auto_approve, daemon=True).start()

print(f"[setup] binary={KIRO}  tag={TAG}  log={LOG_PATH}")
rid = send("initialize", {"protocolVersion": 1,
    "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False},
                           "terminal": False},
    "clientInfo": {"name": "cyril", "version": "0.2.0"}})
r = wait_for(rid, 20)
if not r:
    print("[ERROR] initialize timed out"); sys.exit(1)
init = r.get("result", {})
print(f"[init] agentInfo={json.dumps(init.get('agentInfo'))}")
print(f"[init] capabilities={json.dumps(init.get('agentCapabilities'))[:400]}")

rid = send("session/new", {"cwd": CWD, "mcpServers": []})
r = wait_for(rid, 30)
if not r or "error" in r:
    print(f"[ERROR] session/new: {r}"); sys.exit(1)
session_id = r["result"]["sessionId"]
print(f"[ok] session={session_id}")

# ---- A. commands/available ----
time.sleep(3.0)
with lock:
    frames = list(incoming)
cmd_names = []
goal_spec = None
for f in frames:
    if "commands/available" in str(f.get("method", "")):
        for c in f.get("params", {}).get("commands", []):
            cmd_names.append(c.get("name"))
            if c.get("name") == "goal":
                goal_spec = c
print(f"\n===== A. commands/available ({len(cmd_names)}) =====")
print(" ".join(sorted(cmd_names)))
print(f"goal spec: {json.dumps(goal_spec)}")

# set model (must be request with sessionId)
rid = send("_kiro.dev/commands/execute",
           {"command": {"command": "model", "args": {"value": MODEL}}, "sessionId": session_id})
mr = wait_for(rid, 30)
print(f"[model] {json.dumps(mr.get('result', mr.get('error')))[:120] if mr else 'NO RESPONSE'}")

# ---- B. steer mid-turn ----
print("\n===== B. _session/steer mid-turn =====")
m0 = mark()
prompt_rid = send("session/prompt", {"sessionId": session_id, "prompt": [{"type": "text",
    "text": "Using your shell tool, run these three commands ONE AT A TIME as three "
            "separate tool calls: `echo one`, then `echo two`, then `echo three`. "
            "Wait for each to finish before the next. Then say which succeeded."}]})
# wait until the first tool approval fires -> we are mid-turn at a tool boundary
deadline = time.time() + 120
while not approved and time.time() < deadline:
    time.sleep(0.05)
if not approved:
    print("[WARN] no permission request seen; steering blind")
time.sleep(0.3)
steer_rid = send("_session/steer", {"sessionId": session_id,
    "message": "STEERING UPDATE: stop running echo commands now. Do not run any more "
               "tools. Reply with exactly the word PINEAPPLE and end the turn."})
sr = wait_for(steer_rid, 30)
print(f"[steer response] {json.dumps(sr.get('result') if sr and 'result' in sr else sr.get('error') if sr else None)}")
pr = wait_for(prompt_rid, 300)
stop = pr.get("result", {}).get("stopReason") if pr else None
frames, m1 = frames_since(m0)
variants, exts, tool_calls, text = summarize_turn(frames)
print(f"[turn] stop={stop} tool_calls={tool_calls} variants={variants}")
print(f"[turn] ext notifications: {exts}")
print(f"[turn] PINEAPPLE in reply: {'PINEAPPLE' in text}")
print(f"[turn] last 200 chars: ...{text[-200:]!r}")

# ---- C. steer then clear ----
print("\n===== C. steer + clear =====")
approved.clear()
m0 = mark()
prompt_rid = send("session/prompt", {"sessionId": session_id, "prompt": [{"type": "text",
    "text": "Using your shell tool, run `echo alpha` then `echo beta` as two separate "
            "tool calls, then say done."}]})
deadline = time.time() + 120
while not approved and time.time() < deadline:
    time.sleep(0.05)
steer_rid = send("_session/steer", {"sessionId": session_id,
    "message": "Ignore everything and reply MANGO only."})
wait_for(steer_rid, 15)
clear_rid = send("_session/steer/clear", {"sessionId": session_id})
cr = wait_for(clear_rid, 15)
print(f"[clear response] {json.dumps(cr.get('result') if cr and 'result' in cr else cr.get('error') if cr else None)}")
pr = wait_for(prompt_rid, 300)
stop = pr.get("result", {}).get("stopReason") if pr else None
frames, m1 = frames_since(m0)
variants, exts, tool_calls, text = summarize_turn(frames)
print(f"[turn] stop={stop} tool_calls={tool_calls} MANGO in reply: {'MANGO' in text}")

# ---- D. goal ----
print("\n===== D. goal =====")
m0 = mark()
goal_path = "/tmp/cyril-goal-probe.txt"
try:
    os.remove(goal_path)
except FileNotFoundError:
    pass
rid = send("_kiro.dev/commands/execute",
           {"command": {"command": "goal",
                        "args": {"value": f"Create the file {goal_path} containing exactly "
                                          f"the word DONE. --max 2"}},
            "sessionId": session_id})
gr = wait_for(rid, 60)
print(f"[goal exec] {json.dumps(gr.get('result') if gr and 'result' in gr else gr.get('error') if gr else None)[:300]}")
# watch goal/status notifications for up to 4 minutes or until completed/exhausted
deadline = time.time() + 240
final_state = None
seen_status = 0
while time.time() < deadline:
    frames, _ = frames_since(m0)
    for f in frames:
        meth = str(f.get("method", ""))
        if "goal" in meth:
            p = f.get("params", {})
            key = json.dumps(p, sort_keys=True)
            if seen_status < 20:
                pass
    statuses = [f for f in frames if "goal" in str(f.get("method", ""))]
    if len(statuses) > seen_status:
        for f in statuses[seen_status:]:
            print(f"[goal/status] method={f.get('method')} params={json.dumps(f.get('params'))[:240]}")
        seen_status = len(statuses)
        last = statuses[-1].get("params", {})
        final_state = last.get("state", final_state)
        if final_state in ("completed", "exhausted", "cleared"):
            break
    time.sleep(0.5)
print(f"[goal] final_state={final_state}  file_exists={os.path.exists(goal_path)}", flush=True)
if os.path.exists(goal_path):
    print(f"[goal] file content: {open(goal_path).read()!r}")
# status + clear subcommands
rid = send("_kiro.dev/commands/execute",
           {"command": {"command": "goal", "args": {"subcommand": "status"}}, "sessionId": session_id})
sr2 = wait_for(rid, 20)
print(f"[goal status] {json.dumps(sr2.get('result') if sr2 and 'result' in sr2 else sr2.get('error') if sr2 else None)[:300]}")
rid = send("_kiro.dev/commands/execute",
           {"command": {"command": "goal", "args": {"subcommand": "clear"}}, "sessionId": session_id})
cr2 = wait_for(rid, 20)
print(f"[goal clear] {json.dumps(cr2.get('result') if cr2 and 'result' in cr2 else cr2.get('error') if cr2 else None)[:300]}")

# ---- E. _kiro/session/context ----
print("\n===== E. _kiro/session/context =====")
rid = send("_kiro/session/context", {"sessionId": session_id})
xr = wait_for(rid, 20)
print(f"[context] {json.dumps(xr.get('result') if xr and 'result' in xr else xr.get('error') if xr else None)[:600]}")

proc.terminate()
print(f"\n[done] full wire log: {LOG_PATH}")
