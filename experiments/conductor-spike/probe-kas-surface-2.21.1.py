#!/usr/bin/env python3
"""Fresh paired KAS wire probe for the 2.21.1 audit.

Use KIRO_BIN=<versioned kiro-cli>, LABEL=2.21.0|2.21.1 and optionally
KIRO_KAS_SERVER_PATH=<versioned acp-server.js>.  The CLI launcher is always
used; direct node spawns are intentionally out of scope.  Wire captures redact
all auth callback values before writing them.
"""
import json, os, queue, re, signal, sqlite3, subprocess, tempfile, threading, time

OUTDIR = os.environ.get("PROBE_OUT", os.path.dirname(os.path.abspath(__file__)))
LABEL = os.environ.get("LABEL", "2.21.1")
KIND = os.environ.get("PROBE_KIND", "surface")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.environ.get("KIRO_XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
AUTH_DB = os.environ.get("KIRO_AUTH_DB", os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3"))
PIN = os.environ.get("KIRO_KAS_SERVER_PATH")
TRACE = os.path.join(OUTDIR, f"kas-{KIND}-{LABEL}-2.21.1.jsonl")
VERDICT = os.path.join(OUTDIR, f"kas-{KIND}-{LABEL}-2.21.1-verdict.json")
STDERR = os.path.join(OUTDIR, f"kas-{KIND}-{LABEL}-2.21.1-stderr.log")
os.makedirs(OUTDIR, exist_ok=True)

FAKE_HOME = tempfile.mkdtemp(prefix=f"kas-surface-{LABEL}-home-")
CWD = tempfile.mkdtemp(prefix=f"kas-surface-{LABEL}-cwd-")
RUNTIME = tempfile.mkdtemp(prefix=f"kas-surface-{LABEL}-runtime-")
TMP = tempfile.mkdtemp(prefix=f"kas-surface-{LABEL}-tmp-")
TARGET = os.path.join(CWD, "audit-created.txt")

# Never persist credentials or callback response values in the trace.
SENSITIVE_KEYS = {"accesstoken", "refreshtoken", "authorization", "clientsecret", "secret", "password", "profilearn", "expiresat"}
def scrub(x, key=""):
    kl = key.lower()
    if kl in SENSITIVE_KEYS or "token" in kl or kl.endswith("credential"):
        return "<REDACTED>"
    if isinstance(x, dict):
        return {k: scrub(v, k) for k, v in x.items()}
    if isinstance(x, list):
        return [scrub(v, key) for v in x]
    return x

def safe_msg(msg):
    # Auth callback params/results are entirely sensitive even if a future
    # server changes the field names.
    if msg.get("method") == "_kiro/auth/getAccessToken":
        msg = dict(msg); msg["params"] = "<REDACTED>"
    if msg.get("id") is not None and msg.get("result") and msg.get("id") == AUTH_REQUEST_ID[0]:
        msg = dict(msg); msg["result"] = "<REDACTED>"
    return scrub(msg)

trace = open(TRACE, "w", buffering=1)
AUTH_REQUEST_ID = [None]

def record(direction, msg):
    trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": safe_msg(msg)}, sort_keys=True) + "\n")

def profile_arn():
    explicit = os.environ.get("KIRO_PROFILE_ARN")
    if explicit:
        return explicit
    try:
        out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True, timeout=15).stdout
    except Exception:
        out = ""
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()
def read_token():
    try:
        c = sqlite3.connect(AUTH_DB)
        try:
            row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        finally:
            c.close()
        if not row:
            return None
        value = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
        d = json.loads(value)
        return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": PROFILE_ARN}
    except Exception as e:
        print("auth callback unavailable:", type(e).__name__)
        return None

env = dict(os.environ)
env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME, "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP})
if PIN:
    env["KIRO_KAS_SERVER_PATH"] = PIN
print(f"== label={LABEL} launcher={KIRO} kas_pin={PIN or '<launcher-resolved>'}")
print(f"== cwd={CWD} target={TARGET}")

stderr = open(STDERR, "w")
proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr,
                        text=True, bufsize=1, start_new_session=True)
assert proc.stdin and proc.stdout
msgs = queue.Queue()

def read_stdout():
    try:
        for line in proc.stdout:
            if line.strip():
                msgs.put(line.strip())
    finally:
        msgs.put(None)
threading.Thread(target=read_stdout, daemon=True).start()

next_id = [10]
def send(msg):
    record("client->agent", msg)
    proc.stdin.write(json.dumps(msg) + "\n")
    proc.stdin.flush()

def request(method, params=None):
    next_id[0] += 1
    msg = {"jsonrpc": "2.0", "id": next_id[0], "method": method}
    if params is not None:
        msg["params"] = params
    send(msg)
    return next_id[0]

SERVER_REQUESTS = {}
UPDATES = []
CALLBACKS = []

TERMS = {}
def callback_result(obj):
    method = obj.get("method", "")
    params = obj.get("params") or {}
    SERVER_REQUESTS[method] = SERVER_REQUESTS.get(method, 0) + 1
    CALLBACKS.append({"method": method, "param_keys": sorted(params.keys())})
    if method == "_kiro/auth/getAccessToken":
        AUTH_REQUEST_ID[0] = obj.get("id")
        return read_token() or {}
    if method == "_kiro/terminal/shell_type":
        return {"shellType": "bash"}
    path = params.get("path")
    if method in ("fs/read_text_file", "_kiro/fs/read_file") and path:
        try:
            with open(path, encoding="utf-8") as f:
                return {"content": f.read()}
        except OSError as e:
            return {"content": f"(err {e})"}
    if method in ("fs/write_text_file", "_kiro/fs/write_file") and path:
        try:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as f:
                f.write(params.get("content", params.get("text", "")))
            return {}
        except OSError as e:
            return {"error": str(e)}
    if method in ("fs/stat", "_kiro/fs/stat") and path:
        try:
            st = os.stat(path)
            return {"type": "directory" if os.path.isdir(path) else "file", "size": st.st_size}
        except OSError:
            return {}
    if method in ("fs/read_directory", "_kiro/fs/read_directory") and path:
        try:
            return {"entries": [{"name": n, "type": "directory" if os.path.isdir(os.path.join(path, n)) else "file"} for n in sorted(os.listdir(path))]}
        except OSError as e:
            return {"error": str(e)}
    if method in ("fs/delete", "_kiro/fs/delete") and path:
        try:
            os.unlink(path)
            return {}
        except OSError as e:
            return {"error": str(e)}
    if method == "terminal/create":
        cwd = params.get("cwd") or CWD
        if os.path.commonpath([os.path.abspath(CWD), os.path.abspath(cwd)]) != os.path.abspath(CWD):
            return {"terminalId": "term-rejected"}
        try:
            p = subprocess.Popen(["bash", "-lc", params.get("command", "")], cwd=cwd,
                                 stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            tid = f"term-{len(TERMS) + 1}"
            TERMS[tid] = p
            return {"terminalId": tid}
        except OSError:
            return {"terminalId": "term-rejected"}
    if method == "terminal/wait_for_exit":
        tid = params.get("terminalId")
        p = TERMS.get(tid)
        if not p:
            return {"exitCode": -1, "signal": None}
        try:
            p.wait(timeout=30)
        except subprocess.TimeoutExpired:
            p.kill(); p.wait()
        return {"exitCode": p.returncode if p.returncode >= 0 else None, "signal": -p.returncode if p.returncode < 0 else None}
    if method == "terminal/output":
        p = TERMS.get(params.get("terminalId"))
        if not p or p.stdout is None:
            return {"output": "", "truncated": False, "exitStatus": {"exitCode": -1, "signal": None}}
        output = p.stdout.read() if p.poll() is not None else ""
        return {"output": output, "truncated": False, "exitStatus": {"exitCode": p.returncode if p.returncode is not None and p.returncode >= 0 else None, "signal": -p.returncode if p.returncode is not None and p.returncode < 0 else None}}
    if method in ("terminal/release", "terminal/kill"):
        tid = params.get("terminalId")
        p = TERMS.pop(tid, None)
        if method == "terminal/kill" and p and p.poll() is None:
            p.kill()
        return {}
    if method == "session/request_permission":
        options = params.get("options") or []
        pick = next((o for o in options if o.get("kind") == "allow_once" or "allow" in str(o.get("optionId", "")).lower()), options[0] if options else None)
        return {"outcome": {"outcome": "selected", "optionId": pick.get("optionId")}} if pick else {"outcome": {"outcome": "cancelled"}}
    return {}

def pump(until_id=None, timeout=60, idle_exit=None):
    deadline = time.time() + timeout
    last = time.time()
    while time.time() < deadline:
        try:
            raw = msgs.get(timeout=1)
        except queue.Empty:
            if idle_exit is not None and time.time() - last >= idle_exit:
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
        if method and "id" in obj:
            result = callback_result(obj)
            send({"jsonrpc": "2.0", "id": obj["id"], "result": result})
        elif obj.get("id") == until_id:
            return obj
        if method == "session/update":
            u = (obj.get("params") or {}).get("update") or {}
            UPDATES.append({"sessionUpdate": u.get("sessionUpdate"), "kind": u.get("kind"), "status": u.get("status"), "title": u.get("title"), "toolCallId": u.get("toolCallId")})
    return None

def terminate_group():
    if proc.poll() is not None:
        return
    try:
        os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
        proc.wait(timeout=5)
    except Exception:
        try:
            os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
            proc.wait(timeout=5)
        except Exception:
            pass

TARGET_META = {"kiro": {"settings": {"unifiedAgent": {"enabled": True}, "unified_agent_enabled": True}, "featureConfig": {"unified_agent_enabled": True}, "features": {"unified_agent_enabled": True}}} if KIND == "unified" else None
result = {}
try:
    init_params = {"protocolVersion": 1,
        "clientInfo": {"name": "cyril-2.21.1-kas-audit", "version": "2.21.1"},
        "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}}
    if TARGET_META: init_params["_meta"] = TARGET_META
    iid = request("initialize", init_params)
    init = pump(iid, 90) or {}
    ir = init.get("result") or {}
    new_params = {"cwd": CWD, "mcpServers": []}
    if TARGET_META: new_params["_meta"] = TARGET_META
    nid = request("session/new", new_params)
    new = pump(nid, 120) or {}
    nr = new.get("result") or {}
    sid = nr.get("sessionId")
    pump(timeout=10, idle_exit=4)
    prompt = os.environ.get("PROMPT", f"In the workspace {CWD}, use your file tool to create {TARGET} containing exactly KAS_FILE_OK, then use your shell/terminal tool to run printf KAS_SHELL_OK. Read the file back and reply with exactly KAS_DONE.")
    t0 = time.time()
    pid = request("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt}]})
    terminal = pump(pid, 300) or {}
    elapsed = round(time.time() - t0, 2)
    pump(timeout=12, idle_exit=4)
    result = {"label": LABEL, "launcher": KIRO, "kasPin": PIN,
              "sessionIdPresent": bool(sid), "initialize": ir,
              "sessionNew": nr, "prompt": terminal.get("result") or terminal.get("error"),
              "elapsedSec": elapsed, "serverRequests": SERVER_REQUESTS,
              "callbacks": CALLBACKS, "updateSummary": UPDATES,
              "targetExists": os.path.exists(TARGET),
              "targetContent": open(TARGET, encoding="utf-8").read() if os.path.exists(TARGET) else None}
    print("== init agentInfo:", json.dumps(ir.get("agentInfo")))
    print("== session/new keys:", sorted(nr.keys()), "sid:", bool(sid))
    print("== prompt terminal:", json.dumps(result["prompt"])[:300], "elapsed:", elapsed)
    print("== callbacks:", json.dumps(SERVER_REQUESTS), "target:", result["targetExists"], repr(result["targetContent"]))
finally:
    terminate_group()
    trace.close(); stderr.close()

# Verdict intentionally retains full non-secret initialize/session payloads so
# static and wire surface comparisons can inspect fields/types/order separately.
with open(VERDICT, "w") as f:
    json.dump(scrub(result), f, indent=2, sort_keys=True)
print("== trace:", TRACE)
print("== verdict:", VERDICT)
