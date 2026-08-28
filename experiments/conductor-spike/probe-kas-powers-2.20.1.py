#!/usr/bin/env python3
"""
KAS 0.54.3 (kiro-cli 2.20.1) `_kiro/powers/*` live probe.

kiro-cli 2.20.1 shipped a `/powers` TUI command; the method census shows the
WIRE surface (_kiro/powers/{list,refresh,items_changed}) was already present in
0.52.1 — this probe pins down the response shape and the change notification.

Static recon:
  "_kiro/powers/list":   () => this.handlePowersList()        # NO params
  handlePowersList()  -> { powers: [...], errors: [...] }
  install root:          ~/.kiro/powers/installed/<name>/mcp.json
  a "power" is an installable MCP-server bundle, mentionable as @powers

Legs: (1) list on an empty throwaway HOME, (2) plant a fixture power,
(3) _kiro/powers/refresh, (4) list again + watch for items_changed.
"""
import json, os, re, subprocess, threading, queue, time, tempfile, sqlite3

OUTDIR = os.environ.get("PROBE_OUT", ".")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
TRACE = os.path.join(OUTDIR, "kas-powers-2.20.1.jsonl")

def profile_arn():
    arn = os.environ.get("KIRO_PROFILE_ARN")
    if arn: return arn
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()
FAKE_HOME = tempfile.mkdtemp(prefix="kas-pw-home-")
CWD = tempfile.mkdtemp(prefix="kas-pw-cwd-")
env = dict(os.environ)
env["HOME"] = FAKE_HOME
env["XDG_DATA_HOME"] = os.path.expanduser("~/.local/share")

# Seed the throwaway HOME with the real powers tree so list() returns the real
# item schema (powers are registry-tracked via installed.json, not dir presence).
import shutil
_real = os.path.expanduser("~/.kiro/powers")
if os.environ.get("SEED_POWERS") == "1" and os.path.isdir(_real):
    os.makedirs(os.path.join(FAKE_HOME, ".kiro"), exist_ok=True)
    shutil.copytree(_real, os.path.join(FAKE_HOME, ".kiro", "powers"), dirs_exist_ok=True)
    print("== seeded real powers tree into throwaway HOME")

def read_token():
    c = sqlite3.connect(AUTH_DB)
    try: row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
    finally: c.close()
    if not row: return None
    v = row[0]; v = v.decode() if isinstance(v,(bytes,bytearray)) else v
    d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": PROFILE_ARN}

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open(os.path.join(OUTDIR, "kas-powers-2.20.1-stderr.log"), "w"),
                        text=True, bufsize=1)
PIN_, POUT = proc.stdin, proc.stdout
msgs = queue.Queue(); trace = open(TRACE, "w")
def record(d,o): trace.write(json.dumps({"ts":time.time(),"dir":d,"msg":o})+"\n"); trace.flush()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id=[10]
def send(o): record("client->agent",o); PIN_.write(json.dumps(o)+"\n"); PIN_.flush()
def req(m,p=None):
    _id[0]+=1
    msg={"jsonrpc":"2.0","id":_id[0],"method":m}
    if p is not None: msg["params"]=p
    send(msg); return _id[0]

CHANGED=[]
def handle_server_req(o):
    m=o["method"]
    if m=="_kiro/auth/getAccessToken": send({"jsonrpc":"2.0","id":o["id"],"result":read_token() or {}})
    elif m=="_kiro/terminal/shell_type": send({"jsonrpc":"2.0","id":o["id"],"result":{"shellType":"bash"}})
    else: send({"jsonrpc":"2.0","id":o["id"],"result":{}})

def pump(until_id=None,timeout=40,idle_exit=None):
    end=time.time()+timeout; last=time.time()
    while time.time()<end:
        try: raw=msgs.get(timeout=1)
        except queue.Empty:
            if idle_exit and time.time()-last>idle_exit: return None
            continue
        if raw is None: return None
        last=time.time()
        try: o=json.loads(raw)
        except Exception: continue
        record("agent->client",o)
        meth=o.get("method")
        if meth=="_kiro/powers/items_changed":
            CHANGED.append(o.get("params"))
            print("   >>> items_changed:", json.dumps(o.get("params"))[:300])
        if meth and "id" in o: handle_server_req(o)
        elif "id" in o and until_id is not None and o["id"]==until_id: return o
    return None

iid=req("initialize",{"protocolVersion":1,"clientInfo":{"name":"cyril-audit-probe","version":"0.0.1"},
                      "clientCapabilities":{"fs":{"readTextFile":False,"writeTextFile":False}}})
init=pump(iid,60)
ext=(((init or {}).get("result") or {}).get("agentCapabilities") or {}).get("_meta",{})
print("== extensionMethods advertised:", json.dumps(ext)[:900])

nid=req("session/new",{"cwd":CWD,"mcpServers":[]})
new=pump(nid,60); sid=((new or {}).get("result") or {}).get("sessionId")
print("== sessionId:",(sid or "")[:16])
pump(timeout=5,idle_exit=3)

lid=req("_kiro/powers/list")
lres=pump(lid,30)
print("== powers/list (empty HOME):", json.dumps((lres or {}).get("result") or (lres or {}).get("error"))[:600])

# plant a fixture power
pdir=os.path.join(FAKE_HOME,".kiro","powers","installed","audit-fixture")
os.makedirs(pdir,exist_ok=True)
with open(os.path.join(pdir,"mcp.json"),"w") as fh:
    json.dump({"mcpServers":{"audit-fixture":{"command":"true","args":[],"disabled":False}}},fh)
print("== planted fixture at",pdir)

rid=req("_kiro/powers/refresh")
rres=pump(rid,30)
print("== powers/refresh:", json.dumps((rres or {}).get("result") or (rres or {}).get("error"))[:400])
pump(timeout=8,idle_exit=4)

l2=req("_kiro/powers/list")
l2res=pump(l2,30)
print("== powers/list (after fixture):", json.dumps((l2res or {}).get("result") or (l2res or {}).get("error"))[:900])

print("== items_changed count:",len(CHANGED))
with open(os.path.join(OUTDIR,"kas-powers-2.20.1-verdict.json"),"w") as fh:
    json.dump({"list_empty":(lres or {}).get("result"),
               "refresh":(rres or {}).get("result") or (rres or {}).get("error"),
               "list_after":(l2res or {}).get("result"),
               "items_changed":CHANGED}, fh, indent=2)
trace.close(); proc.terminate()
