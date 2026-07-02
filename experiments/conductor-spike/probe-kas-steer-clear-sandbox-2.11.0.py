#!/usr/bin/env python3
"""No-turn existence/response-shape probe for the two client->agent methods new
in @kiro/agent 0.8.0 (kiro-cli 2.11.0): _session/steer/clear and
_kiro/sandbox/applyConfig. Static contract (read from the 0.8.0 bundle):

  _session/steer/clear  {sessionId} -> {cleared: true, messageIds: [...]}
                        (+ session_info_update kind=steering_cleared broadcast;
                        clearing an empty queue is a no-op returning [])
  _kiro/sandbox/applyConfig {configId, value} -> {}   (engine-global, NO sessionId;
                        configIds: sandbox|sandboxNetworkMode|mcpSandboxing;
                        unknown value = warn + silent {} success)

Run against BOTH bundles (0.8.0 expects the above; 0.3.299 control expects
"Unknown ext method" for both). A bogus method calibrates the unknown-method
error shape. Usage: probe-kas-steer-clear-sandbox-2.11.0.py <path-to-acp-server.js>
Direct-spawn, no auth needed (no turn is started)."""
import json, os, subprocess, threading, queue, time, tempfile, sys

SERVER = sys.argv[1]
assert os.path.exists(SERVER), SERVER
runtime = os.environ.get("KIRO_AGENT_PATH", "node")
CWD = tempfile.mkdtemp(prefix="kas-steerclear-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
proc = subprocess.Popen([runtime, "--experimental-wasm-modules", SERVER, "--transport=stdio"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]
NOTIFS = []

def req(m, p):
    i[0] += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}) + "\n")
    proc.stdin.flush()
    return i[0]

def rep(rid, res):
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    proc.stdin.flush()

def pump(until, to):
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
        m = o.get("method")
        if m:
            if o.get("id") is not None:
                rep(o["id"], {})
            else:
                NOTIFS.append(o)
        if until is not None and o.get("id") == until and ("result" in o or "error" in o):
            return o
    return None

def show(label, resp):
    if resp is None:
        print(f"{label}: TIMEOUT")
    elif "error" in resp:
        print(f"{label}: ERROR {json.dumps(resp['error'])[:220]}")
    else:
        print(f"{label}: RESULT {json.dumps(resp['result'])[:220]}")

pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 20)
new = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 40)
SID = (new or {}).get("result", {}).get("sessionId")
print(f"sessionId: {SID}")
NOTIFS.clear()  # only collect notifications caused by the probes below

show("steer/clear valid (empty queue)", pump(req("_session/steer/clear", {"sessionId": SID}), 15))
show("steer/clear missing sessionId", pump(req("_session/steer/clear", {}), 15))
show("steer/clear unknown session", pump(req("_session/steer/clear", {"sessionId": "sess-nonexistent"}), 15))
show("applyConfig networkMode=default_blocked", pump(req("_kiro/sandbox/applyConfig", {"configId": "sandboxNetworkMode", "value": "default_blocked"}), 15))
show("applyConfig bogus configId (silent-noop check)", pump(req("_kiro/sandbox/applyConfig", {"configId": "bogus", "value": "x"}), 15))
show("applyConfig missing value (InvalidParams check)", pump(req("_kiro/sandbox/applyConfig", {"configId": "sandbox"}), 15))
show("control bogus method", pump(req("_session/steer/bogus", {"sessionId": SID}), 15))
pump(None, 3)
print("NOTIFS during probes:", json.dumps(
    [{"method": n["method"], "params": n.get("params")} for n in NOTIFS
     if "steer" in json.dumps(n).lower() or "sandbox" in json.dumps(n).lower()
     or "session_info" in json.dumps(n)])[:1500])
proc.stdin.close()
proc.terminate()
