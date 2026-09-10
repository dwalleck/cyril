#!/usr/bin/env python3
"""Controlled KAS stream-idle watchdog probe for 2.21.1, launcher path only.

LABEL identifies the host (2.21.0/2.21.1); KIRO_BIN selects that host and
KIRO_KAS_SERVER_PATH optionally pins a bundle.  The default 100/300 ms window
is deliberately short so the controlled error path is bounded.  Captures
redact auth values and cleanup kills the complete launcher process group.
"""
import json, os, queue, re, signal, sqlite3, subprocess, tempfile, threading, time

OUT = os.environ.get("PROBE_OUT", os.path.dirname(os.path.abspath(__file__)))
LABEL = os.environ.get("LABEL", "2.21.1")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.environ.get("KIRO_XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
AUTH_DB = os.environ.get("KIRO_AUTH_DB", os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3"))
PIN = os.environ.get("KIRO_KAS_SERVER_PATH")
WARN = os.environ.get("WD_WARN_MS", "100")
TIMEOUT = os.environ.get("WD_TIMEOUT_MS", "300")
TAG = f"{LABEL}-{os.environ.get('LEG', 'hard')}-2.21.1"
TRACE = os.path.join(OUT, f"kas-watchdog-{TAG}.jsonl")
VERDICT = os.path.join(OUT, f"kas-watchdog-{TAG}-verdict.json")
STDERR = os.path.join(OUT, f"kas-watchdog-{TAG}-stderr.log")
os.makedirs(OUT, exist_ok=True)
FAKE_HOME = tempfile.mkdtemp(prefix=f"kas-wd-{TAG}-home-")
CWD = tempfile.mkdtemp(prefix=f"kas-wd-{TAG}-cwd-")
RUNTIME = tempfile.mkdtemp(prefix=f"kas-wd-{TAG}-runtime-")
TMP = tempfile.mkdtemp(prefix=f"kas-wd-{TAG}-tmp-")

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

env = dict(os.environ)
env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME, "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP,
            "KIRO_STREAM_IDLE_WARN_MS": WARN, "KIRO_STREAM_IDLE_TIMEOUT_MS": TIMEOUT})
if PIN: env["KIRO_KAS_SERVER_PATH"] = PIN
print(f"== label={LABEL} launcher={KIRO} kas_pin={PIN or '<launcher-resolved>'} warn={WARN} timeout={TIMEOUT}")
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
def req(method, params):
    _id[0] += 1; send({"jsonrpc": "2.0", "id": _id[0], "method": method, "params": params}); return _id[0]

NOTIFY, REQUESTS = [], {}
def callback(obj):
    method = obj.get("method", ""); p = obj.get("params") or {}; REQUESTS[method] = REQUESTS.get(method, 0) + 1
    if method == "_kiro/auth/getAccessToken": return read_token() or {}
    if method == "_kiro/terminal/shell_type": return {"shellType": "bash"}
    if method == "session/request_permission":
        opts = p.get("options") or []; pick = next((x for x in opts if "allow" in (str(x.get("kind", "")) + str(x.get("optionId", "")).lower())), opts[0] if opts else None)
        return {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
    return {}

def pump(until_id=None, timeout=90, t0=0):
    end = time.time() + timeout; terminal = None
    while time.time() < end:
        try: raw = msgs.get(timeout=1)
        except queue.Empty: continue
        if raw is None: break
        try: obj = json.loads(raw)
        except json.JSONDecodeError: continue
        record("agent->client", obj)
        method = obj.get("method")
        if method == "_kiro/system/notify":
            NOTIFY.append({"atSec": round(time.time() - t0, 3) if t0 else 0, "params": obj.get("params")})
        if method and "id" in obj: send({"jsonrpc": "2.0", "id": obj["id"], "result": callback(obj)})
        elif obj.get("id") == until_id:
            terminal = obj; break
    return terminal

def cleanup():
    if proc.poll() is None:
        try: os.killpg(os.getpgid(proc.pid), signal.SIGTERM); proc.wait(timeout=5)
        except Exception:
            try: os.killpg(os.getpgid(proc.pid), signal.SIGKILL); proc.wait(timeout=5)
            except Exception: pass
    trace.close(); stderr.close()

verdict = {"label": LABEL, "launcher": KIRO, "kasPin": PIN, "warnMs": WARN, "timeoutMs": TIMEOUT}
try:
    iid = req("initialize", {"protocolVersion": 1, "clientInfo": {"name": "cyril-2.21.1-watchdog-audit", "version": "2.21.1"}, "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}, "terminal": True}})
    init = pump(iid, 90)
    nid = req("session/new", {"cwd": CWD, "mcpServers": []})
    new = pump(nid, 120)
    sid = (new or {}).get("result", {}).get("sessionId") if new else None
    prompt = ("Count from 1 to 15. Put each number on its own line and add a short sentence about that number. "
              "Do not use any tools. Continue generating enough content that a stream-idle watchdog can observe an idle model response.")
    t0 = time.time(); pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]})
    terminal = pump(pid, float(os.environ.get("DRIVER_TIMEOUT_SEC", "90")), t0)
    elapsed = round(time.time() - t0, 3)
    verdict.update({"initialize": (init or {}).get("result") or (init or {}).get("error"), "sessionNew": (new or {}).get("result") or (new or {}).get("error"),
                    "terminal": (terminal or {}).get("result") if terminal else None, "terminalError": (terminal or {}).get("error") if terminal else None,
                    "elapsedSec": elapsed, "notify": NOTIFY, "serverRequests": REQUESTS})
    print("== terminal:", json.dumps(verdict["terminal"] or verdict["terminalError"])[:500], "elapsed:", elapsed)
    print("== notify:", json.dumps(NOTIFY))
finally:
    cleanup()
with open(VERDICT, "w") as f: json.dump(scrub(verdict), f, indent=2, sort_keys=True)
print("== trace:", TRACE); print("== verdict:", VERDICT)
