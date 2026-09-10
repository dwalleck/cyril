#!/usr/bin/env python3
"""Turn-lifecycle wire sweep: 2.21.1 vs 2.21.2, v2 engine.

WHY: the 2.21.2 audit's first live lane never sent a session/prompt, so it
captured ZERO turn frames -- no agent_message_chunk, tool_call,
tool_call_update, plan, usage, or turn-end. That is exactly the family cyril
renders, and sweep-new-fields.py's own docstring warns that "targeted probes
answer their own hypothesis and will not notice a field that nobody asked
about". This probe exists to be swept, not to test a hypothesis.

Workload (identical on both binaries, so the path sets are comparable):
  1. initialize / session/new
  2. one prompt that forces a FILE READ tool call -- reads need no permission
     on v2, so the turn completes without approval friction
  3. drain to the session/prompt response (stop_reason), then settle

A permission request is answered by selecting the first allow-shaped option
rather than the template's `{}`, which these requests reject.

COST: exactly one tiny real turn per binary.

    probe-v2-turn-sweep-2.21.2.py <kiro-cli-binary> <out.jsonl>
"""
import json, os, queue, subprocess, sys, tempfile, threading, time

BIN = sys.argv[1]
OUT = open(sys.argv[2], "w")
REAL_HOME = os.path.expanduser("~")
PROBE_HOME = tempfile.mkdtemp(prefix="turnsweep-home-")
CWD = tempfile.mkdtemp(prefix="turnsweep-cwd-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
# Deterministic, tiny read target -- identical on both legs so the tool call
# and its content frames are comparable.
with open(os.path.join(CWD, "PROBE.txt"), "w") as fh:
    fh.write("ALPHA\n")

env = dict(os.environ)
env["HOME"] = PROBE_HOME
env.setdefault("XDG_DATA_HOME", os.path.join(REAL_HOME, ".local", "share"))

who = subprocess.run([BIN, "user", "whoami", "--format", "json"],
                     env=env, capture_output=True, text=True)
if who.returncode != 0:
    print(f"DEVIATION: whoami failed under HOME={PROBE_HOME}; falling back to real HOME")
    env["HOME"] = REAL_HOME
    who = subprocess.run([BIN, "user", "whoami", "--format", "json"],
                         env=env, capture_output=True, text=True)
    if who.returncode != 0:
        sys.exit(f"ABORT: not authenticated (rc={who.returncode})")

ver = subprocess.run([BIN, "--version"], env=env, capture_output=True, text=True).stdout.strip()
print("binary :", BIN, "|", ver)
OUT.write(json.dumps({"probe": "preflight", "binary": BIN, "version": ver}) + "\n")

p = subprocess.Popen([BIN, "acp"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
METHODS = {}
PERMS = [0]

def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]

def answer_permission(rid, params):
    """Pick an allow-shaped option; `{}` is rejected by request_permission."""
    opts = (params or {}).get("options") or []
    pick = None
    for o in opts:
        k = (o.get("kind") or "").lower()
        if "allow" in k:
            pick = o.get("optionId") or o.get("id")
            if "once" in k:
                break
    if pick is None and opts:
        pick = opts[0].get("optionId") or opts[0].get("id")
    PERMS[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid,
                              "result": {"outcome": {"outcome": "selected",
                                                     "optionId": pick}}}) + "\n")
    p.stdin.flush()

def pump(until, to=120, tag=""):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = q.get(timeout=2)
        except queue.Empty:
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        o["_tag"] = tag
        OUT.write(json.dumps(o) + "\n"); OUT.flush()
        m, rid = o.get("method"), o.get("id")
        if m:
            key = m
            if m == "session/update":
                key = "session/update:" + str(((o.get("params") or {}).get("update") or {}).get("sessionUpdate"))
            METHODS[key] = METHODS.get(key, 0) + 1
        if rid is not None and m:                       # server->client request
            if "request_permission" in m:
                answer_permission(rid, o.get("params"))
            else:
                p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {}}) + "\n")
                p.stdin.flush()
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None

pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {},
                        "clientInfo": {"name": "cyril-probe", "version": "0"}}), 30, "init")
r = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 60, "session_new")
sid = ((r or {}).get("result") or {}).get("sessionId")
if not sid:
    sys.exit("ABORT: no sessionId")

t0 = time.time()
rid = req("session/prompt", {"sessionId": sid, "prompt": [{
    "type": "text",
    "text": "Read the file PROBE.txt in the current directory and reply with only the single word it contains. Do not explain."}]})
resp = pump(rid, 180, "turn")
elapsed = round(time.time() - t0, 1)
pump(-1, 6, "settle")

stop = ((resp or {}).get("result") or {}).get("stopReason")
summary = {"probe": "result", "binary": BIN, "version": ver,
           "stop_reason": stop, "turn_seconds": elapsed,
           "permission_requests": PERMS[0],
           "methods": dict(sorted(METHODS.items()))}
OUT.write(json.dumps(summary) + "\n"); OUT.flush()
print(json.dumps(summary, indent=2)[:1400])
try:
    p.stdin.close(); p.terminate(); p.wait(timeout=10)
except Exception:
    p.kill()
