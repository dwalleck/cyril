#!/usr/bin/env python3
"""
Knock out three KAS follow-ups in one session:

1. HOOKS DIRECTION — `_kiro/hooks/list` is gated behind the `v2Hooks` setting
   (errors "not available when v2Hooks is disabled" otherwise), so we enable it via
   _meta.kiro.settings. Then call `_kiro/hooks/list` (client->server request) and
   observe whether KAS ever calls a `_kiro/hooks/*` method BACK to the host
   (server->client) during a tool-using turn. Resolves "does the host manage hooks,
   or does the server fire them?"
2. `_kiro/account/getUsage` MESSAGE SHAPE — call it and log a STRUCTURALLY REDACTED
   view (keys + value *types*, never the values) so we get the wire shape without
   committing account/billing data.
3. `session_info_update` BREAKDOWN — run a tiny real turn and capture the
   `session_info_update` notification's `_meta.kiro` (token breakdown + turnEnd),
   the KAS analog of v2's `kiro.dev/metadata`.

In-process I/O (no fs/terminal caps). Auth token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-hooks-usage-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-hooks-")

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    if not row:
        return {}
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": d.get("profile_arn")}

def shape(v):
    """Recursively replace leaf VALUES with their type name; keep keys + structure.
    Lists collapse to [shape(first)] + a count marker. Safe to commit (no values)."""
    if isinstance(v, dict):
        return {k: shape(x) for k, x in v.items()}
    if isinstance(v, list):
        return ([shape(v[0]), f"...(len={len(v)})"] if v else [])
    return type(v).__name__  # int / float / str / bool / NoneType

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [0]
def req(m, p):
    _id[0] += 1; PIN.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p}) + "\n"); PIN.flush(); return _id[0]
def reply(rid, res):
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n"); PIN.flush()

SERVER_HOOK_CALLS = []   # any _kiro/hooks/* the SERVER calls back to us (direction signal)
SESSION_INFO = []        # session_info_update update objects
def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if m == "_kiro/auth/getAccessToken":
        reply(rid, read_token()); return
    if m == "_kiro/terminal/shell_type":
        reply(rid, {"shellType": "bash"}); return
    if m and m.startswith("_kiro/hooks/"):
        SERVER_HOOK_CALLS.append((m, rid is not None))
        if rid is not None:
            reply(rid, {})
        return
    if m == "session/request_permission":
        opts = p.get("options", [])
        pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()), opts[0] if opts else None)
        reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}})
        return
    if m and "session/update" in m or m == "session/update":
        u = (p.get("update") or {}) if isinstance(p, dict) else {}
        if isinstance(u, dict) and u.get("sessionUpdate") == "session_info_update":
            SESSION_INFO.append(u)
        return
    if rid is not None:
        reply(rid, {})

def pump(until, to=60):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = msgs.get(timeout=2)
        except queue.Empty:
            continue
        if raw is None:
            return None
        try:
            o = json.loads(raw)
        except Exception:
            continue
        if "method" in o and "id" not in o:
            handle(o)
        elif "method" in o:
            handle(o)
        elif "id" in o and o["id"] == until:
            return o
    return None

# NOTE (corrected by probe-kas-hooks-enabled-2.7.1.py): hooks are gated behind
# `clientMeta.hooks.enabled && clientMeta.hooks.v2 === true`. This probe sends the
# flag under `_meta.kiro.settings.hooks`, which is the WRONG location — the gate
# reads `_meta.kiro.hooks` directly (a SIBLING of `settings`). So with this probe
# hooks/list still errors "v2Hooks is disabled"; that's expected and documents the
# dead-end. The working enable path is in probe-kas-hooks-enabled-2.7.1.py. (The
# getUsage + session_info_update captures below are unaffected and valid.)
SETTINGS = {"kiro": {"settings": {"hooks": {"enabled": True, "v2": True}}}}
req("initialize", {"protocolVersion": 1, "clientCapabilities": {"_meta": SETTINGS}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": [], "_meta": SETTINGS})
nr = pump(nid, 40)
assert nr and "result" in nr, "session/new failed"
sid = nr["result"]["sessionId"]
log("# sessionId:", sid, "| attempted hooks.enabled+v2 via _meta.kiro.settings (NOT honored — global flag)")

# --- 1. hooks list (client -> server) ---
hid = req("_kiro/hooks/list", {"sessionId": sid})
hr = pump(hid, 20)
log("\n===== _kiro/hooks/list response =====")
if hr and "result" in hr:
    log(json.dumps(hr["result"], indent=2)[:1500])
elif hr and "error" in hr:
    log("ERROR:", json.dumps(hr["error"])[:300])
else:
    log("(no response)")

# --- 2. account/getUsage (SHAPE ONLY, redacted) ---
uid = req("_kiro/account/getUsage", {})
ur = pump(uid, 20)
log("\n===== _kiro/account/getUsage MESSAGE SHAPE (values redacted to types) =====")
if ur and "result" in ur:
    log(json.dumps(shape(ur["result"]), indent=2))
elif ur and "error" in ur:
    log("ERROR:", json.dumps(ur["error"])[:300])
else:
    log("(no response)")

# --- 3. tiny real turn -> session_info_update breakdown ---
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "Run the shell command `echo hi` and report its output."}]})
pump(pid, 150)
log("\n===== session_info_update notifications (%d) — _meta.kiro shape =====" % len(SESSION_INFO))
for u in SESSION_INFO:
    meta = (u.get("_meta") or {}).get("kiro") or {}
    log("  update keys:", sorted(u.keys()))
    log("  _meta.kiro:", json.dumps(meta)[:600])

log("\n===== HOOKS DIRECTION =====")
log("  server->client _kiro/hooks/* calls during session:", SERVER_HOOK_CALLS or "(none)")
log("  NOTE: hooks/list errored above because this probe set the flag in the WRONG place")
log("  (_meta.kiro.settings.hooks). The working enable path is _meta.kiro.hooks={enabled,v2}")
log("  (sibling of settings) — see probe-kas-hooks-enabled-2.7.1.py. Bundle shows hooks run")
log("  SERVER-SIDE (CommandAction({processRunner})); host is not called back to run them.")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
