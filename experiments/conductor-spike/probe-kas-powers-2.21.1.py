#!/usr/bin/env python3
"""Paired launcher-path KAS powers contract probe for 2.21.1.

LABEL/KIRO_BIN select 2.21.0 or 2.21.1; KIRO_KAS_SERVER_PATH can pin the
bundle.  Fake HOME is intentionally empty, then receives a harmless local
fixture. Auth values are redacted before persistence and process groups are
reaped on every exit.
"""
import json, os, queue, re, signal, sqlite3, subprocess, tempfile, threading, time

OUT = os.environ.get("PROBE_OUT", os.path.dirname(os.path.abspath(__file__)))
LABEL = os.environ.get("LABEL", "2.21.1")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.environ.get("KIRO_XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
AUTH_DB = os.environ.get("KIRO_AUTH_DB", os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3"))
PIN = os.environ.get("KIRO_KAS_SERVER_PATH")
TAG = f"{LABEL}-2.21.1"
TRACE = os.path.join(OUT, f"kas-powers-{TAG}.jsonl")
VERDICT = os.path.join(OUT, f"kas-powers-{TAG}-verdict.json")
STDERR = os.path.join(OUT, f"kas-powers-{TAG}-stderr.log")
os.makedirs(OUT, exist_ok=True)
FAKE_HOME = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-home-")
CWD = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-cwd-")
RUNTIME = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-runtime-")
TMP = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-tmp-")

PROFILE_ARN = os.environ.get("KIRO_PROFILE_ARN")
if not PROFILE_ARN:
    try: PROFILE_ARN = re.search(r"arn:aws:codewhisperer:\S+", subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True, timeout=15).stdout).group(0)
    except Exception: PROFILE_ARN = None

def read_token():
    try:
        c = sqlite3.connect(AUTH_DB)
        try: row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        finally: c.close()
        if not row: return None
        v = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]; d = json.loads(v)
        return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": PROFILE_ARN}
    except Exception: return None

SENSITIVE = {"accesstoken", "refreshtoken", "authorization", "clientsecret", "secret", "password", "profilearn", "expiresat"}
def scrub(x, key=""):
    kl = key.lower()
    if kl in SENSITIVE or "token" in kl or kl.endswith("credential"): return "<REDACTED>"
    if isinstance(x, dict): return {k: scrub(v, k) for k, v in x.items()}
    if isinstance(x, list): return [scrub(v, key) for v in x]
    return x

trace = open(TRACE, "w", buffering=1)
def record(direction, obj):
    if obj.get("method") == "_kiro/auth/getAccessToken": obj = dict(obj); obj["params"] = "<REDACTED>"
    trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": scrub(obj)}, sort_keys=True) + "\n")

env = dict(os.environ); env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME, "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP})
if PIN: env["KIRO_KAS_SERVER_PATH"] = PIN
print(f"== label={LABEL} launcher={KIRO} kas_pin={PIN or '<launcher-resolved>'}")
stderr = open(STDERR, "w")
proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr,
                        text=True, bufsize=1, start_new_session=True)
msgs = queue.Queue()
def reader():
    for line in proc.stdout:
        if line.strip(): msgs.put(line.strip())
    msgs.put(None)
threading.Thread(target=reader, daemon=True).start()
_id = [10]
def send(obj): record("client->agent", obj); proc.stdin.write(json.dumps(obj) + "\n"); proc.stdin.flush()
def req(method, params=None):
    _id[0] += 1; obj = {"jsonrpc": "2.0", "id": _id[0], "method": method}
    if params is not None: obj["params"] = params
    send(obj); return _id[0]

CHANGED, REQUESTS = [], {}
def callback(obj):
    method = obj.get("method", ""); p = obj.get("params") or {}; REQUESTS[method] = REQUESTS.get(method, 0) + 1
    if method == "_kiro/auth/getAccessToken": return read_token() or {}
    if method == "_kiro/terminal/shell_type": return {"shellType": "bash"}
    return {}

def pump(until_id=None, timeout=60, idle_exit=None):
    end = time.time() + timeout; last = time.time()
    while time.time() < end:
        try: raw = msgs.get(timeout=1)
        except queue.Empty:
            if idle_exit and time.time() - last >= idle_exit: return None
            continue
        if raw is None: return None
        last = time.time()
        try: obj = json.loads(raw)
        except json.JSONDecodeError: continue
        record("agent->client", obj)
        method = obj.get("method")
        if method == "_kiro/powers/items_changed": CHANGED.append(obj.get("params"))
        if method and "id" in obj: send({"jsonrpc": "2.0", "id": obj["id"], "result": callback(obj)})
        elif obj.get("id") == until_id: return obj
    return None

def cleanup():
    if proc.poll() is None:
        try: os.killpg(os.getpgid(proc.pid), signal.SIGTERM); proc.wait(timeout=5)
        except Exception:
            try: os.killpg(os.getpgid(proc.pid), signal.SIGKILL); proc.wait(timeout=5)
            except Exception: pass
    trace.close(); stderr.close()

verdict = {"label": LABEL, "launcher": KIRO, "kasPin": PIN}
try:
    iid = req("initialize", {"protocolVersion": 1, "clientInfo": {"name": "cyril-2.21.1-powers-audit", "version": "2.21.1"}, "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}, "terminal": True}})
    init = pump(iid, 90) or {}
    nid = req("session/new", {"cwd": CWD, "mcpServers": []})
    new = pump(nid, 120) or {}; sid = (new.get("result") or {}).get("sessionId")
    pump(timeout=8, idle_exit=4)
    empty_id = req("_kiro/powers/list"); empty = pump(empty_id, 60) or {}
    pdir = os.path.join(FAKE_HOME, ".kiro", "powers", "installed", "audit-fixture")
    os.makedirs(pdir, exist_ok=True)
    with open(os.path.join(pdir, "mcp.json"), "w") as f: json.dump({"mcpServers": {"audit-fixture": {"command": "true", "args": [], "disabled": False}}}, f)
    refresh_id = req("_kiro/powers/refresh"); refresh = pump(refresh_id, 60) or {}
    pump(timeout=8, idle_exit=4)
    after_id = req("_kiro/powers/list"); after = pump(after_id, 60) or {}
    verdict.update({"initialize": init.get("result") or init.get("error"), "sessionNew": new.get("result") or new.get("error"), "sessionIdPresent": bool(sid),
                    "listEmptyHome": empty.get("result") or empty.get("error"), "refresh": refresh.get("result") or refresh.get("error"),
                    "listAfterFixture": after.get("result") or after.get("error"), "itemsChanged": CHANGED, "serverRequests": REQUESTS})
    print("== empty list:", json.dumps(verdict["listEmptyHome"])[:500]); print("== refresh:", json.dumps(verdict["refresh"])[:300]); print("== after list:", json.dumps(verdict["listAfterFixture"])[:500]); print("== items_changed:", len(CHANGED))
finally:
    cleanup()
with open(VERDICT, "w") as f: json.dump(scrub(verdict), f, indent=2, sort_keys=True)
print("== trace:", TRACE); print("== verdict:", VERDICT)
