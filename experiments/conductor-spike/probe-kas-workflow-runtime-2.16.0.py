#!/usr/bin/env python3
"""KAS workflow-runtime functional probe for the 2.16.0 audit (@kiro/agent 0.27.8).

2.16.0 lands the whole `_kiro/workflow/*` surface in the KAS server bundle
(26 new method literals, 26 new `src/workflow/**` modules, incl. the
`workflow-notification-bridge` emitter that was missing through 2.15.0). The
gate is `resolveWorkflows()` = `settings.workflows.enabled ?? false`, and
`session/new._meta.workflowsEnabled` reports it (false by default).

Schema-accepted != functional, so this probe flips the gate ON via
`session/new._meta.kiro.settings.workflows.enabled` and then actually CALLS the
read-only workflow methods to see whether they answer or 404:

    _kiro/workflow/list, listRecipes, listWatchHandlers

initialize + session/new + read-only calls ONLY — no prompt turn, no workflow
invoke, so zero model call and zero credits.

    probe-kas-workflow-runtime-2.16.0.py <kiro-cli-chat> <out.jsonl>

HOME-isolated per feedback_isolate_kiro_probes_with_home.
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")


def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute(
            "select value from auth_kv where key in "
            "('kirocli:odic:token','kirocli:social:token') order by key desc"
        ).fetchone()
        prow = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        c.close()
    if row is None:
        raise SystemExit("logged out — no token")
    v = row[0]
    v = v.decode() if isinstance(v, (bytes, bytearray)) else v
    d = json.loads(v)
    parn = d.get("profile_arn")
    if not parn and prow:
        pv = prow[0]
        pv = pv.decode() if isinstance(pv, (bytes, bytearray)) else pv
        try:
            parn = json.loads(pv).get("arn")
        except Exception:
            pass
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": parn}


TOK = read_token()
CWD = tempfile.mkdtemp(prefix="kas-wf-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-wfhome-")
env = dict(os.environ)
env["HOME"] = TMPH
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def rep(rid, res):
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    p.stdin.flush()


def pump(until, to=60):
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
        m = o.get("method")
        rid = o.get("id")
        if rid is not None and m:
            # host callbacks the agent needs answered to make progress
            if m == "_kiro/auth/getAccessToken":
                rep(rid, TOK)
            elif m == "_kiro/terminal/shell_type":
                rep(rid, {"shellType": "bash"})
            else:
                rep(rid, {})
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}},
    "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True}},
})
pump(1, 30)

# Flip the workflow gate ON at session creation.
nid = req("session/new", {
    "cwd": CWD,
    "mcpServers": [],
    "_meta": {"kiro": {"settings": {"workflows": {"enabled": True}}}},
})
sess = pump(nid, 60)
sid = (sess or {}).get("result", {}).get("sessionId")
meta = (sess or {}).get("result", {}).get("_meta", {})
print("sessionId:", sid)
print("workflowsEnabled:", meta.get("workflowsEnabled"))
pump(-1, 5)  # drain settle

for method, params in (
    # `list` reads workflow definitions off disk, so it needs the workspace roots
    # explicitly — without them the handler throws "workspacePaths is not iterable".
    ("_kiro/workflow/list", {"sessionId": sid, "workspacePaths": [CWD]}),
    ("_kiro/workflow/listRecipes", {"sessionId": sid, "workspacePaths": [CWD]}),
    ("_kiro/workflow/listWatchHandlers", {"sessionId": sid}),
):
    rid = req(method, params)
    r = pump(rid, 45)
    print(f"\n=== {method} ===")
    res = (r or {}).get("result")
    if isinstance(res, dict) and "recipes" in res:
        # summarise: full plans are long, the inventory is what matters
        for rc in res["recipes"]:
            plan = rc.get("plan") or []
            kinds = ",".join(sorted({n.get("type", "?") for n in plan}))
            print(f"  - {rc.get('name'):<22} source={rc.get('source'):<28} "
                  f"nodes={len(plan)} types=[{kinds}] inputs={list((rc.get('inputs') or {}).keys())}")
    else:
        print(json.dumps(r, indent=2)[:2000] if r else "  NO RESPONSE")

OUT.close()
p.stdin.close()
p.terminate()
