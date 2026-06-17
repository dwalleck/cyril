#!/usr/bin/env python3
"""
Explore the KAS SPEC workflow (requirements -> design -> tasks engine) live.

Flow (from @kiro/acp-type-covenant agent-capabilities/index.d.ts):
  _kiro/spec/resolveSession {featureName?, strategy:'fresh'|'reuse', workspacePaths} -> {sessionId}
  _kiro/spec/invoke {operation:'createSpec', sessionId, userPrompt}                 -> {sessionId, executionId?}
    (other ops: generateDocument {documentType:'requirements'|'design'|'tasks'|'bugfix', action},
     analyzeRequirements, executeTask, runAllTasks)
  _kiro/spec/getTaskStatuses {tasksFilePath, featureName, workspacePaths}           -> {tasks: SpecTaskStatusItem[]}

This RUNS THE MODEL (generates docs) in a throwaway git repo, then inspects what
landed on disk (.kiro/specs/**). Captures the notification stream during the
invoke (session/update kinds + _kiro/* incl. spec/taskStatusChanged) and the
agent text. fs writes happen in-process (no fs cap advertised), so files appear
directly in CWD. Auth token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib, collections

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-spec-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-spec-")
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

# spec needs the spec engine on; enable the related settings defensively
SETTINGS = {"kiro": {"settings": {"_quickSpec": {"enabled": True}, "_requirementAnalyzer": {"enabled": True}}}}
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

NOTIF = collections.Counter()
UPDATE_KINDS = collections.Counter()
SPEC_NOTIFS = []
TOOLS = []
AGENT = []
TURN_ENDS = [0]
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
    NOTIF[m] += 1
    if "spec" in m:
        SPEC_NOTIFS.append((m, json.dumps(p)[:300]))
    if "session/update" in m:
        u = (p.get("update") or {}) if isinstance(p, dict) else {}
        if isinstance(u, dict):
            v = u.get("sessionUpdate")
            if v == "session_info_update":
                kk = ((u.get("_meta") or {}).get("kiro") or {}).get("kind", "?")
                UPDATE_KINDS["session_info_update:" + kk] += 1
                if kk == "turn_end":
                    TURN_ENDS[0] += 1
            else:
                UPDATE_KINDS[v or "?"] += 1
            if v == "agent_message_chunk":
                AGENT.append(u.get("content", {}).get("text", ""))
            elif v in ("tool_call", "tool_call_update"):
                TOOLS.append((v, u.get("title") or u.get("toolCallId"), u.get("status")))

def pump(until, to=260):
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
            if o.get("id") == until and "result" in o:
                return o
        elif "id" in o and o["id"] == until:
            return o
    return None

req("initialize", {"protocolVersion": 1, "clientCapabilities": {"_meta": SETTINGS}})
pump(1, 20)

# 1. resolve a fresh spec session
rid = req("_kiro/spec/resolveSession", {"strategy": "fresh", "workspacePaths": [CWD]})
rr = pump(rid, 40)
log("\n===== _kiro/spec/resolveSession (fresh) =====")
log("  ", json.dumps(rr.get("result")) if rr and "result" in rr else (json.dumps(rr.get("error")) if rr else "(none)"))
spec_sid = (rr.get("result") or {}).get("sessionId") if rr and "result" in rr else None
if not spec_sid:
    log("  !! no spec session id; aborting"); PIN.close(); proc.terminate(); raise SystemExit

# 2. createSpec — generate the spec from a prompt (runs the model)
USERPROMPT = "Create a spec for a small CLI tool `csv2json` that reads a CSV file and writes a JSON array of row objects, with a --pretty flag."
iid = req("_kiro/spec/invoke", {"operation": "createSpec", "sessionId": spec_sid, "userPrompt": USERPROMPT})
ir = pump(iid, 260)
log("\n===== _kiro/spec/invoke (createSpec) response (async — returns before the turn finishes) =====")
log("  ", json.dumps(ir.get("result")) if ir and "result" in ir else (json.dumps(ir.get("error")) if ir else "(none/timeout)"))
# createSpec is async: it injected a user message + started a turn. Wait for the
# turn to complete (turn_end) — spec generation runs the model and writes docs.
log("  waiting for the spec turn to complete (turn_end)...")
deadline = time.time() + 280
while time.time() < deadline and TURN_ENDS[0] < 1:
    pump(-1, 10)
log(f"  turn_end seen: {TURN_ENDS[0]} (after {'completion' if TURN_ENDS[0] else 'TIMEOUT'})")
pump(-1, 5)

log("\n===== notification methods during spec invoke =====")
for m, c in NOTIF.most_common():
    log(f"  {c:>3}x {m}")
log("\n===== session/update kinds =====")
for k, c in UPDATE_KINDS.most_common():
    log(f"  {c:>3}x {k}")
log("\n===== spec-specific notifications (head) =====")
for m, body in SPEC_NOTIFS[:12]:
    log(f"  {m}: {body}")
log("\n===== tool calls (file writes etc., head) =====")
for k, t, st in TOOLS[:25]:
    log(f"  [{k}] {t} -> {st}")

log("\n===== files written under CWD (.md / .kiro) =====")
written = sorted(str(p.relative_to(CWD)) for p in pathlib.Path(CWD).rglob("*")
                 if p.is_file() and (p.suffix == ".md" or ".kiro" in p.parts) and ".git" not in p.parts)
for f in written:
    sz = pathlib.Path(CWD, f).stat().st_size
    log(f"  {f}  ({sz} bytes)")

# 3. if a tasks.md materialized, query task statuses
tasks_files = [f for f in written if f.endswith("tasks.md")]
if tasks_files:
    tf = os.path.join(CWD, tasks_files[0])
    feature = pathlib.Path(tasks_files[0]).parent.name
    tid = req("_kiro/spec/getTaskStatuses", {"tasksFilePath": tf, "featureName": feature, "workspacePaths": [CWD]})
    tr = pump(tid, 40)
    log("\n===== _kiro/spec/getTaskStatuses =====")
    log("  ", json.dumps(tr.get("result"))[:1200] if tr and "result" in tr else (json.dumps(tr.get("error")) if tr else "(none)"))

log("\n===== requirements.md head (if written) =====")
reqs = [f for f in written if f.endswith("requirements.md")]
if reqs:
    log((pathlib.Path(CWD, reqs[0]).read_text()[:1200]))
else:
    log("  (no requirements.md found)")
log("\n===== agent message (head) =====")
log("".join(AGENT)[:800] or "(none)")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
