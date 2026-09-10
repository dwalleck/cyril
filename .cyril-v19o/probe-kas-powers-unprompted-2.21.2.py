#!/usr/bin/env python3
"""cyril-v19o premise P1b: is `_kiro/powers/items_changed` truly UNPROMPTED?

The main probe (probe-kas-powers-2.21.2.py) sent `_kiro/powers/list` 1ms after
the `session/new` response because its drain helper returned early, so its trace
cannot distinguish "pushed at session creation" from "sent in answer to list".
This probe removes the race: after `session/new` it issues NOTHING for 12s and
records what the agent pushes on its own.

Answers exactly one question: does the push arrive with no client request after
`session/new`?
"""
import json, os, queue, re, shutil, signal, sqlite3, subprocess, tempfile, threading, time

OUT = os.environ.get("PROBE_OUT", os.path.dirname(os.path.abspath(__file__)))
LABEL = os.environ.get("LABEL", "2.21.2")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.environ.get("KIRO_XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
AUTH_DB = os.environ.get("KIRO_AUTH_DB", os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3"))
IDLE_SECONDS = float(os.environ.get("IDLE_SECONDS", "12"))
TAG = f"unprompted-{LABEL}"
TRACE = os.path.join(OUT, f"kas-powers-{TAG}.jsonl")
VERDICT = os.path.join(OUT, f"kas-powers-{TAG}-verdict.json")
STDERR = os.path.join(OUT, f"kas-powers-{TAG}-stderr.log")
os.makedirs(OUT, exist_ok=True)
FAKE_HOME = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-home-")
CWD = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-cwd-")
RUNTIME = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-runtime-")
TMP = tempfile.mkdtemp(prefix=f"kas-powers-{TAG}-tmp-")

REAL_POWERS = os.path.expanduser("~/.kiro/powers")
SEEDED = False
if os.environ.get("SEED_POWERS") == "1" and os.path.isdir(REAL_POWERS):
    os.makedirs(os.path.join(FAKE_HOME, ".kiro"), exist_ok=True)
    shutil.copytree(REAL_POWERS, os.path.join(FAKE_HOME, ".kiro", "powers"), dirs_exist_ok=True)
    SEEDED = True

PROFILE_ARN = None
try:
    PROFILE_ARN = re.search(r"arn:aws:codewhisperer:\S+", subprocess.run(
        [KIRO, "user", "whoami"], capture_output=True, text=True, timeout=15).stdout).group(0)
except Exception:
    pass

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

trace = open(TRACE, "w", buffering=1)
def record(direction, obj):
    if obj.get("method") == "_kiro/auth/getAccessToken":
        obj = dict(obj); obj["params"] = "<REDACTED>"
    trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": obj}, sort_keys=True) + "\n")

env = dict(os.environ)
env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME, "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP})
print(f"== unprompted probe label={LABEL} seeded={SEEDED} idle={IDLE_SECONDS}s")
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
    record("client->agent", obj)
    proc.stdin.write(json.dumps(obj) + "\n"); proc.stdin.flush()
def req(method, params=None):
    _id[0] += 1
    obj = {"jsonrpc": "2.0", "id": _id[0], "method": method}
    if params is not None:
        obj["params"] = params
    send(obj); return _id[0]

PUSHES, REQUESTS, CLIENT_METHODS = [], {}, []
def tick(deadline, until_id=None):
    """Read frames until `deadline`. Returns the matching response if seen."""
    found = None
    while time.time() < deadline:
        try:
            raw = msgs.get(timeout=0.2)
        except queue.Empty:
            continue
        if raw is None:
            break
        obj = json.loads(raw)
        record("agent->client", obj)
        method = obj.get("method")
        if method:
            REQUESTS[method] = REQUESTS.get(method, 0) + 1
            if method == "_kiro/powers/items_changed":
                PUSHES.append({"ts": obj.get("ts"), "params": obj.get("params")})
        if method and "id" in obj:  # server->client request: answer it
            body = read_token() or {} if method == "_kiro/auth/getAccessToken" else (
                {"shellType": "bash"} if method == "_kiro/terminal/shell_type" else {})
            send({"jsonrpc": "2.0", "id": obj["id"], "result": body})
        elif until_id is not None and obj.get("id") == until_id and "result" in obj:
            found = obj
    return found

verdict = {"label": LABEL, "seeded": SEEDED, "idleSeconds": IDLE_SECONDS}
try:
    iid = req("initialize", {"protocolVersion": 1,
                             "clientInfo": {"name": "cyril-v19o-unprompted-probe", "version": "0.1.0"},
                             "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False},
                                                    "terminal": True}})
    new_sent_at = None
    tick(time.time() + 90, until_id=iid)
    nid = req("session/new", {"cwd": CWD, "mcpServers": []})
    new_sent_at = time.time()
    tick(time.time() + 120, until_id=nid)
    new_replied_at = time.time()
    # THE POINT: issue nothing at all, and watch.
    tick(time.time() + IDLE_SECONDS)
    after_idle = time.time()
    verdict.update({
        "sessionNewReplied": new_replied_at is not None,
        "secondsAfterSessionNewReply": round(after_idle - new_replied_at, 3),
        "clientMethodsAfterSessionNew": [m for m in CLIENT_METHODS],
        "itemsChanged": PUSHES,
        "itemCount": len(((PUSHES[0] or {}).get("params") or {}).get("powers") or []) if PUSHES else 0,
        "agentMethods": REQUESTS,
    })
    print("== items_changed pushes while idle:", len(PUSHES))
    print("== powers in first push:", verdict["itemCount"])
finally:
    if proc.poll() is None:
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGTERM); proc.wait(timeout=5)
        except Exception:
            try:
                os.killpg(os.getpgid(proc.pid), signal.SIGKILL); proc.wait(timeout=5)
            except Exception:
                pass
    trace.close(); stderr.close()
with open(VERDICT, "w") as f:
    json.dump(verdict, f, indent=2, sort_keys=True)
print("== trace:", TRACE)
print("== verdict:", VERDICT)
