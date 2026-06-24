#!/usr/bin/env python3
"""Compare KAS session/new shape across @kiro/agent versions via the direct-spawn
free path (acp-server.js --transport=stdio, no --auth -> FileAuthProvider).
Usage: probe-kas-session-new-2.9.0.py <path-to-acp-server.js>
Prints agentCapabilities + session/new {configOptions, modes, _meta}."""
import json, os, subprocess, threading, queue, time, tempfile, sys

SERVER = sys.argv[1]
assert os.path.exists(SERVER), f"not found: {SERVER}"
runtime = os.environ.get("KIRO_AGENT_PATH", "node")
CWD = tempfile.mkdtemp(prefix="kas-snew-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
argv = [runtime, "--experimental-wasm-modules", SERVER, "--transport=stdio"]
proc = subprocess.Popen(argv, cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]
def req(m, p):
    i[0] += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}) + "\n")
    proc.stdin.flush(); return i[0]
def rep(rid, res):
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n"); proc.stdin.flush()

def pump(until, to=60):
    end = time.time() + to
    while time.time() < end:
        try: raw = q.get(timeout=2)
        except queue.Empty: continue
        try: o = json.loads(raw)
        except Exception: continue
        if o.get("method") and o.get("id") is not None:
            rep(o["id"], {})  # ack any agent->client request blindly
        if o.get("id") == until and ("result" in o or "error" in o):
            return o
    return None

r = pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 20)
caps = (r or {}).get("result", {}).get("agentCapabilities", {}) if r else {}
print("=== agentCapabilities ===")
print(json.dumps(caps, indent=1, sort_keys=True))

nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sn = pump(nid, 60)
res = (sn or {}).get("result", {}) if sn else {}
print("\n=== session/new: modes ===")
print(json.dumps(res.get("modes"), indent=1, sort_keys=True))
print("\n=== session/new: configOptions ===")
print(json.dumps(res.get("configOptions"), indent=1, sort_keys=True))
print("\n=== session/new: _meta ===")
print(json.dumps(res.get("_meta"), indent=1, sort_keys=True))
proc.stdin.close(); proc.terminate()
