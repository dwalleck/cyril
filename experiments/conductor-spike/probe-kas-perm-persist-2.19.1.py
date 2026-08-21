#!/usr/bin/env python3
"""
KAS 0.48.0 (kiro-cli 2.19.1) permission-persistability probe.

A/B in one session (autopilot -> supervised so approvals fire):
  turn 1: benign `echo hello`            -> expect options incl. allow_always
  turn 2: unparseable-ish shell command  -> expect allow_always/reject_always
          DROPPED + _meta.kiro.consent {persistableConsent:false,
          persistableConsentReason}
Records every request_permission frame's options[] + consent meta.
Isolation: HOME=<tmp>, real XDG_DATA_HOME.
"""
import json, os, re, subprocess, threading, queue, time, tempfile, sqlite3, signal

SCRATCH = os.path.dirname(os.path.abspath(__file__))
TRACE = os.path.join(SCRATCH, "kas-perm-persist-2.19.1-trace.jsonl")
KIRO = os.path.expanduser("~/.local/bin/kiro-cli")
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

def profile_arn():
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()
FAKE_HOME = tempfile.mkdtemp(prefix="perm-home-")
CWD = tempfile.mkdtemp(prefix="perm-cwd-")
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
                        stderr=open(os.path.join(SCRATCH, "perm-stderr.log"), "w"),
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

PERMS = []
def handle_server_req(o):
    m = o["method"]
    if m == "_kiro/auth/getAccessToken":
        send({"jsonrpc": "2.0", "id": o["id"], "result": read_token() or {}})
    elif m == "session/request_permission":
        p = o.get("params", {})
        PERMS.append(p)
        opts = p.get("options", [])
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
                         "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}}})
pump(iid, 60)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
new = pump(nid, 60)
sid = ((new or {}).get("result") or {}).get("sessionId")
cfg = ((new or {}).get("result") or {}).get("configOptions") or []
auto = next((c for c in cfg if (c.get("id") or c.get("configId")) == "autopilot"), {})
vals = [o.get("value") for o in (auto.get("options") or [])]
print("autopilot options:", vals, "current:", auto.get("currentValue"))
target = next((v for v in vals if v and v != "on"), "off")
sres = pump(req("session/set_config_option", {"sessionId": sid, "configId": "autopilot", "value": target}), 30)
print("set autopilot ->", target, "| result configOptions:",
      [(c.get("id"), c.get("currentValue")) for c in ((sres or {}).get("result") or {}).get("configOptions") or []][:4])

ARMS = [
    ("benign", "Use the execute_bash tool to run exactly this command: echo hello"),
    ("unparseable", "Use the execute_bash tool to run EXACTLY this command, verbatim, without simplifying it: "
                    "for f in $(ls | head -2); do eval \"echo ${f}\"; done"),
]
for label, prompt in ARMS:
    n_before = len(PERMS)
    pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]})
    resp = pump(pid, 240)
    print(f"\n== [{label}] response:", json.dumps((resp or {}).get("result") or (resp or {}).get("error"))[:200])
    for p in PERMS[n_before:]:
        tc = p.get("toolCall") or {}
        consent = ((p.get("_meta") or {}).get("kiro") or {}).get("consent") or {}
        print(f"   perm: title={tc.get('title','')!r}")
        print(f"   options: {[(o.get('optionId'), o.get('kind')) for o in p.get('options', [])]}")
        print(f"   consent: {json.dumps(consent)[:400]}")

trace.close()
try:
    os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
except Exception:
    proc.terminate()
print("\nTRACE:", TRACE)
