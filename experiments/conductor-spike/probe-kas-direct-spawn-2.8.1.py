#!/usr/bin/env python3
"""
Smoke test: spawn the embedded KAS ACP server DIRECTLY (not via `kiro-cli acp
--agent-engine kas`) over stdio, and capture a live `_kiro/*` init + session/new
+ one tiny turn. Validates the server-side launch contract recovered 2026-06-21
from acp-server.js and exercises the "free path" for cyril's KAS-1 milestone.

WHY DIRECT SPAWN: the existing probe-kas-*.py harnesses go through the kiro-cli
wrapper, which launches the server with `--auth=acp-callback` (so every probe
must answer `_kiro/auth/getAccessToken` from the sqlite auth store). This probe
tests the OTHER path: `acp-server.js --transport=stdio` with NO `--auth` flag,
which `selectAuthProvider` resolves to the default FileAuthProvider reading
`~/.aws/sso/cache/kiro-auth-token.json` (self-refreshing via the file's
refreshToken). If that works, cyril can run KAS with ZERO auth-callback code as
long as the user has run `kiro-cli login`.

THE VERDICT THIS PRODUCES: whether `_kiro/auth/getAccessToken` is ever called.
  - NOT called  -> direct spawn authenticated from the token file (free path CONFIRMED)
  - called      -> server still delegates auth to the host (free-path claim is wrong)
A sqlite-backed responder is wired as a safety net either way, so the turn never
hangs and the verdict is observable, not fatal.

Run: python3 probe-kas-direct-spawn-2.8.1.py
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3

SERVER = os.path.expanduser(
    "~/.local/share/kiro-cli/kas/node_modules/@kiro/agent/dist/server/acp-server.js"
)
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
CWD = tempfile.mkdtemp(prefix="kas-direct-")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-direct-spawn-2.8.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

def pretty(obj, limit=2000):
    s = json.dumps(obj, indent=2, sort_keys=True)
    return s if len(s) <= limit else s[:limit] + f"\n  ...(+{len(s)-limit} chars)"

# --- safety-net token source (only used if the server DOES call back) ----------
def read_token():
    try:
        c = sqlite3.connect(AUTH_DB)
        try:
            row = c.execute(
                "select value from auth_kv where key='kirocli:social:token'"
            ).fetchone()
        finally:
            c.close()
    except Exception as e:
        log("[WARN] sqlite token read failed:", e); return None
    if not row:
        return None
    v = row[0]
    v = v.decode("utf-8", "replace") if isinstance(v, (bytes, bytearray)) else v
    d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"],
            "profileArn": d.get("profile_arn"), "provider": d.get("provider")}

# --- spawn the server DIRECTLY: no --auth -> default file auth ------------------
assert os.path.exists(SERVER), f"KAS server not found at {SERVER}"
runtime = os.environ.get("KIRO_AGENT_PATH", "node")
argv = [runtime, "--experimental-wasm-modules", SERVER, "--transport=stdio"]
log("# spawn:", " ".join(argv))
log("# cwd:  ", CWD)
log("# auth: default (no --auth flag) -> ~/.aws/sso/cache/kiro-auth-token.json\n")

stderr_log = open(LOG + ".stderr", "w")
proc = subprocess.Popen(argv, cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=stderr_log, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout

msgs = queue.Queue()
threading.Thread(
    target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)),
    daemon=True,
).start()

_id = [0]
def req(method, params):
    _id[0] += 1
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": method, "params": params}) + "\n")
    PIN.flush()
    return _id[0]
def reply(rid, res):
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    PIN.flush()

# observability accumulators -> become the verdict
state = {
    "getAccessToken_calls": 0,
    "inbound_methods": {},      # agent->client request methods + counts
    "update_variants": {},      # session/update sessionUpdate kinds + counts
}

def note_inbound(method):
    state["inbound_methods"][method] = state["inbound_methods"].get(method, 0) + 1

def pump(until_id, timeout=180):
    """Drive the connection until response `until_id` arrives, servicing every
    agent->client request so nothing deadlocks. Returns the matching response."""
    end = time.time() + timeout
    while time.time() < end:
        try:
            raw = msgs.get(timeout=2)
        except queue.Empty:
            continue
        if raw is None:
            log("[server stdout closed]"); return None
        try:
            o = json.loads(raw)
        except Exception:
            continue
        # notification (no id)
        if "method" in o and "id" not in o:
            if o["method"] == "session/update":
                kind = (o.get("params", {}).get("update", {}) or {}).get("sessionUpdate", "?")
                state["update_variants"][kind] = state["update_variants"].get(kind, 0) + 1
            continue
        # agent -> client request (has method AND id)
        if "method" in o and "id" in o:
            m = o["method"]; note_inbound(m)
            if m == "_kiro/auth/getAccessToken":
                state["getAccessToken_calls"] += 1
                tok = read_token()
                reply(o["id"], tok or {})
            elif m == "session/request_permission":
                opts = o["params"].get("options", [])
                pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                            opts[0] if opts else None)
                reply(o["id"], {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                      if pick else {"outcome": {"outcome": "cancelled"}})
            else:
                # fs/terminal/hooks/etc — ack empty so the turn proceeds
                reply(o["id"], {})
            continue
        # response to one of our requests
        if "id" in o and o.get("id") == until_id:
            return o
    log(f"[timeout waiting for id={until_id}]")
    return None

try:
    # 1) initialize ------------------------------------------------------------
    iid = req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
    iresp = pump(iid, timeout=60)
    assert iresp and "result" in iresp, f"initialize failed: {iresp}"
    log("==== initialize result ====")
    log(pretty(iresp["result"], limit=100000))

    # 2) session/new -----------------------------------------------------------
    nid = req("session/new", {"cwd": CWD, "mcpServers": []})
    nresp = pump(nid, timeout=60)
    assert nresp and "result" in nresp, f"session/new failed: {nresp}"
    sid = nresp["result"]["sessionId"]
    log("\n==== session/new result ====")
    log(pretty(nresp["result"], limit=100000))
    log("\nsessionId:", sid)

    # 3) one tiny turn ---------------------------------------------------------
    log("\n==== running one tiny turn ====")
    pid = req("session/prompt", {"sessionId": sid,
              "prompt": [{"type": "text", "text": "Reply with exactly: ok. Do not use any tools."}]})
    presp = pump(pid, timeout=180)
    if presp and "result" in presp:
        log("prompt response:", pretty(presp["result"], limit=600))
    elif presp and "error" in presp:
        log("prompt ERROR:", pretty(presp["error"]))
    else:
        log("prompt: no response/timeout")
finally:
    try:
        PIN.close()
    except Exception:
        pass
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except Exception:
        proc.kill()

# --- verdict ------------------------------------------------------------------
log("\n" + "=" * 60)
log("VERDICT")
log("=" * 60)
free = state["getAccessToken_calls"] == 0
log(f"_kiro/auth/getAccessToken calls : {state['getAccessToken_calls']}")
log(f"FREE PATH (file auth, no callback): {'CONFIRMED' if free else 'NO — server delegated auth to host'}")
log(f"agent->client request methods    : {json.dumps(state['inbound_methods'])}")
log(f"session/update variants seen     : {json.dumps(state['update_variants'])}")
log(f"\n# stdout log : {LOG}")
log(f"# stderr log : {LOG}.stderr")
