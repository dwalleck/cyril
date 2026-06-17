#!/usr/bin/env python3
"""
Definitive KAS hooks behavior probe (gate ON, real hook, throwaway repo).

The hooks surface is gated behind `clientMeta.hooks.enabled && clientMeta.hooks.v2 === true`
— i.e. the CLIENT's `_meta.kiro.hooks` (SIBLING to `_meta.kiro.settings`, NOT inside it).
Bundle: `if (kiroMeta?.hooks?.enabled) { if (hooksConfig.v2 === true) { ... HooksModuleCache ... } }`.
(Earlier probes wrongly used `_meta.kiro.settings.{v2Hooks|hooks}` and `~/.kiro/settings/cli.json`
hooks — neither toggles it.)

This probe:
  1. Builds a TEMP repo with a real `.kiro/hooks/test.json` PreToolUse command hook that
     touches a marker file (does NOT touch the user's ~/.kiro).
  2. Enables hooks via `_meta.kiro.hooks={enabled:true,v2:true}`.
  3. Calls `_kiro/hooks/list` (expect the hook to appear).
  4. Runs a shell-tool turn and checks (a) whether the marker file was written
     (= hook ran server-side) and (b) whether any `_kiro/hooks/*` server->client
     notification fired.

NOTE on direction: the authoritative contract is `@kiro/acp-type-covenant/dist/capabilities/
hooks/types.d.ts`, which shows hooks are a HOST-CALLBACK model — when the client advertises
`_meta.kiro.hooks.enabled`, the agent calls `_kiro/hooks/list` + `_kiro/hooks/executeHook`
BACK on the client, and the CLIENT runs runCommand hooks. This probe is a passive client so it
does NOT reproduce firing (it returns no hooks and omits the required `trigger` param) — it only
confirms the enable path. The `@kiro/agent` processRunner is the in-process fallback for clients
that don't advertise hooks. Auth token self-sourced; never logged.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-hooks-enabled-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-hooks-en-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
MARKER = os.path.join(CWD, "HOOK_FIRED")
hooks_dir = pathlib.Path(CWD, ".kiro", "hooks"); hooks_dir.mkdir(parents=True)
(hooks_dir / "test.json").write_text(json.dumps({
    "version": 1,
    "hooks": [{
        "name": "mark-pretooluse",
        "description": "probe: record that a PreToolUse hook ran",
        "trigger": "preToolUse",
        "action": {"type": "command", "command": f"echo fired >> {MARKER}"},
    }],
}))
log(f"# CWD={CWD}  hook: PreToolUse command -> {MARKER}")

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": d.get("profile_arn")}

def shape(v, depth=0):
    if depth > 6:
        return "..."
    if isinstance(v, dict):
        return {k: shape(x, depth + 1) for k, x in v.items()}
    if isinstance(v, list):
        return ([shape(v[0], depth + 1), f"...(len={len(v)})"] if v else [])
    return type(v).__name__

META = {"kiro": {"hooks": {"enabled": True, "v2": True}}}
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

HOOK_NOTIFS = []
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
    if m and m.startswith("_kiro/hooks/"):
        HOOK_NOTIFS.append((m, shape(p)))

def pump(until, to=60):
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
log("# sessionId:", sid, "| hooks enabled via _meta.kiro.hooks={enabled:true,v2:true}")

hid = req("_kiro/hooks/list", {"sessionId": sid})
hr = pump(hid, 20)
log("\n===== _kiro/hooks/list (gate ON, one PreToolUse hook configured) =====")
if hr and "result" in hr:
    log("SHAPE:", json.dumps(shape(hr["result"])))
    log("RAW  :", json.dumps(hr["result"])[:800])
elif hr and "error" in hr:
    log("ERROR:", json.dumps(hr["error"])[:300])
else:
    log("(no response)")

pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "Run the shell command `echo hi` and report its output."}]})
pump(pid, 150)
time.sleep(1)

# re-list after the turn, in case workspace hook discovery is lazy
hid2 = req("_kiro/hooks/list", {"sessionId": sid})
hr2 = pump(hid2, 20)
log("\n===== _kiro/hooks/list (AFTER turn) =====")
log("  ", json.dumps(hr2.get("result")) if hr2 and "result" in hr2 else (json.dumps(hr2.get("error")) if hr2 else "(none)"))

fired = os.path.exists(MARKER)
log("\n===== HOOK EXECUTION =====")
log("  workspace .kiro/hooks/test.json loaded (hooks/list non-empty):", bool(hr and "result" in hr and hr["result"].get("hooks")))
log("  marker file written (PreToolUse command actually ran):", fired,
    "->", (open(MARKER).read().strip() if fired else "(absent)"))
log("  server->client _kiro/hooks/* notifications during turn:", HOOK_NOTIFS or "(none)")
log("\n===== CONCLUSION (see @kiro/acp-type-covenant/dist/capabilities/hooks/types.d.ts) =====")
log("  Enable path: client _meta.kiro.hooks={enabled:true} at initialize (SIBLING of _meta.kiro.settings,")
log("    NOT inside it; NOT ~/.kiro/settings/cli.json). Covenant: KiroClientMetaHooksExtension{hooks?:{enabled:true}}.")
log("  DIRECTION = HOST-CALLBACK (corrected): once advertised, the AGENT calls back to the CLIENT —")
log("    _kiro/hooks/list {trigger,sessionId,toolId?,...} -> client returns matching hooks; then")
log("    _kiro/hooks/executeHook {hookId,command,userPrompt,...} -> the CLIENT spawns the command and")
log("    returns {output,exitCode,cancelled}. Only runCommand crosses ACP; askAgent is agent-side prompt.")
log("  This probe could NOT reproduce firing because it acts as a passive client: it answered KAS's")
log("    _kiro/hooks/list callback with generic {} (no hooks) and its own client->server hooks/list omitted")
log("    the REQUIRED `trigger` param. A faithful repro must act as the hooks HOST: return a hook from list,")
log("    then service the executeHook callback. The @kiro/agent processRunner path is the in-process fallback")
log("    for clients that do NOT advertise hooks.")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
