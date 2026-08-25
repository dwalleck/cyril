#!/usr/bin/env python3
"""
KAS 0.52.1 (kiro-cli 2.19.2) `_meta.kiro.outputTransformation` live capture.

0.52.1 stamps a NEW field onto the terminal tool_call_update `_meta.kiro` (and
the persisted tool_result) whenever a tool's output was transformed before the
model saw it:
   {kind: "offloaded", absFilePath, totalChars}   (>= LARGE_OUTPUT_CONFIG.CHAR_THRESHOLD = 30_000 chars,
                                                    full output written to a file, model gets head/tail preview)
   {kind: "clipped",   originalChars}             (clipped per large-output config)
Variant 2: initialize._meta.kiro.settings ALSO carries largeToolOutputHandler.enabled=true (isFeatureEnabled reads the connection-level feature config, not session settings). session/new carries _meta.kiro.settings.largeToolOutputHandler.enabled=true (the model-side offload handler is gated by isFeatureEnabled("largeToolOutputHandler"), default off).
Two turns: (1) `seq 1 8000` (~39 KB, over the threshold) and (2) `seq 1 300`
(control, under it). Reports the terminal update's _meta.kiro, content shape and
size, plus any file the agent wrote under the throwaway HOME.
Isolation: HOME=<tmp>, real XDG_DATA_HOME.
"""
import glob, json, os, re, subprocess, threading, queue, time, tempfile, sqlite3, signal

SCRATCH = os.environ.get("PROBE_OUT", os.path.dirname(os.path.abspath(__file__)))
TRACE = os.path.join(SCRATCH, "kas-output-transform-lth-init-2.19.2-trace.jsonl")
KIRO = os.path.expanduser("~/.local/bin/kiro-cli")
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

def profile_arn():
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()
FAKE_HOME = tempfile.mkdtemp(prefix="ot-home-")
CWD = tempfile.mkdtemp(prefix="ot-cwd-")
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

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open(os.path.join(SCRATCH, "ot-lth-init-stderr.log"), "w"),
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

def handle_server_req(o):
    m = o["method"]
    if m == "_kiro/auth/getAccessToken":
        send({"jsonrpc": "2.0", "id": o["id"], "result": read_token() or {}})
    elif m == "session/request_permission":
        opts = o.get("params", {}).get("options", [])
        pick = next((x for x in opts if x.get("kind") == "allow_once"), opts[0] if opts else None)
        res = {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
        send({"jsonrpc": "2.0", "id": o["id"], "result": res})
    elif m == "_kiro/terminal/shell_type":
        send({"jsonrpc": "2.0", "id": o["id"], "result": {"shellType": "bash"}})
    else:
        send({"jsonrpc": "2.0", "id": o["id"], "result": {}})

def pump(until_id=None, timeout=60, idle_exit=None):
    end = time.time() + timeout
    last = time.time()
    while time.time() < end:
        try:
            raw = msgs.get(timeout=1)
        except queue.Empty:
            if idle_exit and time.time() - last > idle_exit:
                return None
            continue
        if raw is None:
            return None
        last = time.time()
        try:
            o = json.loads(raw)
        except Exception:
            continue
        record("agent->client", o)
        if "method" in o and "id" in o:
            handle_server_req(o)
        elif "id" in o and until_id is not None and o["id"] == until_id:
            return o
    return None

iid = req("initialize", {"protocolVersion": 1,
                         "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"},
                         "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}}, "_meta": {"kiro": {"settings": {"largeToolOutputHandler": {"enabled": True}}}}})
pump(iid, 60)
nid = req("session/new", {"cwd": CWD, "mcpServers": [], "_meta": {"kiro": {"settings": {"largeToolOutputHandler": {"enabled": True}}}}})
new = pump(nid, 60)
sid = ((new or {}).get("result") or {}).get("sessionId")
print("session:", sid)

for label, cmd in (("BIG (~39KB)", "seq 1 8000"), ("SMALL (control)", "seq 1 300")):
    prompt = (f"Use the execute_bash tool ONCE to run EXACTLY this command, verbatim, then reply with the single word DONE: {cmd}")
    t0 = time.time()
    pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]})
    resp = pump(pid, 300)
    print(f"\n== {label}: prompt response:", json.dumps((resp or {}).get("result") or (resp or {}).get("error"))[:200],
          f"({time.time()-t0:.1f}s)")
trace.close()
try:
    os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
except Exception:
    proc.terminate()
time.sleep(1)

frames = [json.loads(l) for l in open(TRACE)]
print("\n== tool_call lifecycle frames (terminal _meta.kiro + content size):")
for f in frames:
    if f["dir"] != "agent->client":
        continue
    m = f["msg"]
    if m.get("method") != "session/update":
        continue
    u = (m.get("params") or {}).get("update", {})
    k = u.get("sessionUpdate")
    if k in ("tool_call", "tool_call_update"):
        content = u.get("content")
        size = 0
        if isinstance(content, list):
            for c in content:
                inner = c.get("content") or {}
                size += len(inner.get("text") or "")
        kiro = ((u.get("_meta") or {}).get("kiro") or {})
        print(f"  {k:16} status={u.get('status'):12} title={u.get('title','')!r:22} content_chars={size:6} _meta.kiro keys={sorted(kiro.keys())}")
        if "outputTransformation" in kiro:
            print(f"      outputTransformation = {json.dumps(kiro['outputTransformation'])}")
        if k == "tool_call_update" and u.get("status") == "completed" and isinstance(content, list) and content:
            txt = (content[0].get("content") or {}).get("text") or ""
            print(f"      terminal text head: {txt[:160]!r}")
            print(f"      terminal text tail: {txt[-160:]!r}")
print("\n== files written under fake HOME (offload target hunt):")
for path in glob.glob(os.path.join(FAKE_HOME, "**", "*"), recursive=True):
    if os.path.isfile(path) and os.path.getsize(path) > 20000 and "logs" not in path:
        print(f"   {path.replace(FAKE_HOME, '$HOME')} ({os.path.getsize(path)} bytes)")
print("\nTRACE:", TRACE, f"({len(frames)} frames)")
