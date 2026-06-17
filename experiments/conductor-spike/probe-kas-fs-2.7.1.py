#!/usr/bin/env python3
"""
Probe: does KAS (2.7.1) invoke ACP fs/* client callbacks, or do file I/O in-process?

Drives an authenticated KAS turn with a write-then-read-back task and records EVERY
server->client request method. Advertises fs read/write client capability and ACTUALLY
performs the I/O when called (so the turn completes through callbacks if KAS uses them).
After the turn, checks whether the file exists on disk and whether any fs/* request arrived.

Key question for cyril: if KAS calls fs/* callbacks, cyril must implement filesystem
responders (it implements none today — v2 never calls them; see reference_kiro_no_fs_callbacks).

Usage: python3 probe-kas-fs-2.7.1.py    (run `kiro-cli whoami` first if the token is idle)
"""
import json, os, subprocess, sys, threading, queue, time, tempfile, pathlib, sqlite3

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
CWD = tempfile.mkdtemp(prefix="kas-fs-probe-")
OUT = pathlib.Path(__file__).with_name("logs") / "probe-kas-fs-2.7.1.log"
OUT.parent.mkdir(exist_ok=True)
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

MARKER = "KAS-FS-PROBE-9F3A21"
TARGET = "hello.txt"
PROMPT = (
    f"Do this YOURSELF using your built-in file tools — do NOT delegate to any subagent. "
    f"Step 1: create a file named {TARGET} in the current working directory containing exactly "
    f"this single line: {MARKER}. "
    f"Step 2: read {TARGET} back from disk and tell me its exact contents. "
)

def read_kiro_token():
    c = sqlite3.connect(AUTH_DB)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    if not row:
        return None
    v = row[0]
    if isinstance(v, (bytes, bytearray)):
        v = v.decode("utf-8", "replace")
    d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"],
            "profileArn": d.get("profile_arn"), "provider": d.get("provider")}

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin is not None and proc.stdout is not None
PROC_IN, PROC_OUT = proc.stdin, proc.stdout

log_f = open(OUT, "w")
def log(*a):
    line = " ".join(str(x) for x in a)
    print(line); log_f.write(line + "\n"); log_f.flush()

msgs = queue.Queue()
def reader():
    for line in PROC_OUT:
        line = line.strip()
        if line:
            msgs.put(line)
    msgs.put(None)
threading.Thread(target=reader, daemon=True).start()

_id = [10]
def send(method, params):
    _id[0] += 1
    PROC_IN.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": method, "params": params}) + "\n")
    PROC_IN.flush(); return _id[0]
def reply(rid, result):
    PROC_IN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": result}) + "\n"); PROC_IN.flush()

FS_CALLS = []          # (method, params) for any fs-related server request
SERVER_METHODS = {}    # method -> count

def handle_server_request(o):
    method, rid, params = o.get("method"), o.get("id"), o.get("params", {})
    SERVER_METHODS[method] = SERVER_METHODS.get(method, 0) + 1
    is_fs = method and ("fs/" in method or "fs/" in method.replace("_kiro/", "")
                        or "readTextFile" in method or "writeTextFile" in method
                        or "read_text_file" in method or "write_text_file" in method
                        or "read_file" in method or "write_file" in method)
    log(f"\n>>> SERVER REQUEST  {method}  id={rid}{'   <<< FS CALLBACK' if is_fs else ''}\n    {json.dumps(params)[:700]}")
    if method == "_kiro/auth/getAccessToken":
        tok = read_kiro_token(); reply(rid, tok or {})
        log("    -> token supplied" if tok else "    -> NO TOKEN")
    elif method == "session/request_permission":
        opts = params.get("options", [])
        pick = next((op for op in opts if "allow" in (op.get("kind","")+op.get("optionId","")).lower()), opts[0] if opts else None)
        reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}})
        log(f"    -> permission allowed ({pick.get('optionId') if pick else None})")
    elif is_fs and ("read" in method.lower()):
        FS_CALLS.append((method, params))
        p = params.get("path") or params.get("uri") or ""
        try: content = pathlib.Path(p).read_text()
        except Exception as e: content = ""; log(f"    (read miss: {e})")
        reply(rid, {"content": content})          # ACP fs/read_text_file shape
        log(f"    -> ANSWERED fs read for {p}")
    elif is_fs and ("write" in method.lower()):
        FS_CALLS.append((method, params))
        p = params.get("path") or params.get("uri") or ""
        c = params.get("content", "")
        try: pathlib.Path(p).write_text(c); log(f"    -> HOST WROTE {p}")
        except Exception as e: log(f"    (write fail: {e})")
        reply(rid, {})
    else:
        reply(rid, {}); log("    -> generic ack")

def pump(until_id=None, timeout=180):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try: raw = msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: log("[stream closed]"); return None
        try: o = json.loads(raw)
        except Exception: continue
        if "method" in o and "id" in o:
            handle_server_request(o)
        elif "method" in o:
            u = o.get("params", {}).get("update", {})
            v = u.get("sessionUpdate") if isinstance(u, dict) else None
            if v in ("tool_call", "tool_call_update", "agent_message_chunk"):
                log(f"NOTIF {v}: {json.dumps(o['params']['update'])[:300]}")
        elif "id" in o:
            if "error" in o: log(f"<<< RESPONSE id={o['id']} ERR: {json.dumps(o['error'])[:200]}")
            if until_id is not None and o["id"] == until_id: return o
    log("[timeout]"); return None

log(f"# KIRO={KIRO}\n# CWD={CWD}\n# task: write {TARGET} ({MARKER}) then read back")
# ADVERTISE_FS=0 tests whether KAS falls back to in-process I/O when the client
# does NOT advertise fs capability (determines if fs responders are mandatory for the host).
_fs = os.environ.get("ADVERTISE_FS", "1") != "0"
log(f"# clientCapabilities.fs advertised: {_fs}")
send("initialize", {"protocolVersion": 1,
                    "clientCapabilities": ({"fs": {"readTextFile": True, "writeTextFile": True}} if _fs else {})})
pump(until_id=11, timeout=30)
sid_id = send("session/new", {"cwd": CWD, "mcpServers": []})
resp = pump(until_id=sid_id, timeout=40)
sid = resp["result"]["sessionId"] if resp and "result" in resp else None
log(f"# sessionId={sid}")
if not sid: proc.kill(); sys.exit(1)
pid = send("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
pump(until_id=pid, timeout=200)

log("\n===== RESULT =====")
log("server->client methods seen:", json.dumps(SERVER_METHODS))
log(f"fs/* callbacks invoked: {len(FS_CALLS)}")
for m, p in FS_CALLS: log("   FS:", m, json.dumps(p)[:200])
on_disk = pathlib.Path(CWD, TARGET)
log(f"file on disk ({on_disk}): exists={on_disk.exists()}",
    f"contents={on_disk.read_text()!r}" if on_disk.exists() else "")
log("VERDICT:",
    "KAS USES fs/* callbacks (host must implement responders)" if FS_CALLS
    else "KAS does file I/O IN-PROCESS (no fs callbacks; file written by agent directly)"
         if on_disk.exists() else "INCONCLUSIVE (no callbacks, no file)")
PROC_IN.close(); proc.terminate()
log(f"# full log: {OUT}")
