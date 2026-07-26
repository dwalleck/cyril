#!/usr/bin/env python3
"""LIVE KAS OrchestrateSubAgent wire capture (2.14.1 / @kiro/agent 0.22.7).

Spawns the faithful CLI path `kiro-cli-chat acp --agent-engine kas`, runs one real turn
that asks the agent to orchestrate a multi-stage subagent pipeline, and records every
session/update + _kiro/* frame with full subagent tagging. Specifically hunts for:
  - an OrchestrateSubAgent tool_call whose rawInput carries {task, stages[], repeat}
  - agent-subtask tagging (_meta.kiro.{kind, agentSubtaskId}) + the ACP ToolKind
  - any workflow-progress / loop_iteration frames (expected: none)
Answers _kiro/auth/getAccessToken from the CLI token file (full shape incl. profileArn),
auto-approves permissions, answers _kiro/userInput if asked. Costs credits.

Usage: probe-kas-orchestrate-capture-2.14.1.py <path-to-kiro-cli-chat> <out.jsonl>
"""
import json, os, subprocess, threading, queue, time, tempfile, sys

KIRO = sys.argv[1]
OUT = sys.argv[2]
# Token file with a FRESH access token. The on-disk kiro-auth-token*.json are often
# stale (kiro-cli refreshes into its SQLite auth_kv, not the JSON); pass a freshly
# assembled file as argv[3] — {accessToken (fresh), expiresAt (fresh), profileArn (stable)}.
# Recipe: read auth_kv 'kirocli:odic:token' (access_token/expires_at, snake_case) + merge
# profileArn from kiro-auth-token-cli.json. See docs/kiro-2.14.1-wire-audit.md.
TOKEN = sys.argv[3] if len(sys.argv) > 3 else os.path.expanduser("~/.aws/sso/cache/kiro-auth-token-cli.json")
CWD = tempfile.mkdtemp(prefix="kas-orch-cap-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
log = open(OUT, "w")

def rec(direction, obj):
    log.write(json.dumps({"d": direction, **obj}) + "\n"); log.flush()

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]

def send(o): proc.stdin.write(json.dumps(o) + "\n"); proc.stdin.flush(); rec("C->A", o)
def req(m, p):
    i[0] += 1; send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}); return i[0]
def rep(rid, res): send({"jsonrpc": "2.0", "id": rid, "result": res})

def auth_reply():
    try:
        d = json.load(open(TOKEN))
        return {"accessToken": d.get("accessToken"), "expiresAt": d.get("expiresAt"),
                "profileArn": d.get("profileArn"), "provider": d.get("provider"),
                "authMethod": d.get("authMethod")}
    except Exception as e:
        return {}

inbound = {}; updates = {}; subtask_ids = set(); meta_kinds = {}
tool_rows = []            # (kind, acpToolKind, title, metaKind, subtaskId, rawInputKeys)
orchestrate_inputs = []   # rawInput of any orchestrate/invoke tool_call
workflow_frames = [0]; loop_frames = [0]; auth_calls = [0]; userinput_calls = [0]

def on_notify(o):
    m = o.get("method"); p = o.get("params", {}) or {}
    inbound[m] = inbound.get(m, 0) + 1
    rec("A->C", o)
    if o.get("id") is not None:  # agent->client REQUEST
        if m == "_kiro/auth/getAccessToken":
            auth_calls[0] += 1; rep(o["id"], auth_reply())
        elif m == "session/request_permission":
            opts = (p.get("options") or [])
            allow = next((x for x in opts if "allow" in json.dumps(x).lower()), opts[0] if opts else None)
            oid = allow.get("optionId") if isinstance(allow, dict) else None
            rep(o["id"], {"outcome": {"outcome": "selected", "optionId": oid}})
        elif m == "_kiro/userInput":
            userinput_calls[0] += 1
            rep(o["id"], {"action": "answered", "answer": "Yes, proceed with the defaults."})
        else:
            rep(o["id"], {})
        return
    if m == "session/update":
        upd = p.get("update") or {}
        kind = upd.get("sessionUpdate", "?")
        updates[kind] = updates.get(kind, 0) + 1
        meta = ((upd.get("_meta") or {}).get("kiro") or {})
        sid = meta.get("agentSubtaskId") or upd.get("agentSubtaskId")
        if sid: subtask_ids.add(sid)
        if str(meta.get("kind")) == "workflow-progress" or str(upd.get("kind")) == "workflow-progress":
            workflow_frames[0] += 1
        if "loop_iteration" in json.dumps(upd): loop_frames[0] += 1
        if kind in ("tool_call", "tool_call_update"):
            mk = meta.get("kind")
            if mk: meta_kinds[mk] = meta_kinds.get(mk, 0) + 1
            ri = upd.get("rawInput") or {}
            rik = sorted(ri.keys()) if isinstance(ri, dict) else None
            title = upd.get("title") or upd.get("toolCallId")
            tool_rows.append((kind, upd.get("kind"), title, mk, (sid or "")[:8], rik))
            name = (ri.get("name") if isinstance(ri, dict) else "") or (title or "")
            if isinstance(ri, dict) and ("stages" in ri or "orchestrate" in str(name).lower() or "task" in ri and "stages" in ri):
                orchestrate_inputs.append(ri)

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

init = pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 25)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sn = pump(nid, 45)
sid = (sn or {}).get("result", {}).get("sessionId") if sn else None
print("sessionId:", sid, "| init ok:", bool(init))

PROMPT = ("Use the OrchestrateSubAgent tool to run a multi-stage pipeline for this task. "
          "Stage 1 ('explore'): list what .txt files exist in the workspace. "
          "Stage 2 ('write-alpha', depends on explore): create alpha.txt containing only ALPHA. "
          "Stage 3 ('write-beta', depends on explore): create beta.txt containing only BETA. "
          "Stages 2 and 3 should run in parallel. Keep each stage minimal. "
          "Then summarize in one sentence what was created.")
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
r = pump(pid, 480)
stop = (r or {}).get("result", {}).get("stopReason") if r else "TIMEOUT/none"

print("\n=== stopReason:", stop, "===")
print("auth getAccessToken calls:", auth_calls[0], "| userInput calls:", userinput_calls[0])
print("INBOUND methods:", json.dumps(inbound))
print("session/update kinds:", json.dumps(updates))
print("tool_call _meta.kiro.kind histogram:", json.dumps(meta_kinds))
print("distinct agentSubtaskId:", len(subtask_ids))
print("workflow-progress frames:", workflow_frames[0], "| loop_iteration frames:", loop_frames[0])
print("orchestrate/invoke rawInputs captured:", len(orchestrate_inputs))
for ri in orchestrate_inputs[:3]:
    print("  rawInput keys:", sorted(ri.keys()), "| has stages:", "stages" in ri, "| has repeat:", "repeat" in ri)
    print("  rawInput:", json.dumps(ri)[:900])
print("\n--- tool_call timeline (upd | acpKind | title | metaKind | subtask | rawInputKeys) ---")
for t in tool_rows[:60]:
    print("  ", " | ".join(str(x) for x in t))
print(f"\nfull raw wire log -> {OUT}")
proc.stdin.close(); proc.terminate()
