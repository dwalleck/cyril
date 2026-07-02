#!/usr/bin/env python3
"""Trigger + capture _kiro/knowledge/indexing{Started,Completed} (new in
@kiro/agent 0.8.0, kiro-cli 2.11.0). Static contract (from the bundle):

  syncActiveAgentKnowledge(session) runs at session setup / mode switch when the
  bound custom agent declares resources.knowledgeBases[]. It emits:
    _kiro/knowledge/indexingStarted   {sessionId, name, fileCount}
    _kiro/knowledge/indexingCompleted {sessionId, name, status, itemCount?}
      status "success" carries itemCount; otherwise just status.
  indexType "fast" -> BM25 (local, no model download); "best" -> MiniLM embeddings.

This probe: build a temp workspace with .kiro/agents/kbtest.json declaring a
knowledgeBase over a local docs dir (indexType fast to stay offline), then bind
that agent by setting the session mode to "kbtest" and watch for the two
notifications. NO backend turn -> no auth needed. Control: a second session left
on the default mode (no knowledge base) must emit neither notification.
Usage: <script> <path-to-acp-server.js> [fast|best]"""
import json, os, subprocess, threading, queue, time, tempfile, sys

SERVER = sys.argv[1]
INDEX_TYPE = sys.argv[2] if len(sys.argv) > 2 else "fast"
runtime = os.environ.get("KIRO_AGENT_PATH", "node")
CWD = tempfile.mkdtemp(prefix="kas-kb-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

# knowledge source
kb_dir = os.path.join(CWD, "kb")
os.makedirs(kb_dir)
for n in range(3):
    with open(os.path.join(kb_dir, f"note{n}.md"), "w") as f:
        f.write(f"# Note {n}\nThe lighthouse protocol requires signal code ALPHA-{n}.\n" * 5)

# custom agent declaring the knowledge base (minimal — no CLI-only fields so it
# is not dropped by the KAS profile loader)
agents_dir = os.path.join(CWD, ".kiro", "agents")
os.makedirs(agents_dir)
agent = {
    "name": "kbtest",
    "description": "KB indexing probe agent",
    "prompt": "You are a test agent.",
    "resources": [{
        "type": "knowledgeBase",
        "source": "file://kb",
        "name": "lighthouse-notes",
        "description": "test notes",
        "indexType": INDEX_TYPE,
        "autoUpdate": True,
    }],
}
with open(os.path.join(agents_dir, "kbtest.json"), "w") as f:
    json.dump(agent, f)

proc = subprocess.Popen([runtime, "--experimental-wasm-modules", SERVER, "--transport=stdio"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]
PENDING = {}
KB_NOTIFS = []

def req(m, p):
    i[0] += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}) + "\n")
    proc.stdin.flush()
    return i[0]

def rep(rid, res):
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    proc.stdin.flush()

def drain(deadline):
    while time.time() < deadline:
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
            if "knowledge/indexing" in m:
                KB_NOTIFS.append({"method": m, "params": o.get("params")})
            if o.get("id") is not None:
                rep(o["id"], {})
        elif o.get("id") is not None:
            PENDING[o["id"]] = o

def wait_resp(rid, to):
    end = time.time() + to
    while time.time() < end:
        if rid in PENDING:
            return PENDING.pop(rid)
        drain(time.time() + 0.5)
    return None

wait_resp(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 20)
new = wait_resp(req("session/new", {"cwd": CWD, "mcpServers": []}), 40)
SID = (new or {}).get("result", {}).get("sessionId")
# report the mode config option so we know the exact configId + that kbtest is a value
cfgs = (new or {}).get("result", {}).get("configOptions") or []
mode_opt = next((c for c in cfgs if c.get("id") == "mode" or c.get("configId") == "mode"), None)
print("sessionId:", SID)
print("mode option values:", json.dumps([o.get("value") for o in (mode_opt or {}).get("options", [])]) if mode_opt else "NONE")

# bind the custom agent by switching mode to it
mode_id = (mode_opt or {}).get("id") or (mode_opt or {}).get("configId") or "mode"
r = wait_resp(req("session/set_config_option", {"sessionId": SID, "configId": mode_id, "value": "kbtest"}), 30)
print("set_config_option(mode=kbtest):", json.dumps(r.get("result") if r and "result" in r else r)[:200])

# give the (fire-and-forget) knowledge sync time to run + emit
drain(time.time() + 25)
print(f"\nKB NOTIFS ({len(KB_NOTIFS)}):")
for n in KB_NOTIFS:
    print(" ", json.dumps(n))
print("\nVERDICT: indexingStarted seen:", any("Started" in n["method"] for n in KB_NOTIFS),
      "| indexingCompleted seen:", any("Completed" in n["method"] for n in KB_NOTIFS))
proc.stdin.close()
proc.terminate()
