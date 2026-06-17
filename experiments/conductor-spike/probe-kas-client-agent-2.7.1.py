#!/usr/bin/env python3
"""
Fire CLIENT-INJECTED custom agents — the native skill/agent-injection hook.

Per the covenant (session/types.d.ts): `session/new` accepts
`_meta.kiro.customAgents: ClientCustomAgent[]`, and CustomAgentSource.CLIENT_PROVIDED
is "highest precedence — overrides all file-based sources." This is the alternative
to proxy file-rewriting: cyril hands KAS an agent definition at session start.

Test: inject a `pirate-reviewer` agent with a DISTINCTIVE prompt (always pirate
speak, ends every sentence with "Arrr!"), enable subagentOrchestration, then ask the
main agent to run an `orchestrate_subagent` stage whose `role` is the injected agent.
If the subagent's output is pirate speak, the client-injected agent loaded AND ran as
a first-class role. Also watches for `_kiro/customAgent/{not_found,config_error}`
(would mean the injection was rejected). Auth token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-client-agent-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-clientagent-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p", cwd=CWD, shell=True)
pathlib.Path(CWD, "README.md").write_text("# widget\nA function add(a,b) returns a+b. No tests yet.\n")
subprocess.run("git add -A && git commit -qm baseline", cwd=CWD, shell=True)
log(f"# CWD={CWD}")

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": d.get("profile_arn")}

CLIENT_AGENT = {
    "id": "pirate-reviewer",
    "description": "A code reviewer who speaks like a pirate.",
    "prompt": ("You are Pirate Reviewer. You ALWAYS write in exaggerated pirate speak "
               "(ahoy, matey, ye, arrr, ye scurvy code) and you END EVERY SENTENCE WITH the word "
               "'Arrr!'. Review whatever you are asked to review, briefly."),
    "tools": ["fs_read", "grep_search"],
    "permissions": {"rules": [{"capability": "fs_read", "match": ["./**"], "effect": "allow"}]},
}
SETTINGS = {"kiro": {"settings": {"subagentOrchestration": {"enabled": True}}}}
NEW_META = {"kiro": {"settings": {"subagentOrchestration": {"enabled": True}}, "customAgents": [CLIENT_AGENT]}}

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

AGENT = []
TOOLS = []
ROLES = set()
CUSTOM_AGENT_NOTIFS = []
ERRORS = []
TURN_ENDS = [0]
def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is not None:
        if m == "_kiro/auth/getAccessToken":
            reply(rid, read_token())
        elif m == "_kiro/terminal/shell_type":
            reply(rid, {"shellType": "bash"})
        elif m == "session/request_permission":
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()), opts[0] if opts else None)
            reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}})
        else:
            reply(rid, {})
        return
    if not m:
        return
    if "customAgent" in m:
        CUSTOM_AGENT_NOTIFS.append((m, json.dumps(p)[:200]))
    if "session/update" in m:
        u = (p.get("update") or {}) if isinstance(p, dict) else {}
        if isinstance(u, dict):
            v = u.get("sessionUpdate")
            if v == "agent_message_chunk":
                AGENT.append(u.get("content", {}).get("text", ""))
            elif v == "session_info_update":
                if (((u.get("_meta") or {}).get("kiro") or {}).get("kind")) == "turn_end":
                    TURN_ENDS[0] += 1
            elif v in ("tool_call", "tool_call_update"):
                TOOLS.append((v, u.get("title") or u.get("toolCallId"), u.get("status")))
                ri = u.get("rawInput") or {}
                for st in (ri.get("stages") or []):
                    if st.get("role"):
                        ROLES.add(st["role"])
                pl = ((u.get("_meta") or {}).get("kiro") or {}).get("pipeline")
                if pl:
                    for st in pl.get("stages", []):
                        if st.get("role"):
                            ROLES.add(st["role"])

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
        elif "id" in o and "error" in o:
            ERRORS.append(o["error"])
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

call_sync("initialize", {"protocolVersion": 1, "clientCapabilities": {"_meta": SETTINGS}})
nr = call_sync("session/new", {"cwd": CWD, "mcpServers": [], "_meta": NEW_META})
sid = (nr.get("result") or {}).get("sessionId") if nr and "result" in nr else None
log("# sessionId:", sid, "| injected customAgent id=pirate-reviewer")

PROMPT = ("Use the orchestrate_subagent tool to run a single stage named 'review' with "
          "role 'pirate-reviewer' (use that exact role name): have it review README.md in this repo. "
          "Then report the sub-agent's review verbatim.")
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
before = TURN_ENDS[0]
end = time.time() + 220
while time.time() < end and TURN_ENDS[0] <= before:
    pump_once(10)
log("# turn complete:", TURN_ENDS[0] > before)
pump_once(4)

log("\n===== customAgent notifications (not_found/config_error = injection rejected) =====")
for m, b in CUSTOM_AGENT_NOTIFS:
    log(f"  {m}: {b}")
if not CUSTOM_AGENT_NOTIFS:
    log("  (none — no rejection)")
log("\n===== JSON-RPC errors =====", [json.dumps(e)[:160] for e in ERRORS] or "(none)")
log("\n===== roles seen in orchestrate pipeline/rawInput =====", sorted(ROLES))
log("\n===== tool calls (head) =====")
for k, t, st in TOOLS[:25]:
    log(f"  [{k}] {t} -> {st}")
text = "".join(AGENT)
markers = [w for w in ("Arrr", "arrr", "matey", "ahoy", "ye ", "scurvy") if w in text]
log("\n===== pirate-speak markers in output =====", markers or "(none)")
log("\n===== VERDICT =====")
log("  injected agent used as a role:", "pirate-reviewer" in ROLES)
log("  injected agent's distinctive behavior present (pirate speak):", bool(markers))
log("  injection rejected:", bool(CUSTOM_AGENT_NOTIFS) or bool(ERRORS))
log("\n===== output (head) =====")
log(text[:900] or "(none)")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
