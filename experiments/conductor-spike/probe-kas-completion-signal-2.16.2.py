#!/usr/bin/env python3
"""Did workflow hazard 4 change in 2.16.2? (completionSignal / completionSignalSource)

2.16.2 adds a NodeState field the 2.16.0 schema does not have:

    completionSignal:       enum(["success","need_input","error"]).optional()
    completionSignalSource: enum(["send_message","status_update"]).optional()   <- NEW

Hazard 4 as documented for 2.16.0 (docs/kiro-2.16.0-wire-audit.md, and the spec
in rivets cyril-6beh) says a step's outcome is decided by a MODEL-ISSUED
`send_message` call, and that omitting it leaves the node paused with
"Awaiting next user message on step session." A `status_update` source implies a
second, non-model path to completion.

A single gate-off run on each binary hinted at exactly that (2.16.0 recorded
completionSignal='success'; 2.16.2 omitted the key yet still completed), but
completionSignal is MODEL-ELECTED, so n=1 cannot separate a semantics change
from model variance. This probe replicates it properly.

THREE ARMS (binary + prompt mode are both parameters):

  neutral   prompt never mentions send_message -> the model elects freely.
            Run on BOTH binaries. This is the falsifier: if 2.16.0 pauses (or
            records a signal) where 2.16.2 completes without one, the change is
            real and not variance.
  explicit  prompt instructs a send_message with severity success -> should
            yield completionSignalSource == "send_message" on 2.16.2. Confirms
            the label is populated at all, and is the CONTROL for the arm above
            (per feedback_kiro_schema_vs_runtime: always probe with a control).

Each arm runs REPS times because the deciding value is model-elected.
COSTS CREDITS: one trivial agent turn per rep.

    probe-kas-completion-signal-2.16.2.py <kiro-cli-chat> <out.jsonl> <neutral|explicit> [reps]

HOME-isolated per feedback_isolate_kiro_probes_with_home.
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
MODE = sys.argv[3] if len(sys.argv) > 3 else "neutral"
REPS = int(sys.argv[4]) if len(sys.argv) > 4 else 2
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

PROMPTS = {
    # Says nothing about completion signalling. The model elects.
    "neutral": "Reply with the word ok. Do not use any tools.",
    # Explicitly asks for the send_message completion signal.
    "explicit": ("Reply with the word ok. Do not use any file or shell tools. "
                 "Then call the send_message tool once with severity \"success\" "
                 "to report that this step is complete."),
}


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
    v = row[0]; v = v.decode() if isinstance(v,(bytes,bytearray)) else v
    d = json.loads(v); parn = d.get("profile_arn")
    if not parn and prow:
        pv = prow[0]; pv = pv.decode() if isinstance(pv,(bytes,bytearray)) else pv
        try: parn = json.loads(pv).get("arn")
        except Exception: pass
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": parn}


TOK = read_token()
CWD = tempfile.mkdtemp(prefix="kas-csig-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-csighome-")
env = dict(os.environ); env["HOME"] = TMPH
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()], daemon=True).start()
i = [0]


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc":"2.0","id":i[0],"method":m,"params":pr})+"\n"); p.stdin.flush()
    return i[0]


def rep(rid, res):
    p.stdin.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); p.stdin.flush()


def serve(o):
    m, rid = o.get("method"), o.get("id")
    if rid is None or not m: return False
    if m == "_kiro/auth/getAccessToken": rep(rid, TOK)
    elif m == "_kiro/terminal/shell_type": rep(rid, {"shellType":"bash"})
    else: rep(rid, {})
    return True


def pump(until, to=60):
    end = time.time()+to
    while time.time() < end:
        try: raw = q.get(timeout=2)
        except queue.Empty: continue
        try: o = json.loads(raw)
        except Exception: continue
        OUT.write(raw+"\n")
        if serve(o): continue
        if o.get("id") == until and ("result" in o or "error" in o): return o
    return None


def follow(to=300):
    """Collect run_complete + any paused/node_paused for one run."""
    end = time.time()+to; ev = []
    while time.time() < end:
        try: raw = q.get(timeout=2)
        except queue.Empty: continue
        try: o = json.loads(raw)
        except Exception: continue
        OUT.write(raw+"\n")
        if serve(o): continue
        m = o.get("method","")
        if m.startswith("_kiro/workflow/"):
            ev.append((m, o.get("params") or {}))
            if m.endswith("/run_complete"):
                return ev
    return ev


req("initialize", {"protocolVersion":1,
                   "clientCapabilities":{"fs":{"readTextFile":True,"writeTextFile":True}},
                   "_meta":{"kiro":{"clientName":"cyril-audit","checkpoints":True}}})
pump(1, 30)
r = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 60) or {}
sid = r.get("result",{}).get("sessionId")
print(f"binary={os.path.basename(os.path.dirname(KIRO))} mode={MODE} reps={REPS} "
      f"workflowsEnabled={r.get('result',{}).get('_meta',{}).get('workflowsEnabled')}")
pump(-1, 5)

DAG = {"name": f"cyril-csig-{MODE}", "description": "one step; completion-signal probe",
       "inputs": {}, "steps": [{"type":"step","id":"only","agent":"wf-coder",
                                "prompt": PROMPTS[MODE]}]}

rows = []
for n in range(REPS):
    c = pump(req("_kiro/workflow/new", {"workflow":DAG,"inputs":{},
                                        "parentSessionId":sid,"workspacePaths":[CWD]}), 45)
    wf = ((c or {}).get("result") or {}).get("workflowId")
    if not wf:
        rows.append((n, "NEW-FAILED", None, None, None)); continue
    iv = pump(req("_kiro/workflow/invoke", {"workflowId": wf}), 60)
    if not iv or "error" in iv:
        rows.append((n, "INVOKE-FAILED", None, None, None)); continue
    ev = follow(300)
    rc = next((pr for m, pr in ev if m.endswith("/run_complete")), None)
    paused = sum(1 for m, _ in ev if m.endswith("/paused") or m.endswith("/node_paused"))
    node = None
    if rc:
        for ch in (rc.get("finalState",{}).get("root",{}) or {}).get("children",[]) or []:
            node = ch; break
    rows.append((n,
                 (rc or {}).get("status"),
                 (node or {}).get("status"),
                 (node or {}).get("completionSignal"),
                 (node or {}).get("completionSignalSource"),
                 paused))

print(f"\n{'rep':<4}{'run':<12}{'node':<12}{'completionSignal':<20}{'source':<16}pausedEvents")
for row in rows:
    n, runst, nodest, sig, src, *rest = (list(row) + [None])[:6]
    print(f"{n:<4}{str(runst):<12}{str(nodest):<12}{str(sig):<20}{str(src):<16}{rest[0] if rest else ''}")

OUT.close(); p.stdin.close(); p.terminate()
