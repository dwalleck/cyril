#!/usr/bin/env python3
"""
Pin down the correct commands/options method for the cyril fix.

cyril sends generic `kiro.dev/commands/options` {command, sessionId, partial}.
Each command's commands/available meta carries `optionsMethod` (e.g.
`_kiro.dev/commands/model/options`). Test which actually works for `model`:
  1) kiro.dev/commands/options            (current cyril; expect 404)
  2) _kiro.dev/commands/options           (generic, prefixed)
  3) <the model command's optionsMethod>  (per-command, from meta)
v2 engine, self-auth.
"""
import json, os, subprocess, threading, queue, time, tempfile

KIRO=os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat"))
CWD=tempfile.mkdtemp(prefix="cmdopts-")
proc=subprocess.Popen([KIRO,"acp"], cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN,POUT=proc.stdin,proc.stdout
msgs=queue.Queue()
threading.Thread(target=lambda:([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)),daemon=True).start()
_id=[10]
def req(m,p):
    _id[0]+=1; PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":m,"params":p})+"\n"); PIN.flush(); return _id[0]
def reply(rid,res): PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); PIN.flush()
NOTIFS=[]
def pump(until_id, timeout=40):
    end=time.time()+timeout
    while time.time()<end:
        try: raw=msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return None
        try: o=json.loads(raw)
        except: continue
        if "method" in o and "id" in o: reply(o["id"], {})
        elif "method" in o: NOTIFS.append(o)
        elif "id" in o and o["id"]==until_id: return o
    return None

req("initialize", {"protocolVersion":1,"clientCapabilities":{}}); pump(11,30)
nid=req("session/new", {"cwd":CWD,"mcpServers":[]}); sid=pump(nid,40)["result"]["sessionId"]
# wait for populated commands/available
deadline=time.time()+60
def cmds():
    av=[n for n in NOTIFS if "commands/available" in n.get("method","")]
    return (av[-1]["params"].get("commands") if av else None) or []
while time.time()<deadline and not cmds():
    try: raw=msgs.get(timeout=2)
    except queue.Empty: continue
    if raw is None: break
    try: o=json.loads(raw)
    except: continue
    if "method" in o and "id" in o: reply(o["id"], {})
    elif "method" in o: NOTIFS.append(o)

model=next((c for c in cmds() if c.get("name") in ("/model","model")), None)
opt_method=(model or {}).get("meta",{}).get("optionsMethod")
print("model command meta.optionsMethod:", opt_method)

def test(method, params, label):
    rid=req(method, params); r=pump(rid,30)
    if r is None: print(f"  {label}: NO RESPONSE")
    elif "error" in r: print(f"  {label}: ERROR {json.dumps(r['error'])[:110]}")
    else: print(f"  {label}: OK -> {json.dumps(r.get('result'))[:200]}")

print("\n# options method tests for 'model':")
test("kiro.dev/commands/options",  {"command":"model","sessionId":sid,"partial":""}, "kiro.dev/commands/options (current cyril)")
test("_kiro.dev/commands/options", {"command":"model","sessionId":sid,"partial":""}, "_kiro.dev/commands/options (generic prefixed)")
if opt_method:
    test(opt_method, {"sessionId":sid,"partial":""}, f"{opt_method} (per-command, no command arg)")
    test(opt_method, {"command":"model","sessionId":sid,"partial":""}, f"{opt_method} (per-command, with command arg)")

PIN.close(); proc.terminate()
