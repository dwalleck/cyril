#!/usr/bin/env python3
"""The three OrchestrateSubAgent variants the happy-path captures missed.

Everything captured for OrchestrateSubAgent so far is the SUCCESS path with
inlineAgents ON. That is a real skew: renderers break on error states, and
cyril-ebqu has to draw them.

Three variants, one session, two turns:

  1. onMaxIterations "abort"  — the schema says abort "returns error". Never observed.
     Open: does the tool_call go status:failed or complete with an error payload? What
     do the pipeline stages show when the loop aborts? Does the parent turn still
     end_turn? THIS IS THE ONE THAT MATTERS.

  2. a SATISFIED stopCondition — every prior capture used an unreachable string, so the
     loop always exhausted. Confirms the early-exit branch (repeat.complete) and shows
     whether fewer iterations look any different on the wire.

  3. role as a REGISTERED AGENT ID — `inlineAgents` is left OFF here, so buildSchema
     makes `role` an enum over registered agent ids instead of accepting "inline", and
     the stage object should carry no `inlineAgent` key. Low value on its own (a value
     change plus one absent optional key) but free while we are here.

Requires subagentOrchestration in initialize._meta.kiro.settings — the session/new form
does not reach the flag prompts.ts reads. See cyril-ucii.

    probe-kas-orchestrate-variants-2.16.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

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
CWD = tempfile.mkdtemp(prefix="kas-var-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-var-home-")
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
            FRAMES.append((tag, u))
        if u.get("sessionUpdate") == "agent_message_chunk":
            AGENT[tag] = AGENT.get(tag, "") + (u.get("content") or {}).get("text", "")
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


# inlineAgents deliberately ABSENT -> variant 3 (role must be a registered agent id)
SETTINGS = {"subagentOrchestration": {"enabled": True}}
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
print("sessionId:", sid, "(inlineAgents OFF)")
pump(-1, 8)

TURNS = [
    ("abort",
     "Use the Orchestrate Sub-agent tool with a REPEAT loop configured to ABORT.\n"
     "  task: 'name a colour'\n"
     "  ONE stage named 'pick' that outputs a single colour word\n"
     "  repeat: maxIterations 2, stopCondition containsText 'ZZNEVERZZ', "
     "onMaxIterations 'abort'\n"
     "The stop string will never appear, so the loop must hit its cap and abort. "
     "Delegate; do not do the work yourself. Afterwards tell me plainly whether the "
     "orchestration succeeded or errored."),
    ("satisfied",
     "Use the Orchestrate Sub-agent tool with a REPEAT loop that WILL stop early.\n"
     "  task: 'emit the sentinel'\n"
     "  ONE stage named 'emit' whose prompt_template instructs the sub-agent to output "
     "exactly the word STOPNOW and nothing else\n"
     "  repeat: maxIterations 5, stopCondition containsText 'STOPNOW', "
     "onMaxIterations 'continue'\n"
     "The first iteration should satisfy the stop condition. Delegate; do not do the "
     "work yourself."),
]
for tag, text in TURNS:
    print(f"\n########## turn: {tag} ##########")
    r = pump(req("session/prompt", {"sessionId": sid,
                                    "prompt": [{"type": "text", "text": text}]}), 600, tag=tag)
    print("  stopReason:", ((r or {}).get("result") or {}).get("stopReason"))
    pump(-1, 15, tag=tag)

for tag, _ in TURNS:
    print(f"\n=== {tag.upper()} ===")
    orch = [u for t, u in FRAMES if t == tag and "stages" in (u.get("rawInput") or {})]
    if orch:
        ri = max(orch, key=lambda u: len(json.dumps(u.get("rawInput"))))["rawInput"]
        print("  rawInput:", json.dumps(ri)[:700])
        st = (ri.get("stages") or [{}])[0]
        print(f"  stage keys={sorted(st)}  role={st.get('role')!r}  "
              f"inlineAgent present={'inlineAgent' in st}")
    else:
        print("  (no orchestrate rawInput)")
    print("  --- Orchestrate tool_call status transitions ---")
    for t, u in FRAMES:
        if t != tag:
            continue
        pl = ((u.get("_meta") or {}).get("kiro") or {}).get("pipeline")
        if u.get("title") == "Orchestrate Sub-agent" or pl:
            stages = [(s.get("name"), s.get("status")) for s in (pl or {}).get("stages") or []]
            print(f"    {u.get('sessionUpdate'):16} status={u.get('status')!r} stages={stages}")
    fin = [u for t, u in FRAMES if t == tag and u.get("sessionUpdate") == "tool_call_update"
           and u.get("status") in ("failed", "completed") and (u.get("content") or u.get("rawOutput"))]
    for u in fin[-2:]:
        print(f"  final: status={u.get('status')!r} "
              f"content={json.dumps(u.get('content'))[:300]} "
              f"rawOutput={json.dumps(u.get('rawOutput'))[:300]}")
    subs = {((u.get("_meta") or {}).get("kiro") or {}).get("agentSubtaskId")
            for t, u in FRAMES if t == tag
            and ((u.get("_meta") or {}).get("kiro") or {}).get("kind") == "agent-subtask"}
    print(f"  distinct agentSubtaskId (= iterations): {len(subs - {None})}")
    print(f"  agent said: {(AGENT.get(tag) or '')[:260]!r}")

OUT.close()
p.stdin.close()
p.terminate()
