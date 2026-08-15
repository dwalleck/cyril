#!/usr/bin/env python3
"""Does `_kiro/workflow/cancel` route with the gate OFF? (2.16.2)

The one verb in cyril-0qe6's v1 command surface still carrying ADR-0011's
"not individually verified gate-off" caveat. Zero-credit: cancel a run that
was created (`new`) but never invoked. Also records cancel's reply shape and
the post-cancel status `list` reports.

    probe-kas-workflow-cancel-gateoff-2.16.2.py <kiro-cli> <out.jsonl>
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


CWD = tempfile.mkdtemp(prefix="kas-wfcancel-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-wfcancelhome-")
env = dict(os.environ)
env["HOME"] = TMPH
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1,
                     start_new_session=True)
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
        OUT.flush()
        m, rid = o.get("method"), o.get("id")
        if rid is not None and m:
            rep(rid, read_token() if m == "_kiro/auth/getAccessToken"
                else {"shellType": "bash"} if m == "_kiro/terminal/shell_type" else {})
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


def show(label, r):
    if r is None:
        print(f"  {label}: NO RESPONSE", flush=True)
    elif "error" in r:
        e = r["error"]
        print(f"  {label}: ERROR {e.get('code')} "
              f"{json.dumps(e.get('data', {}).get('details') or e.get('message'))[:240]}", flush=True)
    else:
        print(f"  {label}: {json.dumps(r.get('result'))[:220]}", flush=True)
    return r


pump(req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}},
    "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True}},
}), 30)
r = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 60) or {}
sid = r.get("result", {}).get("sessionId")
print(f"  sessionId={sid} workflowsEnabled="
      f"{r.get('result', {}).get('_meta', {}).get('workflowsEnabled')!r}", flush=True)

DAG = {"name": "cyril-cancel-probe", "description": "Created, never invoked, cancelled.",
       "inputs": {},
       "steps": [{"type": "step", "id": "only", "agent": "wf-coder",
                  "prompt": "Reply with the word ok. Do not use any tools."}]}
r = show("new", pump(req("_kiro/workflow/new",
                         {"workflow": DAG, "inputs": {}, "parentSessionId": sid,
                          "workspacePaths": [CWD]}), 45))
wid = ((r or {}).get("result") or {}).get("workflowId")
show("cancel", pump(req("_kiro/workflow/cancel", {"workflowId": wid}), 45))
show("list", pump(req("_kiro/workflow/list", {"sessionId": sid, "workspacePaths": [CWD]}), 45))
OUT.close()
import signal
os.killpg(os.getpgid(p.pid), signal.SIGKILL)
