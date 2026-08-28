#!/usr/bin/env python3
"""
KAS 0.54.3 (kiro-cli 2.20.1) stream-idle watchdog probe.

Static recon (see docs/kiro-2.20.1-wire-audit.md) recovered the whole contract
from the now-MINIFIED bundle via surviving string literals:

    var Wwc=6e4, Gwc=3e5,
        jwc="KIRO_STREAM_IDLE_WARN_MS", Vwc="KIRO_STREAM_IDLE_TIMEOUT_MS";
    function Z7i(e=process.env){return{warnMs:Wj(e[jwc],Wwc),timeoutMs:Wj(e[Vwc],Gwc)}}

  * feature flag STREAM_IDLE_WATCHDOG = "stream_idle_watchdog", default TRUE
  * warn  60_000 ms -> onStall(idleMs)
  * hard 300_000 ms -> makeTimeoutError -> throw P$(idleMs)
  * streaming path only: first stall per turn fires
        onRecoverySignal("The model response paused unexpectedly. Waiting for
                          it to resume…", "warning")
    and the handler is
        (msg, level) => connection.extNotification("_kiro/system/notify",
                                                   {level, message: msg})

Both env vars let us collapse 60s/300s into seconds, so the watchdog becomes
DETERMINISTICALLY testable instead of waiting on a rare backend stall window
(the cyril-bh7g problem: the stall is a window, not a rate).

Legs (argv[1]):
  soft    0.54.3, warn=250ms  timeout=600s  -> expect _kiro/system/notify warning
  hard    0.54.3, warn=250ms  timeout=4s    -> expect timeout + auto-recovery
  control 0.52.1, warn=250ms  timeout=4s    -> expect NEITHER (code absent)

Auth: real XDG_DATA_HOME token store; HOME is a throwaway tmpdir.
"""
import json, os, re, subprocess, threading, queue, time, tempfile, sqlite3, sys

LEG = sys.argv[1] if len(sys.argv) > 1 else "soft"
OUTDIR = os.environ.get("PROBE_OUT", ".")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
KASROOT = os.path.expanduser("~/.local/share/kiro-cli/kas")

def bundle(prefix):
    import glob
    hits = sorted(h for h in glob.glob(os.path.join(KASROOT, prefix + "-*"))
                  if os.path.isdir(h) and not h.endswith(".lock"))
    if not hits:
        raise SystemExit(f"no KAS bundle for {prefix}")
    return os.path.join(hits[-1], "node_modules/@kiro/agent/dist/server/acp-server.js")

CFG = {
    "soft":    dict(warn="250", timeout="600000", pin=None),
    "hard":    dict(warn="250", timeout="4000",   pin=None),
    "control": dict(warn="250", timeout="4000",   pin="2.19.2"),
    # cyril-34yq: does the env provider gate the watchdog, and are the declared
    # `client` / `session` providers actually wired?
    "envoff":     dict(warn="100", timeout="300", pin=None, envflag="false"),
    "envon":      dict(warn="100", timeout="300", pin=None, envflag="true"),
    "clientmeta": dict(warn="100", timeout="300", pin=None, meta=True),
}[LEG]
# allow per-run override so the hard path can be tuned onto the real gap profile
CFG["warn"] = os.environ.get("WD_WARN", CFG["warn"])
CFG["timeout"] = os.environ.get("WD_TIMEOUT", CFG["timeout"])

TRACE = os.path.join(OUTDIR, f"kas-watchdog-{LEG}-2.20.1.jsonl")

def profile_arn():
    arn = os.environ.get("KIRO_PROFILE_ARN")
    if arn:
        return arn
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()
FAKE_HOME = tempfile.mkdtemp(prefix="kas-wd-home-")
CWD = tempfile.mkdtemp(prefix="kas-wd-cwd-")
env = dict(os.environ)
env["HOME"] = FAKE_HOME
env["XDG_DATA_HOME"] = os.path.expanduser("~/.local/share")
env["KIRO_STREAM_IDLE_WARN_MS"] = CFG["warn"]
env["KIRO_STREAM_IDLE_TIMEOUT_MS"] = CFG["timeout"]
if CFG["pin"]:
    env["KIRO_KAS_SERVER_PATH"] = bundle(CFG["pin"])
if CFG.get("envflag") is not None:
    env["KIRO_FEATURE_STREAM_IDLE_WATCHDOG_ENABLED"] = CFG["envflag"]
USE_META = CFG.get("meta", False)

print(f"== leg={LEG} warn={CFG['warn']}ms timeout={CFG['timeout']}ms "
      f"pin={CFG['pin'] or 'live 0.54.3'} envflag={CFG.get('envflag')} meta={CFG.get('meta', False)}")

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
                        stderr=open(os.path.join(OUTDIR, f"kas-watchdog-{LEG}-2.20.1-stderr.log"), "w"),
                        text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN_, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
trace = open(TRACE, "w")

def record(direction, obj):
    trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": obj}) + "\n")
    trace.flush()

threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [10]

def send(obj):
    record("client->agent", obj)
    PIN_.write(json.dumps(obj) + "\n"); PIN_.flush()

def req(m, p):
    _id[0] += 1
    send({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p})
    return _id[0]

NOTIFY = []
def handle_server_req(o):
    m = o["method"]
    if m == "_kiro/auth/getAccessToken":
        send({"jsonrpc": "2.0", "id": o["id"], "result": read_token() or {}})
    elif m == "session/request_permission":
        opts = o.get("params", {}).get("options", [])
        pick = next((x for x in opts if "allow" in (str(x.get("kind", "")) + str(x.get("optionId", ""))).lower()), opts[0] if opts else None)
        res = {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
        send({"jsonrpc": "2.0", "id": o["id"], "result": res})
    elif m == "_kiro/terminal/shell_type":
        send({"jsonrpc": "2.0", "id": o["id"], "result": {"shellType": "bash"}})
    else:
        send({"jsonrpc": "2.0", "id": o["id"], "result": {}})

T0 = [0.0]
def pump(until_id=None, timeout=40, idle_exit=None):
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
        meth = o.get("method")
        if meth == "_kiro/system/notify":
            dt = time.time() - T0[0] if T0[0] else 0
            NOTIFY.append((dt, o.get("params")))
            print(f"   >>> _kiro/system/notify @+{dt:.1f}s {json.dumps(o.get('params'))[:220]}")
        if meth and "id" in o:
            handle_server_req(o)
        elif "id" in o and until_id is not None and o["id"] == until_id:
            return o
    return None

# Every plausible client-side shape for the flag, tried at once. The declared
# provider precedence is [governance, env, client, session, experiment]; if a
# `client` provider existed, one of these should reach it.
FLAG_META = {"kiro": {"settings": {"streamIdleWatchdog": {"enabled": False},
                                   "stream_idle_watchdog": False,
                                   "featureConfig": {"stream_idle_watchdog": False}},
                      "featureConfig": {"stream_idle_watchdog": False},
                      "features": {"stream_idle_watchdog": False}}}
init_params = {"protocolVersion": 1,
               "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"},
               "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}}}
if USE_META:
    init_params["_meta"] = FLAG_META
iid = req("initialize", init_params)
init = pump(iid, 60)
ai = ((init or {}).get("result") or {}).get("agentInfo")
print("== agentInfo:", json.dumps(ai))

new_params = {"cwd": CWD, "mcpServers": []}
if USE_META:
    new_params["_meta"] = FLAG_META
nid = req("session/new", new_params)
new = pump(nid, 60)
sid = ((new or {}).get("result") or {}).get("sessionId")
print("== sessionId:", (sid or "")[:16])
pump(timeout=6, idle_exit=3)

PROMPT = ("Count from 1 to 15. Put each number on its own line and add a "
          "short sentence about that number. Do not use any tools.")
T0[0] = time.time()
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
resp = pump(pid, 300)
elapsed = time.time() - T0[0]
r = (resp or {}).get("result")
e = (resp or {}).get("error")
print(f"== prompt terminal after {elapsed:.1f}s: result={json.dumps(r)[:300]} error={json.dumps(e)[:300]}")
pump(timeout=10, idle_exit=5)

print(f"== NOTIFY count={len(NOTIFY)}")
for dt, p in NOTIFY:
    print(f"   +{dt:.1f}s {json.dumps(p)}")

summary = {"leg": LEG, "warnMs": CFG["warn"], "timeoutMs": CFG["timeout"],
           "pin": CFG["pin"] or "0.54.3-live", "elapsed": round(elapsed, 2),
           "terminal_result": r, "terminal_error": e,
           "notify": [{"atSec": round(dt, 2), "params": p} for dt, p in NOTIFY]}
with open(os.path.join(OUTDIR, f"kas-watchdog-{LEG}-2.20.1-verdict.json"), "w") as fh:
    json.dump(summary, fh, indent=2)

trace.close()
proc.terminate()
