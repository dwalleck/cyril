#!/usr/bin/env python3
"""Sweep every client-callable `_kiro/*` RPC and record its real response shape.

Coverage motivation: the KAS method surface is 106 literals, and a scan of every
committed capture in this tree shows only ~21 have ever appeared on the wire. Most
of the rest are ordinary client->agent RPCs that nothing has ever called — so their
response shapes are known only from the bundle, if at all.

This calls them all against one live session and records what comes back, including
typed errors. An error is a RESULT here, not a failure: `-32601` means unadvertised,
`-32602`/`-32000` mean the method is wired and rejecting our arguments, and both are
more informative than never calling it.

Also probes a capability question the audit surfaced: KAS has only ever used the
ACP-form `fs/read_text_file` against our probes, never `_kiro/fs/*`. capabilitiesFrom()
reads separate `kiroFsReadFile`/`kiroFsWriteFile`/`kiroFsStat` flags that no probe has
advertised. This one advertises them, so the fs dialect KAS chooses is observable.

Cost: ONE short turn (so history/export/context/compact have something to act on).
Everything else is free.

Ordering is deliberate: read-only methods first, mutating ones last, and the
destructive `_kiro/session/delete` runs against a THROWAWAY forked session, never
the main one.

    probe-kas-rpc-sweep-2.16.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, pathlib, queue, sqlite3, subprocess, sys, tempfile, threading, time

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
    if row is None:
        raise SystemExit("logged out — no token")
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
CWD = tempfile.mkdtemp(prefix="kas-sweep-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
pathlib.Path(CWD, "probe.txt").write_text("magic 4242\n")
subprocess.run("git add -A && git commit -qm baseline", cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-sweep-home-")
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
RESULTS = {}
CALLBACKS = {}


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


def reply(rid, res):
    send({"jsonrpc": "2.0", "id": rid, "result": res}, envelope="response")


def pump(until, to=90):
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
        if rid is not None and m:
            CALLBACKS[m] = CALLBACKS.get(m, 0) + 1
            if m == "_kiro/auth/getAccessToken":
                reply(rid, TOK)
            elif m == "_kiro/terminal/shell_type":
                reply(rid, {"shellType": "bash"})
            elif m in ("fs/read_text_file", "_kiro/fs/read_file"):
                try:
                    reply(rid, {"content": pathlib.Path(pr.get("path")).read_text()})
                except Exception as e:
                    reply(rid, {"content": f"(err {e})"})
            elif m == "session/request_permission":
                opts = pr.get("options", [])
                pick = next((x for x in opts if "allow" in
                             (x.get("kind", "") + x.get("optionId", "")).lower()),
                            opts[0] if opts else None)
                reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                      if pick else {"outcome": {"outcome": "cancelled"}})
            else:
                reply(rid, {})
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


def call(label, method, params, to=60):
    r = pump(req(method, params), to)
    if r is None:
        RESULTS[label] = ("TIMEOUT", None)
    elif "error" in r:
        e = r["error"]
        RESULTS[label] = (f"ERR {e.get('code')}", e.get("message"))
    else:
        RESULTS[label] = ("OK", r.get("result"))
    code, detail = RESULTS[label]
    print(f"  {code:10} {label}"
          + (f"  {json.dumps(detail)[:150]}" if code.startswith("ERR") else
             f"  {json.dumps(detail)[:150]}" if detail is not None else ""))
    return r


# Advertise the _kiro/fs/* capability family too, so the fs dialect is observable.
req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {
        "fs": {"readTextFile": True, "writeTextFile": True},
        "terminal": True,
        "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True,
                           "kiroFsReadFile": True, "kiroFsWriteFile": True,
                           "kiroFsStat": True, "infrastructureSafety": True}},
    },
    "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True,
                       "kiroFsReadFile": True, "kiroFsWriteFile": True,
                       "kiroFsStat": True, "infrastructureSafety": True}},
})
pump(1, 40)
nid = req("session/new", {"cwd": CWD, "mcpServers": [],
                          "_meta": {"kiro": {"settings": {"workflows": {"enabled": True}}}}})
sess = pump(nid, 90)
sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 6)

print("\n########## read-only session RPCs ##########")
for label, m, pr in [
    ("session/list", "_kiro/session/list", {}),
    ("session/context", "_kiro/session/context", {"sessionId": sid}),
    ("session/history", "_kiro/session/history", {"sessionId": sid}),
    ("session/export", "_kiro/session/export", {"sessionId": sid}),
    ("config/template", "_kiro/config/template", {"sessionId": sid}),
    ("account/getUsage", "_kiro/account/getUsage", {}),
    ("knowledge", "_kiro/knowledge", {"sessionId": sid}),
    ("codeIntelligence", "_kiro/codeIntelligence", {"sessionId": sid}),
    ("permissions/list", "_kiro/permissions/list", {"sessionId": sid}),
    ("permissions/explain", "_kiro/permissions/explain", {"sessionId": sid, "toolName": "fs_read"}),
    ("hooks/list", "_kiro/hooks/list", {"sessionId": sid}),
    ("sandbox/status", "_kiro/sandbox/status", {"sessionId": sid}),
    ("safety/getProperties", "_kiro/safety/getProperties", {"sessionId": sid}),
    ("sourceProviders/list", "_kiro/sourceProviders/list", {}),
    ("secret/get", "_kiro/secret/get", {"key": "cyril-audit-probe"}),
    ("spec/getTaskStatuses", "_kiro/spec/getTaskStatuses", {"sessionId": sid}),
    ("spec/resolveSession", "_kiro/spec/resolveSession", {"sessionId": sid}),
    ("tool/get_diagnostics", "_kiro/tool/get_diagnostics",
     {"sessionId": sid, "path": os.path.join(CWD, "probe.txt")}),
    ("workflow/list", "_kiro/workflow/list", {"sessionId": sid, "workspacePaths": [CWD]}),
    ("workflow/listRecipes", "_kiro/workflow/listRecipes",
     {"sessionId": sid, "workspacePaths": [CWD]}),
    ("workflow/listWatchHandlers", "_kiro/workflow/listWatchHandlers", {"sessionId": sid}),
]:
    call(label, m, pr)

print("\n########## one short turn (gives history/compact something real) ##########")
r = pump(req("session/prompt", {"sessionId": sid,
                                "prompt": [{"type": "text",
                                            "text": "Read probe.txt and reply with just the number."}]}), 300)
print("  stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
pump(-1, 8)

print("\n########## post-turn / stateful RPCs ##########")
call("session/history (post-turn)", "_kiro/session/history", {"sessionId": sid})
call("session/context (post-turn)", "_kiro/session/context", {"sessionId": sid})
call("steer", "_session/steer", {"sessionId": sid, "text": "cyril audit steer probe"})
call("steer/clear", "_session/steer/clear", {"sessionId": sid})
call("hooks/setEnabled", "_kiro/hooks/setEnabled", {"sessionId": sid, "enabled": True})
call("powers/refresh", "_kiro/powers/refresh", {"sessionId": sid})
call("mcp/toggle", "_kiro/mcp/toggle", {"sessionId": sid, "serverName": "nonexistent", "enabled": False})
call("session/rename", "_kiro/session/rename", {"sessionId": sid, "title": "cyril audit sweep"})
call("session/compact", "_kiro/session/compact", {"sessionId": sid}, to=240)

print("\n########## workflow control surface ##########")
nw = call("workflow/new", "_kiro/workflow/new", {
    "workflow": {"name": "sweep-wf", "inputs": {},
                 "steps": [{"type": "step", "id": "s1", "agent": "wf-coder",
                            "prompt": "Do not use tools. Signal completion immediately."}]},
    "inputs": {}, "parentSessionId": sid, "workspacePaths": [CWD]})
wfid = ((nw or {}).get("result") or {}).get("workflowId")
if wfid:
    call("workflow/inspect", "_kiro/workflow/inspect", {"workflowId": wfid})
    call("workflow/pause", "_kiro/workflow/pause", {"workflowId": wfid})
    call("workflow/resume", "_kiro/workflow/resume", {"workflowId": wfid})
    call("workflow/cancel", "_kiro/workflow/cancel", {"workflowId": wfid})
    call("workflow/retry", "_kiro/workflow/retry", {"workflowId": wfid})
    call("workflow/delete", "_kiro/workflow/delete", {"workflowId": wfid,
                                                      "workspacePaths": [CWD]})
call("workflow/resumeAll", "_kiro/workflow/resumeAll", {"workspacePaths": [CWD]})

print("\n########## destructive — throwaway forked session only ##########")
fk = pump(req("session/fork", {"sessionId": sid, "cwd": CWD}), 90)
throwaway = ((fk or {}).get("result") or {}).get("sessionId")
print("  throwaway fork:", throwaway)
if throwaway and throwaway != sid:
    call("session/delete (throwaway)", "_kiro/session/delete", {"sessionId": throwaway})

print("\n=== SUMMARY ===")
ok = [k for k, (c, _) in RESULTS.items() if c == "OK"]
notfound = [k for k, (c, _) in RESULTS.items() if c == "ERR -32601"]
othererr = [k for k, (c, _) in RESULTS.items() if c.startswith("ERR") and c != "ERR -32601"]
to_ = [k for k, (c, _) in RESULTS.items() if c == "TIMEOUT"]
print(f"  OK          ({len(ok)}): {', '.join(ok)}")
print(f"  -32601      ({len(notfound)}): {', '.join(notfound)}")
print(f"  other error ({len(othererr)}): {', '.join(othererr)}")
print(f"  timeout     ({len(to_)}): {', '.join(to_)}")
print(f"\n  host callbacks seen: {json.dumps(CALLBACKS)}")

OUT.close()
p.stdin.close()
p.terminate()
