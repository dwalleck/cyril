#!/usr/bin/env python3
"""
KAS-2b prove-it-prototype: capture RAW `context_usage` session_info_update
envelopes from a live KAS (v3) turn, so cyril's serde path can be checked
against genuine wire bytes (not a hand-built fixture).

Derived from experiments/conductor-spike/probe-kas-context-breakdown-2.8.0.py;
two changes: (1) read the IdC/odic token (user is logged in via AWS IdC, key
`kirocli:odic:token`, not the old `social` key); (2) dump the full raw envelope
of one breakdown-PRESENT frame and one breakdown-ABSENT frame to .cyril-5et2/.
Token self-sourced from the kiro-cli store; NEVER logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.10.0/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
OUT  = os.path.dirname(os.path.abspath(__file__))
def log(*a): print(" ".join(str(x) for x in a), flush=True)

CWD = tempfile.mkdtemp(prefix="kas-2b-ctxbd-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p", cwd=CWD, shell=True)
big = "\n".join(f"line {i}: the quick brown fox jumps over the lazy dog {i*7}" for i in range(400))
pathlib.Path(CWD, "notes.md").write_text(f"# Project Notes\nThe magic number is 4242.\n\n{big}\n")
subprocess.run("git add -A && git commit -qm baseline", cwd=CWD, shell=True)
log(f"# CWD={CWD}  notes.md={pathlib.Path(CWD,'notes.md').stat().st_size}b")

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
    finally:
        c.close()
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": d.get("profile_arn")}

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "v3"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [0]
def req(m, p):
    _id[0] += 1; PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":m,"params":p})+"\n"); PIN.flush(); return _id[0]
def reply(rid, res): PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); PIN.flush()
def err(rid, msg):   PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"error":{"code":-32000,"message":msg}})+"\n"); PIN.flush()

CTX_PARAMS = []   # full session/update PARAMS for each context_usage frame (raw envelopes)
TURN_ENDS = [0]
def abspath(p): return p if os.path.isabs(p) else os.path.join(CWD, p)

def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is None:
        if m and "session/update" in m:
            u = (p.get("update") or {}) if isinstance(p, dict) else {}
            if isinstance(u, dict) and u.get("sessionUpdate") == "session_info_update":
                kiro = ((u.get("_meta") or {}).get("kiro") or {})
                if kiro.get("kind") == "context_usage":
                    CTX_PARAMS.append(p)            # keep the FULL raw envelope params
                elif kiro.get("kind") == "turn_end":
                    TURN_ENDS[0] += 1
        return
    if m == "_kiro/auth/getAccessToken": reply(rid, read_token()); return
    if m == "_kiro/terminal/shell_type": reply(rid, {"shellType": "bash"}); return
    if m in ("fs/read_text_file", "_kiro/fs/read_file"):
        try: reply(rid, {"content": pathlib.Path(abspath(p.get("path",""))).read_text()})
        except Exception as e: err(rid, f"read failed: {e}")
        return
    if m in ("fs/write_text_file", "_kiro/fs/write_file"):
        try:
            ap = pathlib.Path(abspath(p.get("path",""))); ap.parent.mkdir(parents=True, exist_ok=True); ap.write_text(p.get("content",""))
            reply(rid, {})
        except Exception as e: err(rid, f"write failed: {e}")
        return
    if m == "terminal/create":
        cmd = p.get("command",""); args = p.get("args") or []; tid=f"term-{rid}"
        try:
            r = subprocess.run([cmd,*args], cwd=p.get("cwd") or CWD, capture_output=True, text=True, timeout=60)
            handle.terms = getattr(handle,"terms",{}); handle.terms[tid]={"out":r.stdout+r.stderr,"code":r.returncode}
        except Exception as e:
            handle.terms = getattr(handle,"terms",{}); handle.terms[tid]={"out":f"(host error: {e})","code":1}
        reply(rid, {"terminalId": tid}); return
    if m == "terminal/output":
        t = getattr(handle,"terms",{}).get(p.get("terminalId"),{"out":"","code":0})
        reply(rid, {"output":t["out"],"truncated":False,"exitStatus":{"exitCode":t["code"]}}); return
    if m == "terminal/wait_for_exit":
        t = getattr(handle,"terms",{}).get(p.get("terminalId"),{"code":0})
        reply(rid, {"exitStatus":{"exitCode":t["code"],"signal":None}}); return
    if m in ("terminal/release","terminal/kill"): reply(rid, {}); return
    if m == "session/request_permission":
        opts = p.get("options",[]); pick = next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()), opts[0] if opts else None)
        reply(rid, {"outcome":{"outcome":"selected","optionId":pick["optionId"]}} if pick else {"outcome":{"outcome":"cancelled"}}); return
    reply(rid, {})

def pump(to=10):
    end=time.time()+to
    while time.time()<end:
        try: raw=msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return False
        try: o=json.loads(raw)
        except Exception: continue
        if "method" in o: handle(o)
    return True
def call_sync(method, params, to=40):
    rid=req(method,params); end=time.time()+to
    while time.time()<end:
        try: raw=msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return None
        try: o=json.loads(raw)
        except Exception: continue
        if "method" in o: handle(o)
        elif o.get("id")==rid: return o
    return None

CAPS = {"fs":{"readTextFile":True,"writeTextFile":True}, "terminal":True}
ir = call_sync("initialize", {"protocolVersion":1,"clientCapabilities":CAPS})
if not (ir and "result" in ir):
    log("!! initialize failed:", json.dumps(ir)[:300] if ir else "None"); PIN.close(); proc.terminate(); raise SystemExit(1)
nr = call_sync("session/new", {"cwd":CWD,"mcpServers":[]})
sid = (nr.get("result") or {}).get("sessionId") if nr and "result" in nr else None
log("# sessionId:", sid)
if not sid:
    log("!! session/new failed:", json.dumps(nr)[:300] if nr else "None"); PIN.close(); proc.terminate(); raise SystemExit(1)

PROMPT = ("Using your tools, do ALL of these in order, one tool call at a time: "
          "1) read notes.md and tell me the magic number; "
          "2) run the shell command `echo done-42` and report its output; "
          "3) run `ls -la` and summarize; "
          "4) write a file summary.txt containing the magic number. "
          "Report briefly after each step.")
pid = req("session/prompt", {"sessionId":sid,"prompt":[{"type":"text","text":PROMPT}]})
before=TURN_ENDS[0]; end=time.time()+240
while time.time()<end and TURN_ENDS[0]<=before: pump(10)
pump(4)
log("# turn complete:", TURN_ENDS[0]>before, " context_usage frames:", len(CTX_PARAMS))

def kiro_of(params): return params["update"]["_meta"]["kiro"]
def has_breakdown(params): return isinstance(kiro_of(params).get("breakdown"), dict)
for i, p in enumerate(CTX_PARAMS):
    k = kiro_of(p)
    log(f"  [{i}] usagePercentage={k.get('usagePercentage')} breakdown={'present' if has_breakdown(p) else 'ABSENT'}")
present = [p for p in CTX_PARAMS if has_breakdown(p)]
absent  = [p for p in CTX_PARAMS if not has_breakdown(p)]
def score(params):
    bd = kiro_of(params).get("breakdown") or {}
    return sum(1 for b in bd.values() if isinstance(b,dict) and (b.get("tokens") or 0)>0)
if present:
    best = max(present, key=score)
    pathlib.Path(OUT,"context_usage_raw.json").write_text(json.dumps(best, indent=2)+"\n")
    log("# wrote context_usage_raw.json (breakdown buckets:", list(kiro_of(best).get("breakdown",{}).keys()), ")")
if absent:
    pathlib.Path(OUT,"context_usage_absent_raw.json").write_text(json.dumps(absent[0], indent=2)+"\n")
    log("# wrote context_usage_absent_raw.json (usagePercentage-only frame)")
PIN.close(); proc.terminate()
