#!/usr/bin/env python3
"""Flip the KAS `v2Hooks` gate and exercise the hooks surface.

probe-kas-pushed-methods-2.16.0.py advertised `_meta.kiro.hooks: true` (a BOOLEAN)
and nothing fired; the RPC sweep got "not available when v2Hooks is disabled" from
_kiro/hooks/setEnabled. The bundle says why — the gate reads a nested OBJECT:

    if (kiroMeta?.hooks?.enabled) {
      const hooksConfig = kiroMeta.hooks;
      ...
      if (hooksConfig.v2 === true) { this.v2HooksCache = new HooksModuleCache({...}) }
    }

`true?.enabled` is undefined, so the whole block was skipped. The correct
advertisement is:

    clientCapabilities._meta.kiro.hooks = { "enabled": true, "v2": true }

A second site also registers a `CreateHookTool` for the agent when
`clientMeta.hooks` is an object with `v2 === true`.

v2 hooks are loaded from `<workspace>/.kiro/hooks/*.json` and validated against
kasHookFileSchema:

    { "version": "v1",
      "hooks": [ { name, description?, trigger, matcher?, action,
                   timeout?, enabled?, confirm? } ] }

trigger is one of: preToolUse | postToolUse | sessionStart | stop
action is a discriminated union on `type`: command | prompt | agent

The earlier probe's hook file used a flat {name,trigger,command} shape, which that
schema rejects — so even with the gate flipped it would have loaded nothing.

Exercises: hooks/list (client->agent, v2-only), hooks/setEnabled, a real
preToolUse hook firing during a tool-using turn, and any hooks/* pushes.

Costs credits (one short turn).

    probe-kas-v2hooks-2.16.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, pathlib, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
MARKER = "cyril-audit-hook-fired"

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
CWD = tempfile.mkdtemp(prefix="kas-v2h-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
pathlib.Path(CWD, "probe.txt").write_text("magic 4242\n")
HOOKLOG = os.path.join(CWD, "hook-evidence.txt")
hd = pathlib.Path(CWD, ".kiro", "hooks")
hd.mkdir(parents=True, exist_ok=True)
(hd / "audit.json").write_text(json.dumps({
    "version": "v1",
    "hooks": [
        {"name": "cyril-audit-pre", "description": "audit preToolUse probe",
         "trigger": "preToolUse",
         "action": {"type": "command", "command": f"echo {MARKER}-pre >> {HOOKLOG}"},
         "timeout": 30, "enabled": True},
        {"name": "cyril-audit-start", "description": "audit sessionStart probe",
         "trigger": "sessionStart",
         "action": {"type": "command", "command": f"echo {MARKER}-start >> {HOOKLOG}"},
         "timeout": 30, "enabled": True},
    ],
}, indent=2))
subprocess.run("git add -A && git commit -qm baseline", cwd=CWD, shell=True)

TMPH = tempfile.mkdtemp(prefix="kas-v2h-home-")
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
TERMS = {}
HOOK_PUSHES = []


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


def ap(x):
    return x if os.path.isabs(x or "") else os.path.join(CWD, x or "")


def answer(m, rid, pr):
    if m == "_kiro/auth/getAccessToken":
        return reply(rid, TOK)
    if m == "_kiro/terminal/shell_type":
        return reply(rid, {"shellType": "bash"})
    if m in ("fs/read_text_file", "_kiro/fs/read_file"):
        try:
            return reply(rid, {"content": pathlib.Path(ap(pr.get("path"))).read_text()})
        except Exception as e:
            return reply(rid, {"content": f"(err {e})"})
    if m in ("fs/write_text_file", "_kiro/fs/write_file"):
        try:
            f = pathlib.Path(ap(pr.get("path")))
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_text(pr.get("content", ""))
        except Exception:
            pass
        return reply(rid, {})
    if m == "_kiro/fs/stat":
        f = pathlib.Path(ap(pr.get("path")))
        return reply(rid, {"type": "directory" if f.is_dir() else "file",
                           "size": f.stat().st_size} if f.exists() else {})
    if m == "_kiro/fs/read_directory":
        try:
            return reply(rid, {"entries": [
                {"name": e.name, "type": "directory" if e.is_dir() else "file"}
                for e in pathlib.Path(ap(pr.get("path"))).iterdir()]})
        except Exception:
            return reply(rid, {"entries": []})
    if m == "session/request_permission":
        opts = pr.get("options", [])
        pick = next((x for x in opts if "allow" in
                     (x.get("kind", "") + x.get("optionId", "")).lower()),
                    opts[0] if opts else None)
        return reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                     if pick else {"outcome": {"outcome": "cancelled"}})
    if m == "terminal/create":
        cmd, args = pr.get("command", ""), pr.get("args") or []
        tid = f"term-{len(TERMS)+1}"
        try:
            r = (subprocess.run([cmd, *args], cwd=pr.get("cwd") or CWD, capture_output=True,
                                text=True, timeout=60) if args else
                 subprocess.run(cmd, shell=True, cwd=pr.get("cwd") or CWD,
                                capture_output=True, text=True, timeout=60))
            TERMS[tid] = {"out": r.stdout + r.stderr, "code": r.returncode}
        except Exception as e:
            TERMS[tid] = {"out": f"(host error: {e})", "code": 127}
        return reply(rid, {"terminalId": tid})
    if m == "terminal/output":
        t = TERMS.get(pr.get("terminalId"), {"out": "", "code": 0})
        return reply(rid, {"output": t["out"], "truncated": False,
                           "exitStatus": {"exitCode": t["code"], "signal": None}})
    if m == "terminal/wait_for_exit":
        t = TERMS.get(pr.get("terminalId"), {"code": 0})
        return reply(rid, {"exitCode": t["code"], "signal": None})
    if m in ("terminal/release", "terminal/kill"):
        return reply(rid, {})
    return reply(rid, {})


def pump(until, to=120):
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
        if m and "hooks" in m:
            HOOK_PUSHES.append((m, "REQUEST" if rid is not None else "notification", pr))
            print(f"  <- {m} ({'req' if rid is not None else 'notif'}): {json.dumps(pr)[:240]}")
        if rid is not None and m:
            answer(m, rid, pr)
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


def call(label, m, pr, to=60):
    r = pump(req(m, pr), to)
    if r is None:
        print(f"  TIMEOUT   {label}")
    elif "error" in r:
        print(f"  ERR {r['error'].get('code')} {label}  {json.dumps(r['error'].get('message'))[:200]}")
    else:
        print(f"  OK        {label}  {json.dumps(r.get('result'))[:400]}")
    return r


req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {
        "fs": {"readTextFile": True, "writeTextFile": True,
               "_meta": {"kiro": {"readFile": True, "writeFile": True, "stat": True,
                                  "readDirectory": True}}},
        "terminal": True,
        # THE FIX: hooks must be an OBJECT with enabled + v2, not a boolean.
        "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True,
                           "hooks": {"enabled": True, "v2": True}}},
    },
    "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True,
                       "hooks": {"enabled": True, "v2": True}}},
})
pump(1, 40)
sid = (pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 90) or {}) \
    .get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 8)

print("\n########## hooks RPCs (v2-gated) ##########")
lst = call("hooks/list", "_kiro/hooks/list", {"sessionId": sid, "workspacePaths": [CWD],
                                              "includeDisabled": True})
hooks = ((lst or {}).get("result") or {}).get("hooks") or []
print(f"  -> {len(hooks)} hook(s) loaded from .kiro/hooks/")
if hooks:
    hid = hooks[0].get("id") or hooks[0].get("hookId")
    call("hooks/setEnabled(false)", "_kiro/hooks/setEnabled",
         {"sessionId": sid, "hookId": hid, "enabled": False})
    call("hooks/setEnabled(true)", "_kiro/hooks/setEnabled",
         {"sessionId": sid, "hookId": hid, "enabled": True})

print("\n########## turn that uses a tool (should fire preToolUse) ##########")
r = pump(req("session/prompt", {"sessionId": sid,
                                "prompt": [{"type": "text",
                                            "text": "Read probe.txt and reply with just the number."}]}), 420)
print("  stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
pump(-1, 12)

print("\n=== hook side-effect evidence (host-side file) ===")
try:
    print("  ", pathlib.Path(HOOKLOG).read_text().strip().replace("\n", " | "))
except Exception as e:
    print(f"   (no {HOOKLOG}: {e})")

print("\n=== hooks/* frames seen ===")
if HOOK_PUSHES:
    for m, kind, pr in HOOK_PUSHES:
        print(f"  {kind:12} {m}: {json.dumps(pr)[:300]}")
else:
    print("  (none)")

OUT.close()
p.stdin.close()
p.terminate()
