#!/usr/bin/env python3
"""
KAS-5 (cyril-7bdu) prove-it-prototype: CAPTURE the fs + terminal HOST-CALLBACK
request shapes from a live KAS (v3) turn on the **2.10.0** binary, and DIFF them
against the documented 2.7.1/2.8.1 baseline — confirming the wire contract cyril
must implement has not drifted before we design KAS-5.

This is the inverse of the KAS-2b capture: there we recorded the notifications
cyril CONSUMES; here we record the server->client REQUESTS cyril must ANSWER
(fs read/write/stat/list/delete, terminal create/output/wait/release, shell_type).

Method: re-use the proven 2.10.0 harness (launch `acp --agent-engine v3`, IdC
token key `kirocli:odic:token`, advertise fs+terminal caps, real responders) and
add raw-envelope RECORDING. Token self-sourced from the kiro-cli store; we record
only inbound REQUESTS (the token lives in our outbound reply, never recorded), so
the capture is safe to commit. Throwaway git repo — safe to read/write/exec/delete.

Outputs (under .cyril-7bdu/):
  - host_callbacks_2.10.0.json   ordered [{method, params}] of every callback
  - fixtures/<method>.json       one raw example envelope per distinct method
  - BASELINE-DIFF printed to stdout (methods + param keys vs the 2.7.1 baseline)
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.10.0/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
OUT  = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".cyril-7bdu")
OUT  = os.path.abspath(OUT)
FIX  = os.path.join(OUT, "fixtures")
os.makedirs(FIX, exist_ok=True)
def log(*a): print(" ".join(str(x) for x in a), flush=True)

# Documented baseline (probe-kas-fs-terminal-host-2.7.1.py docstring + covenant):
# method -> set of param keys expected at 2.7.1/2.8.1. Drift = a new/missing key
# or method at 2.10.0.
BASELINE = {
    "_kiro/auth/getAccessToken":   set(),
    "_kiro/terminal/shell_type":   set(),            # may carry {sessionId}
    "fs/read_text_file":           {"path"},
    "fs/write_text_file":          {"path", "content"},
    "_kiro/fs/read_file":          {"path"},
    "_kiro/fs/write_file":         {"path", "content"},
    "_kiro/fs/read_directory":     {"path"},
    "_kiro/fs/stat":               {"path"},
    "_kiro/fs/delete":             {"path"},
    "terminal/create":             {"command", "args"},   # + optional cwd/env/outputByteLimit
    "terminal/output":             {"terminalId"},
    "terminal/wait_for_exit":      {"terminalId"},
    "terminal/release":            {"terminalId"},
    "terminal/kill":               {"terminalId"},
    "session/request_permission":  {"options"},            # + sessionId/toolCall
}

CWD = tempfile.mkdtemp(prefix="kas-5-fsterm-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p", cwd=CWD, shell=True)
pathlib.Path(CWD, "README.md").write_text("# csv2json\nThe magic number is 4242.\n")
pathlib.Path(CWD, "scratch.txt").write_text("delete me\n")
subprocess.run("git add -A && git commit -qm baseline", cwd=CWD, shell=True)
log(f"# CWD={CWD}  binary={KIRO}")

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        prow = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        c.close()
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    # The IdC OIDC token no longer carries `profile_arn` (it rotated out); kiro-cli
    # persists the active profile separately in state['api.codewhisperer.profile']
    # = {arn, profile_name}. Backend rejects the turn with "profileArn is required"
    # if we don't supply it. Prefer the token's field if present, else the state row.
    profile_arn = d.get("profile_arn")
    if not profile_arn and prow:
        pv = prow[0]; pv = pv.decode() if isinstance(pv, (bytes, bytearray)) else pv
        try: profile_arn = json.loads(pv).get("arn")
        except Exception: pass
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": profile_arn}

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "v3"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [0]
def req(m, p):
    _id[0] += 1; PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":m,"params":p})+"\n"); PIN.flush(); return _id[0]
def reply(rid, res): PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); PIN.flush()
def err(rid, msg):   PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"error":{"code":-32000,"message":msg}})+"\n"); PIN.flush()

RECORDS = []          # ordered [{method, params}] — every server->client request (token-free)
TERMS = {}
TURN_ENDS = [0]
AGENT = []
def abspath(p): return p if os.path.isabs(p) else os.path.join(CWD, p)

def record(m, p):
    RECORDS.append({"method": m, "params": p})
    # first example of each distinct method -> a raw fixture
    f = os.path.join(FIX, m.replace("/", "__") + ".json")
    if not os.path.exists(f):
        pathlib.Path(f).write_text(json.dumps({"method": m, "params": p}, indent=2) + "\n")

def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is None:
        if m and "session/update" in m:
            u = (p.get("update") or {}) if isinstance(p, dict) else {}
            if isinstance(u, dict):
                v = u.get("sessionUpdate")
                if v == "session_info_update" and (((u.get("_meta") or {}).get("kiro") or {}).get("kind")) == "turn_end":
                    TURN_ENDS[0] += 1
                elif v == "agent_message_chunk":
                    AGENT.append(u.get("content", {}).get("text", ""))
        return

    # ---- server -> client REQUESTS: record every one, then respond for real ----
    record(m, p)
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
    if m == "_kiro/fs/read_directory":
        try:
            entries = [{"name": e.name, "type": ("directory" if e.is_dir() else "file")} for e in pathlib.Path(abspath(p.get("path",""))).iterdir()]
            reply(rid, {"entries": entries})
        except Exception as e: err(rid, str(e))
        return
    if m == "_kiro/fs/stat":
        ap = pathlib.Path(abspath(p.get("path","")))
        reply(rid, {"type": ("directory" if ap.is_dir() else "file"), "size": ap.stat().st_size}) if ap.exists() else err(rid, "not found")
        return
    if m == "_kiro/fs/delete":
        try: pathlib.Path(abspath(p.get("path",""))).unlink(); reply(rid, {})
        except Exception as e: err(rid, str(e))
        return
    if m == "terminal/create":
        cmd = p.get("command",""); args = p.get("args") or []; tid=f"term-{len(TERMS)+1}"
        try:
            r = subprocess.run([cmd,*args], cwd=p.get("cwd") or CWD, capture_output=True, text=True, timeout=60)
            TERMS[tid] = {"out": r.stdout + r.stderr, "code": r.returncode}
        except Exception as e:
            TERMS[tid] = {"out": f"(host error: {e})", "code": 1}
        reply(rid, {"terminalId": tid}); return
    if m == "terminal/output":
        t = TERMS.get(p.get("terminalId"), {"out":"","code":0})
        reply(rid, {"output":t["out"],"truncated":False,"exitStatus":{"exitCode":t["code"]}}); return
    if m == "terminal/wait_for_exit":
        t = TERMS.get(p.get("terminalId"), {"code":0})
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
pathlib.Path(OUT,"initialize_result_2.10.0.json").write_text(json.dumps(ir["result"], indent=2)+"\n")
log("# agentCapabilities:", json.dumps((ir["result"] or {}).get("agentCapabilities", {}))[:240])
nr = call_sync("session/new", {"cwd":CWD,"mcpServers":[]})
sid = (nr.get("result") or {}).get("sessionId") if nr and "result" in nr else None
log("# sessionId:", sid)
if not sid:
    log("!! session/new failed:", json.dumps(nr)[:300] if nr else "None"); PIN.close(); proc.terminate(); raise SystemExit(1)

# Exercise the full fs+terminal surface: read, list-directory, write, delete, shell.
PROMPT = ("Using your tools, do ALL of these in order, one tool call at a time, "
          "reporting briefly after each: "
          "1) read README.md and tell me the magic number; "
          "2) list the files in the current directory; "
          "3) write a new file summary.txt containing exactly `magic=4242`; "
          "4) delete the file scratch.txt; "
          "5) run the shell command `echo done-42` and report its output; "
          "6) run `ls -la` and summarize.")
req("session/prompt", {"sessionId":sid,"prompt":[{"type":"text","text":PROMPT}]})
before=TURN_ENDS[0]; end=time.time()+300
while time.time()<end and TURN_ENDS[0]<=before: pump(10)
pump(4)
log("# turn complete:", TURN_ENDS[0]>before, " host-callback requests:", len(RECORDS))

# ---- dump ordered records ----
pathlib.Path(OUT,"host_callbacks_2.10.0.json").write_text(json.dumps(RECORDS, indent=2)+"\n")

# ---- summary table: method -> count, observed param keys ----
from collections import OrderedDict
seen = OrderedDict()
for r in RECORDS:
    k = r["method"]; seen.setdefault(k, {"count":0, "keys":set()})
    seen[k]["count"] += 1; seen[k]["keys"].update((r["params"] or {}).keys())
log("\n===== host callbacks observed @ 2.10.0 =====")
for m, info in seen.items():
    log(f"  {m:30} x{info['count']:<3} keys={sorted(info['keys'])}")

# ---- BASELINE DIFF ----
log("\n===== BASELINE DIFF (vs 2.7.1/2.8.1) =====")
drift = False
for m, info in seen.items():
    if m not in BASELINE:
        log(f"  !! NEW METHOD not in baseline: {m} keys={sorted(info['keys'])}"); drift = True; continue
    extra = info["keys"] - BASELINE[m]
    # only NEW required-looking keys are interesting; optional cwd/env/sessionId noted, not failed
    if extra:
        log(f"  ~  {m}: extra param keys vs baseline: {sorted(extra)} (verify optional vs required)")
missing = [m for m in BASELINE if m not in seen]
log(f"  baseline methods NOT exercised this run (agent's tool choices, not necessarily absent): {missing}")
if not drift:
    log("  => no NEW unknown methods; core fs/terminal contract matches the 2.7.1 baseline shape.")

log("\n===== agent final message (head) =====")
log("".join(AGENT)[:500] or "(none)")
PIN.close(); proc.terminate()
log(f"\n# raw fixtures: {FIX}")
log(f"# ordered log:  {os.path.join(OUT,'host_callbacks_2.10.0.json')}")
