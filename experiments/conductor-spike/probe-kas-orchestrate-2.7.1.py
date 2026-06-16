#!/usr/bin/env python3
"""
Probe: capture KAS subagent/orchestration notification wire format (2.7.1).

Runs an authenticated KAS ACP session (--agent-engine kas), drives one prompt
turn that forces subagent orchestration with benign text-only work, and records
EVERY server->client message: session/update notifications, session/request_permission
requests, and any fs/* client callbacks. Auto-approves permissions and answers
fs callbacks so the turn completes unattended.

Usage:
  python3 probe-kas-subagent-2.7.1.py [KIRO_BIN]
Default KIRO_BIN = ~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat
"""
import json, os, subprocess, sys, threading, queue, time, tempfile, pathlib, sqlite3

AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
AUTH_KEY = "kirocli:social:token"

def read_kiro_token():
    """Read the current bearer token from kiro's own auth store.
    Returns {accessToken, expiresAt} for the _kiro/auth/getAccessToken callback.
    The secret never leaves this process / is never logged."""
    c = sqlite3.connect(AUTH_DB)
    try:
        row = c.execute("select value from auth_kv where key=?", (AUTH_KEY,)).fetchone()
    finally:
        c.close()
    if not row:
        return None
    v = row[0]
    if isinstance(v, (bytes, bytearray)):
        v = v.decode("utf-8", "replace")
    d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"],
            "profileArn": d.get("profile_arn"), "provider": d.get("provider")}

KIRO = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser(
    "~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
CWD = tempfile.mkdtemp(prefix="kas-probe-")
OUT = pathlib.Path(__file__).with_name("logs") / "probe-kas-orchestrate-2.7.1.log"
OUT.parent.mkdir(exist_ok=True)

PROMPT = (
    "Use the orchestrate_subagent tool — the multi-stage DAG pipeline tool, NOT individual "
    "invoke_subagent calls — to run a THREE-stage dependent pipeline (each stage a subagent). "
    "Stage 1 'pick': state the number 7. "
    "Stage 2 'double' (depends_on stage 'pick'): double the number from stage pick and state it. "
    "Stage 3 'report' (depends_on stage 'double'): write one sentence giving the final doubled value. "
    "The stages MUST run as a dependency chain (pick -> double -> report), not in parallel. "
    "Do not read or write files."
)

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin is not None and proc.stdout is not None
PROC_IN, PROC_OUT = proc.stdin, proc.stdout

log_f = open(OUT, "w")
def log(*a):
    line = " ".join(str(x) for x in a)
    print(line); log_f.write(line + "\n"); log_f.flush()

msgs = queue.Queue()
def reader():
    for line in PROC_OUT:
        line = line.strip()
        if line:
            msgs.put(line)
    msgs.put(None)
threading.Thread(target=reader, daemon=True).start()

_id = [10]
def send(method, params, is_req=True):
    m = {"jsonrpc": "2.0", "method": method, "params": params}
    if is_req:
        _id[0] += 1; m["id"] = _id[0]
    PROC_IN.write(json.dumps(m) + "\n"); PROC_IN.flush()
    return m.get("id")

def reply(req_id, result):
    PROC_IN.write(json.dumps({"jsonrpc": "2.0", "id": req_id, "result": result}) + "\n")
    PROC_IN.flush()

def handle_server_request(o):
    """Answer server->client requests so the turn can proceed."""
    method, rid, params = o.get("method"), o.get("id"), o.get("params", {})
    log(f"\n>>> SERVER REQUEST  {method}  id={rid}\n    {json.dumps(params)[:600]}")
    if method == "_kiro/auth/getAccessToken":
        tok = read_kiro_token()
        if tok:
            reply(rid, tok)
            log("    -> supplied kiro token (redacted), expiresAt=" + tok["expiresAt"])
        else:
            reply(rid, {})
            log("    -> NO TOKEN FOUND")
    elif method == "session/request_permission":
        opts = params.get("options", [])
        pick = next((op for op in opts if "allow" in (op.get("kind","")+op.get("optionId","")).lower()), opts[0] if opts else None)
        reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick
                   else {"outcome": {"outcome": "cancelled"}})
        log(f"    -> auto-allowed ({pick.get('optionId') if pick else None})")
    elif method and ("fs/read" in method or "readTextFile" in method or "read_text_file" in method):
        p = params.get("path", "")
        try: content = pathlib.Path(p).read_text()
        except Exception: content = ""
        reply(rid, {"content": content})
        log(f"    -> fs read answered for {p}")
    elif method and ("fs/write" in method or "writeTextFile" in method or "write_text_file" in method):
        reply(rid, {})
        log("    -> fs write ack")
    else:
        reply(rid, {})  # generic ack
        log("    -> generic ack")

def pump(until_id=None, timeout=180):
    """Read messages until response to until_id arrives, or timeout."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        try: raw = msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None:
            log("[stream closed]"); return None
        try: o = json.loads(raw)
        except Exception:
            log("[non-json]", raw[:200]); continue
        if "method" in o and "id" in o:
            handle_server_request(o)
        elif "method" in o:  # notification
            yield_notif(o)
        elif "id" in o:  # response
            tag = "ERR" if "error" in o else "ok"
            log(f"<<< RESPONSE id={o['id']} {tag}: {json.dumps(o.get('error') or o.get('result'))[:300]}")
            if until_id is not None and o["id"] == until_id:
                return o
    log("[timeout]"); return None

SUBAGENT_HITS = []
def yield_notif(o):
    m = o.get("method"); p = o.get("params", {})
    su = p.get("update") or p.get("sessionUpdate") or p
    s = json.dumps(o)
    interesting = any(k in s.lower() for k in ("subagent","orchestrat","crew","delegate","stage","child","sub_agent","invoke"))
    tag = "  ⟵SUBAGENT" if interesting else ""
    if interesting: SUBAGENT_HITS.append(o)
    # compact: print method + the sessionUpdate variant/type if present
    variant = su.get("sessionUpdate") or su.get("type") or (list(su.keys())[0] if isinstance(su,dict) and su else "")
    log(f"NOTIF {m} [{variant}]{tag}: {s[:400]}")

# ---- drive the session ----
log(f"# KIRO={KIRO}\n# CWD={CWD}\n# {time.strftime('%Y-%m-%dT%H:%M:%S')}")
send("initialize", {"protocolVersion": 1,
                    "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True},
                                           "_meta": {"kiro": {"settings": {"subagentOrchestration": {"enabled": True}}}}}})
pump(until_id=11, timeout=30)
# Enable the DAG orchestrator (off by default) per-session via _meta.kiro.settings —
# KAS reads parseSettings(kiroMeta?.settings); no global/agent-config mutation needed.
sid_id = send("session/new", {"cwd": CWD, "mcpServers": [],
                              "_meta": {"kiro": {"settings": {"subagentOrchestration": {"enabled": True}}}}})
resp = pump(until_id=sid_id, timeout=40)
sid = resp["result"]["sessionId"] if resp and "result" in resp else None
log(f"\n# sessionId = {sid}\n")
if not sid:
    log("FATAL: no session"); proc.kill(); sys.exit(1)

pid = send("session/prompt", {"sessionId": sid,
                              "prompt": [{"type": "text", "text": PROMPT}]})
pump(until_id=pid, timeout=240)

log(f"\n===== {len(SUBAGENT_HITS)} subagent-related notifications captured =====")
for o in SUBAGENT_HITS:
    log(json.dumps(o, indent=2))

PROC_IN.close(); proc.terminate()
log(f"\n# full log: {OUT}")
