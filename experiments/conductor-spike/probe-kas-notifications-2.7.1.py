#!/usr/bin/env python3
"""
Catalog the full KAS server->client NOTIFICATION stream (the `_kiro/*` pushes and
`session/update` variants) over one representative session: initialize -> session/new
-> a tool-using turn. For each distinct notification method we record a count and a
STRUCTURALLY-REDACTED sample (leaf values -> type names) so the wire shape is captured
without committing payload values.

Targets the not-yet-captured streams from the 2.7.1 audit's "Not verified" list:
`_kiro/progressive_context/items_changed`, `_kiro/governance/state`,
`_kiro/powers/items_changed`, `_kiro/steering/documents_changed` — plus whatever else
fires. Streams that don't appear are reported as "not observed (this config)".

In-process I/O (no fs/terminal caps). Auth token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-notifications-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-notif-")

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

def shape(v, depth=0):
    if depth > 6:
        return "..."
    if isinstance(v, dict):
        return {k: shape(x, depth + 1) for k, x in v.items()}
    if isinstance(v, list):
        return ([shape(v[0], depth + 1), f"...(len={len(v)})"] if v else [])
    return type(v).__name__

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

# method -> {count, sample}; session/update broken out by sessionUpdate kind
NOTIFS = {}
UPDATES = {}
def record(method, params):
    if method == "session/update" or method.endswith("/session/update"):
        u = (params.get("update") or {}) if isinstance(params, dict) else {}
        kind = u.get("sessionUpdate", "?") if isinstance(u, dict) else "?"
        # KAS session_info_update further keys on _meta.kiro.kind
        if kind == "session_info_update":
            kind = "session_info_update:" + (((u.get("_meta") or {}).get("kiro") or {}).get("kind", "?"))
        e = UPDATES.setdefault(kind, {"count": 0, "sample": None})
        e["count"] += 1
        if e["sample"] is None:
            e["sample"] = shape(u)
        return
    e = NOTIFS.setdefault(method, {"count": 0, "sample": None})
    e["count"] += 1
    if e["sample"] is None:
        e["sample"] = shape(params)

def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is not None:  # server->client REQUEST: answer host callbacks, don't catalog as notif
        if m == "_kiro/auth/getAccessToken":
            reply(rid, read_token())
        elif m == "_kiro/terminal/shell_type":
            reply(rid, {"shellType": "bash"})
        elif m == "session/request_permission":
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()), opts[0] if opts else None)
            reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}})
        else:
            reply(rid, {})
        return
    record(m, p)  # pure notification

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
        if "method" in o:
            handle(o)
            if o.get("id") == until and "result" in o:
                return o
        elif "id" in o and o["id"] == until:
            return o
    return None

req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
nr = pump(nid, 40)
assert nr and "result" in nr, "session/new failed"
sid = nr["result"]["sessionId"]
# a tool-using turn to provoke mid-turn streams
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "Run the shell command `echo hi` and report its output."}]})
pump(pid, 150)
pump(-1, 4)

log("# KAS 2.7.1 server->client NOTIFICATION catalog (one tool-using turn, default settings)")
log(f"# sessionId: {sid}\n")
log("===== _kiro/* and other notification METHODS =====")
for m in sorted(NOTIFS):
    log(f"\n[{NOTIFS[m]['count']:>2}x] {m}")
    log("      " + json.dumps(NOTIFS[m]["sample"])[:400])
log("\n===== session/update variants (session_info_update broken out by _meta.kiro.kind) =====")
for k in sorted(UPDATES):
    log(f"  [{UPDATES[k]['count']:>2}x] {k}")

TARGETS = ["_kiro/progressive_context/items_changed", "_kiro/governance/state",
           "_kiro/powers/items_changed", "_kiro/steering/documents_changed",
           "_kiro/tools/didChange", "_kiro/mcp/status"]
log("\n===== targeted streams from the audit's 'Not verified' list =====")
for t in TARGETS:
    log(f"  {'OBSERVED' if t in NOTIFS else 'not observed':12s} {t}")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
