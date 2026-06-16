#!/usr/bin/env python3
"""
KAS host-responsibility callback map (2.7.1).

Drives a KAS turn that writes a file, runs a shell command, deletes a file, and asks to
open a URL, while advertising BOTH fs and terminal client capability and implementing
best-effort responders for every server->client method KAS might call. Records the full
set of callbacks KAS actually invokes = what a host (cyril) must implement to drive KAS.

Responses are minimal/plausible (enough to map the calls, not to fully execute). Token
self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
CWD = tempfile.mkdtemp(prefix="kas-cb-")
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-callbacks-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf=open(LOG,"w")
def log(*a):
    s=" ".join(str(x) for x in a); print(s); logf.write(s+"\n"); logf.flush()

def read_token():
    c=sqlite3.connect(AUTH_DB)
    try: row=c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally: c.close()
    if not row: return None
    v=row[0]; v=v.decode() if isinstance(v,(bytes,bytearray)) else v
    d=json.loads(v)
    return {"accessToken":d["access_token"],"expiresAt":d["expires_at"],"profileArn":d.get("profile_arn"),"provider":d.get("provider")}

proc=subprocess.Popen([KIRO,"acp","--agent-engine","kas"], cwd=CWD, stdin=subprocess.PIPE,
                      stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN,POUT=proc.stdin,proc.stdout
msgs=queue.Queue()
threading.Thread(target=lambda:([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)),daemon=True).start()
_id=[10]
def req(m,p):
    _id[0]+=1; PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":m,"params":p})+"\n"); PIN.flush(); return _id[0]
def reply(rid,res): PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); PIN.flush()
def errreply(rid,code,msg): PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"error":{"code":code,"message":msg}})+"\n"); PIN.flush()

CALLS={}                  # method -> count
TERMS={}
AGENT=[]                  # agent_message_chunk text
TOOLS=[]                  # (kind, title, status) per tool call
def handle(o):
    m=o.get("method"); rid=o.get("id"); p=o.get("params",{}) or {}
    CALLS[m]=CALLS.get(m,0)+1
    # log first occurrence of each method in full, later ones compact
    if CALLS[m]==1:
        log(f"\n>>> [{m}]  (params: {json.dumps(p)[:300]})")
    if m=="_kiro/auth/getAccessToken":
        reply(rid, read_token() or {})
    elif m=="session/request_permission":
        opts=p.get("options",[]); pick=next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()),opts[0] if opts else None)
        reply(rid, {"outcome":{"outcome":"selected","optionId":pick["optionId"]}} if pick else {"outcome":{"outcome":"cancelled"}})
    elif m in ("fs/read_text_file",) or m.endswith("/read_text_file"):
        path=p.get("path","");
        try: content=pathlib.Path(path).read_text()
        except Exception: content=""
        reply(rid, {"content":content})
    elif m in ("fs/write_text_file",) or m.endswith("/write_text_file"):
        path=p.get("path","");
        try: pathlib.Path(path).write_text(p.get("content",""));
        except Exception: pass
        reply(rid, {})
    elif "fs/delete" in m:
        path=p.get("path","");
        try: pathlib.Path(path).unlink()
        except Exception: pass
        reply(rid, {})
    elif "fs/stat" in m:
        path=p.get("path",""); ex=pathlib.Path(path).exists()
        reply(rid, {"exists":ex,"isFile":pathlib.Path(path).is_file() if ex else False,"isDirectory":pathlib.Path(path).is_dir() if ex else False})
    elif m=="_kiro/terminal/shell_type":
        reply(rid, {"shellType":"bash"})
    elif m=="terminal/create":
        tid=f"term-{len(TERMS)+1}"; TERMS[tid]=p.get("command","")
        reply(rid, {"terminalId":tid})
    elif m=="terminal/output":
        reply(rid, {"output":"hello\n","truncated":False,"exitStatus":{"exitCode":0,"signal":None}})
    elif m=="terminal/wait_for_exit":
        reply(rid, {"exitStatus":{"exitCode":0,"signal":None}})
    elif m in ("terminal/release","terminal/kill"):
        reply(rid, {})
    elif m=="_kiro/openExternalUrl":
        reply(rid, {"opened":True})
    elif m=="_kiro/system/notify":
        reply(rid, {})
    elif m=="_kiro/userInput":
        reply(rid, {"cancelled":True})
    elif "method" in o and rid is not None:
        reply(rid, {})   # generic ack for anything else server->client

def pump(until_id, timeout=180):
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
            if isinstance(u,dict) and u.get("sessionUpdate")=="agent_message_chunk":
                AGENT.append(u.get("content",{}).get("text",""))
            elif isinstance(u,dict) and u.get("sessionUpdate") in ("tool_call","tool_call_update"):
                TOOLS.append((u.get("sessionUpdate"), u.get("title") or u.get("toolCallId"), u.get("status")))
        elif "id" in o and o["id"]==until_id: return o
    return None

CAPS={"fs":{"readTextFile":True,"writeTextFile":True},"terminal":True}
req("initialize", {"protocolVersion":1,"clientCapabilities":CAPS})
pump(11,30)
nid=req("session/new", {"cwd":CWD,"mcpServers":[]})
nr=pump(nid,40); sid=nr["result"]["sessionId"]
log("sessionId:", sid, "| advertised caps:", json.dumps(CAPS))
PROMPT=(f"Do ALL of these yourself with your tools, in order, no subagents: "
        f"1) create a file named probe.txt containing the text hi; "
        f"2) run the shell command: echo hello ; "
        f"3) delete the file probe.txt; "
        f"4) open the URL https://example.com . "
        f"Report what you did.")
pid=req("session/prompt", {"sessionId":sid,"prompt":[{"type":"text","text":PROMPT}]})
pump(pid, timeout=200)

log("\n===== agent tool calls =====")
for k,t,st in TOOLS: log(f"  [{k}] {t} -> {st}")
log("===== agent said =====")
log("  " + ("".join(AGENT)[:700] or "(nothing)"))
log("\n===== HOST CALLBACK MAP (server->client methods KAS invoked) =====")
for m,c in sorted(CALLS.items(), key=lambda kv:(-kv[1],kv[0])):
    log(f"  {c:3d}  {m}")
fs_terminal=[m for m in CALLS if "fs/" in m or "terminal/" in m]
log("\nfs/terminal callbacks used:", fs_terminal or "(none)")
log("client-UI callbacks used:", [m for m in CALLS if m in ("_kiro/openExternalUrl","_kiro/system/notify","_kiro/userInput")] or "(none)")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
