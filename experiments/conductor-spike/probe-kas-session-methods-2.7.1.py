#!/usr/bin/env python3
"""
Probe: KAS (2.7.1) _kiro/session/* + session/list + session/fork method contracts.

Advertised extensionMethods include _kiro/session/{context,compact,export,history};
the bundle also handles _kiro/session/{list,delete,rename}, session/list, session/fork.
This runs ONE tiny turn (so history/export/context have content), then calls each
read-ish method and dumps the response shape. Mutating ones (compact, fork) are called
on the throwaway session; delete/rename are NOT exercised (destructive). Token self-sourced.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
CWD = tempfile.mkdtemp(prefix="kas-sessmeth-")
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-session-methods-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s+"\n"); logf.flush()

def read_token():
    c = sqlite3.connect(AUTH_DB)
    try: row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally: c.close()
    if not row: return None
    v = row[0]; v = v.decode("utf-8","replace") if isinstance(v,(bytes,bytearray)) else v
    d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"],
            "profileArn": d.get("profile_arn"), "provider": d.get("provider")}

proc = subprocess.Popen([KIRO,"acp","--agent-engine","kas"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()

_id=[10]
def req(method, params):
    _id[0]+=1
    PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":method,"params":params})+"\n"); PIN.flush()
    return _id[0]
def reply(rid,res):
    PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); PIN.flush()

def pump(until_id, timeout=120):
    end=time.time()+timeout
    while time.time()<end:
        try: raw=msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return None
        try: o=json.loads(raw)
        except: continue
        if "method" in o and "id" in o:
            m=o["method"]
            if m=="_kiro/auth/getAccessToken":
                tok=read_token()
                if tok is None: log("[WARN] NO KIRO TOKEN in auth store — replying empty; any INCONCLUSIVE/feature-absent verdict below is a SETUP FAILURE, not a finding (run `kiro-cli whoami`)")
                reply(o["id"], tok or {})
            elif m=="session/request_permission":
                opts=o["params"].get("options",[])
                pick=next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()), opts[0] if opts else None)
                reply(o["id"], {"outcome":{"outcome":"selected","optionId":pick["optionId"]}} if pick else {"outcome":{"outcome":"cancelled"}})
            else: reply(o["id"], {})
        elif "method" not in o and "id" in o and o["id"]==until_id:
            return o
    return None

req("initialize", {"protocolVersion":1,"clientCapabilities":{}})
pump(11)
nid=req("session/new", {"cwd":CWD,"mcpServers":[]})
nresp=pump(nid)
assert nresp and "result" in nresp, "session/new failed"
sid=nresp["result"]["sessionId"]
log("sessionId:", sid)

# one tiny turn so history/context/export have content
log("\n# running one tiny turn to populate history...")
pid=req("session/prompt", {"sessionId":sid,"prompt":[{"type":"text","text":"Reply with exactly: ok. Do not use any tools."}]})
pump(pid, timeout=120)
log("# turn done\n")

def call(method, params, note=""):
    rid=req(method, params)
    r=pump(rid, timeout=40)
    log(f"\n==== {method} {json.dumps(params)} {note} ====")
    if r is None: log("  (no response/timeout)"); return None
    if "error" in r: log("  ERROR:", json.dumps(r["error"])[:400]); return None
    res=r.get("result")
    blob=json.dumps(res)
    log(f"  result keys: {list(res.keys()) if isinstance(res,dict) else type(res).__name__}")
    log("  result:", blob[:900])
    return res

call("_kiro/session/history", {"sessionId":sid})
call("_kiro/session/context", {"sessionId":sid})
call("_kiro/session/export",  {"sessionId":sid})
call("session/list", {})
call("_kiro/session/list", {})
call("session/fork", {"sessionId":sid}, note="(creates a forked session)")
call("_kiro/session/compact", {"sessionId":sid}, note="(mutating)")
# bonus: undocumented-but-handled methods
call("_kiro/account/getUsage", {})
call("_kiro/permissions/list", {"sessionId":sid})

PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
