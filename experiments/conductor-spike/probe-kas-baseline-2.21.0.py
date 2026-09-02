#!/usr/bin/env python3
"""
KAS baseline capture for the 2.21.0 audit — identical workload on two bundles.

    LEG=live   python3 probe-kas-baseline-2.21.0.py      # KAS shipped in kiro-cli 2.21.0 (0.54.8)
    LEG=pinned KAS_PIN=<acp-server.js> python3 ...       # KAS 0.54.3 via KIRO_KAS_SERVER_PATH

Workload: initialize -> session/new -> drain pushes -> one cheap prompt ->
drain. Writes kas-baseline-<leg>-2.21.0.jsonl for sweep-new-fields.py --diff.
Prints: extensionMethods, agentCapabilities keys, session/new keys, the model
configOption list, the *Enabled settings echo, session_info_update kinds, and
every session-less _kiro/system/notify.
HOME-isolated (real XDG_DATA_HOME keeps the IdC token reachable).
"""
import json, os, re, sqlite3, subprocess, sys, tempfile, threading, queue, time

LEG = os.environ.get("LEG", "live")
OUTDIR = os.environ.get("PROBE_OUT", ".")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
TRACE = os.path.join(OUTDIR, f"kas-baseline-{LEG}-2.21.0.jsonl")
PROMPT = os.environ.get("PROMPT", "Reply with exactly: OK")

def profile_arn():
    arn = os.environ.get("KIRO_PROFILE_ARN")
    if arn: return arn
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()
FAKE_HOME = tempfile.mkdtemp(prefix=f"kas-base-{LEG}-home-")
CWD = tempfile.mkdtemp(prefix=f"kas-base-{LEG}-cwd-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
env = dict(os.environ)
env["HOME"] = FAKE_HOME
env["XDG_DATA_HOME"] = os.path.expanduser("~/.local/share")
if LEG == "pinned":
    pin = os.environ.get("KAS_PIN")
    if not pin or not os.path.exists(pin):
        sys.exit("LEG=pinned needs KAS_PIN=<path to acp-server.js>")
    env["KIRO_KAS_SERVER_PATH"] = pin

def read_token():
    c = sqlite3.connect(AUTH_DB)
    try: row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
    finally: c.close()
    if not row: return None
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v
    d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": PROFILE_ARN}

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open(os.path.join(OUTDIR, f"kas-baseline-{LEG}-2.21.0-stderr.log"), "w"),
                        text=True, bufsize=1)
PIN_, POUT = proc.stdin, proc.stdout
msgs = queue.Queue(); trace = open(TRACE, "w")
def record(d, o): trace.write(json.dumps({"ts": time.time(), "dir": d, "msg": o}) + "\n"); trace.flush()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [10]
def send(o): record("client->agent", o); PIN_.write(json.dumps(o) + "\n"); PIN_.flush()
def req(m, p=None):
    _id[0] += 1
    msg = {"jsonrpc": "2.0", "id": _id[0], "method": m}
    if p is not None: msg["params"] = p
    send(msg); return _id[0]

NOTIFY, KINDS, SERVER_REQS = [], {}, {}
def handle_server_req(o):
    m = o["method"]; SERVER_REQS[m] = SERVER_REQS.get(m, 0) + 1
    if m == "_kiro/auth/getAccessToken": send({"jsonrpc": "2.0", "id": o["id"], "result": read_token() or {}})
    elif m == "_kiro/terminal/shell_type": send({"jsonrpc": "2.0", "id": o["id"], "result": {"shellType": "bash"}})
    elif m == "session/request_permission":
        opts = o["params"].get("options", [])
        pick = next((x for x in opts if x.get("kind") == "allow_once"), opts[0] if opts else None)
        send({"jsonrpc": "2.0", "id": o["id"], "result": {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}})
    else: send({"jsonrpc": "2.0", "id": o["id"], "result": {}})

def pump(until_id=None, timeout=40, idle_exit=None):
    end = time.time() + timeout; last = time.time()
    while time.time() < end:
        try: raw = msgs.get(timeout=1)
        except queue.Empty:
            if idle_exit and time.time() - last > idle_exit: return None
            continue
        if raw is None: return None
        last = time.time()
        try: o = json.loads(raw)
        except Exception: continue
        record("agent->client", o)
        meth = o.get("method")
        if meth == "_kiro/system/notify": NOTIFY.append(o.get("params"))
        if meth == "session/update":
            u = (o.get("params") or {}).get("update") or {}
            k = u.get("sessionUpdate")
            if k == "session_info_update": k += ":" + str(u.get("kind"))
            KINDS[k] = KINDS.get(k, 0) + 1
        if meth and "id" in o: handle_server_req(o)
        elif "id" in o and until_id is not None and o["id"] == until_id: return o
    return None

print(f"######## LEG={LEG} bin={KIRO} pin={env.get('KIRO_KAS_SERVER_PATH','-')}")
iid = req("initialize", {"protocolVersion": 1, "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"},
                         "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}})
init = pump(iid, 60)
res = (init or {}).get("result") or {}
ac = res.get("agentCapabilities") or {}
print("== agentInfo:", json.dumps(res.get("agentInfo")))
print("== agentCapabilities keys:", sorted(ac.keys()))
print("== extensionMethods:", json.dumps((ac.get("_meta") or {}).get("extensionMethods"))[:600])
print("== sessionCapabilities:", json.dumps(ac.get("sessionCapabilities"))[:300])
print("== initialize._meta:", json.dumps(res.get("_meta"))[:400])

nid = req("session/new", {"cwd": CWD, "mcpServers": []})
new = pump(nid, 90); nres = (new or {}).get("result") or {}
sid = nres.get("sessionId")
print("== session/new keys:", sorted(nres.keys()))
print("== session/new _meta:", json.dumps(nres.get("_meta"))[:600])
for co in nres.get("configOptions") or []:
    opts = co.get("options") or []
    print(f"== configOption {co.get('id')!r} current={co.get('currentValue')!r} n={len(opts)}")
    if co.get("id") == "model":
        for o in opts: print("    ", o.get("value"), "|", o.get("name"), "|", (o.get("description") or "")[:60])
pump(timeout=8, idle_exit=4)
print("== pushes after session/new:", json.dumps(KINDS), "| server reqs:", json.dumps(SERVER_REQS))

t0 = time.time()
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
resp = pump(pid, 240)
r = (resp or {})
print("== prompt:", "ERROR " + json.dumps(r.get("error"))[:200] if "error" in r else "RESULT " + json.dumps(r.get("result"))[:200], f"({time.time()-t0:.1f}s)")
pump(timeout=6, idle_exit=3)
print("== session_info_update kinds / updates:", json.dumps(KINDS))
print("== server reqs:", json.dumps(SERVER_REQS))
print("== _kiro/system/notify:", len(NOTIFY), json.dumps(NOTIFY)[:400])
with open(os.path.join(OUTDIR, f"kas-baseline-{LEG}-2.21.0-verdict.json"), "w") as fh:
    json.dump({"agentInfo": res.get("agentInfo"), "agentCapabilities": ac, "init_meta": res.get("_meta"),
               "session_new_keys": sorted(nres.keys()), "session_new_meta": nres.get("_meta"),
               "configOptions": nres.get("configOptions"), "kinds": KINDS, "server_reqs": SERVER_REQS,
               "notify": NOTIFY, "prompt": r.get("result") or r.get("error")}, fh, indent=2)
trace.close(); proc.terminate()
