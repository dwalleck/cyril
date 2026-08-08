#!/usr/bin/env python3
"""Do on-disk `.workflow.json` recipes load with the workflow gate OFF? (2.16.0)

probe-kas-workflow-gateoff-2.16.0.py proved the METHODS route ungated using an
INLINE workflow object. But the documented authoring path is a file --
`.kiro/workflows/<name>.workflow.json`, loaded by `workflowPath` -- and the
kiro-workflow-authoring skill still states the gate is required. If cyril's
`/workflow run <recipe>` is going to load recipes off disk, the file path has to
work ungated too.

Three questions, all free (no `invoke`, so no step ever executes):
  1. does a workspace `.workflow.json` appear in `listRecipes` alongside the
     7 `bundled://` ones, with the gate off?
  2. does `_kiro/workflow/new {workflowPath}` accept a FILE (not just an inline
     `workflow` object) with the gate off?
  3. is the `.workflow.json` suffix really enforced -- does a plain `.json`
     sibling get ignored?

    probe-kas-workflow-diskrecipe-gateoff-2.16.0.py <kiro-cli-chat> <out.jsonl>

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
CWD = tempfile.mkdtemp(prefix="kas-wfdisk-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
os.makedirs(os.path.join(CWD, ".kiro", "workflows"), exist_ok=True)

RECIPE = {
    "name": "cyril-disk-probe",
    "description": "One-step recipe loaded from disk; created but never invoked.",
    "inputs": {"task": "prompt"},
    "steps": [{"type": "step", "id": "only", "agent": "wf-coder",
               "prompt": "Reply with the word ok. Do not use any tools. {{task}}"}],
}
GOOD = os.path.join(CWD, ".kiro", "workflows", "cyril-disk-probe.workflow.json")
BAD = os.path.join(CWD, ".kiro", "workflows", "cyril-plain-json.json")   # wrong suffix
with open(GOOD, "w") as f:
    json.dump(RECIPE, f, indent=2)
with open(BAD, "w") as f:
    json.dump(dict(RECIPE, name="cyril-plain-json"), f, indent=2)

TMPH = tempfile.mkdtemp(prefix="kas-wfdiskhome-")
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
        m, rid = o.get("method"), o.get("id")
        if rid is not None and m:
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

# NO workflow settings anywhere -- gate stays off.
r = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 60) or {}
sid = r.get("result", {}).get("sessionId")
print("workflowsEnabled:", r.get("result", {}).get("_meta", {}).get("workflowsEnabled"))
pump(-1, 5)

# Q1 -- does the disk recipe show up next to the bundled ones?
r = pump(req("_kiro/workflow/listRecipes", {"sessionId": sid, "workspacePaths": [CWD]}), 45)
recipes = ((r or {}).get("result") or {}).get("recipes") or []
print(f"\nlistRecipes -> {len(recipes)}")
for rc in recipes:
    print(f"   {rc.get('name'):<28} source={rc.get('source')!r}")
names = {rc.get("name") for rc in recipes}
print(f"\nQ1 disk recipe visible gate-off: {'YES' if 'cyril-disk-probe' in names else 'NO'}")
print(f"Q3 plain .json ignored:          {'YES' if 'cyril-plain-json' not in names else 'NO — it loaded'}")

# Q2 -- does new{workflowPath} (the FILE form) work gate-off?
r = pump(req("_kiro/workflow/new", {
    "workflowPath": GOOD, "inputs": {"task": "say ok"},
    "parentSessionId": sid, "workspacePaths": [CWD],
}), 45)
if r and "error" not in r:
    print(f"Q2 new{{workflowPath}} gate-off:   YES — workflowId={(r.get('result') or {}).get('workflowId')}")
else:
    print(f"Q2 new{{workflowPath}} gate-off:   NO — {json.dumps(r)[:200]}")

OUT.close()
p.stdin.close()
p.terminate()
