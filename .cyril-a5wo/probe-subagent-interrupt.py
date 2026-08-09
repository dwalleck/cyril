#!/usr/bin/env python3
"""Live 2.16.2 KAS subagent-cancel capture; max three fresh attempts."""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time
KIRO = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser(
    "~/.local/share/kiro-research/binaries/2.16.2/kiro-cli-chat")
OUTDIR = sys.argv[2] if len(sys.argv) > 2 else ".cyril-a5wo/captures"
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
SECRETS = {"accessToken", "access_token", "refreshToken", "refresh_token",
           "idToken", "id_token", "profileArn", "profile_arn", "authorization"}
os.makedirs(OUTDIR, exist_ok=True)
def token():
    db = sqlite3.connect(AUTH)
    try:
        row = db.execute("select value from auth_kv where key in "
                         "('kirocli:odic:token','kirocli:social:token') "
                         "order by key desc").fetchone()
        profile = db.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        db.close()
    if row is None:
        raise RuntimeError("no Kiro auth token")
    raw = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
    data = json.loads(raw)
    arn = None
    if profile:
        pv = profile[0].decode() if isinstance(profile[0], (bytes, bytearray)) else profile[0]
        try:
            arn = json.loads(pv).get("arn")
        except (TypeError, json.JSONDecodeError):
            pass
    return {"accessToken": data["access_token"], "expiresAt": data["expires_at"], "profileArn": arn}
def scrub(value):
    if isinstance(value, dict):
        return {k: ("<redacted>" if k in SECRETS and value[k] else scrub(value[k])) for k in value}
    return [scrub(x) for x in value] if isinstance(value, list) else value
def attempt(n, tok):
    cwd, home = tempfile.mkdtemp(prefix="kas-interrupt-"), tempfile.mkdtemp(prefix="kas-interrupt-home-")
    env = dict(os.environ, HOME=home, XDG_DATA_HOME=os.path.expanduser("~/.local/share"))
    proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=cwd, env=env,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, text=True, bufsize=1)
    q, frames, ids, rpc = queue.Queue(), [], [0], {"sid": None, "cancelled": False}
    threading.Thread(target=lambda: [q.put(x.strip()) for x in proc.stdout if x.strip()], daemon=True).start()
    path = os.path.join(OUTDIR, f"attempt-{n}.jsonl")
    out = open(path, "w", encoding="utf-8")
    def emit(direction, obj, envelope=None):
        rec = {"direction": direction, "envelope": envelope or ("notification" if obj.get("method") and obj.get("id") is None else "request"), "method": obj.get("method"), "parsed": scrub(obj)}
        frames.append(rec); out.write(json.dumps(rec) + "\n"); out.flush()
    def send(obj):
        proc.stdin.write(json.dumps(obj) + "\n"); proc.stdin.flush(); emit("client_to_agent", obj)
    def req(method, params):
        ids[0] += 1; send({"jsonrpc": "2.0", "id": ids[0], "method": method, "params": params}); return ids[0]
    def reply(obj):
        if obj.get("method") == "_kiro/auth/getAccessToken": result = tok
        elif obj.get("method") == "_kiro/terminal/shell_type": result = {"shellType": "bash"}
        elif obj.get("method") in ("terminal/create", "_kiro/terminal/create"): result = {"terminalId": "cancelled"}
        elif obj.get("method") in ("terminal/output", "_kiro/terminal/output"): result = {"output": "", "truncated": False, "exitStatus": {"exitCode": 0, "signal": None}}
        elif obj.get("method") in ("terminal/wait_for_exit", "_kiro/terminal/wait_for_exit"): result = {"exitCode": 0, "signal": None}
        elif obj.get("method", "").endswith(("terminal/release", "terminal/kill")): result = {}
        elif obj.get("method") == "session/request_permission": result = {"outcome": {"outcome": "cancelled"}}
        else: result = {}
        send({"jsonrpc": "2.0", "id": obj["id"], "result": result})
    def pump(wait_id, limit=180):
        end = time.time() + limit
        while time.time() < end:
            try: raw = q.get(timeout=2)
            except queue.Empty: continue
            try: obj = json.loads(raw)
            except json.JSONDecodeError: continue
            emit("agent_to_client", obj)
            if obj.get("method") and obj.get("id") is not None: reply(obj)
            p = obj.get("params") or {}; u = p.get("update") or {}
            if u.get("sessionUpdate") == "tool_call":
                meta = ((u.get("_meta") or {}).get("kiro") or {})
                if meta.get("kind") == "agent-subtask":
                    if not rpc["cancelled"] and rpc["sid"]:
                        rpc["cancelled"] = True; send({"jsonrpc": "2.0", "method": "session/cancel", "params": {"sessionId": rpc["sid"]}})
            if obj.get("id") == wait_id and ("result" in obj or "error" in obj): return obj
        return None
    try:
        pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": True}, "terminal": True}, "_meta": {"kiro": {"clientName": "cyril-a5wo-probe", "settings": {"subagentOrchestration": {"enabled": True}, "inlineAgents": {"enabled": True}}}}}), 60)
        created = pump(req("session/new", {"cwd": cwd, "mcpServers": []}), 90)
        rpc["sid"] = ((created or {}).get("result") or {}).get("sessionId")
        if not rpc["sid"]: raise RuntimeError("session/new returned no sessionId")
        prompt = ("Use a subagent to run a long terminal command: sleep 20. "
                  "Do not run the command yourself. Report only after the subagent finishes.")
        pump(req("session/prompt", {"sessionId": rpc["sid"], "prompt": [{"type": "text", "text": prompt}]}), 180)
        time.sleep(3)
        return path, rpc["cancelled"], sum(1 for f in frames if f["method"] == "session/update")
    finally:
        out.close(); proc.terminate(); proc.wait(timeout=10)

if not os.path.exists(KIRO): raise SystemExit(f"missing binary: {KIRO}")
tok = token()
for n in range(1, 4):
    try:
        path, cancelled, updates = attempt(n, tok)
        print(json.dumps({"attempt": n, "capture": path, "cancel_injected": cancelled, "session_updates": updates}))
    except Exception as exc:
        print(json.dumps({"attempt": n, "error": repr(exc)}))
