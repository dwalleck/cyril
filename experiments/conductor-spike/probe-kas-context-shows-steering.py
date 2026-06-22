#!/usr/bin/env python3
"""
QUESTION: after a fileMatch steering doc is injected, does the `/context` command show it?

KAS surfaces (3 distinct things, easy to conflate):
  1. `_kiro/session/context` {subcommand:show}  -> ContextFileManager = USER-ATTACHED files only (/context add). NOT steering.
  2. context_usage breakdown (session_info_update) -> 5 token buckets (contextFiles/tools/yourPrompts/kiroResponses/sessionFiles). No steering bucket.
  3. steering channels: `_kiro/steering/documents_changed` (catalog) + session_info_update `steering_inclusion` (what got injected). <-- THIS is where steering shows.

This probe injects tsx-canary (agent reads App.tsx), then dumps all three and reports
whether the steering doc appears in #1/#2 (expected: NO) vs #3 (expected: YES).

Auth recipe: see v2/v3.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, shutil, re as _re

CANDIDATES = [os.environ.get("KIRO_BIN"), shutil.which("kiro-cli"), os.path.expanduser("~/.local/bin/kiro-cli")]
KIRO = next((c for c in CANDIDATES if c and os.path.exists(c)), None)
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-context-shows-steering.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()
if not KIRO: log("FATAL: no kiro-cli"); raise SystemExit(1)
CMD = [KIRO, "acp", "--agent-engine", "kas"]
log(f"# binary: {KIRO}")

def _profile_arn():
    try:
        c = sqlite3.connect(AUTH)
        try: row = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
        finally: c.close()
        if row:
            v = row[0]; v = v.decode() if isinstance(v,(bytes,bytearray)) else str(v)
            m = _re.search(r'arn:aws:codewhisperer:[a-z0-9-]+:[0-9]+:profile/[A-Za-z0-9]+', v)
            if m: return m.group(0)
    except Exception: pass
    return None
def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = (c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
               or c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone())
    finally: c.close()
    if not row: return {}
    v = row[0]; v = v.decode() if isinstance(v,(bytes,bytearray)) else v; d = json.loads(v)
    out = {"accessToken": d["access_token"], "expiresAt": d["expires_at"]}
    pa = d.get("profile_arn") or _profile_arn()
    if pa: out["profileArn"] = pa
    return out

def shape(v, depth=0):
    if depth > 6: return "..."
    if isinstance(v, dict): return {k: shape(x, depth+1) for k,x in v.items()}
    if isinstance(v, list): return ([shape(v[0], depth+1), f"...(len={len(v)})"] if v else [])
    return type(v).__name__

CWD = tempfile.mkdtemp(prefix="kas-ctx-")
os.makedirs(os.path.join(CWD, ".kiro", "steering"), exist_ok=True)
os.makedirs(os.path.join(CWD, "src"), exist_ok=True)
open(os.path.join(CWD, ".kiro", "steering", "always-canary.md"), "w").write("---\ninclusion: always\n---\nALWAYS steering rule body.\n")
open(os.path.join(CWD, ".kiro", "steering", "tsx-canary.md"), "w").write('---\ninclusion: fileMatch\nfileMatchPattern: "**/*.tsx"\n---\nTSX steering rule body.\n')
open(os.path.join(CWD, "src", "App.tsx"), "w").write("export const App = () => null;\n")

proc = subprocess.Popen(CMD, cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id=[0]
def req(m,p):
    _id[0]+=1; PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":m,"params":p})+"\n"); PIN.flush(); return _id[0]
def reply(rid,res):
    PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); PIN.flush()

cap = {"breakdown": None, "steer_incl": [], "docs_changed": []}
def handle(o):
    m=o.get("method"); rid=o.get("id"); p=o.get("params",{}) or {}
    if rid is not None:
        if m=="_kiro/auth/getAccessToken": reply(rid, read_token())
        elif m=="_kiro/terminal/shell_type": reply(rid, {"shellType":"bash"})
        elif m=="session/request_permission":
            opts=p.get("options",[]); pick=next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()), opts[0] if opts else None)
            reply(rid, {"outcome":{"outcome":"selected","optionId":pick["optionId"]}} if pick else {"outcome":{"outcome":"cancelled"}})
        else: reply(rid, {})
        return
    if m=="_kiro/steering/documents_changed":
        cap["docs_changed"]=[(d.get("name"),d.get("inclusion")) for d in (p.get("documents") or []) if isinstance(d,dict)]
    elif m=="session/update" or (m or "").endswith("/session/update"):
        u=(p.get("update") or {}) if isinstance(p,dict) else {}
        if isinstance(u,dict) and u.get("sessionUpdate")=="session_info_update":
            kk=((u.get("_meta") or {}).get("kiro")) or {}
            if kk.get("kind")=="context_usage" and kk.get("breakdown"):
                cap["breakdown"]=kk["breakdown"]
            elif kk.get("kind")=="steering_inclusion":
                cap["steer_incl"].append(kk.get("steeringDocuments"))

def pump(until, to=300):
    end=time.time()+to
    while time.time()<end:
        try: raw=msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return None
        try: o=json.loads(raw)
        except Exception: continue
        if "method" in o:
            handle(o)
            if o.get("id")==until and "result" in o: return o
        elif "id" in o and o["id"]==until: return o
    return "timeout"

req("initialize", {"protocolVersion":1, "clientCapabilities":{"fs":{"readTextFile":True,"writeTextFile":True},"terminal":True}})
pump(1,20)
nr=pump(req("session/new", {"cwd":CWD,"mcpServers":[]}),40)
assert isinstance(nr,dict) and "result" in nr, f"new failed {nr}"
sid=nr["result"]["sessionId"]

# Turn: make the agent read App.tsx -> injects tsx-canary
pid=req("session/prompt", {"sessionId":sid,"prompt":[{"type":"text","text":"Use read_file to read `src/App.tsx`, then reply with just the word DONE."}]})
pump(pid,300)

# Now query the /context surface (KAS-native). Try a couple param shapes.
log("\n===== _kiro/session/context (the '/context' surface) =====")
for params in ({"sessionId":sid,"subcommand":"show"}, {"sessionId":sid,"command":"show"}, {"sessionId":sid}):
    r=pump(req("_kiro/session/context", params), 30)
    if isinstance(r,dict) and "result" in r:
        log(f"  params={list(params)} -> result: {json.dumps(r['result'])[:500]}")
        break
    elif isinstance(r,dict) and "error" in r:
        log(f"  params={list(params)} -> error: {json.dumps(r['error'])[:200]}")
    else:
        log(f"  params={list(params)} -> {r}")

log("\n===== context_usage breakdown (5 token buckets) =====")
if cap["breakdown"]:
    for bucket, val in cap["breakdown"].items():
        if isinstance(val, dict):
            log(f"  {bucket}: tokens={val.get('tokens')} percent={val.get('percent')} items={'present('+str(len(val['items']))+')' if isinstance(val.get('items'),list) else 'ABSENT'}")
        else:
            log(f"  {bucket}: {val}")
else:
    log("  (no breakdown captured)")

log("\n===== steering-specific channels =====")
log(f"  steering/documents_changed (catalog): {cap['docs_changed']}")
log(f"  session_info_update steering_inclusion (injected): {cap['steer_incl']}")

PIN.close(); proc.terminate()
try: proc.wait(timeout=5)
except Exception: proc.kill()
log(f"\n# full log: {LOG}")
logf.close()
