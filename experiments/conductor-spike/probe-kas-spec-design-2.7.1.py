#!/usr/bin/env python3
"""
Drive the full KAS spec arc: requirements -> design -> tasks, then read task statuses.

  _kiro/spec/resolveSession {strategy:'fresh', workspacePaths}                          -> {sessionId}
  _kiro/spec/invoke {operation:'createSpec', sessionId, userPrompt}                     (requirements phase)
  _kiro/spec/invoke {operation:'generateDocument', sessionId, featureName, specDocuments,
                     documentType:'design'|'tasks', action:'create'}                    (design / tasks phases)
  _kiro/spec/getTaskStatuses {tasksFilePath, featureName, workspacePaths}               -> {tasks:[...]}

Each invoke is ASYNC (returns {sessionId} then streams a turn), so we wait for each
phase's turn_end before the next. Real model work — minutes per phase; run in bg.
Inspects .kiro/specs/<feature>/{requirements,design,tasks}.md on disk after each phase.
Auth token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib, collections

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-spec-design-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-specfull-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p", cwd=CWD, shell=True)
pathlib.Path(CWD, "README.md").write_text("# csv2json\nA tiny CLI.\n")
subprocess.run("git add -A && git commit -qm baseline", cwd=CWD, shell=True)
log(f"# CWD={CWD}")

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": d.get("profile_arn")}

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [0]
def req(m, p):
    _id[0] += 1; PIN.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p}) + "\n"); PIN.flush(); return _id[0]
def reply(rid, res):
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n"); PIN.flush()

TURN_ENDS = [0]
SUBAGENTS = collections.Counter()
def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is not None:
        if m == "_kiro/auth/getAccessToken":
            reply(rid, read_token())
        elif m == "_kiro/terminal/shell_type":
            reply(rid, {"shellType": "bash"})
        elif m == "session/request_permission":
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()), opts[0] if opts else None)
            reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}})
        else:
            reply(rid, {})
        return
    if not m:
        return
    if "session/update" in m:
        u = (p.get("update") or {}) if isinstance(p, dict) else {}
        if isinstance(u, dict):
            v = u.get("sessionUpdate")
            if v == "session_info_update":
                if (((u.get("_meta") or {}).get("kiro") or {}).get("kind")) == "turn_end":
                    TURN_ENDS[0] += 1
            elif v == "tool_call":
                t = u.get("title") or ""
                if t.startswith("Sub-agent: "):
                    SUBAGENTS[t[len("Sub-agent: "):]] += 1

def pump_once(to=10):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = msgs.get(timeout=2)
        except queue.Empty:
            continue
        if raw is None:
            return False
        try:
            o = json.loads(raw)
        except Exception:
            continue
        if "method" in o:
            handle(o)
    return True

def call_sync(method, params, to=40):
    """Send a request and return its response (drives callbacks meanwhile)."""
    rid = req(method, params)
    end = time.time() + to
    while time.time() < end:
        try:
            raw = msgs.get(timeout=2)
        except queue.Empty:
            continue
        if raw is None:
            return None
        try:
            o = json.loads(raw)
        except Exception:
            continue
        if "method" in o:
            handle(o)
        elif o.get("id") == rid:
            return o
    return None

def wait_turn(label, timeout=280):
    before = TURN_ENDS[0]
    log(f"  [{label}] waiting for turn_end ...")
    end = time.time() + timeout
    while time.time() < end and TURN_ENDS[0] <= before:
        pump_once(10)
    log(f"  [{label}] turn_end {'seen' if TURN_ENDS[0] > before else 'TIMEOUT'}")

def specdir_docs():
    base = pathlib.Path(CWD, ".kiro", "specs")
    if not base.exists():
        return None, []
    feats = [d for d in base.iterdir() if d.is_dir()]
    if not feats:
        return None, []
    f = feats[0]
    return f.name, sorted(str(p) for p in f.glob("*.md"))

req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump_once(8)
rr = call_sync("_kiro/spec/resolveSession", {"strategy": "fresh", "workspacePaths": [CWD]})
sid = (rr.get("result") or {}).get("sessionId") if rr and "result" in rr else None
log("resolveSession ->", sid)
assert sid, "no spec session"

# --- requirements ---
log("\n=== PHASE 1: createSpec (requirements) ===")
call_sync("_kiro/spec/invoke", {"operation": "createSpec", "sessionId": sid,
          "userPrompt": "Create a spec for a small CLI tool `csv2json` that reads a CSV file and writes a JSON array of row objects, with a --pretty flag."}, to=30)
wait_turn("requirements")
feat, docs = specdir_docs()
log("  feature:", feat, "| docs:", [os.path.basename(d) for d in docs])

# --- design ---
log("\n=== PHASE 2: generateDocument design ===")
dr = call_sync("_kiro/spec/invoke", {"operation": "generateDocument", "sessionId": sid,
               "featureName": feat, "specDocuments": docs, "documentType": "design", "action": "create"}, to=30)
log("  invoke(design) response:", json.dumps(dr.get("result")) if dr and "result" in dr else (json.dumps(dr.get("error")) if dr else "(none)"))
wait_turn("design")
feat, docs = specdir_docs()
log("  docs now:", [os.path.basename(d) for d in docs])

# --- tasks ---
log("\n=== PHASE 3: generateDocument tasks ===")
tr = call_sync("_kiro/spec/invoke", {"operation": "generateDocument", "sessionId": sid,
               "featureName": feat, "specDocuments": docs, "documentType": "tasks", "action": "create"}, to=30)
log("  invoke(tasks) response:", json.dumps(tr.get("result")) if tr and "result" in tr else (json.dumps(tr.get("error")) if tr else "(none)"))
wait_turn("tasks")
feat, docs = specdir_docs()
log("  docs now:", [os.path.basename(d) for d in docs])

# --- task statuses ---
tasks_md = next((d for d in docs if d.endswith("tasks.md")), None)
if tasks_md:
    sr = call_sync("_kiro/spec/getTaskStatuses", {"tasksFilePath": tasks_md, "featureName": feat, "workspacePaths": [CWD]}, to=40)
    log("\n=== getTaskStatuses ===")
    log("  ", json.dumps(sr.get("result"))[:1500] if sr and "result" in sr else (json.dumps(sr.get("error")) if sr else "(none)"))

log("\n===== files on disk =====")
for d in docs:
    log(f"  {os.path.relpath(d, CWD)}  ({pathlib.Path(d).stat().st_size} bytes)")
log("\n===== subagents used across the arc =====", dict(SUBAGENTS))
for name, f in [("design", "design.md"), ("tasks", "tasks.md")]:
    pth = next((d for d in docs if d.endswith(f)), None)
    if pth:
        log(f"\n===== {f} head =====")
        log(pathlib.Path(pth).read_text()[:1400])
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
