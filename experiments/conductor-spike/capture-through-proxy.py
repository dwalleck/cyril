#!/usr/bin/env python3
"""Drive a full ACP session THROUGH the kiro-proxy-rs logging proxy so every
frame (both directions) is captured to a JSONL trace, for cross-version wire
diffing. The proxy is a drop-in for `kiro-cli-chat acp`: we spawn `[proxy, acp]`
and it forwards to `$KIRO_PROXY_REAL_BACKEND acp`, logging each JSON-RPC line as
`{time,direction,envelope,method,id,typed,len,parsed}`.

Usage: capture-through-proxy.py <proxy-bin> <real-backend-chat-bin> <log-path>
Drives: initialize, session/new, model/agent/effort option queries, /tools,
        and one real prompt turn (needs the user logged in). Auto-acks server
        requests. Writes the proxy log to <log-path> (truncated first)."""
import json, os, subprocess, threading, queue, time, tempfile, sys

PROXY, BACKEND, LOGPATH = sys.argv[1], sys.argv[2], sys.argv[3]
if os.path.exists(LOGPATH):
    os.remove(LOGPATH)
CWD = tempfile.mkdtemp(prefix="proxycap-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

env = dict(os.environ)
env["KIRO_PROXY_REAL_BACKEND"] = BACKEND
env["KIRO_PROXY_LOG"] = LOGPATH

proc = subprocess.Popen([PROXY, "acp"], cwd=CWD, stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                        text=True, bufsize=1, env=env)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]
PEND = {}

def req(m, p):
    i[0] += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}) + "\n")
    proc.stdin.flush()
    return i[0]

def rep(rid, res):
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    proc.stdin.flush()

def drain(dl):
    while time.time() < dl:
        try:
            raw = q.get(timeout=0.5)
        except queue.Empty:
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        m = o.get("method")
        if m:
            if o.get("id") is not None:
                rep(o["id"], {})
        elif o.get("id") is not None:
            PEND[o["id"]] = o

def wait(rid, to):
    e = time.time() + to
    while time.time() < e:
        if rid in PEND:
            return PEND.pop(rid)
        drain(time.time() + 0.5)
    return None

wait(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 20)
new = wait(req("session/new", {"cwd": CWD, "mcpServers": []}), 40)
SID = (new or {}).get("result", {}).get("sessionId")
print("sessionId:", SID)
if SID:
    for cmd in ("model", "agent", "effort", "prompts"):
        req("_kiro.dev/commands/options", {"sessionId": SID, "command": cmd})
        drain(time.time() + 3)
    # /tools via commands/execute (adjacently-tagged TuiCommand)
    req("_kiro.dev/commands/execute", {"sessionId": SID, "command": {"command": "tools", "args": {}}})
    drain(time.time() + 3)
    # one real prompt turn
    pid = req("session/prompt", {"sessionId": SID, "prompt": [{"type": "text", "text": "Say hello in one word."}]})
    wait(pid, 60)
    drain(time.time() + 3)
proc.stdin.close()
proc.terminate()
time.sleep(0.5)
nlines = sum(1 for _ in open(LOGPATH)) if os.path.exists(LOGPATH) else 0
print(f"proxy log lines: {nlines} -> {LOGPATH}")
