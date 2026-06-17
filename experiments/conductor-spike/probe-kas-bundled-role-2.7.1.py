#!/usr/bin/env python3
"""
Verify a BUNDLED agent (functional_task_alignment) works as an OrchestrateSubAgent stage `role`.

Enables subagentOrchestration (initialize) + fta (session/new) and asks the brain to run a
2-stage crew where the validate stage's role is `functional_task_alignment`, over a real
buggy change (is_even returns True for ODD). Confirms: the orchestrate tool fires, its
pipeline.stages includes role="functional_task_alignment", that stage runs as a child
agent-subtask, and no "unknown role"/validation error occurs.

In-process I/O (no fs/terminal caps). Token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO=os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH_DB=os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG=os.path.join(os.path.dirname(__file__),"logs","probe-kas-bundled-role-2.7.1.log")
os.makedirs(os.path.dirname(LOG),exist_ok=True)
logf=open(LOG,"w")
def log(*a):
    s=" ".join(str(x) for x in a); print(s); logf.write(s+"\n"); logf.flush()

CWD=tempfile.mkdtemp(prefix="kas-role-")
def sh(c): subprocess.run(c,cwd=CWD,shell=True,check=False,capture_output=True)
sh("git init -q -b main && git config user.email p@p && git config user.name p")
pathlib.Path(CWD,"README.md").write_text("# probe\n"); sh("git add -A && git commit -qm baseline")
pathlib.Path(CWD,"math_utils.py").write_text(
    "def is_even(n):\n    # Task: return True iff n is even.\n    return n % 2 == 1  # BUG: True for ODD\n")
log(f"# CWD={CWD}")

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

TOOLS=[]; AGENT=[]; PIPELINE=[]; ROLES=set(); ERRORS=[]
def handle(o):
    m=o.get("method"); rid=o.get("id"); p=o.get("params",{}) or {}
    if m=="_kiro/auth/getAccessToken":
        tok=read_token()
        if tok is None: log("[WARN] NO KIRO TOKEN in auth store — replying empty; any INCONCLUSIVE/feature-absent verdict below is a SETUP FAILURE, not a finding (run `kiro-cli whoami`)")
        reply(rid, tok or {})
    elif m=="_kiro/terminal/shell_type": reply(rid,{"shellType":"bash"})
    elif m=="session/request_permission":
        opts=p.get("options",[]); pick=next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()),opts[0] if opts else None)
        reply(rid,{"outcome":{"outcome":"selected","optionId":pick["optionId"]}} if pick else {"outcome":{"outcome":"cancelled"}})
    elif rid is not None: reply(rid,{})

def pump(until_id,timeout=300):
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
                v=u.get("sessionUpdate")
                if v=="agent_message_chunk": AGENT.append(u.get("content",{}).get("text",""))
                elif v in ("tool_call","tool_call_update"):
                    TOOLS.append((v,u.get("title") or u.get("toolCallId"),u.get("status")))
                    ri=u.get("rawInput") or {}
                    for st in (ri.get("stages") or []):
                        if st.get("role"): ROLES.add(st["role"])
                    pl=((u.get("_meta") or {}).get("kiro") or {}).get("pipeline")
                    if pl and not PIPELINE: PIPELINE.extend(pl.get("stages",[]))
                    nm=ri.get("name") or ri.get("role")
                    if nm: ROLES.add(nm)
        elif "id" in o:
            if "error" in o: ERRORS.append(o["error"])
            if o["id"]==until_id: return o
    return None

SETTINGS={"kiro":{"settings":{"subagentOrchestration":{"enabled":True},"fta":{"enabled":True}}}}
req("initialize",{"protocolVersion":1,"clientCapabilities":{"_meta":SETTINGS}})
pump(11,30)
nid=req("session/new",{"cwd":CWD,"mcpServers":[],"_meta":SETTINGS}); nr=pump(nid,40)
assert nr and "result" in nr
sid=nr["result"]["sessionId"]; log("sessionId:",sid)
PROMPT=("Use the orchestrate_subagent DAG tool to run a TWO-stage pipeline over this repo: "
        "stage 'describe' with role 'general-task-execution' — state exactly what is_even(n) in "
        "math_utils.py currently returns for n=2 and n=3; "
        "stage 'validate' with role 'functional_task_alignment' (depends_on describe) — validate whether "
        "math_utils.py's is_even(n) fulfills the spec 'returns True iff n is even', citing the on-disk code. "
        "Use those exact role names. Report each stage's outcome.")
pid=req("session/prompt",{"sessionId":sid,"prompt":[{"type":"text","text":PROMPT}]})
pump(pid,280)

log("\n===== tool calls =====")
for k,t,st in TOOLS: log(f"  [{k}] {t} -> {st}")
log("\n===== pipeline stages (role per stage, from _meta.kiro.pipeline) =====")
for st in PIPELINE: log(f"  {st.get('name')}: role={st.get('role')} dependsOn={st.get('dependsOn')} status={st.get('status')}")
log("\n===== distinct roles seen across orchestrate rawInput/pipeline =====", sorted(ROLES))
log("\n===== JSON-RPC errors =====", [json.dumps(e)[:160] for e in ERRORS] or "(none)")
log("\n===== agent final report (head) =====")
log("  "+("".join(AGENT)[:700] or "(nothing)"))
log("\n===== VERDICT =====")
fta_as_role = any((st.get("role")=="functional_task_alignment") for st in PIPELINE) or ("functional_task_alignment" in ROLES)
fta_ran = any("functional_task_alignment" in (t[1] or "") or t[1]=="Sub-agent: functional_task_alignment" for t in TOOLS) \
          or any(st.get("role")=="functional_task_alignment" and st.get("status") in ("completed","running","in_progress") for st in PIPELINE)
log(f"  functional_task_alignment used as a stage role: {fta_as_role}")
log(f"  that stage actually ran: {fta_ran}")
log(f"  unknown-role/validation error: {bool(ERRORS)}")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
