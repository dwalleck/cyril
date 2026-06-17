#!/usr/bin/env python3
"""
Live-fire the SAFE, read-only client->agent request methods KAS exposes
(`AgentCapabilityTypes` in @kiro/acp-type-covenant) and capture their real
response shapes. These are the methods CYRIL would call ON KAS — the inverse of
the host callbacks.

Fired (all read-only / non-destructive):
  - _kiro/permissions/list      {scope?}                  -> policy rules
  - _kiro/permissions/explain   {capability?, resource}   -> effect + matched rule
  - _kiro/policy/check          {capability, paths|command} -> allow/deny
  - _kiro/codeIntelligence      {subcommand:'status'}     -> LSP/workspace state
  - _kiro/session/context       {subcommand:'show'}       -> context entries
  - _kiro/session/history       {beforeMessageId, limit}  -> paginated replay

Deliberately NOT fired (destructive/state-changing): session/{delete,rename,compact},
checkpoint/revert*, spec/invoke, mcp/resetServer. A small "say ok" turn runs first to
populate history and capture a real userMessageId for the history cursor.

Auth token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-client-methods-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-clientm-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": d.get("profile_arn")}

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

USER_MSG_ID = [None]
PERM_ASKS = []
def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is not None:
        if m == "_kiro/auth/getAccessToken":
            reply(rid, read_token())
        elif m == "_kiro/terminal/shell_type":
            reply(rid, {"shellType": "bash"})
        elif m == "session/request_permission":
            PERM_ASKS.append(p.get("toolCall", {}).get("title") or "?")
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()), opts[0] if opts else None)
            reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}})
        else:
            reply(rid, {})
        return
    if m and ("session/update" in m):
        u = (p.get("update") or {}) if isinstance(p, dict) else {}
        if isinstance(u, dict) and u.get("sessionUpdate") == "session_info_update":
            k = ((u.get("_meta") or {}).get("kiro") or {})
            if k.get("kind") == "user_message_id_assigned":
                USER_MSG_ID[0] = k.get("userMessageId")

def pump(until, to=120):
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
        if "method" in o:
            handle(o)
            if o.get("id") == until and "result" in o:
                return o
        elif "id" in o and o["id"] == until:
            return o
    return None

def call(method, params, to=30, full=False):
    rid = req(method, params)
    r = pump(rid, to)
    log(f"\n===== {method}  params={json.dumps(params)[:160]} =====")
    if r is None:
        log("  (no response / timeout)")
    elif "result" in r:
        log("  RESULT:", json.dumps(r["result"], indent=2) if full else json.dumps(r["result"])[:1200])
    elif "error" in r:
        log("  ERROR:", json.dumps(r["error"])[:300])
    return r

# enable codeIntelligence so its status returns real data (else "not enabled")
SETTINGS = {"kiro": {"settings": {"codeIntelligence": {"enabled": True}}}}
req("initialize", {"protocolVersion": 1, "clientCapabilities": {"_meta": SETTINGS}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": [], "_meta": SETTINGS})
nr = pump(nid, 40)
assert nr and "result" in nr, "session/new failed"
sid = nr["result"]["sessionId"]
log("# sessionId:", sid)

# small turn to populate history + capture a real userMessageId
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "Reply with the single word: ok"}]})
pump(pid, 90)
log("# captured userMessageId:", USER_MSG_ID[0])

# ---- read-only client->agent methods ----
call("_kiro/permissions/list", {"sessionId": sid}, full=True)
call("_kiro/permissions/list", {"sessionId": sid, "scope": "session"})
call("_kiro/permissions/explain", {"sessionId": sid, "capability": "shell", "resource": "rm -rf /"})
call("_kiro/permissions/explain", {"sessionId": sid, "capability": "fs_read", "resource": "/etc/passwd"})
call("_kiro/policy/check", {"sessionId": sid, "capability": "fs_read", "paths": ["/etc/passwd"]})
call("_kiro/policy/check", {"sessionId": sid, "capability": "shell", "command": "ls -la"})
call("_kiro/codeIntelligence", {"sessionId": sid, "subcommand": "status"})
call("_kiro/session/context", {"sessionId": sid, "subcommand": "show"})
if USER_MSG_ID[0]:
    call("_kiro/session/history", {"sessionId": sid, "beforeMessageId": USER_MSG_ID[0], "limit": 10})
else:
    log("\n(no userMessageId captured — skipping session/history)")

log("\n# permission prompts triggered during this probe:", PERM_ASKS or "(none)")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
