#!/usr/bin/env python3
"""
Fire KAS hooks END-TO-END by acting as the HOST (the role the covenant says the
client plays). Per @kiro/acp-type-covenant/dist/capabilities/hooks/types.d.ts the
flow is agent->client:
  1. client advertises _meta.kiro.hooks={enabled:true} at initialize
  2. agent calls `_kiro/hooks/list` {trigger, sessionId, toolId?, toolTags?} -> client returns matching hooks
  3. for a runCommand hook the agent calls `_kiro/hooks/executeHook`
     {hookId, hookName, command, sessionId, userPrompt, timeout?} -> CLIENT runs it, returns {output, exitCode, cancelled}

This probe implements real `list` + `executeHook` responders: it OWNS a small hook
registry (a runCommand hook per trigger) and actually runs the command. Then it runs
a shell-tool turn (`echo hi`) and records the full callback sequence so we can see:
  - which triggers the agent queries, and with what toolId/toolTags
  - the userPrompt payload per trigger (covenant: preToolUse=JSON args; postToolUse=
    JSON {toolName,toolArgs,toolResult,toolSuccess}; promptSubmit/agentStop=prompt text)
  - whether the agent incorporates hook output into the turn

The hook command is host-defined (`echo ...`), so running it is safe. Auth token
self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-hooks-host-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-hooks-host-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": d.get("profile_arn")}

META = {"kiro": {"hooks": {"enabled": True}}}
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

LIST_CALLS = []     # (trigger, toolId, toolTags)
EXEC_CALLS = []     # (hookName, command, userPrompt[:200], exitCode)
AGENT = []

def hook_for(trigger, tool_id):
    """Owned registry: one runCommand hook per trigger, echoing an identifiable marker."""
    return {
        "id": f"hook-{trigger}",
        "name": f"probe-{trigger}",
        "action": {"type": "runCommand", "command": f"echo HOOKFIRED trigger={trigger} tool={tool_id or '-'}"},
        "approved": True,
    }

def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is not None:
        if m == "_kiro/auth/getAccessToken":
            reply(rid, read_token())
        elif m == "_kiro/terminal/shell_type":
            reply(rid, {"shellType": "bash"})
        elif m == "_kiro/hooks/list":
            trig = p.get("trigger"); tool_id = p.get("toolId"); tags = p.get("toolTags")
            LIST_CALLS.append((trig, tool_id, tags))
            reply(rid, {"hooks": [hook_for(trig, tool_id)]})
        elif m == "_kiro/hooks/executeHook":
            cmd = p.get("command", ""); up = p.get("userPrompt", ""); name = p.get("hookName", "")
            # HOOK_BLOCK mode: deny the PreToolUse hook (non-zero exit + message) to test
            # whether a runCommand hook can BLOCK the tool (Claude-Code exit-code-2 convention).
            if os.environ.get("HOOK_BLOCK") and "preToolUse" in (name or ""):
                EXEC_CALLS.append((name, cmd, up[:200], 2))
                reply(rid, {"output": "DENY: blocked by probe hook policy (no echo allowed)", "exitCode": 2, "cancelled": False})
                return
            try:
                r = subprocess.run(cmd, shell=True, cwd=CWD, capture_output=True, text=True, timeout=p.get("timeout") or 30)
                out, code = (r.stdout + r.stderr).strip(), r.returncode
            except Exception as e:
                out, code = f"(host error: {e})", 1
            EXEC_CALLS.append((name, cmd, up[:200], code))
            reply(rid, {"output": out, "exitCode": code, "cancelled": False})
        elif m == "_kiro/hooks/sessionStart":
            reply(rid, {"results": []})
        elif m == "session/request_permission":
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()), opts[0] if opts else None)
            reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}})
        else:
            reply(rid, {})
        return
    # notifications
    if m and ("session/update" in m):
        u = (p.get("update") or {}) if isinstance(p, dict) else {}
        if isinstance(u, dict) and u.get("sessionUpdate") == "agent_message_chunk":
            AGENT.append(u.get("content", {}).get("text", ""))

def pump(until, to=180):
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
            if o.get("id") == until and "result" in o:
                return o
        elif "id" in o and o["id"] == until:
            return o
    return None

req("initialize", {"protocolVersion": 1, "clientCapabilities": {"_meta": META}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": [], "_meta": META})
nr = pump(nid, 40)
assert nr and "result" in nr, "session/new failed"
sid = nr["result"]["sessionId"]
log("# sessionId:", sid, "| acting as hooks HOST (own registry + run executeHook)")

pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "Run the shell command `echo hi` and report its output verbatim."}]})
pump(pid, 170)
time.sleep(1)

log("\n===== _kiro/hooks/list callbacks (agent -> host): which triggers, what tool =====")
for trig, tid, tags in LIST_CALLS:
    log(f"  trigger={trig!r:24} toolId={tid!r} toolTags={tags!r}")
log(f"  ({len(LIST_CALLS)} list calls)")

log("\n===== _kiro/hooks/executeHook callbacks (agent -> host; host ran the command) =====")
for name, cmd, up, code in EXEC_CALLS:
    log(f"  hook={name!r} exit={code}  cmd={cmd!r}")
    log(f"     userPrompt[:200]={up!r}")
log(f"  ({len(EXEC_CALLS)} execute calls)")

review = "".join(AGENT)
log("\n===== agent final message (did it incorporate hook output 'HOOKFIRED'?) =====")
log("  contains 'HOOKFIRED':", "HOOKFIRED" in review)
log("  head:", review[:500] or "(nothing)")

triggers = [t for t, _, _ in LIST_CALLS]
tool_ran = "postToolUse" in triggers  # postToolUse only fires after the tool executes
log("\n===== VERDICT =====")
log("  mode:", "BLOCK (preToolUse denied, exit 2)" if os.environ.get("HOOK_BLOCK") else "observe (benign)")
log("  hooks/list called by agent:", bool(LIST_CALLS), "triggers:", triggers)
log("  executeHook called (host ran a runCommand hook):", bool(EXEC_CALLS))
log("  postToolUse fired => the shell tool actually executed:", tool_ran)
log("  => end-to-end host-callback hooks", "FIRED" if EXEC_CALLS else "did NOT fire")
if os.environ.get("HOOK_BLOCK"):
    log("  => BLOCKING:", "tool was BLOCKED by the PreToolUse hook" if not tool_ran else "tool still ran (NOT blocked)")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
