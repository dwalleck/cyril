#!/usr/bin/env python3
"""
KAS 0.48.0 (kiro-cli 2.19.1) `_kiro/tools/content_chunk` live capture.

Advertises initialize.clientCapabilities._meta.kiro.streamingShellContent=true,
prompts a slow ticking shell command (+ a fake AWS-key-shaped string to observe
the wire-side redactor), records every frame, then prints:
  - the content_chunk sequence with timestamps + payloads
  - the surrounding tool_call lifecycle (initial call, updates, terminal)
Isolation: HOME=<tmp>, real XDG_DATA_HOME.
"""
import json, os, re, subprocess, threading, queue, time, tempfile, sqlite3, signal

SCRATCH = os.path.dirname(os.path.abspath(__file__))
TRACE = os.path.join(SCRATCH, "kas-content-chunk-2.19.1-trace.jsonl")
KIRO = os.path.expanduser("~/.local/bin/kiro-cli")
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

def profile_arn():
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()
FAKE_HOME = tempfile.mkdtemp(prefix="chunk-home-")
CWD = tempfile.mkdtemp(prefix="chunk-cwd-")
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
                        stderr=open(os.path.join(SCRATCH, "chunk-stderr.log"), "w"),
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

# THE opt-in: streamingShellContent in clientCapabilities._meta.kiro
iid = req("initialize", {"protocolVersion": 1,
                         "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"},
                         "clientCapabilities": {
                             "fs": {"readTextFile": False, "writeTextFile": False},
                             "_meta": {"kiro": {"streamingShellContent": True}}}})
pump(iid, 60)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
new = pump(nid, 60)
sid = ((new or {}).get("result") or {}).get("sessionId")
print("session:", sid)

prompt = ("Use the execute_bash tool ONCE to run EXACTLY this command, verbatim: "
          "for i in 1 2 3 4 5; do echo \"tick $i at $(date +%T)\"; sleep 1; done; "
          "echo \"key: AKIA1234567890ABCDEF\"; echo done")
t0 = time.time()
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]})
resp = pump(pid, 300)
print("prompt response:", json.dumps((resp or {}).get("result") or (resp or {}).get("error"))[:200],
      f"({time.time()-t0:.1f}s)")
trace.close()
try:
    os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
except Exception:
    proc.terminate()

# Analysis: chunk sequence + tool lifecycle
frames = [json.loads(l) for l in open(TRACE)]
tp = next((f["ts"] for f in frames if f["dir"] == "client->agent" and f["msg"].get("method") == "session/prompt"), 0)
print("\n== tool lifecycle + chunks (t=0 at prompt):")
for f in frames:
    if f["dir"] != "agent->client":
        continue
    m = f["msg"]
    t = f["ts"] - tp
    if m.get("method") == "_kiro/tools/content_chunk":
        p = m["params"]
        inner = ((p.get("content") or {}).get("content") or {})
        print(f"  +{t:6.2f}s content_chunk  toolCallId={p.get('toolCallId','')[:24]}… text={inner.get('text')!r}")
        extra = {k: v for k, v in p.items() if k not in ("sessionId", "toolCallId", "content")}
        if extra:
            print(f"           extra params: {json.dumps(extra)}")
    elif m.get("method") == "session/update":
        u = (m.get("params") or {}).get("update", {})
        k = u.get("sessionUpdate")
        if k in ("tool_call", "tool_call_update"):
            content = u.get("content")
            ctext = ""
            if isinstance(content, list) and content:
                c0 = content[0]
                ctext = json.dumps(c0)[:160]
            print(f"  +{t:6.2f}s {k:16} status={u.get('status')} title={u.get('title','')!r} content[0]={ctext}")
print("\nTRACE:", TRACE, f"({len(frames)} frames)")
