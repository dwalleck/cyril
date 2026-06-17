#!/usr/bin/env python3
"""
Drive KAS `spec/invoke {operation:'executeTask'}` on ONE leaf task — i.e. watch the
spec engine actually implement code from the plan.

Arc: resolveSession -> createSpec (reqs) -> generateDocument design -> generateDocument
tasks -> getTaskStatuses (pick a small leaf task) -> invoke executeTask {tasksFilePath,
taskId} -> inspect the SOURCE files it wrote (outside .kiro/specs) + whether the task's
markdownStatus/executionStatus flips.

SpecExecuteTaskRequest: {operation:'executeTask', sessionId, featureName, specDocuments,
tasksFilePath, taskId}. Each invoke is async (returns {sessionId}, then streams a turn);
executeTask edits real files (safe — throwaway git repo). Heavy/slow: run in background.
Auth token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-spec-executetask-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-exec-")
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
EXEC_TOOLS = []     # tool_calls observed during the executeTask phase
CAPTURE = [False]
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
            elif v in ("tool_call", "tool_call_update") and CAPTURE[0]:
                EXEC_TOOLS.append((v, u.get("title") or u.get("toolCallId"), u.get("status")))

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

def wait_turn(label, timeout=320):
    before = TURN_ENDS[0]
    log(f"  [{label}] waiting for turn_end ...")
    end = time.time() + timeout
    while time.time() < end and TURN_ENDS[0] <= before:
        pump_once(10)
    log(f"  [{label}] turn_end {'seen' if TURN_ENDS[0] > before else 'TIMEOUT'}")

def specdir():
    base = pathlib.Path(CWD, ".kiro", "specs")
    feats = [d for d in base.iterdir() if d.is_dir()] if base.exists() else []
    return feats[0] if feats else None

def docs(f):
    return sorted(str(p) for p in f.glob("*.md")) if f else []

def source_files():
    """Files NOT under .kiro and not .git — i.e. real implementation output."""
    out = {}
    for p in pathlib.Path(CWD).rglob("*"):
        if p.is_file() and ".git" not in p.parts and ".kiro" not in p.parts and p.name != "README.md":
            out[str(p.relative_to(CWD))] = p.stat().st_size
    return out

def first_leaf(tasks):
    """Pick a small leaf task, preferring one that writes code over project-init/install."""
    leaves = []
    def walk(ts):
        for t in ts:
            if t.get("isLeaf"):
                leaves.append(t)
            walk(t.get("subTasks") or [])
    walk(tasks)
    pref = [t for t in leaves if not any(w in t.get("taskId", "").lower() for w in ("initialize", "set up", "install", "project"))]
    return (pref or leaves)[0] if (pref or leaves) else None

req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump_once(8)
rr = call_sync("_kiro/spec/resolveSession", {"strategy": "fresh", "workspacePaths": [CWD]})
sid = (rr.get("result") or {}).get("sessionId") if rr and "result" in rr else None
log("resolveSession ->", sid)
assert sid

log("\n=== building the spec (requirements -> design -> tasks) ===")
call_sync("_kiro/spec/invoke", {"operation": "createSpec", "sessionId": sid,
          "userPrompt": "Create a spec for a small CLI tool `csv2json` that reads a CSV file and writes a JSON array of row objects, with a --pretty flag."}, to=30)
wait_turn("requirements")
f = specdir(); assert f, "no spec dir after createSpec"; feat = f.name
call_sync("_kiro/spec/invoke", {"operation": "generateDocument", "sessionId": sid, "featureName": feat,
          "specDocuments": docs(f), "documentType": "design", "action": "create"}, to=30)
wait_turn("design")
call_sync("_kiro/spec/invoke", {"operation": "generateDocument", "sessionId": sid, "featureName": feat,
          "specDocuments": docs(f), "documentType": "tasks", "action": "create"}, to=30)
wait_turn("tasks")

tasks_md = next((d for d in docs(f) if d.endswith("tasks.md")), None)
sr = call_sync("_kiro/spec/getTaskStatuses", {"tasksFilePath": tasks_md, "featureName": feat, "workspacePaths": [CWD]}, to=40)
tasks = (sr.get("result") or {}).get("tasks", []) if sr and "result" in sr else []
target = first_leaf(tasks)
log("\n=== chosen leaf task ===")
log("  ", json.dumps(target)[:300] if target else "(none — abort)")
assert target, "no leaf task"

src_before = source_files()
log("\n=== PHASE: executeTask ===  taskId=", repr(target["taskId"]))
CAPTURE[0] = True
er = call_sync("_kiro/spec/invoke", {"operation": "executeTask", "sessionId": sid, "featureName": feat,
               "specDocuments": docs(f), "tasksFilePath": tasks_md, "taskId": target["taskId"]}, to=30)
log("  invoke(executeTask) response:", json.dumps(er.get("result")) if er and "result" in er else (json.dumps(er.get("error")) if er else "(none)"))
wait_turn("executeTask", timeout=360)
CAPTURE[0] = False

src_after = source_files()
new_files = {k: v for k, v in src_after.items() if k not in src_before}
changed = {k: v for k, v in src_after.items() if k in src_before and src_before[k] != v}
log("\n===== SOURCE files created by executeTask =====")
for k, v in sorted(new_files.items()):
    log(f"  + {k}  ({v} bytes)")
log("  changed:", changed or "(none)")
log("\n===== tool calls during executeTask (head) =====")
for k, t, st in EXEC_TOOLS[:35]:
    log(f"  [{k}] {t} -> {st}")

# did the task status flip?
sr2 = call_sync("_kiro/spec/getTaskStatuses", {"tasksFilePath": tasks_md, "featureName": feat, "workspacePaths": [CWD]}, to=40)

def find_status(ts, tid):
    for t in ts:
        if t.get("taskId") == tid:
            return t.get("markdownStatus"), t.get("executionStatus")
        r = find_status(t.get("subTasks") or [], tid)
        if r:
            return r
    return None
after_status = find_status((sr2.get("result") or {}).get("tasks", []) if sr2 and "result" in sr2 else [], target["taskId"])
log("\n===== task status after executeTask =====")
log("  before: markdownStatus=", target.get("markdownStatus"), " | after:", after_status)

# show a created source file head
log("\n===== a created source file (head) =====")
pick = next((k for k in sorted(new_files) if k.endswith((".ts", ".js", ".json", ".py"))), None)
if pick:
    log(f"--- {pick} ---")
    log(pathlib.Path(CWD, pick).read_text()[:1200])
else:
    log("  (no obvious source file created)")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
