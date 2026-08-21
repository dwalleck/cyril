#!/usr/bin/env python3
"""
kiro-cli 2.19.0 stream-idle watchdog wire probe (audit: docs/kiro-2.19.0-wire-audit.md).

Usage: probe-v2-watchdog-2.19.0.py <label> <engine v2|kas> <soft> <hard> <prompt...>

Writes an isolated fake-HOME `~/.kiro/settings/cli.json` carrying
api.streamIdleSoftTimeout / api.streamIdleHardTimeout (units: SECONDS; pass 0 0
to omit = control run), spawns `kiro-cli acp [--agent-engine kas]`, sends one
prompt, and records every frame to watchdog-<label>-trace.jsonl.

Live findings this probe produced on 2.19.0 (v2 engine):
  - soft idle -> `_kiro.dev/session/update` {sessionUpdate:"stream_stall_notice",
    message:"Still working, model is thinking..."} at exactly the set seconds
    (TTFB counts as silence);
  - hard idle -> abort + retry x3 (notice message "Response timed out - retrying"),
    then JSON-RPC error response -32603 with data "...The stream timed out
    receiving the response after <hard*1000>ms";
  - KAS: NO watchdog (keys unread; zero streamIdle strings in @kiro/agent 0.46.1).

Auth: real XDG_DATA_HOME supplies the token store; `_kiro/auth/getAccessToken`
is answered from `auth_kv` key `kirocli:odic:token` (IdC). Profile ARN comes
from $KIRO_PROFILE_ARN or `kiro-cli user whoami`.
"""
import json, os, re, subprocess, threading, queue, time, tempfile, sqlite3, sys

label, engine, soft, hard = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
prompt_text = " ".join(sys.argv[5:]) or "Reply with exactly: OK"

OUTDIR = os.environ.get("PROBE_OUT", ".")
TRACE = os.path.join(OUTDIR, f"watchdog-{label}-trace.jsonl")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

def profile_arn():
    arn = os.environ.get("KIRO_PROFILE_ARN")
    if arn:
        return arn
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()
FAKE_HOME = tempfile.mkdtemp(prefix=f"wd-{label}-home-")
CWD = tempfile.mkdtemp(prefix=f"wd-{label}-cwd-")

if soft or hard:
    os.makedirs(os.path.join(FAKE_HOME, ".kiro", "settings"), exist_ok=True)
    with open(os.path.join(FAKE_HOME, ".kiro", "settings", "cli.json"), "w") as f:
        json.dump({"api.streamIdleSoftTimeout": soft, "api.streamIdleHardTimeout": hard}, f)

env = dict(os.environ)
env["HOME"] = FAKE_HOME
env["XDG_DATA_HOME"] = os.path.expanduser("~/.local/share")

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

cmd = [KIRO, "acp"] + (["--agent-engine", "kas"] if engine == "kas" else [])
proc = subprocess.Popen(cmd, cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open(os.path.join(OUTDIR, f"watchdog-{label}-stderr.log"), "w"),
                        text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
trace = open(TRACE, "w")

def record(direction, obj):
    trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": obj}) + "\n")
    trace.flush()

threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [10]

def send(obj):
    record("client->agent", obj)
    PIN.write(json.dumps(obj) + "\n"); PIN.flush()

def req(m, p):
    _id[0] += 1
    send({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p})
    return _id[0]

def handle_server_req(o):
    m = o["method"]
    if m == "_kiro/auth/getAccessToken":
        send({"jsonrpc": "2.0", "id": o["id"], "result": read_token() or {}})
    elif m == "session/request_permission":
        opts = o.get("params", {}).get("options", [])
        pick = next((x for x in opts if "allow" in (str(x.get("kind", "")) + str(x.get("optionId", ""))).lower()), opts[0] if opts else None)
        res = {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
        send({"jsonrpc": "2.0", "id": o["id"], "result": res})
    else:
        send({"jsonrpc": "2.0", "id": o["id"], "result": {}})

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
        if "method" in o and "id" in o:
            handle_server_req(o)
        elif "id" in o and until_id is not None and o["id"] == until_id:
            return o
    return None

iid = req("initialize", {"protocolVersion": 1,
                         "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"},
                         "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}}})
pump(iid, 60)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
new = pump(nid, 90)
sid = ((new or {}).get("result") or {}).get("sessionId")
if not sid:
    print(f"[{label}] SESSION/NEW FAILED:", json.dumps(new)[:400]); sys.exit(1)
pump(timeout=6, idle_exit=3)

t0 = time.time()
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": prompt_text}]})
resp = pump(pid, 420)
t1 = time.time()
body = (resp or {}).get("result") or (resp or {}).get("error")
print(f"[{label}] engine={engine} soft={soft} hard={hard} -> response after {t1-t0:.1f}s: {json.dumps(body)[:500]}")
pump(timeout=5, idle_exit=3)
trace.close()
proc.terminate()

frames = [json.loads(l) for l in open(TRACE)]
turn = [f for f in frames if f["ts"] >= t0 - 0.1 and f["dir"] == "agent->client"]
prev = t0
print(f"[{label}] turn timeline ({len(turn)} agent frames):")
for f in turn:
    m = f["msg"]
    gap = f["ts"] - prev
    prev = f["ts"]
    u = (m.get("params") or {}).get("update", {}) if m.get("method") == "session/update" else {}
    kind = u.get("sessionUpdate", "") if u else ""
    meta_kind = ""
    if isinstance(u.get("_meta"), dict):
        meta_kind = (u["_meta"].get("kiro") or {}).get("kind", "")
    desc = m.get("method") or f"resp:{m.get('id')}"
    detail = ""
    if kind == "agent_message_chunk":
        detail = repr((u.get("content") or {}).get("text", ""))[:60]
    elif kind:
        detail = json.dumps({k: v for k, v in u.items() if k not in ("sessionUpdate",)})[:260]
    elif "result" in m or "error" in m:
        detail = json.dumps(m.get("result") or m.get("error"))[:260]
    marker = " <<<< GAP" if gap > max(1.5, (soft or 99) * 0.8) else ""
    print(f"  +{f['ts']-t0:7.2f}s (gap {gap:5.2f}s) {desc:22} {kind or '':26} {meta_kind:16} {detail}{marker}")
print(f"[{label}] TRACE: {TRACE}")
