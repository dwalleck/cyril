#!/usr/bin/env python3
"""Force the KAS **OrchestrateSubAgent** multi-stage path and capture its rawInput.

cyril-ucii: the `{task, stages[], repeat{...}}` rawInput shape is asserted in
docs/kiro-2.14.1-wire-audit.md and CLAUDE.md on STATIC evidence only (the bundle's
buildSchema + the executor branching on repeat.complete/repeat.exhausted). The one
prior live attempt died on expired credentials.

A re-run of probe-kas-orchestrate-capture-2.14.1.py on 2.16.0 completed cleanly but
captured ZERO orchestrate frames — the model simply did the work itself. And the
generic turn probe only ever triggers the OTHER tool: `InvokeSubAgent`, whose
rawInput is `{name, prompt, explanation, contextFiles}`. OrchestrateSubAgent is a
SEPARATE tool (ToolTags.SUBAGENT_META) and needs a task that genuinely wants stages.

So this probe forces it explicitly, and runs two turns:
  1. STAGES — a two-stage pipeline with `depends_on`, to capture stages[]
  2. REPEAT — a bounded loop, to capture the `repeat` object

Stage schema, from src/tools/orchestrate-subagent/types.ts buildSchema():
  { name: str, role: enum(<registered agent ids>),
    prompt_template: str (may reference {task}), depends_on?: [str] }
Stages with no `depends_on` run in PARALLEL with no shared context.

Costs credits (two real multi-agent turns). Redacts credentials at emit time.

    probe-kas-orchestrate-stages-2.16.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, pathlib, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

TURNS = [
    ("stages",
     "Use the Orchestrate Sub-agent tool — NOT a single sub-agent invocation — to run a "
     "TWO-STAGE pipeline for this task: \"write one short haiku about a terminal\". "
     "Stage one must be named 'draft' and write the haiku. Stage two must be named "
     "'review', must set depends_on to ['draft'], and must critique the draft in one "
     "sentence. Do not perform either stage yourself; delegate both."),
    ("repeat",
     "Use the Orchestrate Sub-agent tool again for the task \"count to three\", with a "
     "single stage named 'tick', and configure it to REPEAT with maxIterations 2 and "
     "onMaxIterations 'abort'. Delegate; do not do it yourself."),
]

SECRET_KEYS = {"accessToken", "access_token", "refreshToken", "refresh_token",
               "idToken", "id_token", "clientSecret", "client_secret", "bearer",
               "profileArn", "profile_arn", "authorization", "Authorization"}


def redact(obj):
    """Deep-copy with credential values replaced. Applied only on the way to the log."""
    if isinstance(obj, dict):
        return {k: ("<redacted>" if k in SECRET_KEYS and obj[k] else redact(obj[k]))
                for k in obj}
    if isinstance(obj, list):
        return [redact(x) for x in obj]
    return obj


def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute(
            "select value from auth_kv where key in "
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
CWD = tempfile.mkdtemp(prefix="kas-orchstage-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-orchstage-home-")
# The gate lives in acp-workspace-connection.ts, which is CONNECTION-scoped — the same
# `settings` object that gates `knowledge` and `codeIntelligence`. A per-session
# session/new._meta.kiro.settings therefore cannot reach it (measured: passing
# subagentOrchestration there changed nothing). Seed the global settings file instead,
# mirroring the content-collection precedent where only global cli.json takes effect.
# Values must be JSON booleans; BaseSettingSchema is {enabled: boolean}.
_sd = pathlib.Path(TMPH, ".kiro", "settings")
_sd.mkdir(parents=True, exist_ok=True)
(_sd / "cli.json").write_text(json.dumps({
    "subagentOrchestration": {"enabled": True},
    "inlineAgents": {"enabled": True},
}, indent=2))
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
ORCH = []      # (turn, sessionUpdate, status, rawInput, _meta.kiro)


def emit(direction, envelope, method, parsed):
    OUT.write(json.dumps({"direction": direction, "envelope": envelope,
                          "method": method, "parsed": redact(parsed)}) + "\n")
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


def abspath(x):
    return x if os.path.isabs(x or "") else os.path.join(CWD, x or "")


def handle(m, rid, pr):
    if m == "_kiro/auth/getAccessToken":
        return reply(rid, TOK)
    if m == "_kiro/terminal/shell_type":
        return reply(rid, {"shellType": "bash"})
    if m in ("fs/read_text_file", "_kiro/fs/read_file"):
        try:
            return reply(rid, {"content": pathlib.Path(abspath(pr.get("path"))).read_text()})
        except Exception as e:
            return reply(rid, {"content": f"(unavailable: {e})"})
    if m in ("fs/write_text_file", "_kiro/fs/write_file"):
        try:
            ap = pathlib.Path(abspath(pr.get("path")))
            ap.parent.mkdir(parents=True, exist_ok=True)
            ap.write_text(pr.get("content", ""))
        except Exception:
            pass
        return reply(rid, {})
    if m == "terminal/create":
        cmd, args = pr.get("command", ""), pr.get("args") or []
        tid = f"term-{len(TERMS) + 1}"
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
        return reply(rid, {"exitCode": t["code"], "signal": None})   # FLAT, see 2.16.0 audit
    if m in ("terminal/release", "terminal/kill"):
        return reply(rid, {})
    if m == "session/request_permission":
        opts = pr.get("options", [])
        pick = next((x for x in opts
                     if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
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
        u = pr.get("update") or {}
        if u.get("sessionUpdate") in ("tool_call", "tool_call_update"):
            ri = u.get("rawInput") or {}
            mk = (u.get("_meta") or {}).get("kiro") or {}
            # OrchestrateSubAgent is identified by its rawInput carrying `stages`,
            # or by the pipeline _meta the handler builds.
            if "stages" in ri or "task" in ri or "groupId" in mk or "stages" in mk:
                ORCH.append((tag, u.get("sessionUpdate"), u.get("status"), ri, mk))
        if rid is not None and m:
            handle(m, rid, pr)
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True},
                           "terminal": True},
    "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True}},
})
pump(1, 40)
# OrchestrateSubAgent is GATED. acp-workspace-connection.ts registers it only under
# `isSettingEnabled(settings, "subagentOrchestration")`, and subagent-tool-ids.ts
# `getDelegationToolId()` falls back to INVOKE_SUB_AGENT_TOOL_ID when it is off — which
# is why an ungated run only ever produces "Sub-agent: <name>" invoke frames and the
# model truthfully reports it has no orchestration tool. The setting also swaps the
# delegation tool named in the system prompt, so it is a coherent switch.
# Same channel as the workflow gate: session/new._meta.kiro.settings.
nid = req("session/new", {"cwd": CWD, "mcpServers": [], "_meta": {"kiro": {"settings": {
    "subagentOrchestration": {"enabled": True},
    "inlineAgents": {"enabled": True},
}}}})
sess = pump(nid, 90)
sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 6)

for tag, text in TURNS:
    print(f"\n########## turn: {tag} ##########")
    t0 = time.time()
    rid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": text}]})
    r = pump(rid, 600, tag=tag)
    print(f"  stopReason={((r or {}).get('result') or {}).get('stopReason')!r} "
          f"{time.time() - t0:.1f}s")
    pump(-1, 10, tag=f"{tag}post")

print(f"\n=== ORCHESTRATE frames captured: {len(ORCH)} ===")
seen = set()
for tag, su, status, ri, mk in ORCH:
    sig = json.dumps(ri, sort_keys=True)[:200]
    if (tag, su, sig) in seen:
        continue
    seen.add((tag, su, sig))
    print(f"\n-- [{tag}] {su} status={status}")
    print(f"   _meta.kiro : {json.dumps(mk)[:500]}")
    print(f"   rawInput   : {json.dumps(ri)[:1400]}")

print("\n=== VERDICT (cyril-ucii acceptance) ===")
has_stages = any("stages" in (ri or {}) for _, _, _, ri, _ in ORCH)
has_repeat = any("repeat" in (ri or {}) for _, _, _, ri, _ in ORCH)
print(f"  stages[] observed on the wire : {has_stages}")
print(f"  repeat{{}} observed on the wire : {has_repeat}")
if not ORCH:
    print("  NO orchestrate frames — the model declined to use the tool; "
          "strengthen the prompt and re-run before drawing any conclusion.")

OUT.close()
p.stdin.close()
p.terminate()
