#!/usr/bin/env python3
"""LIVE v2 crew turn for the 2.18.1 audit — PAID (1 crew turn).

2.18.1 changelog: "Subagents honor per-stage model overrides"; nm shows the fix
as chat_cli_v2::...::spawn_orchestrated_session_with_model (spawn-path, v2 ACP
session manager). Wire question: do `kiro.dev/subagent/list_update` rows now
carry a model field cyril's SubagentTracker isn't parsing? Drive one crew turn
with an explicit per-stage model override and dump every list_update row's full
field set plus the subagent tool_call rawInput.

    probe-v2-crew-model-2.18.1.py <path-to-kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")

CWD = tempfile.mkdtemp(prefix="v2crew-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

env = dict(os.environ)
env["HOME"] = tempfile.mkdtemp(prefix="v2crewhome-")
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp"], cwd=CWD, env=env, stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
LIST_UPDATES = []
RAW_INPUTS = []


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def pump(until, to=420, tag=""):
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
        OUT.write(raw + "\n")
        OUT.flush()
        m, rid, pr = o.get("method"), o.get("id"), o.get("params") or {}
        if m == "_kiro.dev/subagent/list_update" and rid is None:
            LIST_UPDATES.append(pr)
        if m and rid is None and m.endswith("session/update"):
            u = pr.get("update") or {}
            if u.get("sessionUpdate") == "tool_call" and u.get("rawInput") is not None:
                RAW_INPUTS.append((u.get("title") or "", u.get("rawInput")))
        if rid is not None and m:
            if m == "session/request_permission":
                opts = pr.get("options", [])
                pick = next((x for x in opts
                             if "allow" in ((x.get("kind") or "") + (x.get("optionId") or "")).lower()),
                            opts[0] if opts else None)
                res = ({"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                       if pick else {"outcome": {"outcome": "cancelled"}})
                p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
            else:
                p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {}}) + "\n")
            p.stdin.flush()
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 60)
sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 5)

rid = req("session/prompt",
          {"sessionId": sid,
           "prompt": [{"type": "text",
                       "text": "Use the subagent tool to spawn exactly one subagent in a "
                               "single stage named 'echoer' whose task is to reply with the "
                               "single word done and nothing else. Set the stage's model "
                               "override to claude-sonnet-5. Do not create any files."}]})
r = pump(rid, 420, tag="crew")
print("stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
pump(-1, 8)

print("\n=== subagent tool rawInput(s) ===")
for title, ri in RAW_INPUTS:
    print(f"  {title!r}: {json.dumps(ri)[:600]}")

print(f"\n=== list_update frames: {len(LIST_UPDATES)} ===")
fields = set()
for pr in LIST_UPDATES:
    for row in pr.get("subagents") or pr.get("agents") or []:
        if isinstance(row, dict):
            fields |= set(row.keys())
print("union of row fields:", sorted(fields))
if LIST_UPDATES:
    print("last frame:", json.dumps(LIST_UPDATES[-1])[:800])

OUT.close()
p.stdin.close()
p.terminate()
