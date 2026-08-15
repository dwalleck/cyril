#!/usr/bin/env python3
"""Reattach round 2: kill the WHOLE agent tree mid-run, then resume from a
fresh process. (2.16.2; follows probe-kas-workflow-reattach-2.16.2.py)

Round 1 killed only the spawned `kiro-cli` wrapper and learned that the
`kiro-cli-chat -> node acp-server.js` chain survives as an orphan that keeps
beating (and driving) the run — resume from a fresh process is correctly
refused with `liveness verdict: live` naming the orphan's pid. That answers
the crash-with-orphan case. This round answers the genuinely-dead case:

  Q5  killpg the whole tree mid-run; does a fresh process's `resume` succeed
      IMMEDIATELY via the dead-pid short-circuit (`workflow.liveness.
      stale_dead_pid`: processProbe fails -> verdict "stale", no 135s wait)?
  Q6  do lifecycle events for the resumed run stream to the fresh process
      (the late-attach client), and does the run reach run_complete?
  Q7  after completion, what does `list` report and what is on disk?

ORACLE: as round 1 — the run state file on disk, read directly, compared
against the RPC answers; plus `ps` on the killed pgid proving no survivor.

    probe-kas-workflow-reattach2-2.16.2.py <kiro-cli> <out.jsonl>

COSTS CREDITS: one longer step, killed mid-flight and resumed to completion.
"""
import json, os, queue, signal, sqlite3, subprocess, sys, tempfile, threading, time

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
CWD = tempfile.mkdtemp(prefix="kas-wfreattach2-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-wfreattach2home-")
env = dict(os.environ)
env["HOME"] = TMPH
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))


class Agent:
    def __init__(self, tag):
        self.tag = tag
        self.p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, text=True, bufsize=1,
                                  start_new_session=True)
        self.q = queue.Queue()
        threading.Thread(target=lambda: [self.q.put(l.strip()) for l in self.p.stdout if l.strip()],
                         daemon=True).start()
        self.i = 0
        self.events = []
        self.sessions = set()

    def req(self, m, pr):
        self.i += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": m, "params": pr}) + "\n")
        self.p.stdin.flush()
        return self.i

    def rep(self, rid, res):
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
        self.p.stdin.flush()

    def pump(self, until, to=60, stop_on=None):
        end = time.time() + to
        while time.time() < end:
            try:
                raw = self.q.get(timeout=2)
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
                if m == "_kiro/auth/getAccessToken":
                    self.rep(rid, read_token())  # always fresh, not the t0 snapshot
                elif m == "_kiro/terminal/shell_type":
                    self.rep(rid, {"shellType": "bash"})
                else:
                    self.rep(rid, {})
                continue
            if m and m.startswith("_kiro/workflow/"):
                self.events.append((m, o.get("params") or {}))
                if stop_on and any(m.endswith(k) for k in stop_on):
                    return ("event", o)
            elif m == "session/update":
                s = (o.get("params") or {}).get("sessionId")
                if s:
                    self.sessions.add(s)
            if until is not None and rid == until and ("result" in o or "error" in o):
                return o
        return None

    def call(self, m, pr, to=60):
        return self.pump(self.req(m, pr), to)

    def follow_run(self, to=300):
        r = self.pump(None, to, stop_on=["/run_complete"])
        if r and isinstance(r, tuple) and r[0] == "event":
            return (r[1].get("params") or {}).get("status")
        return None

    def start(self):
        self.call("initialize", {
            "protocolVersion": 1,
            "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}},
            "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True}},
        }, 30)
        r = self.call("session/new", {"cwd": CWD, "mcpServers": []}, 60) or {}
        res = r.get("result", {})
        self.sid = res.get("sessionId")
        print(f"  [{self.tag}] sessionId={self.sid} "
              f"workflowsEnabled={res.get('_meta', {}).get('workflowsEnabled')!r}", flush=True)


def show(label, r):
    if r is None:
        print(f"  {label}: NO RESPONSE", flush=True)
    elif isinstance(r, dict) and "error" in r:
        e = r["error"]
        print(f"  {label}: ERROR {e.get('code')} "
              f"{json.dumps(e.get('data', {}).get('details') or e.get('message'))[:300]}", flush=True)
    else:
        print(f"  {label}: {json.dumps(r.get('result'))[:240]}", flush=True)
    return r


DAG = {"name": "cyril-reattach2", "description": "Longer one-step run; whole tree killed mid-flight.",
       "inputs": {},
       "steps": [{"type": "step", "id": "slow", "agent": "wf-coder",
                  "prompt": "Using no tools, write one line for each number from 1 to 40: "
                            "the number, then a short original sentence about it."}]}

print("===== process 1 =====", flush=True)
a = Agent("p1")
a.start()
r = show("new R3", a.call("_kiro/workflow/new",
                          {"workflow": DAG, "inputs": {}, "parentSessionId": a.sid,
                           "workspacePaths": [CWD]}))
R3 = ((r or {}).get("result") or {}).get("workflowId")
a.events.clear()
show("invoke R3", a.call("_kiro/workflow/invoke", {"workflowId": R3}))
seen = (any(m.endswith("/node_start") for m, _ in a.events)
        or a.pump(None, 120, stop_on=["/node_start"]) is not None)
print(f"  node_start observed: {seen}", flush=True)
time.sleep(3)

pgid = os.getpgid(a.p.pid)
os.killpg(pgid, signal.SIGKILL)
print(f"  killpg({pgid}) SIGKILL sent", flush=True)
time.sleep(2)
ps = subprocess.run(["ps", "-o", "pid=,comm=", "-g", str(pgid)], capture_output=True, text=True)
survivors = ps.stdout.strip()
print(f"  survivors in pgid {pgid}: {survivors!r} (empty means the tree is dead)", flush=True)

print("\n===== process 2 (fresh, immediately — no stale wait) =====", flush=True)
b = Agent("p2")
b.start()
t0 = time.time()
r = show("list", b.call("_kiro/workflow/list", {"sessionId": b.sid, "workspacePaths": [CWD]}))
runs = ((r or {}).get("result") or {}).get("runs", [])
for run in runs:
    print(f"    listed: {run.get('workflowId')} status={run.get('status')!r}", flush=True)

b.events.clear()
r = show("resume R3 (attempt 1)", b.call("_kiro/workflow/resume", {"workflowId": R3}, 90))
attempts = 1
while r is not None and "error" in r and time.time() - t0 < 200:
    time.sleep(10)
    attempts += 1
    r = show(f"resume R3 (attempt {attempts})", b.call("_kiro/workflow/resume", {"workflowId": R3}, 90))
print(f"  resume accepted after {attempts} attempt(s), {time.time() - t0:.1f}s post-kill", flush=True)

st = None
if r is not None and "error" not in r:
    st = b.follow_run(300)
kinds = {}
for m, _ in b.events:
    kinds[m] = kinds.get(m, 0) + 1
print(f"  R3 final status after resume: {st!r}", flush=True)
for k in sorted(kinds):
    print(f"    {kinds[k]:2}x {k}", flush=True)

r = show("final list", b.call("_kiro/workflow/list", {"sessionId": b.sid, "workspacePaths": [CWD]}))
show("final inspect", b.call("_kiro/workflow/inspect", {"workflowId": R3}))

print("\n===== ORACLE: run state on disk =====", flush=True)
hits = []
for root in (TMPH, CWD):
    for dirpath, _, filenames in os.walk(root):
        for f in filenames:
            p = os.path.join(dirpath, f)
            try:
                d = json.load(open(p))
            except Exception:
                continue
            wid = d.get("workflowId") or (d.get("state") or {}).get("workflowId")
            if wid == R3:
                stt = d.get("status") or (d.get("state") or {}).get("status")
                hits.append((p, stt))
for p, stt in hits:
    print(f"  disk: status={stt!r}  {p}", flush=True)

print("\n===== VERDICT =====", flush=True)
print(f"Q5 dead-tree resume: {attempts} attempt(s) needed", flush=True)
print(f"Q6 late-attach stream: {sum(kinds.values())} lifecycle event(s), final={st!r}", flush=True)
OUT.close()

# clean up our own agent tree (p2): SIGKILL its process group.
try:
    os.killpg(os.getpgid(b.p.pid), signal.SIGKILL)
except Exception:
    pass
