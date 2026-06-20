#!/usr/bin/env python3
"""
Does the KAS (v3) wire itemize the `tools` bucket of the context-usage breakdown?

cyril renders a `/context` breakdown with five buckets (contextFiles, tools,
yourPrompts, kiroResponses, sessionFiles). On the v1/v2 engine only `contextFiles`
ships an `items[]` array; `tools` is aggregate {tokens, percent}. The
@kiro/acp-type-covenant declares `items?` as OPTIONAL on every bucket including
`tools` — so KAS *could* itemize per-tool. This probe checks whether it actually does.

KAS carries the breakdown on `session/update` -> `session_info_update` with
`_meta.kiro.kind == "context_usage"` (covenant: ContextUsageBreakdown). We:
  1. advertise fs+terminal caps so KAS delegates I/O to us (real responders),
  2. run a turn that reads a sizable file (positive control: contextFiles.items)
     and runs a shell command (loads the tools bucket),
  3. capture EVERY context_usage frame, keep the most-populated breakdown,
  4. report per-bucket: tokens, percent, and whether `items` is present + length,
  5. special focus: dump the full `tools` bucket verbatim.
Secondary: also try the v2-style `kiro.dev/commands/execute {command:"context"}`
to see if KAS even exposes that command surface.

Auth token self-sourced from the kiro-cli store; NEVER logged. Throwaway repo.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.8.0/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-context-breakdown-2.8.0.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-ctxbd-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p", cwd=CWD, shell=True)
# A sizable file so contextFiles is a non-trivial, itemizable bucket (positive control).
big = "\n".join(f"line {i}: the quick brown fox jumps over the lazy dog {i*7}" for i in range(400))
pathlib.Path(CWD, "notes.md").write_text(f"# Project Notes\nThe magic number is 4242.\n\n{big}\n")
subprocess.run("git add -A && git commit -qm baseline", cwd=CWD, shell=True)
log(f"# KIRO={KIRO}")
log(f"# CWD={CWD}  (notes.md = {pathlib.Path(CWD,'notes.md').stat().st_size} bytes)")

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": d.get("profile_arn")}

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "v3"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [0]
def req(m, p):
    _id[0] += 1; PIN.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p}) + "\n"); PIN.flush(); return _id[0]
def reply(rid, res):
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n"); PIN.flush()
def err(rid, msg):
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "error": {"code": -32000, "message": msg}}) + "\n"); PIN.flush()

CTX_FRAMES = []   # every context_usage _meta.kiro payload, in arrival order
TERMS = {}
TURN_ENDS = [0]

def abspath(p):
    return p if os.path.isabs(p) else os.path.join(CWD, p)

def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is None:
        if m and "session/update" in m:
            u = (p.get("update") or {}) if isinstance(p, dict) else {}
            if isinstance(u, dict) and u.get("sessionUpdate") == "session_info_update":
                kiro = ((u.get("_meta") or {}).get("kiro") or {})
                if kiro.get("kind") == "context_usage":
                    CTX_FRAMES.append(kiro)
                elif kiro.get("kind") == "turn_end":
                    TURN_ENDS[0] += 1
        return
    # ---- server -> client REQUESTS (host callbacks) ----
    if m == "_kiro/auth/getAccessToken":
        reply(rid, read_token()); return
    if m == "_kiro/terminal/shell_type":
        reply(rid, {"shellType": "bash"}); return
    if m in ("fs/read_text_file", "_kiro/fs/read_file"):
        try:
            content = pathlib.Path(abspath(p.get("path", ""))).read_text()
        except Exception as e:
            err(rid, f"read failed: {e}"); return
        reply(rid, {"content": content}); return
    if m in ("fs/write_text_file", "_kiro/fs/write_file"):
        try:
            ap = pathlib.Path(abspath(p.get("path", ""))); ap.parent.mkdir(parents=True, exist_ok=True)
            ap.write_text(p.get("content", ""))
        except Exception as e:
            err(rid, f"write failed: {e}"); return
        reply(rid, {}); return
    if m == "_kiro/fs/read_directory":
        try:
            entries = [{"name": e.name, "type": ("directory" if e.is_dir() else "file")} for e in pathlib.Path(abspath(p.get("path", ""))).iterdir()]
        except Exception as e:
            err(rid, str(e)); return
        reply(rid, {"entries": entries}); return
    if m == "_kiro/fs/stat":
        ap = pathlib.Path(abspath(p.get("path", "")))
        if not ap.exists():
            err(rid, "not found"); return
        reply(rid, {"type": ("directory" if ap.is_dir() else "file"), "size": ap.stat().st_size}); return
    if m == "terminal/create":
        cmd = p.get("command", ""); args = p.get("args") or []; tid = f"term-{len(TERMS)+1}"
        try:
            r = subprocess.run([cmd, *args], cwd=p.get("cwd") or CWD, capture_output=True, text=True, timeout=60)
            TERMS[tid] = {"out": (r.stdout + r.stderr), "code": r.returncode}
        except Exception as e:
            TERMS[tid] = {"out": f"(host error: {e})", "code": 1}
        reply(rid, {"terminalId": tid}); return
    if m == "terminal/output":
        t = TERMS.get(p.get("terminalId"), {"out": "", "code": 0})
        reply(rid, {"output": t["out"], "truncated": False, "exitStatus": {"exitCode": t["code"]}}); return
    if m == "terminal/wait_for_exit":
        t = TERMS.get(p.get("terminalId"), {"code": 0})
        reply(rid, {"exitStatus": {"exitCode": t["code"], "signal": None}}); return
    if m in ("terminal/release", "terminal/kill"):
        reply(rid, {}); return
    if m == "session/request_permission":
        opts = p.get("options", [])
        pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()), opts[0] if opts else None)
        reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}); return
    reply(rid, {})

def pump_once(to=10):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = msgs.get(timeout=2)
        except queue.Empty:
            continue
        if raw is None:
            return False
        try:
            o = json.loads(raw)
        except Exception:
            continue
        if "method" in o:
            handle(o)
    return True

def call_sync(method, params, to=40):
    rid = req(method, params); end = time.time() + to
    while time.time() < end:
        try:
            raw = msgs.get(timeout=2)
        except queue.Empty:
            continue
        if raw is None:
            return None
        try:
            o = json.loads(raw)
        except Exception:
            continue
        if "method" in o:
            handle(o)
        elif o.get("id") == rid:
            return o
    return None

CAPS = {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}
ir = call_sync("initialize", {"protocolVersion": 1, "clientCapabilities": CAPS})
if not (ir and "result" in ir):
    log("!! initialize failed (auth expired? KAS not embedded?). raw:", json.dumps(ir)[:300] if ir else "None")
    PIN.close(); proc.terminate(); raise SystemExit(1)
log("# agentCapabilities:", json.dumps((ir["result"] or {}).get("agentCapabilities", {}))[:240])
nr = call_sync("session/new", {"cwd": CWD, "mcpServers": []})
sid = (nr.get("result") or {}).get("sessionId") if nr and "result" in nr else None
log("# sessionId:", sid)
if not sid:
    log("!! session/new failed. raw:", json.dumps(nr)[:300] if nr else "None")
    PIN.close(); proc.terminate(); raise SystemExit(1)

PROMPT = ("Using your tools, do both steps: "
          "1) read the file notes.md and tell me the magic number in it; "
          "2) run the shell command `echo done-42` and report its output verbatim.")
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
before = TURN_ENDS[0]; end = time.time() + 240
while time.time() < end and TURN_ENDS[0] <= before:
    pump_once(10)
log("# turn complete:", TURN_ENDS[0] > before, " context_usage frames seen:", len(CTX_FRAMES))
pump_once(4)

# Secondary: does KAS expose the v2 /context command surface at all?
log("\n===== secondary: v2-style kiro.dev/commands/execute {command:'context'} =====")
cr = call_sync("kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": "context", "args": {}}}, to=30)
if cr is None:
    log("  (no response / timed out)")
elif "error" in cr:
    log("  error:", json.dumps(cr["error"])[:200])
else:
    res = cr.get("result")
    log("  result keys:", list(res.keys()) if isinstance(res, dict) else type(res).__name__)
    bd = (res or {}).get("data", {}).get("breakdown") if isinstance(res, dict) else None
    if bd:
        log("  command-surface breakdown buckets:", list(bd.keys()))
        log("  command-surface tools bucket:", json.dumps(bd.get("tools")))

# ---- analyze the captured context_usage frames ----
BUCKETS = ["contextFiles", "tools", "yourPrompts", "kiroResponses", "sessionFiles"]
def breakdown_of(frame):
    # covenant: {usagePercentage, breakdown?: ContextUsageBreakdown}
    return frame.get("breakdown") if isinstance(frame, dict) else None

log("\n===== context_usage frames (cadence) =====")
if not CTX_FRAMES:
    log("  (NONE — KAS pushed no session_info_update:context_usage in this config)")
for i, f in enumerate(CTX_FRAMES):
    bd = breakdown_of(f)
    log(f"  [{i}] usagePercentage={f.get('usagePercentage')}  breakdown={'present' if bd else 'absent'}"
        + (f" buckets={list(bd.keys())}" if isinstance(bd, dict) else ""))

# pick the most-populated breakdown (max number of buckets with tokens>0)
def score(f):
    bd = breakdown_of(f) or {}
    return sum(1 for b in BUCKETS if isinstance(bd.get(b), dict) and (bd[b].get("tokens") or 0) > 0)
best = max(CTX_FRAMES, key=score) if CTX_FRAMES else None
bd = breakdown_of(best) if best else None

log("\n===== per-bucket analysis (most-populated frame) =====")
if not bd:
    log("  no breakdown object present on any context_usage frame.")
else:
    log(f"  usagePercentage={best.get('usagePercentage')}")
    for b in BUCKETS:
        cat = bd.get(b)
        if not isinstance(cat, dict):
            log(f"  {b:14} : (absent)"); continue
        items = cat.get("items")
        has = isinstance(items, list)
        log(f"  {b:14} : tokens={cat.get('tokens')}, percent={cat.get('percent')}, "
            f"items={'ABSENT' if items is None else (str(len(items)) + ' entries') if has else type(items).__name__}, "
            f"keys={sorted(cat.keys())}")

log("\n===== THE ANSWER: full `tools` bucket verbatim =====")
tools_bucket = (bd or {}).get("tools") if bd else None
log(json.dumps(tools_bucket, indent=2) if tools_bucket is not None else "  (no tools bucket)")
if isinstance(tools_bucket, dict):
    ti = tools_bucket.get("items")
    log("\n  >>> tools.items present?  ->", "YES" if isinstance(ti, list) else "NO",
        (f"({len(ti)} per-tool entries)" if isinstance(ti, list) else ""))

log("\n===== positive control: full `contextFiles` bucket verbatim =====")
cf = (bd or {}).get("contextFiles") if bd else None
log(json.dumps(cf, indent=2)[:1500] if cf is not None else "  (no contextFiles bucket)")
if isinstance(cf, dict):
    log("  >>> contextFiles.items present? ->", "YES" if isinstance(cf.get("items"), list) else "NO")

PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
