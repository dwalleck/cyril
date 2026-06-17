#!/usr/bin/env python3
"""
Fire `_kiro/userInput` — KAS's rich structured-input callback (the replacement for
the lossy session/request_permission bridge).

Gated by `clientCapabilities._meta.kiro.userInput: true`. When advertised, the agent
routes structured questions (e.g. its `get_user_input` tool / clarifying prompts) to
the client as `_kiro/userInput {sessionId, toolCallId, question, options: UserInputOption[]}`,
and the client returns `{action:'answered'|'dismissed', answer?}`.

TRIGGER: the `get_user_input` tool (id `user_input`) is tagged **"spec"** in the bundle —
it is NOT in the default vibe agent's toolkit (a plain "ask me a question" prompt makes
the agent ask in chat instead). It surfaces in the SPEC flow, whose clarifying questions
("new feature or bugfix?", "what to start with?") are this tool. So this probe advertises
userInput AND drives a spec `createSpec`; the clarifying questions then route to
`_kiro/userInput`, which we answer structurally. Captures the on-wire UserInputRequest
shape (options/subOptions/recommended). Auth token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-userinput-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-userinput-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p", cwd=CWD, shell=True)

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": d.get("profile_arn")}

META = {"kiro": {"userInput": True}}
proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD,
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

USERINPUTS = []     # captured request payloads
PERMS = []          # any session/request_permission (the fallback path)
AGENT = []
TURN_ENDS = [0]
def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is not None:
        if m == "_kiro/auth/getAccessToken":
            reply(rid, read_token())
        elif m == "_kiro/terminal/shell_type":
            reply(rid, {"shellType": "bash"})
        elif m == "_kiro/userInput":
            USERINPUTS.append(p)
            opts = p.get("options", [])
            # answer structurally: prefer the recommended option, else the first
            pick = next((o2 for o2 in opts if isinstance(o2, dict) and o2.get("recommended")),
                        opts[0] if opts else None)
            ans = (pick.get("title") if isinstance(pick, dict) else pick) if pick else "yes"
            log(f"  -> _kiro/userInput Q={p.get('question')!r} answering {ans!r}")
            reply(rid, {"action": "answered", "answer": ans})
        elif m == "session/request_permission":
            PERMS.append(p.get("toolCall", {}).get("title") or "?")
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()), opts[0] if opts else None)
            reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}})
        else:
            reply(rid, {})
        return
    if m and "session/update" in m:
        u = (p.get("update") or {}) if isinstance(p, dict) else {}
        if isinstance(u, dict):
            v = u.get("sessionUpdate")
            if v == "agent_message_chunk":
                AGENT.append(u.get("content", {}).get("text", ""))
            elif v == "session_info_update" and (((u.get("_meta") or {}).get("kiro") or {}).get("kind")) == "turn_end":
                TURN_ENDS[0] += 1

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
    rid = req(method, params)
    end = time.time() + to
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

call_sync("initialize", {"protocolVersion": 1, "clientCapabilities": {"_meta": META}})
# the user_input tool is spec-tagged, so trigger it via a spec createSpec flow
rr = call_sync("_kiro/spec/resolveSession", {"strategy": "fresh", "workspacePaths": [CWD]})
sid = (rr.get("result") or {}).get("sessionId") if rr and "result" in rr else None
log("# spec sessionId:", sid, "| advertised _meta.kiro.userInput=true")
req("_kiro/spec/invoke", {"operation": "createSpec", "sessionId": sid,
    "userPrompt": "Create a spec for a small CLI tool csv2json that converts a CSV file to a JSON array."})
before = TURN_ENDS[0]
end = time.time() + 280
while time.time() < end and TURN_ENDS[0] <= before:
    pump_once(10)
log("# turn complete:", TURN_ENDS[0] > before)
pump_once(4)

log("\n===== _kiro/userInput requests captured =====")
for ui in USERINPUTS:
    log("  question:", json.dumps(ui.get("question")))
    log("  options:", json.dumps(ui.get("options"))[:600])
    log("  full params:", json.dumps(ui)[:500])
if not USERINPUTS:
    log("  (NONE — agent did not route a question through _kiro/userInput)")
log("\n===== session/request_permission (fallback path) seen:", PERMS or "(none)")
log("\n===== VERDICT =====")
log("  _kiro/userInput fired:", bool(USERINPUTS))
txt = "".join(AGENT)
log("  agent acknowledged my choice (TypeScript):", "typescript" in txt.lower())
log("\n===== agent final message (head) =====")
log(txt[:700] or "(none)")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
