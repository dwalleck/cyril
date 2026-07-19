#!/usr/bin/env python3
"""cyril-jiyn Probe: enabled-only vs enabled+v2 hooks A/B on kiro-cli 2.13.0 KAS.

ARM=host  -> _meta.kiro.hooks = {"enabled": true}
ARM=v2    -> _meta.kiro.hooks = {"enabled": true, "v2": true}
Constant across arms: a workspace disk hook .kiro/hooks/probe.json
(UserPromptSubmit command hook: touch an absolute marker + echo HOOKV2FIRED)
and one real turn running `echo hi`.

Pre-registered expectations (oracle = the 2.13.0 buildSessionHooks carve,
.cyril-jiyn/oracle-buildSessionHooks.txt — v2 binding REPLACES v1 wholesale
for sessions with a workspace):
  host arm: LIST>0 and EXEC>0 (host owns the registry); marker ABSENT.
  v2 arm:   LIST==0 and EXEC==0 (turn-driven host callbacks unreachable);
            marker PRESENT (standalone loader ran the disk hook agent-side).
Auth: odic token + profileArn from state/api.codewhisperer.profile (the
social-token key the 2.7.1 probes used no longer exists in the store).
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time
from pathlib import Path

ARM = os.environ.get("ARM", "host")
META_HOOKS = {"enabled": True} if ARM == "host" else {"enabled": True, "v2": True}
DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
OUT = Path(__file__).parent / f"ab-results-{ARM}"
OUT.mkdir(exist_ok=True)

CWD = tempfile.mkdtemp(prefix=f"jiyn-{ARM}-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
MARKER = os.path.join(CWD, "v2-marker")
hooks_dir = Path(CWD) / ".kiro" / "hooks"
hooks_dir.mkdir(parents=True)
(hooks_dir / "probe.json").write_text(json.dumps({
    "version": "v1",
    "hooks": [{"name": "jiyn-probe-prompt-hook", "trigger": "UserPromptSubmit",
               "action": {"type": "command",
                          "command": f"touch {MARKER} && echo HOOKV2FIRED"}}],
}))

def token():
    c = sqlite3.connect(DB)
    tok = json.loads(c.execute(
        "select value from auth_kv where key='kirocli:odic:token'").fetchone()[0])
    arn = c.execute(
        "select value from state where key='api.codewhisperer.profile'").fetchone()[0]
    arn = arn.decode() if isinstance(arn, (bytes, bytearray)) else arn
    if arn.strip().startswith('"'):
        arn = json.loads(arn)
    return {"accessToken": tok["access_token"], "expiresAt": tok["expires_at"],
            "profileArn": arn}

proc = subprocess.Popen(["kiro-cli", "acp", "--agent-engine", "kas"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
msgs: "queue.Queue[str|None]" = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in proc.stdout if l.strip()],
                                 msgs.put(None)), daemon=True).start()
_id = [0]
def req(m, p):
    _id[0] += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p}) + "\n")
    proc.stdin.flush(); return _id[0]
def reply(rid, res):
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    proc.stdin.flush()

LIST, EXEC, OTHER_HOOK, AGENT = [], [], [], []

def handle(o):
    m, rid, p = o.get("method"), o.get("id"), o.get("params", {}) or {}
    if rid is not None:
        if m == "_kiro/auth/getAccessToken":
            reply(rid, token())
        elif m == "_kiro/terminal/shell_type":
            reply(rid, {"shellType": "bash"})
        elif m == "_kiro/hooks/list":
            LIST.append((p.get("trigger"), p.get("toolId")))
            reply(rid, {"hooks": [{"id": f"h-{p.get('trigger')}", "name": f"probe-{p.get('trigger')}",
                                   "action": {"type": "runCommand",
                                              "command": f"echo HOSTHOOKFIRED trigger={p.get('trigger')}"},
                                   "approved": True}]})
        elif m == "_kiro/hooks/executeHook":
            cmd = p.get("command", "")
            r = subprocess.run(cmd, shell=True, cwd=CWD, capture_output=True, text=True, timeout=30)
            EXEC.append((p.get("hookName"), cmd, r.returncode))
            reply(rid, {"output": (r.stdout + r.stderr).strip(), "exitCode": r.returncode, "cancelled": False})
        elif m and m.startswith("_kiro/hooks/"):
            OTHER_HOOK.append((m, json.dumps(p)[:120])); reply(rid, {"results": []})
        elif m == "session/request_permission":
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (str(x.get("kind", "")) + str(x.get("optionId", ""))).lower()),
                        opts[0] if opts else None)
            reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick
                  else {"outcome": {"outcome": "cancelled"}})
        else:
            reply(rid, {})
        return
    if m == "session/update":
        u = p.get("update") or {}
        if u.get("sessionUpdate") == "agent_message_chunk":
            AGENT.append(u.get("content", {}).get("text", ""))
        if m2 := (u.get("_meta") or {}).get("kiro", {}).get("hookConfirm"):
            OTHER_HOOK.append(("hookConfirm-meta", json.dumps(m2)[:120]))

def pump(until, to):
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
        except json.JSONDecodeError:
            continue
        if "method" in o:
            handle(o)
        elif o.get("id") == until:
            return o
    return None

req("initialize", {"protocolVersion": 1,
                   "clientCapabilities": {"_meta": {"kiro": {"hooks": META_HOOKS}}}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": [],
                          "_meta": {"kiro": {"hooks": META_HOOKS}}})
nr = pump(nid, 40)
assert nr and "result" in nr, f"session/new failed: {nr}"
sid = nr["result"]["sessionId"]
pid = req("session/prompt", {"sessionId": sid,
                             "prompt": [{"type": "text",
                                         "text": "Run the shell command `echo hi` and report its output verbatim."}]})
pr = pump(pid, 170)
time.sleep(2)
marker = os.path.exists(MARKER)
result = {"arm": ARM, "meta": META_HOOKS, "prompt_completed": bool(pr and "result" in pr),
          "list_calls": LIST, "exec_calls": EXEC, "other_hook": OTHER_HOOK,
          "marker_file_created": marker, "agent_saw_HOOKV2FIRED": "HOOKV2FIRED" in "".join(AGENT),
          "agent_saw_HOSTHOOKFIRED": "HOSTHOOKFIRED" in "".join(AGENT)}
(OUT / "result.json").write_text(json.dumps(result, indent=2))
print(json.dumps(result, indent=2))
if ARM == "host":
    ok = bool(LIST) and bool(EXEC) and not marker
else:
    ok = not LIST and not EXEC and marker
print(f"ARM={ARM} EXPECTATION:", "MATCH" if ok else "MISMATCH")
proc.stdin.close(); proc.terminate()
sys.exit(0 if ok else 1)
