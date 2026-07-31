#!/usr/bin/env python3
"""Capture `_kiro/workflow/node_paused` — the last unverified workflow payload shape.

The 2.16.0 audit live-verified 8 of the 9 `_kiro/workflow/*` payloads. `node_paused`
never fired: the repeat-exhaustion path emits `paused` ONLY (that pairing correction
is recorded in the audit), and the other emit sites need a mid-node pause.

The reachable trigger from a scripted probe is the step-completion protocol itself.
WORKFLOW_STEP_COMPLETION_PROTOCOL tells a step agent to end every turn with
`send_message`, and severity "warning" means "I need user input" — the runner then
pauses the node with reason "Step requested user input via send_message."

So: a one-step workflow whose prompt instructs the step to ask a question.

Costs credits (one workflow step turn).

    probe-kas-workflow-nodepaused-2.16.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

DAG = {
    "name": "cyril-audit-nodepaused",
    "description": "One step that requests user input, to provoke node_paused.",
    "inputs": {},
    "steps": [{
        "type": "step", "id": "ask", "agent": "wf-coder",
        "prompt": ("Do NOT use any tools and do NOT modify any file. You need "
                   "clarification before you can proceed: signal that you require "
                   "user input by calling send_message with severity \"warning\", "
                   "asking the user which colour they prefer. Do not signal success."),
    }],
}

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
CWD = tempfile.mkdtemp(prefix="kas-np-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-np-home-")
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
EVENTS = []
NOTIFY = []


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


def pump(until, to=120, stop_kind=None):
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
        if m and m.startswith("_kiro/workflow/"):
            EVENTS.append((m, pr))
            short = m.rsplit("/", 1)[1]
            extra = ""
            if short in ("node_paused", "paused"):
                extra = f"  reason={(pr.get('reason') or pr.get('pauseReason'))!r}"
            print(f"  <- {short}{extra}")
        if m == "_kiro/session/notify":
            NOTIFY.append(pr)
            print(f"  <- _kiro/session/notify: {json.dumps(pr)[:260]}")
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
        if stop_kind and m == f"_kiro/workflow/{stop_kind}":
            return o
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
nid = req("session/new", {"cwd": CWD, "mcpServers": [],
                          "_meta": {"kiro": {"settings": {"workflows": {"enabled": True}}}}})
sess = pump(nid, 90)
sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid, "| workflowsEnabled:",
      (sess or {}).get("result", {}).get("_meta", {}).get("workflowsEnabled"))
pump(-1, 6)

nw = pump(req("_kiro/workflow/new", {"workflow": DAG, "inputs": {},
                                     "parentSessionId": sid, "workspacePaths": [CWD]}), 60)
wf = ((nw or {}).get("result") or {}).get("workflowId")
print("workflowId:", wf)
if wf:
    pump(req("_kiro/workflow/invoke", {"workflowId": wf}), 45)
    pump(-1, 420, stop_kind="paused")
    pump(-1, 20)

print("\n=== events ===")
kinds = {}
for m, _ in EVENTS:
    kinds[m] = kinds.get(m, 0) + 1
for k in sorted(kinds):
    print(f"  {kinds[k]:3}x {k}")

print("\n=== node_paused payloads ===")
np = [pr for m, pr in EVENTS if m.endswith("/node_paused")]
for x in np:
    print("  ", json.dumps(x)[:600])
print(f"\nVERDICT: node_paused observed = {bool(np)}   "
      f"_kiro/session/notify observed = {bool(NOTIFY)}")

OUT.close()
p.stdin.close()
p.terminate()
