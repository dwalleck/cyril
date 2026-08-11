#!/usr/bin/env python3
"""Do KAS workflow runs survive the agent process, and can a fresh process
list/load/resume them? (2.16.2 / run-disk-operations.ts)

cyril-0qe6's run lifetime rests on "a run is a persisted, workspace-scoped
object that outlives the session and the process" — but every committed
capture calls `_kiro/workflow/list` BEFORE its invoke and gets `{"runs": []}`.
No capture shows a run in `list` at all, and none crosses a process boundary.
The 2.16.2 hazard note (run-disk-operations.ts is new) demands this re-check.

Questions, smallest first (all gate-off, per ADR-0011):
  Q0  `list` without workspacePaths still -32603?          (error contract)
  Q1  after invoke completes, does `list` show the run?    (same process)
  Q2  fresh process, same workspace: does `list` show it?  (disk persistence)
  Q3  what do `load` / `inspect` return for a persisted run?
  Q4  kill the agent mid-run; can a fresh process `resume` the run, and do
      lifecycle events stream to the late-attached client? (AC4's mechanism)

ORACLE: the run objects on disk, read directly. A filesystem snapshot diff
(taken before/after each phase over the fake HOME, the workspace, and
~/.local/share/kiro-cli) locates where run-disk-operations.ts writes; the
files' workflowIds/statuses are then compared item-by-item against the RPC
`list` answer. Filesystem vs live JSON-RPC — independent mechanisms.

    probe-kas-workflow-reattach-2.16.2.py <kiro-cli> <out.jsonl>

HOME-isolated per feedback_isolate_kiro_probes_with_home; both processes
share the same fake HOME + workspace so persistence carries over.
COSTS CREDITS: one trivial step + one longer step (killed mid-flight, then
resumed).
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
CWD = tempfile.mkdtemp(prefix="kas-wfreattach-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-wfreattachhome-")
env = dict(os.environ)
env["HOME"] = TMPH
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

SNAP_ROOTS = [TMPH, CWD]
KIRO_DATA = os.path.expanduser("~/.local/share/kiro-cli")
SKIP = {os.path.join(KIRO_DATA, d) for d in ("kas", "node", "bun", "knowledge_bases")}


def snapshot():
    seen = {}
    for root in SNAP_ROOTS + [KIRO_DATA]:
        for dirpath, dirnames, filenames in os.walk(root):
            if any(dirpath == s or dirpath.startswith(s + os.sep) for s in SKIP):
                dirnames[:] = []
                continue
            for f in filenames:
                p = os.path.join(dirpath, f)
                try:
                    seen[p] = os.stat(p).st_mtime
                except OSError:
                    pass
    return seen


def snap_diff(before, after, label):
    new = [p for p in after if p not in before]
    changed = [p for p in after if p in before and after[p] != before[p]]
    print(f"  [disk:{label}] {len(new)} new, {len(changed)} changed")
    for p in sorted(new):
        print(f"    NEW {p}")
    for p in sorted(changed):
        print(f"    CHG {p}")
    return new, changed


class Agent:
    def __init__(self, tag):
        self.tag = tag
        self.p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, text=True, bufsize=1)
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
        """Read frames until response `until` arrives, or a workflow event kind
        in stop_on arrives (returns ('event', frame)), or timeout (None)."""
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
                    self.rep(rid, TOK)
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
        if r and r[0] == "event":
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
        self.wf_enabled = res.get("_meta", {}).get("workflowsEnabled")
        print(f"  [{self.tag}] sessionId={self.sid} workflowsEnabled={self.wf_enabled!r}")


def show(label, r):
    if r is None:
        print(f"  {label}: NO RESPONSE")
    elif isinstance(r, dict) and "error" in r:
        e = r["error"]
        print(f"  {label}: ERROR {e.get('code')} {e.get('message')!r}")
    else:
        print(f"  {label}: {json.dumps(r.get('result'))[:240]}")
    return r


DAG1 = {"name": "cyril-reattach-r1", "description": "Trivial one-step run; completes in process 1.",
        "inputs": {},
        "steps": [{"type": "step", "id": "only", "agent": "wf-coder",
                   "prompt": "Reply with the word ok. Do not use any tools."}]}
DAG2 = {"name": "cyril-reattach-r2", "description": "Longer one-step run; process killed mid-flight.",
        "inputs": {},
        "steps": [{"type": "step", "id": "slow", "agent": "wf-coder",
                   "prompt": "Using no tools, write one line for each number from 1 to 40: "
                             "the number, then a short original sentence about it."}]}

base = snapshot()

print("===== process 1 =====")
a = Agent("p1")
a.start()

print("-- Q0: list without workspacePaths (error contract)")
show("list(no wsp)", a.call("_kiro/workflow/list", {"sessionId": a.sid}))

print("-- R1: new + invoke -> completion")
r = show("new R1", a.call("_kiro/workflow/new",
                          {"workflow": DAG1, "inputs": {}, "parentSessionId": a.sid,
                           "workspacePaths": [CWD]}))
R1 = ((r or {}).get("result") or {}).get("workflowId")
show("invoke R1", a.call("_kiro/workflow/invoke", {"workflowId": R1}))
st = a.follow_run(300)
print(f"  R1 final status: {st!r}")

print("-- Q1: list AFTER a completed run (same process)")
r = show("list", a.call("_kiro/workflow/list", {"sessionId": a.sid, "workspacePaths": [CWD]}))
q1_runs = ((r or {}).get("result") or {}).get("runs", [])
after_r1 = snapshot()
new1, chg1 = snap_diff(base, after_r1, "after R1")

print("-- R2: new + invoke, then SIGKILL mid-run")
a.events.clear()  # so any node_start seen below is unambiguously R2's
r = show("new R2", a.call("_kiro/workflow/new",
                          {"workflow": DAG2, "inputs": {}, "parentSessionId": a.sid,
                           "workspacePaths": [CWD]}))
R2 = ((r or {}).get("result") or {}).get("workflowId")
show("invoke R2", a.call("_kiro/workflow/invoke", {"workflowId": R2}))
ev = (any(m.endswith("/node_start") for m, _ in a.events)
      or a.pump(None, 120, stop_on=["/node_start"]) is not None)
print(f"  R2 node_start observed: {ev}")
time.sleep(3)  # let the step turn get in flight / state flush
a.p.kill()
print("  p1 SIGKILLED mid-run")
time.sleep(2)
after_kill = snapshot()
new2, chg2 = snap_diff(after_r1, after_kill, "after kill")

print("\n===== process 2 (fresh spawn, same workspace + HOME) =====")
b = Agent("p2")
b.start()

print("-- Q2: list from a FRESH process")
r = show("list", b.call("_kiro/workflow/list", {"sessionId": b.sid, "workspacePaths": [CWD]}))
q2_runs = ((r or {}).get("result") or {}).get("runs", [])

print("-- Q3: load / inspect the persisted runs")
for wid, tag in ((R1, "R1"), (R2, "R2")):
    show(f"load {tag}", b.call("_kiro/workflow/load", {"workflowId": wid}))
    show(f"inspect {tag}", b.call("_kiro/workflow/inspect", {"workflowId": wid}))

print("-- Q4: resume the killed run from the fresh process")
b.events.clear()
show("resume R2", b.call("_kiro/workflow/resume", {"workflowId": R2}, 90))
st2 = b.follow_run(300)
kinds = {}
for m, _ in b.events:
    kinds[m] = kinds.get(m, 0) + 1
print(f"  R2 final status after resume: {st2!r}")
for k in sorted(kinds):
    print(f"    {kinds[k]:2}x {k}")

print("-- final list")
r = show("list", b.call("_kiro/workflow/list", {"sessionId": b.sid, "workspacePaths": [CWD]}))
final_runs = ((r or {}).get("result") or {}).get("runs", [])

print("\n===== ORACLE: run objects on disk vs RPC list =====")
disk = {}
for p in set(new1 + chg1 + new2 + chg2) | set(snapshot()) - set(base):
    if not p.endswith(".json") and ".jsonl" not in p:
        continue
    try:
        d = json.load(open(p))
    except Exception:
        continue
    wid = d.get("workflowId") or (d.get("state") or {}).get("workflowId")
    if wid:
        disk[wid] = (p, d.get("status") or (d.get("state") or {}).get("status"))
for wid, (p, stt) in sorted(disk.items()):
    print(f"  disk: {wid} status={stt!r}  {p}")
rpc = {r.get("workflowId"): r.get("status") for r in final_runs if isinstance(r, dict)}
for wid, stt in sorted(rpc.items()):
    mark = "MATCH" if wid in disk else "RPC-ONLY"
    print(f"  rpc : {wid} status={stt!r}  [{mark}]")
for wid in disk:
    if wid not in rpc:
        print(f"  disk-only (not in rpc list): {wid}")

print("\n===== VERDICT =====")
print(f"Q0 -32603 contract, Q1 same-proc list: {len(q1_runs)} run(s)")
print(f"Q2 fresh-proc list: {len(q2_runs)} run(s)  (expect 2 if runs persist)")
print(f"Q4 resume-after-kill: {st2!r}, {sum(kinds.values())} lifecycle event(s) to late client")
OUT.close()
b.p.stdin.close()
b.p.terminate()
