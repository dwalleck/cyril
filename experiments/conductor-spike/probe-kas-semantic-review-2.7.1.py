#!/usr/bin/env python3
"""
End-to-end probe: run the semantic_reviewer agent over a real change.

Setup: a git repo with a baseline commit (app.py: a trivial health()), then an UNCOMMITTED
change that adds a config loader with planted, reviewable concerns:
  - eval() on file contents  -> arbitrary code execution (security)
  - open() with no close/`with` -> file-handle leak (resource)
  - no error handling
  - handle_request passes a user-controlled path into it (trust boundary)
We run the session in `semantic_reviewer` mode (semanticReview defaults on; set in session/new
_meta to be safe) and ask it to review the uncommitted changes. Expectation: it fetches the
diff via git, produces a behavioral review, and flags the security/resource concerns.

No fs/terminal caps -> in-process I/O against the real repo (real git diff / file reads).
Token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO=os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH_DB=os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG=os.path.join(os.path.dirname(__file__),"logs","probe-kas-semantic-review-2.7.1.log")
os.makedirs(os.path.dirname(LOG),exist_ok=True)
logf=open(LOG,"w")
def log(*a):
    s=" ".join(str(x) for x in a); print(s); logf.write(s+"\n"); logf.flush()

CWD=tempfile.mkdtemp(prefix="kas-review-")
def sh(c): subprocess.run(c, cwd=CWD, shell=True, check=False, capture_output=True)
sh("git init -q -b main && git config user.email p@p && git config user.name p")
pathlib.Path(CWD,"app.py").write_text("def health():\n    return \"ok\"\n")
sh("git add -A && git commit -qm baseline")
# uncommitted change with reviewable concerns
pathlib.Path(CWD,"app.py").write_text(
    "def health():\n    return \"ok\"\n\n"
    "def load_settings(path):\n"
    "    raw = open(path).read()        # file never closed\n"
    "    return eval(raw)               # arbitrary code execution\n\n"
    "def handle_request(req):\n"
    "    # apply user-supplied settings file\n"
    "    return load_settings(req[\"settings_path\"])\n")
log(f"# CWD={CWD}  (baseline on main; uncommitted app.py change: eval/open-leak/no-error-handling)")

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

TOOLS=[]; AGENT=[]
def handle(o):
    m=o.get("method"); rid=o.get("id"); p=o.get("params",{}) or {}
    if m=="_kiro/auth/getAccessToken": reply(rid, read_token() or {})
    elif m=="_kiro/terminal/shell_type": reply(rid,{"shellType":"bash"})
    elif m=="session/request_permission":
        opts=p.get("options",[]); pick=next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()),opts[0] if opts else None)
        reply(rid,{"outcome":{"outcome":"selected","optionId":pick["optionId"]}} if pick else {"outcome":{"outcome":"cancelled"}})
    elif rid is not None: reply(rid,{})

def pump(until_id,timeout=320):
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
        elif "id" in o and o["id"]==until_id: return o
    return None

META={"kiro":{"modeId":"semantic_reviewer","settings":{"semanticReview":{"enabled":True}}}}
req("initialize",{"protocolVersion":1,"clientCapabilities":{"_meta":META}})
pump(11,30)
nid=req("session/new",{"cwd":CWD,"mcpServers":[],"_meta":META}); nr=pump(nid,60)
assert nr and "result" in nr, "session/new failed"
sid=nr["result"]["sessionId"]
mk=(nr["result"].get("_meta") or {}).get("kiro") or {}
modes=[m.get("id") for m in (nr["result"].get("modes") or {}).get("availableModes",[])]
log("sessionId:",sid,"| currentMode:",(nr['result'].get('modes') or {}).get('currentModeId'),"| semantic_reviewer in modes:", "semantic_reviewer" in modes)
PROMPT=("Review the uncommitted changes in this repository (compare the working tree against the "
        "baseline on main). Produce your behavioral review and report your findings and verdict.")
pid=req("session/prompt",{"sessionId":sid,"prompt":[{"type":"text","text":PROMPT}]})
pump(pid,300)

log("\n===== tool calls (git/read/c2s/write) =====")
for k,t,st in TOOLS: log(f"  [{k}] {t} -> {st}")
review="".join(AGENT)
log("\n===== review output (head) =====")
log(review[:1600] or "(nothing)")
# did it write a review file?
sr=list(pathlib.Path(CWD,"semantic-review").glob("*.md")) if pathlib.Path(CWD,"semantic-review").exists() else []
log("\n===== review file written? =====", [str(x.relative_to(CWD)) for x in sr])
log("\n===== VERDICT =====")
rl=review.lower()
log("  flagged eval/code-execution:", any(w in rl for w in ("eval","arbitrary code","code execution","rce")))
log("  flagged resource/file leak:", any(w in rl for w in ("close","leak","resource","context manager","with open")))
log("  flagged security/trust boundary:", any(w in rl for w in ("security","trust boundary","injection","untrusted","user-supplied","user-controlled")))
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
