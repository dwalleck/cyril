#!/usr/bin/env python3
"""
Does the v2 (Rust) engine's `/code init` CREATE .kiro/settings/lsp.json?
(User recalls it "used to create a default file"; current v3/KAS does NOT — source-verified.
 tui.js never writes it either. Remaining hypothesis: the v2 Rust backend wrote it.)

Spawns `kiro-cli acp` (v2 default), session/new in a fresh temp workspace containing a
Cargo.toml + src/main.rs (so a language is detected), sends the same wire call cyril sends
for `/code init` (kiro.dev/commands/execute -> wire `_kiro.dev/commands/execute`, params
{sessionId, command:{command:"code", args:{value:"init"}}}), captures the response, and
checks whether <cwd>/.kiro/settings/lsp.json exists afterward. Auto-approves any permission.
v2 self-authenticates (no _kiro/auth/getAccessToken callback).
"""
import json, os, subprocess, threading, queue, time, tempfile, shutil

KIRO = os.environ.get("KIRO_BIN") or shutil.which("kiro-cli")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-v2-code-init.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s=" ".join(str(x) for x in a); print(s); logf.write(s+"\n"); logf.flush()
if not KIRO: log("no kiro-cli"); raise SystemExit(1)

CWD = tempfile.mkdtemp(prefix="v2-code-")
os.makedirs(os.path.join(CWD,"src"))
open(os.path.join(CWD,"Cargo.toml"),"w").write("[package]\nname=\"x\"\nversion=\"0.1.0\"\nedition=\"2021\"\n")
open(os.path.join(CWD,"src","main.rs"),"w").write("fn main(){println!(\"hi\");}\n")
LSP = os.path.join(CWD,".kiro","settings","lsp.json")
log(f"# binary: {KIRO}\n# cwd: {CWD}\n# pre-existing lsp.json: {os.path.exists(LSP)}")

proc = subprocess.Popen([KIRO,"acp"], cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN,POUT=proc.stdin,proc.stdout
msgs=queue.Queue()
threading.Thread(target=lambda:([msgs.put(l.strip()) for l in POUT if l.strip()],msgs.put(None)),daemon=True).start()
_id=[0]
def req(m,p):
    _id[0]+=1; PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":m,"params":p})+"\n"); PIN.flush(); return _id[0]
def reply(rid,res):
    PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); PIN.flush()

available=[]
def handle(o):
    m=o.get("method"); rid=o.get("id"); p=o.get("params",{}) or {}
    if rid is not None:
        if m=="session/request_permission":
            opts=p.get("options",[]); pick=next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()),opts[0] if opts else None)
            log(f"  [permission requested: {p.get('toolCall',{}).get('title') or p}]")
            reply(rid,{"outcome":{"outcome":"selected","optionId":pick["optionId"]}} if pick else {"outcome":{"outcome":"cancelled"}})
        else:
            reply(rid,{})
        return
    if m and ("commands/available" in m):
        cmds=p.get("commands") or p.get("availableCommands") or (p if isinstance(p,list) else [])
        for c in (cmds or []):
            nm=c.get("name") if isinstance(c,dict) else c
            if nm: available.append(nm)

def pump(until,to=60):
    end=time.time()+to
    while time.time()<end:
        try: raw=msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return None
        try: o=json.loads(raw)
        except: continue
        if "method" in o:
            handle(o)
            if o.get("id")==until and "result" in o: return o
        elif "id" in o and o["id"]==until: return o
    return "timeout"

req("initialize",{"protocolVersion":1,"clientCapabilities":{}})
pump(1,20)
nr=pump(req("session/new",{"cwd":CWD,"mcpServers":[]}),40)
if not (isinstance(nr,dict) and "result" in nr):
    log(f"session/new FAILED: {nr}"); proc.terminate(); raise SystemExit(1)
sid=nr["result"]["sessionId"]
pump(-1,3)  # drain commands/available
log(f"# 'code' in available commands: {'code' in available or '/code' in available}  (sample: {available[:12]})")

# send /code init exactly as cyril does
for method in ("_kiro.dev/commands/execute","kiro.dev/commands/execute"):
    params={"sessionId":sid,"command":{"command":"code","args":{"value":"init"}}}
    r=pump(req(method,params),60)
    if isinstance(r,dict) and "result" in r:
        log(f"\n[{method}] RESPONSE: {json.dumps(r['result'])[:600]}")
        break
    elif isinstance(r,dict) and "error" in r:
        log(f"[{method}] ERROR: {json.dumps(r['error'])[:200]}")
    else:
        log(f"[{method}] -> {r}")

time.sleep(1)
log(f"\n# AFTER /code init -> lsp.json exists: {os.path.exists(LSP)}")
if os.path.exists(LSP):
    log("  CONTENT:\n"+open(LSP).read()[:800])
# also list what .kiro got created
for root,_,files in os.walk(os.path.join(CWD,".kiro")):
    for fn in files: log(f"  .kiro file: {os.path.join(root,fn).replace(CWD+'/','')}")
PIN.close(); proc.terminate()
try: proc.wait(timeout=5)
except: proc.kill()
log(f"\n# log: {LOG}")
