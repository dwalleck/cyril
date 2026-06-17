#!/usr/bin/env python3
"""
Capture KAS's live tool advertisement: the `_kiro/tools/didChange` notification(s)
pushed during `session/new`.

Finding (2.7.1): KAS advertises BUILT-IN tools on the wire as 4 coarse CATEGORY
TAGS (read/write/shell/web), NOT as individual tool ids — the granular built-in
ids (fs_write, execute_bash, c2s_*, …) live in the system prompt + bundle, not on
the wire. MCP tools are advertised one tag per tool (`@server/tool`). This probe
dedupes the repeated pushes and separates builtin tags from MCP per-tool tags.

In-process I/O (no fs/terminal caps). Token self-sourced from the kiro-cli auth
store; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-tools-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-tools-")

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

BUILTIN = {}   # tag -> description
MCP = {}       # @server/tool -> description
def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if m == "_kiro/auth/getAccessToken":
        reply(rid, read_token()); return
    if m == "_kiro/terminal/shell_type":
        reply(rid, {"shellType": "bash"}); return
    if m == "_kiro/tools/didChange":
        for t in (p.get("tags") or p.get("tools") or []):
            if not isinstance(t, dict):
                BUILTIN.setdefault(str(t), ""); continue
            tag = t.get("tag", "?"); desc = (t.get("description") or "").split("\n")[0][:80]
            (MCP if t.get("source") == "mcp" else BUILTIN)[tag] = desc
        return
    if rid is not None:
        reply(rid, {})

def pump(until, to=40):
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
        elif "id" in o and o["id"] == until:
            return o
    return None

req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
nr = pump(nid, 40)
assert nr and "result" in nr, "session/new failed"
pump(-1, 4)  # drain trailing didChange pushes

log("# KAS 2.7.1 tool advertisement via _kiro/tools/didChange")
log("# session/new 'tools' field present:", bool((nr.get("result") or {}).get("tools")))
log(f"\n=== BUILT-IN tool tags ({len(BUILTIN)}) — coarse categories, NOT individual ids ===")
for tag in sorted(BUILTIN):
    log(f"  {tag:10s} {BUILTIN[tag]}")
log(f"\n=== MCP per-tool tags ({len(MCP)}) — from this host's installed powers, not KAS-intrinsic ===")
for tag in sorted(MCP):
    log(f"  {tag}")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
