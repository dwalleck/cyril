#!/usr/bin/env python3
"""
End-to-end probe: enable functional_task_alignment (fta) and run it as a validator
over a real, deliberately-buggy change.

Setup: a temp git repo with a baseline commit, then an uncommitted math_utils.py whose
is_even(n) returns True for ODD numbers (wrong). The stated task is "is_even returns True
iff n is even." We enable fta via initialize _meta.kiro.settings.fta={enabled:true} and ask
the brain to invoke the fta validator (not re-implement). Expectation: fta is invoked as a
sub-agent, inspects the diff/on-disk state, and FLAGS the bug.

No fs/terminal capability advertised -> KAS does I/O in-process against the real cwd, so
git/file results are real. Token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO=os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH_DB=os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG=os.path.join(os.path.dirname(__file__),"logs","probe-kas-fta-2.7.1.log")
os.makedirs(os.path.dirname(LOG),exist_ok=True)
logf=open(LOG,"w")
def log(*a):
    s=" ".join(str(x) for x in a); print(s); logf.write(s+"\n"); logf.flush()

# --- real git repo with a buggy uncommitted change ---
CWD=tempfile.mkdtemp(prefix="kas-fta-")
def sh(c): subprocess.run(c, cwd=CWD, shell=True, check=False, capture_output=True)
sh("git init -q && git config user.email p@p && git config user.name p")
pathlib.Path(CWD,"README.md").write_text("# probe\n")
sh("git add -A && git commit -qm baseline")
pathlib.Path(CWD,"math_utils.py").write_text(
    "def is_even(n):\n"
    "    # Task: return True iff n is even.\n"
    "    return n % 2 == 1  # BUG: this is True for ODD numbers\n")
log(f"# CWD={CWD}  (baseline committed; buggy math_utils.py uncommitted)")

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

TOOLS=[]; AGENT=[]; SUBAGENTS=[]
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
                    ri=u.get("rawInput") or {}
                    TOOLS.append((v,u.get("title") or u.get("toolCallId"),u.get("status")))
                    nm=ri.get("name") or ri.get("role")
                    if nm and "task_align" in str(nm).lower(): SUBAGENTS.append((nm, str(ri.get("prompt",""))[:120]))
                    if u.get("title")=="Subagent Response" and "task_align" in json.dumps(u).lower():
                        SUBAGENTS.append(("RESPONSE", json.dumps(u.get("rawInput"))[:300]))
        elif "id" in o and o["id"]==until_id: return o
    return None

FTA_SETTINGS={"kiro":{"settings":{"fta":{"enabled":True}}}}
req("initialize",{"protocolVersion":1,"clientCapabilities":{"_meta":FTA_SETTINGS}})
pump(11,30)
# resolveFta reads from session/new params._meta.kiro.settings -> set it HERE
nid=req("session/new",{"cwd":CWD,"mcpServers":[],"_meta":FTA_SETTINGS}); nr=pump(nid,40)
assert nr and "result" in nr, "session/new failed"
sid=nr["result"]["sessionId"]
# confirm fta enabled in the session meta
meta=(nr["result"].get("_meta") or {}).get("kiro") or {}
log("sessionId:",sid,"| ftaEnabled:",meta.get("ftaEnabled"),"| semanticReviewEnabled:",meta.get("semanticReviewEnabled"))
PROMPT=("A previous agent was given this task: 'In math_utils.py, implement is_even(n) that returns "
        "True if n is even and False otherwise.' It reported the task complete. Do NOT re-implement it "
        "yourself. Instead, invoke the functional_task_alignment sub-agent (the validator) to independently "
        "verify whether the code on disk actually fulfills that task, and report its verdict and findings.")
pid=req("session/prompt",{"sessionId":sid,"prompt":[{"type":"text","text":PROMPT}]})
pump(pid,280)

log("\n===== tool calls =====")
for k,t,st in TOOLS: log(f"  [{k}] {t} -> {st}")
log("\n===== fta subagent activity =====")
for x in SUBAGENTS: log("  ", x)
log("\n===== agent final report =====")
log("  "+("".join(AGENT)[:900] or "(nothing)"))
log("\n===== VERDICT =====")
fta_invoked = any("task_align" in (t[1] or "").lower() or t[0]=="tool_call" and "Sub-agent" in (t[1] or "") for t in TOOLS) or bool(SUBAGENTS)
caught = any(w in "".join(AGENT).lower() for w in ("odd","bug","incorrect","does not","not even","fails","wrong"))
log(f"  fta invoked as sub-agent: {fta_invoked}")
log(f"  bug surfaced in report: {caught}")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
