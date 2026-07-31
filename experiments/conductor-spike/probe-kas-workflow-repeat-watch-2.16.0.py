#!/usr/bin/env python3
"""LIVE verification of the five `_kiro/workflow/*` shapes the 2.16.0 custom-DAG
probe did NOT exercise: watch_poll, loop_iteration, node_paused, paused, steps_queued.

probe-kas-custom-dag-live-2.16.0.py proved a client-authored DAG runs, but its
plan had no `repeat` and no `watch` node and never paused, so five of the nine
documented payload shapes stayed static-only. This probe drives all five.

Three phases, two workflows:

  A. WATCH  — a single `watch` node on the github-pr handler pointed at a MERGED
     PR. Merged/closed => outcome "terminal-state" on the first poll, so the node
     completes immediately. A `watch` node is explicitly non-LLM, so phase A costs
     ZERO CREDITS. `idleTimeoutSec` is set as a bounded safety net: if `gh` fails
     the handler returns "idle" and would otherwise re-poll forever at
     pollIntervalSec, so the timeout converts that into idle/idle-timeout — which
     still verifies two more outcome values.

  B. REPEAT — a `repeat` node, maxIterations 2, onMaxIterations "pause", with a
     stopCondition that can never be satisfied (fileCheck on a file that does not
     exist). Yields loop_iteration per pass, then the exhaustion pause
     (node_paused + paused). COSTS CREDITS: 2 trivial agent turns.

  C. UPDATE — `_kiro/workflow/update` action=replace_remaining against the paused
     run from phase B, which is the documented emitter for steps_queued.

    probe-kas-workflow-repeat-watch-2.16.0.py <kiro-cli-chat> <out.jsonl>

Safety: temp git workspace, HOME-isolated. GH_CONFIG_DIR points at the real gh
config so the isolated HOME does not break `gh` auth (no token is copied).
Prompts forbid tool use; blast radius is an empty temp dir.
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
PR_URL = "https://github.com/dwalleck/cyril/pull/70"   # MERGED -> terminal-state

WATCH_DAG = {
    "name": "cyril-audit-watch",
    "description": "Single watch node on a merged PR — verifies watch_poll.",
    "inputs": {},
    "steps": [{
        "type": "watch",
        "id": "pr-watch",
        "handler": "github-pr",
        "idleTimeoutSec": 35,
        "config": {"url": PR_URL, "pollIntervalSec": 30},
    }],
}

REPEAT_DAG = {
    "name": "cyril-audit-repeat",
    "description": "Repeat node that exhausts maxIterations — verifies loop_iteration + pause.",
    "inputs": {"token": "string"},
    "steps": [{
        "type": "repeat",
        "id": "loop",
        "maxIterations": 2,
        "onMaxIterations": "pause",
        # sentinel.json is never created, so this can never be satisfied
        "stopCondition": {"fileCheck": {"path": "sentinel.json", "jsonPath": "done", "value": True}},
        "steps": [{
            "type": "step", "id": "tick", "agent": "wf-coder",
            "prompt": ("Do NOT use any tools. Do NOT read, write or modify any file. "
                       "Immediately signal completion, reporting exactly: tick-{{token}}"),
        }],
    }],
}


def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute(
            "select value from auth_kv where key in "
            "('kirocli:odic:token','kirocli:social:token') order by key desc"
        ).fetchone()
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
CWD = tempfile.mkdtemp(prefix="kas-rw-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-rwhome-")
env = dict(os.environ)
env["HOME"] = TMPH
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
# keep `gh` authenticated despite the isolated HOME
env["GH_CONFIG_DIR"] = os.path.expanduser("~/.config/gh")

p = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
EVENTS = []


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def rep(rid, res):
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    p.stdin.flush()


def pump(until, to=60, stop_kind=None):
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
        OUT.write(raw + "\n")
        OUT.flush()
        m, rid, pr = o.get("method"), o.get("id"), o.get("params") or {}

        if m and m.startswith("_kiro/workflow/"):
            EVENTS.append((m, pr))
            short = m.rsplit("/", 1)[1]
            extra = ""
            if short == "watch_poll":
                extra = f" outcome={pr.get('outcome')}"
            elif short == "loop_iteration":
                extra = f" iter={pr.get('iteration')} stopMet={pr.get('stopConditionMet')}"
            elif short in ("node_paused", "paused"):
                extra = f" reason={(pr.get('reason') or pr.get('pauseReason'))!r}"
            elif short == "steps_queued":
                extra = f" pending={len(pr.get('pendingSteps') or [])} resolution={pr.get('resolution')}"
            print(f"  <- {short}{extra}")

        if rid is not None and m:
            if m == "_kiro/auth/getAccessToken":
                rep(rid, TOK)
            elif m == "_kiro/terminal/shell_type":
                rep(rid, {"shellType": "bash"})
            elif m == "session/request_permission":
                opts = pr.get("options", [])
                pick = next((x for x in opts
                             if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                            opts[0] if opts else None)
                rep(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                    if pick else {"outcome": {"outcome": "cancelled"}})
            else:
                rep(rid, {})
            continue

        if stop_kind and m == f"_kiro/workflow/{stop_kind}":
            return o
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}},
    "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True}},
})
pump(1, 30)
nid = req("session/new", {
    "cwd": CWD, "mcpServers": [],
    "_meta": {"kiro": {"settings": {"workflows": {"enabled": True}}}},
})
sess = pump(nid, 60)
sid = (sess or {}).get("result", {}).get("sessionId")
print("parent sessionId:", sid)
pump(-1, 5)


def run_dag(label, dag, inputs, wait, stop_kind):
    print(f"\n########## {label} ##########")
    rid = req("_kiro/workflow/new", {
        "workflow": dag, "inputs": inputs,
        "parentSessionId": sid, "workspacePaths": [CWD],
    })
    created = pump(rid, 60)
    err = (created or {}).get("error")
    if err:
        print("  new FAILED:", json.dumps(err)[:500])
        return None
    wf = (created or {}).get("result", {}).get("workflowId")
    print("  workflowId:", wf)
    iid = req("_kiro/workflow/invoke", {"workflowId": wf})
    pump(iid, 45)
    pump(-1, wait, stop_kind=stop_kind)
    return wf


# A. watch — zero credits
run_dag("PHASE A — watch node", WATCH_DAG, {}, 120, "run_complete")

# B. repeat to exhaustion — costs 2 turns
wf_b = run_dag("PHASE B — repeat node", REPEAT_DAG, {"token": "OK42"}, 300, "paused")

# C. steps_queued via update on the paused run
if wf_b:
    print("\n########## PHASE C — update(replace_remaining) ##########")
    uid = req("_kiro/workflow/update", {
        "workflowId": wf_b,
        "action": "replace_remaining",
        "remainingSteps": [{
            "type": "step", "id": "final", "agent": "wf-coder",
            "prompt": ("Do NOT use any tools. Do NOT read, write or modify any file. "
                       "Immediately signal completion, reporting exactly: final-OK42"),
        }],
    })
    print("  update ->", json.dumps(pump(uid, 90))[:700])
    pump(-1, 240, stop_kind="run_complete")

print("\n=== EVENT SUMMARY ===")
kinds = {}
for m, _ in EVENTS:
    kinds[m] = kinds.get(m, 0) + 1
for k in sorted(kinds):
    print(f"  {kinds[k]:2}x {k}")

TARGETS = ["watch_poll", "loop_iteration", "node_paused", "paused", "steps_queued"]
print("\n=== TARGET SHAPES ===")
for t in TARGETS:
    hits = [pr for m, pr in EVENTS if m == f"_kiro/workflow/{t}"]
    print(f"\n--- {t}: {len(hits)} ---")
    for h in hits[:4]:
        print("   ", json.dumps(h)[:900])

OUT.close()
p.stdin.close()
p.terminate()
