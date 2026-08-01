#!/usr/bin/env python3
"""Re-run the two results the 2.16.0 audit recorded as INCONCLUSIVE, with fixed params.

Both earlier negatives were limits of the probe, not evidence about the feature.

A. subagentOrchestration (cyril-ucii)
   Earlier: sent at session/new only -> no effect; then at initialize -> no HANDSHAKE
   delta. But the surface it selects is chosen during PROMPT CONSTRUCTION:
       getDelegationToolId(getModelConfigProvider().isFeatureEnabled("subagentOrchestration"))
       -> ORCHESTRATE_SUBAGENT_TOOL_ID : INVOKE_SUB_AGENT_TOOL_ID
   and initialize._meta.kiro.settings is what overrides that provider (bridgeFeatureFlags).
   A handshake-only sweep can never see it. FIX: set it at initialize AND take a turn.

B. infraSafetyMonitor / infraSafetyEnforce (cyril-3ald)
   Earlier: flags flipped but the sweep arms did NOT advertise the capability. The real
   gate is:
       this._infraSafetyEnabled = clientSupportsSafety && (monitorEnabled || enforceEnabled)
       clientSupportsSafety = resolvedCapabilities.infrastructureSafety
   i.e. BOTH halves are required. FIX: advertise clientCapabilities._meta.kiro
   .infrastructureSafety AND set both flags at initialize.

Also sends clientInfo{name:"kiro-cli"} — the correct persona per cyril-df5l, rather than
the kiro-ide fall-through every earlier probe got.

Costs credits: two turns (one to force delegation, one to force a tool call that safety
could gate).

    probe-kas-inconclusive-rerun-2.16.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, pathlib, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

SECRET_KEYS = {"accessToken", "access_token", "refreshToken", "refresh_token",
               "idToken", "id_token", "clientSecret", "client_secret", "bearer",
               "profileArn", "profile_arn", "authorization", "Authorization"}


def redact(o):
    if isinstance(o, dict):
        return {k: ("<redacted>" if k in SECRET_KEYS and o[k] else redact(o[k])) for k in o}
    if isinstance(o, list):
        return [redact(x) for x in o]
    return o


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
CWD = tempfile.mkdtemp(prefix="kas-incon-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
pathlib.Path(CWD, "probe.txt").write_text("magic 4242\n")
TMPH = tempfile.mkdtemp(prefix="kas-incon-home-")
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
SAFETY = []
TOOLCALLS = []
AGENT = {}


def emit(d, e, m, parsed):
    OUT.write(json.dumps({"direction": d, "envelope": e, "method": m,
                          "parsed": redact(parsed)}) + "\n")
    OUT.flush()


def send(o, m=None, e="request"):
    p.stdin.write(json.dumps(o) + "\n")
    p.stdin.flush()
    emit("client_to_agent", e, m, o)


def req(m, pr):
    i[0] += 1
    send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}, m)
    return i[0]


def reply(rid, res):
    send({"jsonrpc": "2.0", "id": rid, "result": res}, e="response")


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
            return reply(rid, {"entries": [{"name": e.name,
                                            "type": "directory" if e.is_dir() else "file"}
                                           for e in pathlib.Path(ap(pr.get("path"))).iterdir()]})
        except Exception:
            return reply(rid, {"entries": []})
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
    if m == "session/request_permission":
        opts = pr.get("options", [])
        pick = next((x for x in opts if "allow" in
                     (x.get("kind", "") + x.get("optionId", "")).lower()),
                    opts[0] if opts else None)
        return reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                     if pick else {"outcome": {"outcome": "cancelled"}})
    return reply(rid, {})


def pump(until, to=120, tag="init"):
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
        if m and m.startswith("_kiro/safety/"):
            SAFETY.append((m, pr))
            print(f"   <- SAFETY {m}: {json.dumps(pr)[:250]}")
        u = pr.get("update") or {}
        if u.get("sessionUpdate") in ("tool_call", "tool_call_update"):
            TOOLCALLS.append((tag, u.get("title"), sorted((u.get("rawInput") or {}).keys()),
                              u.get("rawInput") or {}))
        if u.get("sessionUpdate") == "agent_message_chunk":
            AGENT[tag] = AGENT.get(tag, "") + (u.get("content") or {}).get("text", "")
        if rid is not None and m:
            answer(m, rid, pr)
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


SETTINGS = {
    "subagentOrchestration": {"enabled": True},   # A: bridged over the backend flag
    "inlineAgents": {"enabled": True},
    "infraSafetyMonitor": {"enabled": True},      # B: both halves, with the capability below
    "infraSafetyEnforce": {"enabled": True},
}
KM = {"checkpoints": True, "infrastructureSafety": True, "settings": SETTINGS}

req("initialize", {
    "protocolVersion": 1,
    "clientInfo": {"name": "kiro-cli", "version": "2.16.0"},     # correct persona
    "clientCapabilities": {
        "fs": {"readTextFile": True, "writeTextFile": True,
               "_meta": {"kiro": {"readFile": True, "writeFile": True, "stat": True,
                                  "readDirectory": True}}},
        "terminal": True,
        "_meta": {"kiro": dict(KM)},
    },
    "_meta": {"kiro": dict(KM)},
})
pump(1, 40)
sess = pump(req("session/new", {"cwd": CWD, "mcpServers": [],
                                "_meta": {"kiro": {"settings": SETTINGS}}}), 90)
sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 8)

print("\n=== B: safety with BOTH halves satisfied ===")
r = pump(req("_kiro/safety/getProperties", {"sessionId": sid}), 45)
print("  getProperties ->", json.dumps(r)[:400])

print("\n=== A: force multi-stage delegation (turn 1) ===")
r = pump(req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text":
    "Use your sub-agent ORCHESTRATION tool to run a two-stage pipeline for the task "
    "'write one short haiku about a terminal': stage 'draft' writes it, stage 'review' "
    "depends_on draft and critiques it in one sentence. Delegate both stages; do not do "
    "the work yourself. If you only have a single-subagent invoke tool and no multi-stage "
    "orchestration tool, say exactly: NO-ORCHESTRATION-TOOL."}]}), 420, tag="orch")
print("  stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
pump(-1, 10, tag="orch")

print("\n=== B: tool call that safety could gate (turn 2) ===")
r = pump(req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text":
    "Run this exact shell command and report its output: echo infra-safety-probe"}]}),
    420, tag="safety")
print("  stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
pump(-1, 10, tag="safety")

print("\n=== VERDICT A — subagentOrchestration ===")
orch = [t for t in TOOLCALLS if "stages" in t[3] or "task" in t[3]
        or "orchestrat" in (t[1] or "").lower()]
print(f"  orchestrate-shaped tool calls: {len(orch)}")
for t in orch[:3]:
    print("   ", t[1], t[2], json.dumps(t[3])[:400])
said = AGENT.get("orch", "")
print(f"  agent said NO-ORCHESTRATION-TOOL: {'NO-ORCHESTRATION-TOOL' in said}")
print(f"  distinct tool titles this turn: "
      f"{sorted({t[1] for t in TOOLCALLS if t[0]=='orch' and t[1]})}")
if orch:
    print("  => stages[]/repeat SHAPE CAPTURED — cyril-ucii can be closed")
elif "NO-ORCHESTRATION-TOOL" in said:
    print("  => still gated even via the initialize bridge; backend flag stands")
else:
    print("  => inconclusive again (model neither used it nor reported its absence)")

print("\n=== VERDICT B — infra safety ===")
print(f"  _kiro/safety/* frames: {len(SAFETY)}")
for m, pr in SAFETY[:4]:
    print("   ", m, json.dumps(pr)[:300])
print(f"  safety frames seen = {bool(SAFETY)} "
      f"(earlier run WITHOUT the capability advertised saw 0)")

OUT.close()
p.stdin.close()
p.terminate()
