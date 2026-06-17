#!/usr/bin/env python3
"""
Fire the client-side LSP tool callbacks: `_kiro/tool/{get_diagnostics, semantic_rename,
smart_relocate}`. These are agent->client REQUESTS — when the client advertises the
matching `clientTool*` flag, KAS delegates the tool's execution to the client.

Gated by `clientCapabilities._meta.kiro.{clientToolGetDiagnostics, clientToolSemanticRename,
clientToolSmartRelocate}: true`. This probe advertises all three, prompts the agent to
diagnose / rename / relocate, returns canned results, and CAPTURES the exact request
params KAS sends for each (the deliverable). Auth token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-client-tools-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-clienttools-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p", cwd=CWD, shell=True)
pathlib.Path(CWD, "src").mkdir()
pathlib.Path(CWD, "src", "calc.ts").write_text(
    "export function add(a: number, b: number): number {\n  return a + b;\n}\n\n"
    "export function useAdd(): number {\n  return add(2, 3);\n}\n")
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

META = {"kiro": {"clientToolGetDiagnostics": True, "clientToolSemanticRename": True, "clientToolSmartRelocate": True}}
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

TOOL_REQS = []      # (method, params) captured — the deliverable
AGENT = []
TURN_ENDS = [0]
def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is not None:
        if m == "_kiro/auth/getAccessToken":
            reply(rid, read_token())
        elif m == "_kiro/terminal/shell_type":
            reply(rid, {"shellType": "bash"})
        elif m == "_kiro/tool/get_diagnostics":
            TOOL_REQS.append((m, p)); reply(rid, {"diagnostics": {}, "errors": {}, "message": "No diagnostics found."})
        elif m == "_kiro/tool/semantic_rename":
            TOOL_REQS.append((m, p))
            reply(rid, {"success": True, "filesChanged": 1, "editsApplied": 2, "message": "Renamed symbol.",
                        "fileChanges": [{"file": p.get("path", "src/calc.ts"), "local": "", "original": "add", "modified": p.get("newName", "sum")}]})
        elif m == "_kiro/tool/smart_relocate":
            TOOL_REQS.append((m, p)); reply(rid, {"success": True, "message": "Relocated file and updated imports."})
        elif m == "session/request_permission":
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

ir = call_sync("initialize", {"protocolVersion": 1, "clientCapabilities": {"_meta": META}})
log("# agentCapabilities:", json.dumps((ir.get("result") or {}).get("agentCapabilities", {}))[:160] if ir and "result" in ir else "?")
nr = call_sync("session/new", {"cwd": CWD, "mcpServers": [], "_meta": META})
sid = (nr.get("result") or {}).get("sessionId") if nr and "result" in nr else None
log("# sessionId:", sid, "| advertised clientTool{GetDiagnostics,SemanticRename,SmartRelocate}=true")

PROMPT = ("Do these three steps using your dedicated tools (not the generic code tool):\n"
          "1) Run get_diagnostics on src/calc.ts.\n"
          "2) Use semantic_rename to rename the function `add` to `sum` in src/calc.ts.\n"
          "3) Use smart_relocate to move src/calc.ts to src/math/calc.ts.\n"
          "Report what each tool returned.")
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
before = TURN_ENDS[0]
end = time.time() + 200
while time.time() < end and TURN_ENDS[0] <= before:
    pump_once(10)
log("# turn complete:", TURN_ENDS[0] > before)
pump_once(4)

log("\n===== client-tool callbacks captured (method + params KAS sent) =====")
for m, p in TOOL_REQS:
    log(f"\n  {m}")
    log(f"    params: {json.dumps(p)[:500]}")
fired = {m for m, _ in TOOL_REQS}
log("\n===== VERDICT =====")
for mm in ("_kiro/tool/get_diagnostics", "_kiro/tool/semantic_rename", "_kiro/tool/smart_relocate"):
    log(f"  {mm}: {'FIRED' if mm in fired else 'not fired'}")
log("\n===== agent final message (head) =====")
log("".join(AGENT)[:700] or "(none)")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
