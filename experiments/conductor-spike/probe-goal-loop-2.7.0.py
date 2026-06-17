#!/usr/bin/env python3
"""Probe the /goal iterative loop on the 2.7.0 wire: set goal, prompt, watch iterations.

  KIRO_BIN=~/.local/share/kiro-research/binaries/2.7.0/kiro-cli-chat \
      python3 probe-goal-loop-2.7.0.py
"""
import json, os, subprocess, threading, time, sys

KIRO = os.path.expanduser(os.environ.get("KIRO_BIN", "~/.local/bin/kiro-cli-chat"))
CWD = "/tmp"
LOG_PATH = "/tmp/cyril-probe-goal-loop-2.7.0.log"
MODEL = os.environ.get("PROBE_MODEL", "claude-haiku-4.5")
GOAL_PATH = "/tmp/cyril-goal-probe.txt"

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


threading.Thread(target=reader, daemon=True).start()
threading.Thread(target=auto_approve, daemon=True).start()

try:
    os.remove(GOAL_PATH)
except FileNotFoundError:
    pass

rid = send("initialize", {"protocolVersion": 1,
    "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False},
                           "terminal": False},
    "clientInfo": {"name": "cyril", "version": "0.2.0"}})
wait_for(rid, 20)
rid = send("session/new", {"cwd": CWD, "mcpServers": []})
r = wait_for(rid, 30)
if not r or "error" in r:
    print(f"[ERROR] session/new failed: {r} — aborting", flush=True)
    proc.terminate(); sys.exit(1)
session_id = r["result"]["sessionId"]
print(f"[ok] session={session_id}", flush=True)
rid = send("_kiro.dev/commands/execute",
           {"command": {"command": "model", "args": {"value": MODEL}}, "sessionId": session_id})
mr = wait_for(rid, 30)
if not mr or "error" in mr or not mr.get("result", {}).get("success"):
    print(f"[ERROR] model switch to {MODEL} failed: {mr} — aborting "
          f"(goal-loop findings would be attributed to the wrong model)", flush=True)
    proc.terminate(); sys.exit(1)
print(f"[model] {mr['result'].get('message')}", flush=True)

rid = send("_kiro.dev/commands/execute",
           {"command": {"command": "goal",
                        "args": {"value": f"The file {GOAL_PATH} exists and contains exactly "
                                          f"the word DONE. --max 3"}},
            "sessionId": session_id})
gr = wait_for(rid, 30)
print(f"[goal set] {json.dumps(gr.get('result', gr.get('error')))[:400]}", flush=True)

# now drive it with a prompt
prid = send("session/prompt", {"sessionId": session_id, "prompt": [{"type": "text",
    "text": f"Work toward the goal. Create the file {GOAL_PATH} with content DONE "
            f"using your file or shell tools."}]})
pr = wait_for(prid, 420)
if not pr or "error" in pr:
    print(f"[WARN] driving turn did not complete cleanly: {json.dumps(pr.get('error') if pr else None)[:160]}\n"
          f"       => goal-iteration counts below may be INCOMPLETE, not a confirmed 'loop never manifested' finding", flush=True)
print(f"[prompt done] stop={pr.get('result', {}).get('stopReason') if pr else None}", flush=True)

# watch for goal/status + any further turns for 90s
deadline = time.time() + 90
reported = 0
while time.time() < deadline:
    with lock:
        frames = list(incoming)
    goal_frames = [f for f in frames if "goal" in str(f.get("method", ""))]
    if len(goal_frames) > reported:
        for f in goal_frames[reported:]:
            print(f"[GOAL NOTIF] {f.get('method')} {json.dumps(f.get('params'))[:300]}", flush=True)
        reported = len(goal_frames)
        last_state = goal_frames[-1].get("params", {}).get("state")
        if last_state in ("completed", "exhausted", "cleared"):
            break
    time.sleep(0.5)

content = open(GOAL_PATH).read() if os.path.exists(GOAL_PATH) else None
print(f"[file] exists={os.path.exists(GOAL_PATH)} content={content!r}", flush=True)

rid = send("_kiro.dev/commands/execute",
           {"command": {"command": "goal", "args": {"subcommand": "status"}}, "sessionId": session_id})
sr = wait_for(rid, 20)
print(f"[goal status] {json.dumps(sr.get('result', sr.get('error')) if sr else None)[:400]}", flush=True)
rid = send("_kiro.dev/commands/execute",
           {"command": {"command": "goal", "args": {"subcommand": "clear"}}, "sessionId": session_id})
cr = wait_for(rid, 20)
print(f"[goal clear] {json.dumps(cr.get('result', cr.get('error')) if cr else None)[:300]}", flush=True)

# count session/update variants across the whole run
with lock:
    frames = list(incoming)
variants = {}
for f in frames:
    m = f.get("method")
    if m in ("session/update", "_kiro.dev/session/update"):
        v = f.get("params", {}).get("update", {}).get("sessionUpdate")
        variants[f"{m}:{v}"] = variants.get(f"{m}:{v}", 0) + 1
print(f"[variants] {json.dumps(variants, indent=1)}", flush=True)
proc.terminate()
print(f"[done] log: {LOG_PATH}", flush=True)
