#!/usr/bin/env python3
"""
Confirm a crew stage runs as its `role` agent's PERMISSIONS (and carries a model override).

Two workspace-local custom agents (in <CWD>/.kiro/agents/, auto-cleaned):
  probe-ro : tools include fs_write, but permissions DENY fs_write (** ) ; model=haiku
  probe-rw : fs_write allowed
A 2-stage OrchestrateSubAgent crew asks probe-ro to write ro.txt and probe-rw to write rw.txt.
Expectation if per-agent permissions are respected: rw.txt write reaches the host (fs/write_text_file
callback + file on disk); ro.txt does NOT (denied by probe-ro's policy).

fs advertised (so writes surface as host callbacks); orchestration enabled via initialize _meta.
Token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO=os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
CWD=tempfile.mkdtemp(prefix="kas-scope-")
AUTH_DB=os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG=os.path.join(os.path.dirname(__file__),"logs","probe-kas-agent-scope-2.7.1.log")
os.makedirs(os.path.dirname(LOG),exist_ok=True)
logf=open(LOG,"w")
def log(*a):
    s=" ".join(str(x) for x in a); print(s); logf.write(s+"\n"); logf.flush()

# --- workspace-local custom agents ---
agents_dir=pathlib.Path(CWD,".kiro","agents"); agents_dir.mkdir(parents=True)
(agents_dir/"probe-ro.json").write_text(json.dumps({
    "name":"probe-ro","description":"read-only probe (fs_write denied by policy)",
    "prompt":"You are a probe sub-agent. Do exactly what the task says.",
    "tools":["fs_read","fs_write"],"model":"claude-haiku-4.5",
    "permissions":{"rules":[{"capability":"fs_write","match":["**"],"effect":"deny"}]}}))
(agents_dir/"probe-rw.json").write_text(json.dumps({
    "name":"probe-rw","description":"read-write probe",
    "prompt":"You are a probe sub-agent. Do exactly what the task says.",
    "tools":["fs_read","fs_write"]}))
log(f"# CWD={CWD}\n# agents: probe-ro (fs_write DENY, model=haiku), probe-rw (fs_write allow)")

def read_token():
    c=sqlite3.connect(AUTH_DB)
    try: row=c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally: c.close()
    if not row: return None
    v=row[0]; v=v.decode() if isinstance(v,(bytes,bytearray)) else v; d=json.loads(v)
    return {"accessToken":d["access_token"],"expiresAt":d["expires_at"],"profileArn":d.get("profile_arn"),"provider":d.get("provider")}

proc=subprocess.Popen([KIRO,"acp","--agent-engine","kas"],cwd=CWD,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,bufsize=1)
assert proc.stdin and proc.stdout
PIN,POUT=proc.stdin,proc.stdout
msgs=queue.Queue()
threading.Thread(target=lambda:([msgs.put(l.strip()) for l in POUT if l.strip()],msgs.put(None)),daemon=True).start()
_id=[10]
def req(m,p):
    _id[0]+=1; PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":m,"params":p})+"\n"); PIN.flush(); return _id[0]
def reply(rid,res): PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); PIN.flush()

WRITES=[]; PERMS=[]; TOOLS=[]; AGENT=[]
def handle(o):
    m=o.get("method"); rid=o.get("id"); p=o.get("params",{}) or {}
    if m=="_kiro/auth/getAccessToken":
        tok=read_token()
        if tok is None: log("[WARN] NO KIRO TOKEN in auth store — replying empty; any INCONCLUSIVE/feature-absent verdict below is a SETUP FAILURE, not a finding (run `kiro-cli whoami`)")
        reply(rid, tok or {})
    elif m=="_kiro/terminal/shell_type": reply(rid,{"shellType":"bash"})
    elif m=="session/request_permission":
        PERMS.append((p.get("toolCall",{}).get("title"), p.get("_meta",{}).get("kiro",{}).get("consent",{})))
        opts=p.get("options",[]); pick=next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()),opts[0] if opts else None)
        reply(rid,{"outcome":{"outcome":"selected","optionId":pick["optionId"]}} if pick else {"outcome":{"outcome":"cancelled"}})
    elif m and m.endswith("/read_text_file"):
        try: c=pathlib.Path(p.get("path","")).read_text()
        except Exception: c=""
        reply(rid,{"content":c})
    elif m and m.endswith("/write_text_file"):
        WRITES.append(p.get("path",""))
        try: pathlib.Path(p.get("path","")).write_text(p.get("content",""))
        except Exception: pass
        reply(rid,{})
    elif rid is not None: reply(rid,{})

def pump(until_id,timeout=200):
    end=time.time()+timeout
    while time.time()<end:
        try: raw=msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return None
        try: o=json.loads(raw)
        except: continue
        if "method" in o and "id" in o: handle(o)
        elif "method" in o:
            u=o.get("params",{}).get("update",{}) if isinstance(o.get("params"),dict) else {}
            if isinstance(u,dict):
                if u.get("sessionUpdate")=="agent_message_chunk": AGENT.append(u.get("content",{}).get("text",""))
                elif u.get("sessionUpdate") in ("tool_call","tool_call_update"):
                    TOOLS.append((u.get("sessionUpdate"),u.get("title") or u.get("toolCallId"),u.get("status")))
        elif "id" in o and o["id"]==until_id: return o
    return None

req("initialize",{"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":True,"writeTextFile":True},
     "_meta":{"kiro":{"settings":{"subagentOrchestration":{"enabled":True}}}}}})
pump(11,30)
nid=req("session/new",{"cwd":CWD,"mcpServers":[]}); nr=pump(nid,40); sid=nr["result"]["sessionId"]
log("sessionId:",sid)
PROMPT=("Use the orchestrate_subagent DAG tool to run TWO stages: "
        "stage 'locked' with role 'probe-ro' whose task is to create a file named ro.txt containing the text x; "
        "stage 'open' with role 'probe-rw' whose task is to create a file named rw.txt containing the text y. "
        "Run them; report whether each stage succeeded.")
pid=req("session/prompt",{"sessionId":sid,"prompt":[{"type":"text","text":PROMPT}]})
pump(pid,220)

log("\n===== tool calls =====")
for k,t,st in TOOLS: log(f"  [{k}] {t} -> {st}")
log("\n===== fs/write_text_file callbacks (paths host was asked to write) =====")
for w in WRITES: log("  WRITE:", w)
log("\n===== permission requests =====")
for t,c in PERMS: log("  PERM:", t, "| consent:", json.dumps(c)[:160])
log("\n===== files on disk in workspace =====")
for fn in ("ro.txt","rw.txt"): log(f"  {fn}: exists={pathlib.Path(CWD,fn).exists()}")
log("\n===== agent said =====")
log("  "+("".join(AGENT)[:600] or "(nothing)"))
log("\n===== VERDICT =====")
ro_written = any(p.endswith("ro.txt") for p in WRITES) or pathlib.Path(CWD,"ro.txt").exists()
rw_written = any(p.endswith("rw.txt") for p in WRITES) or pathlib.Path(CWD,"rw.txt").exists()
log(f"  probe-ro wrote ro.txt: {ro_written}  (expected False if per-agent deny is enforced)")
log(f"  probe-rw wrote rw.txt: {rw_written}  (expected True)")
log("  => per-agent permissions RESPECTED" if (rw_written and not ro_written)
    else "  => INCONCLUSIVE / not as expected (inspect tool calls + agent text above)")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
