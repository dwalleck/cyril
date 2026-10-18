#!/usr/bin/env python3
"""Minimal ACP initialize-only probe (audit 2.32.0 live lane (copy of probe-acp-init-2.31.1.py), leg 0).

  AUTH=callback|none KAS=<acp-server.js> LABEL=<tag> PROBE_SCRATCH=<dir> \
      python3 -I probe-acp-init-2.29.1.py <out.jsonl>

Spawns the pinned KAS directly, sends one `initialize` with the FULL handshake
path `clientCapabilities._meta.kiro.settings.*` (trap 5), records stderr, answer
records and the raw initialize result/error, then closes stdin. No prompts, so it
is auth-shape-only.
"""
import json, os, queue, shlex, signal, sqlite3, subprocess, sys, tempfile, threading, time
from datetime import datetime

KAS = os.path.abspath(os.environ["KAS"])
LABEL = os.environ.get("LABEL", "init")
AUTH = os.environ.get("AUTH", "callback")          # callback | none
OUT_PATH = sys.argv[1]
NODE = os.environ.get("KAS_NODE", os.path.expanduser("~/.local/share/kiro-cli/node"))
REAL_HOME = os.path.expanduser("~")
DATA_HOME = os.path.join(REAL_HOME, ".local", "share")
AUTH_DB = os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3")
SCRATCH = os.environ.get("PROBE_SCRATCH") or tempfile.gettempdir()
FAKE_HOME = tempfile.mkdtemp(prefix=f"acpinit-{LABEL}-home-", dir=SCRATCH)
WS = tempfile.mkdtemp(prefix=f"acpinit-{LABEL}-ws-", dir=SCRATCH)
TMPD = tempfile.mkdtemp(prefix=f"acpinit-{LABEL}-tmp-", dir=SCRATCH)
OUT = open(OUT_PATH, "w", buffering=1)
LOCK = threading.Lock()
REDACT = ("accesstoken", "refreshtoken", "idtoken", "profilearn", "authorization", "expiresat")
def scrub(o, k=""):
    if str(k).lower() in REDACT: return "<REDACTED>"
    if isinstance(o, dict): return {kk: scrub(v, kk) for kk, v in o.items()}
    if isinstance(o, list): return [scrub(v, k) for v in o]
    return o
def emit(r):
    with LOCK: OUT.write(json.dumps(scrub(r)) + "\n")
def _db(): return sqlite3.connect(f"file:{AUTH_DB}?mode=ro", uri=True, timeout=10)
def read_token():
    c = _db()
    try: row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
    finally: c.close()
    if not row: return None
    return json.loads(row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0])
def profile_arn():
    c = _db()
    try: row = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally: c.close()
    return json.loads(row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]).get("arn") if row else None

argv = [NODE, "--experimental-wasm-modules", KAS, "--transport=stdio"]
if AUTH == "callback": argv.append("--auth=acp-callback")
env = dict(os.environ, HOME=FAKE_HOME, XDG_DATA_HOME=DATA_HOME, TMPDIR=TMPD)
env.pop("KIRO_KAS_SERVER_PATH", None)
stderr_path = OUT_PATH.replace(".jsonl", "-stderr.log")
stderr = open(stderr_path, "w")
proc = subprocess.Popen(argv, cwd=WS, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=stderr, text=True, bufsize=1, start_new_session=True)
emit({"probe": "proc", "pid": proc.pid, "argv": argv, "auth_mode": AUTH,
      "exe": os.readlink(f"/proc/{proc.pid}/exe"),
      "kas_sha256": __import__("hashlib").sha256(open(KAS, "rb").read()).hexdigest()})
responses, notes, callbacks = {}, [], []
def answer(o):
    m = o.get("method"); p = o.get("params") or {}
    callbacks.append(m)
    if m == "_kiro/auth/getAccessToken":
        t = read_token()
        res = {"accessToken": t["access_token"], "expiresAt": t["expires_at"], "profileArn": profile_arn()} if t else {}
    elif m == "_kiro/terminal/shell_type": res = {"shellType": "bash"}
    else: res = {}
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": res}) + "\n"); proc.stdin.flush()
def reader():
    for line in proc.stdout:
        line = line.strip()
        if not line: continue
        try: o = json.loads(line)
        except Exception: emit({"dir": "raw", "line": line[:2000]}); continue
        emit({"ts": round(time.time(), 3), "dir": "agent->client", "msg": o})
        if o.get("method") and "id" in o:
            threading.Thread(target=answer, args=(o,), daemon=True).start()
        elif o.get("method"):
            notes.append((o["method"], o.get("params")))
        elif "id" in o:
            responses[o["id"]] = o
threading.Thread(target=reader, daemon=True).start()

SETTINGS = {"codeIntelligence": {"enabled": True}, "knowledge": {"enabled": True},
            "thinking": {"enabled": True}, "largeToolOutputHandler": {"enabled": True},
            "memory": {"mode": "read_only", "reflection": False}}
params = {"protocolVersion": 1,
          "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True},
                                 "terminal": True,
                                 "_meta": {"kiro": {"configurationState": True,
                                                     "settings": SETTINGS,
                                                     "telemetryEnabled": False}}},
          "clientInfo": {"name": "cyril-probe", "version": "audit-2.32.0"}}
proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": params}) + "\n"); proc.stdin.flush()
emit({"ts": round(time.time(), 3), "dir": "client->agent", "msg": {"id": 1, "method": "initialize", "params": params}})
end = time.time() + 90
while 1 not in responses and time.time() < end and proc.poll() is None:
    time.sleep(0.5)
time.sleep(3)
r = responses.get(1)
emit({"probe": "init_result", "auth_mode": AUTH,
      "result": r.get("result") if r else None, "error": r.get("error") if r else None,
      "notes_seen": [m for m, _ in notes], "callbacks": callbacks})
try: proc.stdin.close()
except Exception: pass
try:
    os.killpg(os.getpgid(proc.pid), signal.SIGTERM); proc.wait(timeout=10)
except Exception:
    try: os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
    except Exception: pass
stderr.close()
emit({"probe": "stderr", "lines": open(stderr_path).read().splitlines()})
