#!/usr/bin/env python3
"""
Live _kiro/workflow/* re-verification on kiro-cli 2.19.2 / KAS 0.52.1.

Leg 1 (baseline, same as the 2.19.0 probe): two-step bundled-wf-coder recipe
driven GATE-OFF: _kiro/workflow/new -> invoke -> follow to terminal
run_complete. Verifies event vocabulary/ordering vs the W1 model, double
node_start, capturedOutput + {{s1.output}} templating (BROKEN through 0.48.0),
step peer-session streams, step session titles, any NEW payload fields.

Leg 2 (2.19.2-specific): KAS 0.52.1 introduces `rootConversationId` (session
tree root) and `_kiro/workflow/new` now consults
`host.getSessionRootConversationId(parentSessionId)`: kind "error" -> REFUSE
("cannot resolve the conversation root of parent session ..."); "absent" ->
root = parentSessionId; "found" -> inherited root. Probes:
  2a. parentSessionId = a bogus id  (expect: absent -> proceeds? or refuses?)
  2b. no parentSessionId            (parentless run: must still work)
Both are create-only (no invoke) to keep cost at zero model calls.

Isolation: HOME=<tmp>, real XDG_DATA_HOME; node child reaped via process group.
"""
import glob, json, os, re, signal, subprocess, threading, queue, time, tempfile, sqlite3

SCRATCH = os.environ.get("PROBE_OUT", os.path.dirname(os.path.abspath(__file__)))
TRACE = os.path.join(SCRATCH, "kas-workflow-live-2.19.2-trace.jsonl")
KIRO = os.path.expanduser("~/.local/bin/kiro-cli")
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

def profile_arn():
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()
FAKE_HOME = tempfile.mkdtemp(prefix="wf-live-home-")
WS = tempfile.mkdtemp(prefix="wf-live-ws-")
os.makedirs(os.path.join(WS, ".kiro", "workflows"), exist_ok=True)
RECIPE = os.path.join(WS, ".kiro", "workflows", "audit-min.workflow.json")
with open(RECIPE, "w") as f:
    json.dump({
        "name": "audit-min",
        "description": "2.19.2 wire re-verification: two tiny steps with output templating",
        "inputs": {"token": "string"},
        "steps": [
            {"type": "step", "id": "s1", "agent": "wf-coder", "effortLevel": "low",
             "prompt": "Reply with exactly this single word and nothing else: {{token}}"},
            {"type": "step", "id": "s2", "agent": "wf-coder", "effortLevel": "low",
             "prompt": "Reply with exactly this single token and nothing else: {{s1.output}}-BETA"},
        ],
    }, f, indent=1)

env = dict(os.environ)
env["HOME"] = FAKE_HOME
env["XDG_DATA_HOME"] = os.path.expanduser("~/.local/share")

def read_token():
    c = sqlite3.connect(AUTH_DB)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
    finally:
        c.close()
    if not row:
        return None
    v = row[0]
    v = v.decode() if isinstance(v, (bytes, bytearray)) else v
    d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": PROFILE_ARN}

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=WS, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open(os.path.join(SCRATCH, "wf-live-2.19.2-stderr.log"), "w"),
                        text=True, bufsize=1, start_new_session=True)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
trace = open(TRACE, "w")

def record(direction, obj):
    trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": obj}) + "\n")
    trace.flush()

threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [10]

def send(obj):
    record("client->agent", obj)
    PIN.write(json.dumps(obj) + "\n"); PIN.flush()

def req(m, p):
    _id[0] += 1
    send({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p})
    return _id[0]

WF_EVENTS = []
ALL_NOTIFS = []
def handle_server_req(o):
    m = o["method"]
    if m == "_kiro/auth/getAccessToken":
        send({"jsonrpc": "2.0", "id": o["id"], "result": read_token() or {}})
    elif m == "session/request_permission":
        opts = o.get("params", {}).get("options", [])
        pick = next((x for x in opts if "allow" in (str(x.get("kind", "")) + str(x.get("optionId", ""))).lower()), opts[0] if opts else None)
        res = {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
        send({"jsonrpc": "2.0", "id": o["id"], "result": res})
    elif m == "_kiro/terminal/shell_type":
        send({"jsonrpc": "2.0", "id": o["id"], "result": {"shellType": "bash"}})
    else:
        send({"jsonrpc": "2.0", "id": o["id"], "result": {}})

def pump(until_id=None, timeout=60, stop_fn=None):
    end = time.time() + timeout
    while time.time() < end:
        try:
            raw = msgs.get(timeout=1)
        except queue.Empty:
            if stop_fn and stop_fn():
                return None
            continue
        if raw is None:
            return None
        try:
            o = json.loads(raw)
        except Exception:
            continue
        record("agent->client", o)
        if "method" in o and "id" in o:
            handle_server_req(o)
        elif "method" in o:
            ALL_NOTIFS.append(o)
            if o["method"].startswith("_kiro/workflow/"):
                WF_EVENTS.append(o)
        elif "id" in o and until_id is not None and o["id"] == until_id:
            return o
        if stop_fn and stop_fn():
            return None
    return None

iid = req("initialize", {"protocolVersion": 1,
                         "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"},
                         "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}}})
pump(iid, 60)
nid = req("session/new", {"cwd": WS, "mcpServers": []})
new = pump(nid, 60)
nres = (new or {}).get("result", {})
sid = nres.get("sessionId")
print("== parent session:", sid, "| _meta:", json.dumps(nres.get("_meta"))[:300])

# ---- Leg 1: baseline two-step run (gate-off) ----
wid_req = req("_kiro/workflow/new", {"workflowPath": RECIPE, "inputs": {"token": "ALPHA"},
                                     "parentSessionId": sid, "workspacePaths": [WS]})
wnew = pump(wid_req, 60)
if "error" in (wnew or {}):
    print("workflow/new ERROR:", json.dumps(wnew["error"])[:600]); raise SystemExit(1)
wres = wnew["result"]
wid = wres.get("workflowId")
print("== workflow/new OK (gate-off):", wid, "| result keys:", sorted(wres.keys()), "| initialState keys:", sorted(wres.get("initialState", {}).keys()))
print("   initialState (no nodeTree):", json.dumps({k: v for k, v in wres.get("initialState", {}).items() if k != "nodeTree"})[:600])

inv = req("_kiro/workflow/invoke", {"workflowId": wid})
t0 = time.time()

def run_done():
    return any(e["method"].endswith("run_complete") and
               e["params"].get("status") in ("completed", "failed", "aborted")
               for e in WF_EVENTS)

pump(inv, 20)  # invoke response
pump(timeout=420, stop_fn=run_done)
t1 = time.time()

print(f"\n== workflow event stream ({t1-t0:.0f}s):")
for e in WF_EVENTS:
    p = e["params"]
    kind = e["method"].rsplit("/", 1)[-1]
    core = {k: v for k, v in p.items() if k not in ("workflowId", "parentSessionId", "nodeTree", "finalState", "prompt", "inputs")}
    print(f"  {kind:14} {json.dumps(core)[:230]}")

rc = next((e for e in WF_EVENTS if e["method"].endswith("run_complete")), None)
if rc:
    fs = rc["params"].get("finalState") or {}
    print("\n== run_complete status:", rc["params"].get("status"))
    print("   capturedOutputs:", json.dumps(fs.get("capturedOutputs"))[:400])
    print("   finalState keys:", sorted(fs.keys()))

kinds = {}
for e in WF_EVENTS:
    kinds.setdefault(e["method"].rsplit("/", 1)[-1], set()).update(e["params"].keys())
print("\n== payload keys per kind:")
for k in sorted(kinds):
    print(f"  {k}: {sorted(kinds[k])}")

# step-session text (what did s2 actually say? = the templating truth)
texts = {}
for n in ALL_NOTIFS:
    if n["method"] == "session/update":
        u = n["params"]["update"]
        if u.get("sessionUpdate") == "agent_message_chunk":
            c = u.get("content") or {}
            texts.setdefault(n["params"]["sessionId"], []).append(c.get("text", ""))
print("\n== step session agent text:")
for s, parts in texts.items():
    print(f"  {s[:20]}…: {''.join(parts)[:120]!r}")

lid = req("session/list", {"cwd": WS})
lres = pump(lid, 30)
print("\n== session/list:")
for s in ((lres or {}).get("result") or {}).get("sessions") or []:
    meta = (s.get("_meta") or {}).get("kiro") or {}
    print(f"  {s.get('sessionId','')[:20]}… title={s.get('title')!r} meta={json.dumps(meta)[:220]}")

# ---- Leg 2a: bogus parentSessionId ----
bogus = req("_kiro/workflow/new", {"workflowPath": RECIPE, "inputs": {"token": "BOGUS"},
                                   "parentSessionId": "00000000-0000-4000-8000-00000000dead", "workspacePaths": [WS]})
bres = pump(bogus, 60)
print("\n== 2a workflow/new with BOGUS parentSessionId:",
      ("ERROR " + json.dumps(bres["error"])[:500]) if "error" in (bres or {}) else
      ("OK workflowId=" + str(bres["result"].get("workflowId")) + " initialState.parentSessionId=" + str(bres["result"].get("initialState", {}).get("parentSessionId"))))

# ---- Leg 2b: parentless ----
orphan = req("_kiro/workflow/new", {"workflowPath": RECIPE, "inputs": {"token": "ORPHAN"}, "workspacePaths": [WS]})
ores = pump(orphan, 60)
print("== 2b workflow/new PARENTLESS:",
      ("ERROR " + json.dumps(ores["error"])[:500]) if "error" in (ores or {}) else
      ("OK workflowId=" + str(ores["result"].get("workflowId")) + " initialState keys=" + str(sorted(ores["result"].get("initialState", {}).keys()))))

# list runs (does the list expose the new root?)
lst = req("_kiro/workflow/list", {"workspacePaths": [WS]})
lr = pump(lst, 30)
print("== workflow/list:", json.dumps((lr or {}).get("result") or (lr or {}).get("error"))[:700])

trace.close()
try:
    os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
except Exception:
    proc.terminate()
time.sleep(1)

frames = [json.loads(l) for l in open(TRACE)]
hits = {}
for f in frames:
    s = json.dumps(f["msg"])
    for key in ("rootConversationId", "outputTransformation", "disableAutoCompaction", "conversationRoot"):
        if key in s:
            hits.setdefault(key, 0); hits[key] += 1
print("\n== new-field token hits on the wire:", hits)
print("== persisted files mentioning rootConversationId:")
for path in glob.glob(os.path.join(FAKE_HOME, ".kiro", "**", "*"), recursive=True) + glob.glob(os.path.join(WS, ".kiro", "**", "*"), recursive=True):
    if os.path.isfile(path):
        try:
            txt = open(path, errors="replace").read()
        except Exception:
            continue
        for m in re.finditer(r'"rootConversationId"\s*:\s*("[^"]*"|[^,}\n]*)', txt):
            print(f"   {path.replace(FAKE_HOME, '$HOME').replace(WS, '$WS')}: {m.group(0)[:100]}")
print("\nTRACE:", TRACE, f"({len(frames)} frames)")
