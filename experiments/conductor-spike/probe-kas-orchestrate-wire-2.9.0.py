#!/usr/bin/env python3
"""LIVE KAS orchestration wire capture (direct-spawn free path). Runs one real turn
that asks the agent to orchestrate parallel subagents, and logs every session/update +
_kiro/* notification with full subagent tagging (_meta.kiro.kind, agentSubtaskId,
subExecutionId). Auto-approves permission requests; answers _kiro/auth/getAccessToken
from the SSO token file as a safety net. Costs credits.
Usage: probe-kas-orchestrate-wire-2.9.0.py <path-to-acp-server.js> <out.jsonl>"""
import json, os, subprocess, threading, queue, time, tempfile, sys

SERVER = sys.argv[1]
OUT = sys.argv[2]
assert os.path.exists(SERVER), SERVER
runtime = os.environ.get("KIRO_AGENT_PATH", "node")
TOKEN = os.path.expanduser("~/.aws/sso/cache/kiro-auth-token.json")
CWD = tempfile.mkdtemp(prefix="kas-orch-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
log = open(OUT, "w")
def rec(direction, obj): log.write(json.dumps({"d": direction, **obj}) + "\n"); log.flush()

proc = subprocess.Popen([runtime, "--experimental-wasm-modules", SERVER, "--transport=stdio"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]
def send(o): proc.stdin.write(json.dumps(o) + "\n"); proc.stdin.flush(); rec("C->A", o)
def req(m, p):
    i[0] += 1; send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}); return i[0]
def rep(rid, res): send({"jsonrpc": "2.0", "id": rid, "result": res})

def read_token():
    try: return json.load(open(TOKEN)).get("accessToken")
    except Exception: return None

inbound = {}
updates = {}        # sessionUpdate kind -> count
subtask_ids = set()
subexec_ids = set()
meta_kinds = {}     # _meta.kiro.kind on tool_calls -> count
tool_titles = []    # (kind, title, metaKind, subtaskId)
auth_calls = [0]

def on_notify(o):
    m = o.get("method"); p = o.get("params", {}) or {}
    inbound[m] = inbound.get(m, 0) + 1
    rec("A->C", o)
    # answer agent->client REQUESTS
    if o.get("id") is not None:
        if m == "_kiro/auth/getAccessToken":
            auth_calls[0] += 1
            rep(o["id"], {"accessToken": read_token()})
        elif m == "session/request_permission":
            # auto-approve: pick an allow option
            opts = (p.get("options") or [])
            allow = next((x for x in opts if "allow" in json.dumps(x).lower()), opts[0] if opts else None)
            oid = allow.get("optionId") if isinstance(allow, dict) else None
            rep(o["id"], {"outcome": {"outcome": "selected", "optionId": oid}})
        else:
            rep(o["id"], {})
        return
    # session/update analysis
    if m == "session/update":
        upd = p.get("update") or {}
        kind = upd.get("sessionUpdate", "?")
        updates[kind] = updates.get(kind, 0) + 1
        meta = ((upd.get("_meta") or {}).get("kiro") or {})
        # subagent tagging can live on the update or nested toolCall
        sid = meta.get("agentSubtaskId") or upd.get("agentSubtaskId")
        sx = meta.get("subExecutionId") or upd.get("subExecutionId")
        if sid: subtask_ids.add(sid)
        if sx: subexec_ids.add(sx)
        if kind in ("tool_call", "tool_call_update"):
            mk = meta.get("kind")
            if mk: meta_kinds[mk] = meta_kinds.get(mk, 0) + 1
            tool_titles.append((kind, upd.get("title") or upd.get("toolCallId"), meta.get("kind"), sid))

def pump(until, to):
    end = time.time() + to
    while time.time() < end:
        try: raw = q.get(timeout=2)
        except queue.Empty: continue
        try: o = json.loads(raw)
        except Exception: continue
        if "method" in o: on_notify(o)
        if until is not None and o.get("id") == until and ("result" in o or "error" in o):
            rec("A->C", o); return o
    return None

pump(req("initialize", {"protocolVersion": 1,
     "clientCapabilities": {}}), 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sn = pump(nid, 40)
sid = (sn or {}).get("result", {}).get("sessionId") if sn else None
print("sessionId:", sid)

PROMPT = ("Use the orchestrate_subagent tool to run two independent subagents in parallel, "
          "then have a reviewer verify their work. Subagent 1: create a file named alpha.txt "
          "whose only content is the word ALPHA. Subagent 2: create a file named beta.txt whose "
          "only content is the word BETA. Keep each subagent's work minimal. After both finish, "
          "summarize what was created in one sentence.")
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
r = pump(pid, 420)   # generous: orchestration + verification agents
stop = (r or {}).get("result", {}).get("stopReason") if r else "TIMEOUT/none"

print("\n=== stopReason:", stop, "===")
print("auth getAccessToken calls:", auth_calls[0])
print("\nINBOUND methods:", json.dumps(inbound, indent=0))
print("\nsession/update kinds:", json.dumps(updates, indent=0))
print("\ntool_call _meta.kiro.kind histogram:", json.dumps(meta_kinds, indent=0))
print("\ndistinct agentSubtaskId:", len(subtask_ids), sorted(subtask_ids))
print("distinct subExecutionId:", len(subexec_ids), sorted(subexec_ids))
print("\n--- tool_call timeline (kind | title | metaKind | subtaskId) ---")
for t in tool_titles[:80]:
    print("  ", " | ".join(str(x) for x in t))
print(f"\nfull raw wire log -> {OUT}")
proc.stdin.close(); proc.terminate()
