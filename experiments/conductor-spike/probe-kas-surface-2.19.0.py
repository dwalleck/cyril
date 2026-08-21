#!/usr/bin/env python3
"""
KAS 0.46.1 (kiro-cli 2.19.0) live surface probe (audit: docs/kiro-2.19.0-wire-audit.md).

Covers: initialize inventory (agentInfo absent; sessionCapabilities list/fork),
session/new configOptions (`model` ABSENT cold — arrives later via
config_option_update with the rebuilt full list, the [V3] cold-registry fix),
one tiny turn (turn-end ordering: session_info_update kind:turn_end BEFORE the
session/prompt response, ~7 ms — cyril-14ou model re-verified on 0.46.1; frames
now carry `messageId`), and session/list title evolution (deterministic
placeholder = filler-stripped first prompt; the LLM title upgrade ships DARK
behind FeatureKey SESSION_TITLE_LLM).

Auth: real XDG_DATA_HOME token store; `_kiro/auth/getAccessToken` answered from
auth_kv key `kirocli:odic:token` (IdC). Profile ARN from $KIRO_PROFILE_ARN or
`kiro-cli user whoami`. HOME is a throwaway tmpdir (probe isolation).
"""
import json, os, re, subprocess, threading, queue, time, tempfile, sqlite3

OUTDIR = os.environ.get("PROBE_OUT", ".")
TRACE = os.path.join(OUTDIR, "kas-surface-trace.jsonl")
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
FAKE_HOME = tempfile.mkdtemp(prefix="kas-probe-home-")
CWD = tempfile.mkdtemp(prefix="kas-probe-cwd-")
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

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open(os.path.join(OUTDIR, "kas-surface-stderr.log"), "w"),
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

NOTIFS = []
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
        elif "method" in o:
            NOTIFS.append(o)
        elif "id" in o and until_id is not None and o["id"] == until_id:
            return o
    return None

iid = req("initialize", {"protocolVersion": 1,
                         "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"},
                         "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}}})
init = pump(iid, 60)
ir = (init or {}).get("result", {})
print("== agentInfo:", json.dumps(ir.get("agentInfo")))
print("== sessionCapabilities:", json.dumps((ir.get("agentCapabilities") or {}).get("sessionCapabilities")))

nid = req("session/new", {"cwd": CWD, "mcpServers": []})
new = pump(nid, 60)
res = (new or {}).get("result", {})
sid = res.get("sessionId")
cfg = res.get("configOptions")
print("== session/new configOptions ids:", [c.get("configId") or c.get("id") for c in (cfg or [])])
pump(timeout=8, idle_exit=4)

t0 = time.time()
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "Reply with exactly: OK"}]})
resp = pump(pid, 240)
print("== prompt response:", json.dumps((resp or {}).get("result") or (resp or {}).get("error"))[:400], f"({time.time()-t0:.1f}s)")
pump(timeout=6, idle_exit=3)

for label, wait in (("immediate", 0), ("after-20s", 20)):
    if wait:
        pump(timeout=wait, idle_exit=wait)
    lid = req("session/list", {"cwd": CWD})
    lres = pump(lid, 30)
    for s in ((lres or {}).get("result") or {}).get("sessions") or []:
        print(f"== session/list ({label}): title={s.get('title')!r} updatedAt={s.get('updatedAt')}")

trace.close()
proc.terminate()

frames = [json.loads(l) for l in open(TRACE)]
resp_idx = next((i for i, f in enumerate(frames) if f["dir"] == "agent->client" and f["msg"].get("id") == pid and ("result" in f["msg"] or "error" in f["msg"])), None)
if resp_idx is not None:
    print("\n== frames around turn end:")
    for i in range(max(0, resp_idx - 6), min(len(frames), resp_idx + 2)):
        f = frames[i]
        m = f["msg"]
        u = (m.get("params") or {}).get("update", {}) if m.get("method") == "session/update" else {}
        desc = m.get("method") or f"resp:{m.get('id')}"
        detail = (u.get("sessionUpdate", "") + " " + json.dumps({k: v for k, v in u.items() if k != "sessionUpdate"})[:200]) if u \
                 else json.dumps(m.get("result") or m.get("error") or {})[:200]
        print(f"  [{i}] {f['ts']-frames[0]['ts']:8.3f}s {f['dir'][:6]:6} {desc:26} {detail[:210]}")
print("\n== notification methods:", sorted({n["method"] for n in NOTIFS}))
print("TRACE:", TRACE, f"({len(frames)} frames)")
