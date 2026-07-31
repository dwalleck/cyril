#!/usr/bin/env python3
"""Re-probe the 8 KAS methods that failed the RPC sweep, with corrections from the bundle.

probe-kas-rpc-sweep-2.16.0.py left 8 methods on a generic -32603. Reading the bundle
shows the sweep was wrong about them in three different ways — only ONE was a real
param-name bug:

  DIRECTION ERRORS (agent -> client; calling them as client -> agent RPCs is
  meaningless, and -32603 was the honest answer):
    _kiro/secret/get            sendClientExtMethod(conn, "_kiro/secret/get", {key})
    _kiro/tool/get_diagnostics  a CLIENT tool: metaKey "clientToolGetDiagnostics"
    _kiro/sandbox/status        outbound.extNotification(...) — a push
    _kiro/hooks/list            sendClientExtMethod(conn, "_kiro/hooks/list",
                                {trigger, sessionId, toolId?, toolTags?}) -> {hooks}

  ENVELOPE ERRORS (handled on the notification path — the branch ends in
  `return Promise.resolve()` with no result, so a REQUEST id gets nothing back):
    _kiro/powers/refresh
    _kiro/mcp/toggle            reads only `params.enabled`

  PARAM-NAME ERRORS (real handlers, wrong keys sent):
    _kiro/permissions/explain   handlePermissionsExplain: needs sessionId plus
                                `capability` OR `toolId` — the sweep sent `toolName`
    _session/steer              handleSessionSteer: needs {sessionId, message,
                                messageId?} — the sweep sent `text`

This re-runs only those, correctly, so each gets a real verdict.

Cost: no model turn. Free.

    probe-kas-rpc-corrections-2.16.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

SECRET_KEYS = {"accessToken", "access_token", "refreshToken", "refresh_token",
               "idToken", "id_token", "clientSecret", "client_secret", "bearer",
               "profileArn", "profile_arn", "authorization", "Authorization"}


def redact(obj):
    if isinstance(obj, dict):
        return {k: ("<redacted>" if k in SECRET_KEYS and obj[k] else redact(obj[k]))
                for k in obj}
    if isinstance(obj, list):
        return [redact(x) for x in obj]
    return obj


def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key in "
                        "('kirocli:odic:token','kirocli:social:token') order by key desc").fetchone()
        prow = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        c.close()
    v = row[0]
    v = v.decode() if isinstance(v, (bytes, bytearray)) else v
    d = json.loads(v)
    parn = d.get("profile_arn")
    if not parn and prow:
        pv = prow[0]
        pv = pv.decode() if isinstance(pv, (bytes, bytearray)) else pv
        try:
            parn = json.loads(pv).get("arn")
        except Exception:
            pass
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": parn}


TOK = read_token()
CWD = tempfile.mkdtemp(prefix="kas-fix-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-fix-home-")
env = dict(os.environ)
env["HOME"] = TMPH
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
PUSHED = []


def emit(d, e, m, parsed):
    OUT.write(json.dumps({"direction": d, "envelope": e, "method": m,
                          "parsed": redact(parsed)}) + "\n")
    OUT.flush()


def send(obj, method=None, envelope="request"):
    p.stdin.write(json.dumps(obj) + "\n")
    p.stdin.flush()
    emit("client_to_agent", envelope, method, obj)


def req(m, pr):
    i[0] += 1
    send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}, method=m)
    return i[0]


def notify(m, pr):
    send({"jsonrpc": "2.0", "method": m, "params": pr}, method=m, envelope="notification")


def reply(rid, res):
    send({"jsonrpc": "2.0", "id": rid, "result": res}, envelope="response")


def pump(until, to=60):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = q.get(timeout=2)
        except queue.Empty:
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        m, rid, pr = o.get("method"), o.get("id"), o.get("params") or {}
        emit("agent_to_client",
             "notification" if (m and rid is None) else ("request" if m else "response"), m, o)
        if m and rid is None and m.startswith(("_kiro/", "_session/")):
            PUSHED.append(m)
        if rid is not None and m:
            if m == "_kiro/auth/getAccessToken":
                reply(rid, TOK)
            elif m == "_kiro/terminal/shell_type":
                reply(rid, {"shellType": "bash"})
            else:
                reply(rid, {})
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


def call(label, m, pr, to=60):
    r = pump(req(m, pr), to)
    if r is None:
        print(f"  TIMEOUT    {label}")
    elif "error" in r:
        print(f"  ERR {r['error'].get('code')}  {label}  {json.dumps(r['error'].get('message'))[:170]}")
    else:
        print(f"  OK         {label}  {json.dumps(r.get('result'))[:230]}")
    return r


req("initialize", {"protocolVersion": 1,
                   "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}},
                   "_meta": {"kiro": {"clientName": "cyril-audit"}}})
pump(1, 40)
sid = (pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 90) or {}) \
    .get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 6)

print("\n########## PARAM-NAME corrections ##########")
call("permissions/explain (toolId)", "_kiro/permissions/explain",
     {"sessionId": sid, "toolId": "fs_read"})
call("permissions/explain (capability)", "_kiro/permissions/explain",
     {"sessionId": sid, "capability": "fs_write"})
call("_session/steer (message)", "_session/steer",
     {"sessionId": sid, "message": "cyril audit steer probe"})

print("\n########## ENVELOPE corrections — send as NOTIFICATIONS ##########")
before = len(PUSHED)
notify("_kiro/powers/refresh", {"sessionId": sid})
notify("_kiro/mcp/toggle", {"sessionId": sid, "enabled": False})
notify("_kiro/policy/ignore_files_changed", {"files": ["**/secret.txt"]})
pump(-1, 12)
print(f"  sent 3 notifications; no error frames expected. "
      f"agent pushes seen after: {PUSHED[before:] or '(none)'}")

print("\n########## DIRECTION errors — confirm these are agent->client only ##########")
for label, m, pr in [
    ("secret/get", "_kiro/secret/get", {"key": "cyril-audit-probe"}),
    ("hooks/list", "_kiro/hooks/list", {"sessionId": sid, "trigger": "sessionStart"}),
    ("sandbox/status", "_kiro/sandbox/status", {"sessionId": sid}),
    ("tool/get_diagnostics", "_kiro/tool/get_diagnostics", {"sessionId": sid, "paths": ["probe.txt"]}),
]:
    call(f"{label} (expect failure — agent->client)", m, pr, to=30)

print("\nNOTE: failures in the last block are the EXPECTED result — these four are "
      "sent BY the agent TO the client, so there is no agent-side handler to answer them.")

OUT.close()
p.stdin.close()
p.terminate()
