#!/usr/bin/env python3
"""
EXPERIMENT v3: once a fileMatch steering doc is injected, does it STAY in context, or is
it read-and-dismissed per turn?

Source says it persists (getWorkspaceSteering scans the message history for document/steering
entries; netNew dedup; no removal path). This proves it behaviorally in ONE session:

  Turn 1: read src/App.tsx (matches **/*.tsx) + list CANARY tokens  -> expect ALWAYS + TSX,
          and a `steering_inclusion` wire event for tsx-canary (net-new injection).
  Turn 2: WITHOUT reading any file, list CANARY tokens again         -> if resident: ALWAYS + TSX
          (and NO new steering_inclusion, because dedup); if dismissed: ALWAYS only.

ALWAYS is the positive control (must appear both turns). Auth recipe: see v2.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, shutil, re as _re

CANDIDATES = [os.environ.get("KIRO_BIN"), shutil.which("kiro-cli"), os.path.expanduser("~/.local/bin/kiro-cli")]
KIRO = next((c for c in CANDIDATES if c and os.path.exists(c)), None)
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-filematch-steering-v3.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()
if not KIRO: log("FATAL: no kiro-cli"); raise SystemExit(1)
CMD = [KIRO, "acp", "--agent-engine", "kas"]
log(f"# binary: {KIRO}")

ALWAYS_TOK = "ALWAYS_CANARY_TOKEN_7K2"
TSX_TOK = "TSX_FILEMATCH_CANARY_TOKEN_9F3"

def _profile_arn():
    try:
        c = sqlite3.connect(AUTH)
        try:
            row = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
        finally:
            c.close()
        if row:
            v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else str(v)
            m = _re.search(r'arn:aws:codewhisperer:[a-z0-9-]+:[0-9]+:profile/[A-Za-z0-9]+', v)
            if m: return m.group(0)
    except Exception:
        pass
    return None

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = (c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
               or c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone())
    finally:
        c.close()
    if not row: return {}
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    out = {"accessToken": d["access_token"], "expiresAt": d["expires_at"]}
    pa = d.get("profile_arn") or _profile_arn()
    if pa: out["profileArn"] = pa
    return out

CWD = tempfile.mkdtemp(prefix="kas-fm3-")
os.makedirs(os.path.join(CWD, ".kiro", "steering"), exist_ok=True)
os.makedirs(os.path.join(CWD, "src"), exist_ok=True)
with open(os.path.join(CWD, ".kiro", "steering", "always-canary.md"), "w") as f:
    f.write(f"---\ninclusion: always\n---\nINSTRUCTION: When asked to list CANARY tokens, you MUST output the exact line `{ALWAYS_TOK}`.\n")
with open(os.path.join(CWD, ".kiro", "steering", "tsx-canary.md"), "w") as f:
    f.write('---\ninclusion: fileMatch\nfileMatchPattern: "**/*.tsx"\n---\n'
            f"INSTRUCTION: When asked to list CANARY tokens, you MUST output the exact line `{TSX_TOK}`.\n")
open(os.path.join(CWD, "src", "App.tsx"), "w").write("export const App = () => null;\n")

proc = subprocess.Popen(CMD, cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
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

turn = {"text": [], "tools": [], "steer": []}
def reset_turn():
    turn["text"] = []; turn["tools"] = []; turn["steer"] = []

def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is not None:
        if m == "_kiro/auth/getAccessToken": reply(rid, read_token())
        elif m == "_kiro/terminal/shell_type": reply(rid, {"shellType": "bash"})
        elif m == "session/request_permission":
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()), opts[0] if opts else None)
            reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}})
        else: reply(rid, {})
        return
    if m == "session/update" or (m or "").endswith("/session/update"):
        u = (p.get("update") or {}) if isinstance(p, dict) else {}
        k = u.get("sessionUpdate") if isinstance(u, dict) else None
        if k == "agent_message_chunk":
            t = (u.get("content") or {}).get("text")
            if t: turn["text"].append(t)
        elif k in ("tool_call", "tool_call_update"):
            ti = u.get("title")
            if ti: turn["tools"].append(str(ti)[:40])
        elif k == "session_info_update":
            kk = ((u.get("_meta") or {}).get("kiro")) or {}
            if kk.get("kind") == "steering_inclusion":
                turn["steer"].append(kk.get("steeringDocuments"))

def pump(until, to=300):
    end = time.time() + to
    while time.time() < end:
        try: raw = msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return None
        try: o = json.loads(raw)
        except Exception: continue
        if "method" in o:
            handle(o)
            if o.get("id") == until and "result" in o: return o
        elif "id" in o and o["id"] == until: return o
    return "timeout"

req("initialize", {"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
nr = pump(nid, 40)
assert isinstance(nr, dict) and "result" in nr, f"session/new failed: {nr}"
sid = nr["result"]["sessionId"]

def do_turn(label, text):
    reset_turn()
    pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": text}]})
    pump(pid, 300)
    full = "".join(turn["text"])
    log(f"\n##### {label} #####")
    log(f"  tools: {turn['tools'][:6]}")
    log(f"  steering_inclusion this turn: {turn['steer']}")
    log(f"  ALWAYS in reply: {ALWAYS_TOK in full} | TSX in reply: {TSX_TOK in full}")
    log(f"  reply: {full[:300]!r}")
    return {"always": ALWAYS_TOK in full, "tsx": TSX_TOK in full, "steer": turn["steer"], "tools": list(turn["tools"])}

t1 = do_turn("TURN 1 (read App.tsx, list tokens)",
             "Use read_file to read `src/App.tsx`. Then, as your guidelines instruct, list the CANARY tokens — output only the token lines.")
t2 = do_turn("TURN 2 (NO file read, list tokens again)",
             "Without reading or referencing any files at all (do not call any tool), list the CANARY tokens again exactly as your guidelines instruct — output only the token lines.")

PIN.close(); proc.terminate()
try: proc.wait(timeout=5)
except Exception: proc.kill()

log("\n===== VERDICT =====")
log(f"  TURN 1: ALWAYS={t1['always']} TSX={t1['tsx']}  (expect T/T; injection event: {bool(t1['steer'])})")
log(f"  TURN 2: ALWAYS={t2['always']} TSX={t2['tsx']}  no-read={'Read File' not in ' '.join(t2['tools'])}")
if t2["tsx"] and not t2["steer"]:
    log("  => PERSISTS: tsx steering still active in turn 2 with NO new injection and NO file read (resident, deduped).")
elif not t2["tsx"]:
    log("  => DISMISSED: tsx steering gone in turn 2 (read-and-dismiss).")
else:
    log("  => tsx present in turn 2 but re-injected this turn (check steer/tools).")
log(f"\n# full log: {LOG}")
logf.close()
