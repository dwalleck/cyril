#!/usr/bin/env python3
"""Capture the OrchestrateSubAgent `repeat` wire shape — the last piece of cyril-ucii.

The stages half was captured once subagentOrchestration was sent at
initialize._meta.kiro.settings (probe-kas-inconclusive-rerun-2.16.0.py). `repeat` was
not, because that run asked for a two-stage pipeline and the model did not elect a loop.
The tool is reachable now, so this asks for a loop explicitly.

THE SCHEMA (src/tools/orchestrate-subagent/types.ts buildSchema) — note this is NOT the
same `repeat` as the workflow DAG's repeat NODE, and conflating them would be wrong:

    repeat: {
      maxIterations : int 1..20            // workflow node allows 1..1000
      stopCondition : { containsText: str } // workflow node also allows fileCheck
      onMaxIterations: "continue" | "abort" | null   // nullish; default "continue"
                                                     // workflow node also allows "pause"
    }.nullish()

    "Re-runs the full pipeline until stopCondition is met or maxIterations reached.
     Previous iteration feedback is passed as context to first-wave stages."

Kept cheap on purpose: ONE stage, maxIterations 2, and a stopCondition that cannot be
satisfied, so the loop runs its full bound and we observe the exhaustion path.

    probe-kas-orchestrate-repeat-2.16.0.py <kiro-cli-chat> <out.jsonl>
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
CWD = tempfile.mkdtemp(prefix="kas-rpt-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-rpt-home-")
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
FRAMES = []


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
        u = pr.get("update") or {}
        if u.get("sessionUpdate") in ("tool_call", "tool_call_update"):
            FRAMES.append(u)
        if rid is not None and m:
            if m == "_kiro/auth/getAccessToken":
                reply(rid, TOK)
            elif m == "_kiro/terminal/shell_type":
                reply(rid, {"shellType": "bash"})
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


SETTINGS = {"subagentOrchestration": {"enabled": True}, "inlineAgents": {"enabled": True}}
KM = {"checkpoints": True, "settings": SETTINGS}
req("initialize", {
    "protocolVersion": 1,
    "clientInfo": {"name": "kiro-cli", "version": "2.16.0"},
    "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True},
                           "terminal": True, "_meta": {"kiro": dict(KM)}},
    "_meta": {"kiro": dict(KM)},
})
pump(1, 40)
sid = (pump(req("session/new", {"cwd": CWD, "mcpServers": [],
                                "_meta": {"kiro": {"settings": SETTINGS}}}), 90) or {}) \
    .get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 8)

PROMPT = (
    "Use the Orchestrate Sub-agent tool with a REPEAT loop. Configure it exactly:\n"
    "  task: 'produce a one-word colour name'\n"
    "  ONE stage named 'pick' that outputs a single colour word\n"
    "  repeat: maxIterations 2, stopCondition containsText 'ZZUNREACHABLEZZ', "
    "onMaxIterations 'continue'\n"
    "The stopCondition string will never appear, so the loop should run its full bound. "
    "Delegate; do not do the work yourself."
)
print("\n########## forcing a repeat loop ##########")
r = pump(req("session/prompt", {"sessionId": sid,
                                "prompt": [{"type": "text", "text": PROMPT}]}), 600)
print("  stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
pump(-1, 15)

print("\n=== orchestrate frames ===")
best = None
for u in FRAMES:
    ri = u.get("rawInput") or {}
    if "stages" in ri and (best is None or len(json.dumps(ri)) > len(json.dumps(best))):
        best = ri
if best:
    print(json.dumps(best, indent=1)[:1600])
    print("\n  top-level keys:", sorted(best))
    print("  REPEAT PRESENT:", "repeat" in best)
    if "repeat" in best:
        print("  repeat =", json.dumps(best["repeat"]))
        print("  repeat keys:", sorted(best["repeat"] or {}))
else:
    print("  (no orchestrate rawInput captured)")

print("\n=== pipeline _meta across the run (iteration visibility) ===")
seen = set()
for u in FRAMES:
    pl = ((u.get("_meta") or {}).get("kiro") or {}).get("pipeline")
    if not pl:
        continue
    k = json.dumps(pl, sort_keys=True)
    if k in seen:
        continue
    seen.add(k)
    stages = [(s.get("name"), s.get("status")) for s in pl.get("stages") or []]
    print(f"  groupId={pl.get('groupId')!r} keys={sorted(pl)} stages={stages}")

print("\n=== subagent tool calls (one per iteration?) ===")
subs = [u for u in FRAMES
        if ((u.get("_meta") or {}).get("kiro") or {}).get("kind") == "agent-subtask"]
ids = {((u.get("_meta") or {}).get("kiro") or {}).get("agentSubtaskId") for u in subs}
print(f"  agent-subtask frames={len(subs)}  distinct agentSubtaskId={len(ids)}")

OUT.close()
p.stdin.close()
p.terminate()
