#!/usr/bin/env python3
"""cyril-v19o probe: does the v2 engine (`kiro-cli acp`) advertise a `powers` command?

Decides whether cyril's `/powers` builtin must be KAS-conditional (as `/hooks`
is, via HooksCommandSource) or can register unconditionally. The registry skips
a builtin name only if cyril already holds it
(`register_agent_commands`, crates/cyril-core/src/commands/mod.rs:394), so an
agent-advertised `powers` is dropped whenever cyril registers its own.

Also reports whether v2 emits any `_kiro/powers/*` notification at all.
"""
import json, os, queue, re, signal, sqlite3, subprocess, tempfile, threading, time

OUT = os.environ.get("PROBE_OUT", os.path.dirname(os.path.abspath(__file__)))
LABEL = os.environ.get("LABEL", "2.21.2")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.environ.get("KIRO_XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
AUTH_DB = os.environ.get("KIRO_AUTH_DB", os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3"))
TAG = f"v2-commands-{LABEL}"
TRACE = os.path.join(OUT, f"{TAG}.jsonl")
VERDICT = os.path.join(OUT, f"{TAG}-verdict.json")
STDERR = os.path.join(OUT, f"{TAG}-stderr.log")
os.makedirs(OUT, exist_ok=True)
FAKE_HOME = tempfile.mkdtemp(prefix=f"{TAG}-home-")
CWD = tempfile.mkdtemp(prefix=f"{TAG}-cwd-")
RUNTIME = tempfile.mkdtemp(prefix=f"{TAG}-runtime-")
TMP = tempfile.mkdtemp(prefix=f"{TAG}-tmp-")

PROFILE_ARN = None
try:
    PROFILE_ARN = re.search(r"arn:aws:codewhisperer:\S+", subprocess.run(
        [KIRO, "user", "whoami"], capture_output=True, text=True, timeout=15).stdout).group(0)
except Exception:
    pass

SENSITIVE = {"accesstoken", "refreshtoken", "authorization", "clientsecret", "secret", "password", "profilearn", "expiriesat"}
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
    trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": scrub(obj)}, sort_keys=True) + "\n")

env = dict(os.environ)
env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME, "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP})
print(f"== v2 probe label={LABEL} launcher={KIRO}")
stderr = open(STDERR, "w")
# Bare `kiro-cli acp` is the v2 engine (cyril's default); no --agent-engine flag.
proc = subprocess.Popen([KIRO, "acp"], cwd=CWD, env=env,
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
    try:
        proc.stdin.write(json.dumps(obj) + "\n"); proc.stdin.flush()
    except BrokenPipeError:
        pass
def req(method, params=None):
    _id[0] += 1
    obj = {"jsonrpc": "2.0", "id": _id[0], "method": method}
    if params is not None:
        obj["params"] = params
    send(obj); return _id[0]

COMMANDS, POWERS_FRAMES = [], []
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
        method = obj.get("method", "")
        params = obj.get("params") or {}
        if "powers" in method:
            POWERS_FRAMES.append(method)
        if method.endswith("available_commands_update") or "availableCommands" in json.dumps(params):
            for c in (params.get("update") or {}).get("availableCommands") or []:
                COMMANDS.append(c)
        if method and "id" in obj and obj.get("id") != until_id:
            # v2 pushes server->client requests too (permissions); answer empties.
            send({"jsonrpc": "2.0", "id": obj["id"], "result": {}})
        elif obj.get("id") == until_id and "result" in obj:
            return obj
        elif obj.get("id") == until_id:
            return obj
    return None

verdict = {"label": LABEL, "launcher": KIRO, "engine": "v2", "commands": [], "powersMethods": []}
try:
    iid = req("initialize", {"protocolVersion": 1,
                             "clientInfo": {"name": "cyril-v19o-v2-command-probe", "version": "0.1.0"},
                             "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False},
                                                    "terminal": True}})
    init = pump(iid, 60) or {}
    nid = req("session/new", {"cwd": CWD, "mcpServers": []})
    new = pump(nid, 90) or {}
    pump(timeout=12, idle_exit=5)
    names = sorted({c.get("name") for c in COMMANDS if c.get("name")})
    verdict.update({
        "sessionIdPresent": bool((new.get("result") or {}).get("sessionId")),
        "commandCount": len(names),
        "commands": names,
        "powersAdvertised": "powers" in names,
        "powersMethods": sorted(set(POWERS_FRAMES)),
    })
    print("== v2 commands advertised:", len(names))
    print("== contains 'powers':", verdict["powersAdvertised"])
    print("== sample:", names[:25])
    print("== powers methods seen:", verdict["powersMethods"])
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
    json.dump(scrub(verdict), f, indent=2, sort_keys=True)
print("== verdict:", VERDICT)
