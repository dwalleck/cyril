#!/usr/bin/env python3
"""cyril-v19o empirical probe: `_kiro/powers/*` contract on the INSTALLED binary.

Derived from the committed audit probes
`experiments/conductor-spike/probe-kas-powers-2.{20.1,21.1}.py`: same direct
`kiro-cli acp --agent-engine kas` spawn cyril uses, same redaction and
process-group cleanup, plus the 2.20.1 script's SEED_POWERS leg so the
throwaway HOME carries the real (registry-tracked) powers tree and the pushed
list is non-empty.

Legs:
  1. initialize + session/new  -> capture the UNPROMPTED `items_changed` push
  2. `_kiro/powers/list`       -> compare against the push
  3. `_kiro/powers/refresh`    -> re-confirm the declared-but-unimplemented trap

Answers, for the installed binary only:
  * does the push fire at session creation, in cyril's spawn shape?
  * what is the exact item shape of a real installed power?
  * is `refresh` still `-32603`?
"""
import json, os, queue, re, shutil, signal, sqlite3, subprocess, tempfile, threading, time

OUT = os.environ.get("PROBE_OUT", os.path.dirname(os.path.abspath(__file__)))
LABEL = os.environ.get("LABEL", "2.21.2")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.environ.get("KIRO_XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
AUTH_DB = os.environ.get("KIRO_AUTH_DB", os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3"))
TAG = f"{LABEL}-2.21.2"
TRACE = os.path.join(OUT, f"kas-powers-{TAG}.jsonl")
VERDICT = os.path.join(OUT, f"kas-powers-{TAG}-verdict.json")
STDERR = os.path.join(OUT, f"kas-powers-{TAG}-stderr.log")
os.makedirs(OUT, exist_ok=True)
FAKE_HOME = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-home-")
CWD = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-cwd-")
RUNTIME = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-runtime-")
TMP = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-tmp-")

# Seed the throwaway HOME with the real powers tree. Powers are
# registry-tracked via installed.json — directory presence alone is ignored
# (docs/kiro-2.20.1-wire-audit.md §3), so the registry files must come too.
REAL_POWERS = os.path.expanduser("~/.kiro/powers")
SEEDED = False
if os.environ.get("SEED_POWERS") == "1" and os.path.isdir(REAL_POWERS):
    os.makedirs(os.path.join(FAKE_HOME, ".kiro"), exist_ok=True)
    shutil.copytree(REAL_POWERS, os.path.join(FAKE_HOME, ".kiro", "powers"), dirs_exist_ok=True)
    SEEDED = True

PROFILE_ARN = os.environ.get("KIRO_PROFILE_ARN")
if not PROFILE_ARN:
    try:
        PROFILE_ARN = re.search(r"arn:aws:codewhisperer:\S+", subprocess.run(
            [KIRO, "user", "whoami"], capture_output=True, text=True, timeout=15).stdout).group(0)
    except Exception:
        PROFILE_ARN = None

def read_token():
    try:
        c = sqlite3.connect(AUTH_DB)
        try:
            row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        finally:
            c.close()
        if not row:
            return None
        v = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
        d = json.loads(v)
        return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": PROFILE_ARN}
    except Exception:
        return None

SENSITIVE = {"accesstoken", "refreshtoken", "authorization", "clientsecret", "secret", "password", "profilearn", "expiresat"}
def scrub(x, key=""):
    kl = key.lower()
    if kl in SENSITIVE or "token" in kl or kl.endswith("credential"):
        return "<REDACTED>"
    if isinstance(x, dict):
        return {k: scrub(v, k) for k, v in x.items()}
    if isinstance(x, list):
        return [scrub(v, key) for v in x]
    return x

trace = open(TRACE, "w", buffering=1)
def record(direction, obj):
    if obj.get("method") == "_kiro/auth/getAccessToken":
        obj = dict(obj); obj["params"] = "<REDACTED>"
    trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": scrub(obj)}, sort_keys=True) + "\n")

env = dict(os.environ)
env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME, "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP})
print(f"== label={LABEL} launcher={KIRO} seeded={SEEDED}")
stderr = open(STDERR, "w")
proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr,
                        text=True, bufsize=1, start_new_session=True)
msgs = queue.Queue()
def reader():
    for line in proc.stdout:
        if line.strip():
            msgs.put(line.strip())
    msgs.put(None)
threading.Thread(target=reader, daemon=True).start()
_id = [10]
def send(obj):
    record("client->agent", obj); proc.stdin.write(json.dumps(obj) + "\n"); proc.stdin.flush()
def req(method, params=None):
    _id[0] += 1
    obj = {"jsonrpc": "2.0", "id": _id[0], "method": method}
    if params is not None:
        obj["params"] = params
    send(obj); return _id[0]

CHANGED, REQUESTS = [], {}
def callback(obj):
    method = obj.get("method", "")
    REQUESTS[method] = REQUESTS.get(method, 0) + 1
    if method == "_kiro/auth/getAccessToken":
        return read_token() or {}
    if method == "_kiro/terminal/shell_type":
        return {"shellType": "bash"}
    return {}

def pump(until_id=None, timeout=60, idle_exit=None):
    end = time.time() + timeout
    last = time.time()
    while time.time() < end:
        try:
            raw = msgs.get(timeout=1)
        except queue.Empty:
            if idle_exit and time.time() - last >= idle_exit:
                return None
            continue
        if raw is None:
            return None
        last = time.time()
        try:
            obj = json.loads(raw)
        except json.JSONDecodeError:
            continue
        record("agent->client", obj)
        method = obj.get("method")
        if method == "_kiro/powers/items_changed":
            CHANGED.append({"ts": obj.get("ts"), "params": obj.get("params")})
        if method and "id" in obj:
            send({"jsonrpc": "2.0", "id": obj["id"], "result": callback(obj)})
        elif obj.get("id") == until_id:
            return obj
    return None

def cleanup():
    if proc.poll() is None:
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGTERM); proc.wait(timeout=5)
        except Exception:
            try:
                os.killpg(os.getpgid(proc.pid), signal.SIGKILL); proc.wait(timeout=5)
            except Exception:
                pass
    trace.close(); stderr.close()

verdict = {"label": LABEL, "launcher": KIRO, "seeded": SEEDED}
try:
    iid = req("initialize", {"protocolVersion": 1,
                             "clientInfo": {"name": "cyril-v19o-powers-probe", "version": "0.1.0"},
                             "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False},
                                                    "terminal": True}})
    init = pump(iid, 90) or {}
    caps = ((init.get("result") or {}).get("agentCapabilities") or {})
    advertised = (((caps.get("_meta") or {}).get("kiro") or {}).get("extensionMethods") or [])
    nid = req("session/new", {"cwd": CWD, "mcpServers": []})
    new = pump(nid, 120) or {}
    sid = (new.get("result") or {}).get("sessionId")
    pump(timeout=8, idle_exit=4)
    list_id = req("_kiro/powers/list")
    listed = pump(list_id, 60) or {}
    refresh_id = req("_kiro/powers/refresh")
    refreshed = pump(refresh_id, 60) or {}
    pump(timeout=6, idle_exit=3)
    verdict.update({
        "advertisedExtensionMethods": advertised,
        "powersAdvertised": any("powers" in m for m in advertised),
        "sessionIdPresent": bool(sid),
        "itemsChanged": CHANGED,
        "list": listed.get("result") or listed.get("error"),
        "refresh": refreshed.get("result") or refreshed.get("error"),
        "serverRequests": REQUESTS,
    })
    print("== powers advertised:", verdict["powersAdvertised"])
    print("== items_changed frames:", len(CHANGED))
    print("== pushed powers:", len(((CHANGED[0] or {}).get("params") or {}).get("powers") or []) if CHANGED else 0)
    print("== list:", json.dumps(verdict["list"])[:400])
    print("== refresh:", json.dumps(verdict["refresh"])[:300])
finally:
    cleanup()
with open(VERDICT, "w") as f:
    json.dump(scrub(verdict), f, indent=2, sort_keys=True)
print("== trace:", TRACE)
print("== verdict:", VERDICT)
