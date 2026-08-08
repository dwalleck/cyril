#!/usr/bin/env python3
"""Do the `_kiro/workflow/*` methods route with the gate OFF? (2.16.0 / KAS 0.27.8)

Every workflow probe in the 2.16.0 audit flipped `settings.workflows.enabled`
ON before calling anything, so one question was never asked: does the METHOD
surface require the gate, or only the five agent-facing TOOLS?

It matters for cyril. `resolveWorkflows()` turns on both at once, and enabling
the tools means the MODEL can start a workflow run mid-turn -- the exact
non-determinism a client-driven workflow UX is trying to avoid (audit hazard 4:
step outcome is decided by a model-issued send_message). If the methods answer
with the gate off, cyril can drive runs natively while the model never sees
run_workflow.

A/B on ONE connection, per feedback_kiro_schema_vs_runtime (always probe with a
control). No settings at `initialize` (that channel is connection-scoped and
would contaminate both arms):

    arm A  session/new with NO workflow settings   -> expect workflowsEnabled false
    arm B  session/new WITH workflows.enabled=true -> known-good control

Then the same three read-only methods against each sessionId. initialize +
2x session/new + 6 read-only calls; no prompt turn, so zero credits.

    probe-kas-workflow-gateoff-2.16.0.py <kiro-cli-chat> <out.jsonl>

HOME-isolated per feedback_isolate_kiro_probes_with_home.
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")


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
CWD = tempfile.mkdtemp(prefix="kas-wfoff-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-wfoffhome-")
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


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def rep(rid, res):
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    p.stdin.flush()


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
        OUT.write(raw + "\n")
        m = o.get("method")
        rid = o.get("id")
        if rid is not None and m:
            if m == "_kiro/auth/getAccessToken":
                rep(rid, TOK)
            elif m == "_kiro/terminal/shell_type":
                rep(rid, {"shellType": "bash"})
            else:
                rep(rid, {})
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


# NO `settings` at initialize: that channel overrides backend feature flags
# connection-wide and would enable workflows for BOTH arms.
req("initialize", {
    "protocolVersion": 1,
    "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}},
    "_meta": {"kiro": {"clientName": "cyril-audit", "checkpoints": True}},
})
pump(1, 30)


def new_session(meta_settings):
    params = {"cwd": CWD, "mcpServers": []}
    if meta_settings is not None:
        params["_meta"] = {"kiro": {"settings": meta_settings}}
    rid = req("session/new", params)
    r = pump(rid, 60) or {}
    res = r.get("result", {})
    return res.get("sessionId"), res.get("_meta", {}).get("workflowsEnabled")


sid_off, en_off = new_session(None)                                  # arm A
sid_on, en_on = new_session({"workflows": {"enabled": True}})        # arm B (control)
pump(-1, 5)

print(f"arm A (gate off): sessionId={sid_off} workflowsEnabled={en_off!r}")
print(f"arm B (control ): sessionId={sid_on} workflowsEnabled={en_on!r}")

METHODS = (
    ("_kiro/workflow/listRecipes", lambda s: {"sessionId": s, "workspacePaths": [CWD]}),
    ("_kiro/workflow/list", lambda s: {"sessionId": s, "workspacePaths": [CWD]}),
    ("_kiro/workflow/listWatchHandlers", lambda s: {"sessionId": s}),
)

verdict = {}
for arm, sid in (("A-off", sid_off), ("B-on", sid_on)):
    print(f"\n===== arm {arm} (session {sid}) =====")
    for method, mk in METHODS:
        r = pump(req(method, mk(sid)), 45)
        if r is None:
            outcome = "NO RESPONSE"
        elif "error" in r:
            e = r["error"]
            outcome = f"ERROR {e.get('code')} {e.get('message')!r}"
        else:
            res = r["result"]
            if isinstance(res, dict) and "recipes" in res:
                outcome = f"OK recipes={len(res['recipes'])}"
            elif isinstance(res, dict) and "runs" in res:
                outcome = f"OK runs={len(res['runs'])}"
            elif isinstance(res, dict) and "handlers" in res:
                outcome = f"OK handlers={len(res['handlers'])}"
            else:
                outcome = f"OK {json.dumps(res)[:160]}"
        verdict[(arm, method)] = outcome
        print(f"  {method:<34} {outcome}")

# Phase 2 -- does AUTHORING route gate-off? `_kiro/workflow/new` only builds and
# persists the run state (returns {workflowId, initialState}); it does not execute,
# so this stays free. `invoke` is the one remaining unknown and it costs credits.
DAG = {
    "name": "cyril-gateoff-probe",
    "description": "One-step DAG; created but never invoked.",
    "inputs": {},
    "steps": [{"type": "step", "id": "only", "agent": "wf-coder",
               "prompt": "Reply with the word ok. Do not use any tools."}],
}
print()
for arm, sid in (("A-off", sid_off), ("B-on", sid_on)):
    r = pump(req("_kiro/workflow/new", {
        "workflow": DAG, "inputs": {}, "parentSessionId": sid, "workspacePaths": [CWD],
    }), 45)
    if r is None:
        outcome = "NO RESPONSE"
    elif "error" in r:
        outcome = f"ERROR {r['error'].get('code')} {r['error'].get('message')!r}"
    else:
        outcome = f"OK workflowId={(r.get('result') or {}).get('workflowId')}"
    verdict[(arm, "_kiro/workflow/new")] = outcome
    print(f"  [{arm}] _kiro/workflow/new                {outcome}")

# Phase 3 -- the one that matters: does INVOKE execute from a gate-off session,
# and do the lifecycle notifications reach that session? A run that executes but
# reports nothing is useless to a client, so both halves are the test. COSTS
# CREDITS (one trivial step; the prompt forbids tool use, workspace is a temp dir).
EVENTS = []
SESSIONS = set()


def follow(to=300):
    """Read until run_complete carries a terminal status, collecting workflow events."""
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
        m, rid = o.get("method"), o.get("id")
        if rid is not None and m:
            if m == "_kiro/auth/getAccessToken":
                rep(rid, TOK)
            elif m == "_kiro/terminal/shell_type":
                rep(rid, {"shellType": "bash"})
            else:
                rep(rid, {})
            continue
        if m and m.startswith("_kiro/workflow/"):
            EVENTS.append((m, o.get("params") or {}))
            if m.endswith("/run_complete"):
                st = (o.get("params") or {}).get("status")
                print(f"    run_complete status={st!r}")
                if st in ("completed", "failed", "aborted"):
                    return st
        elif m == "session/update":
            s = (o.get("params") or {}).get("sessionId")
            if s:
                SESSIONS.add(s)
    return None


wf_off = verdict.get(("A-off", "_kiro/workflow/new"), "")
wf_off = wf_off.split("workflowId=")[-1] if "workflowId=" in wf_off else None
print(f"\n===== phase 3: invoke {wf_off} from the GATE-OFF session =====")
if wf_off:
    r = pump(req("_kiro/workflow/invoke", {"workflowId": wf_off}), 60)
    print("  invoke ->", json.dumps(r)[:300] if r else "NO RESPONSE")
    if r and "error" not in r:
        final = follow(300)
        kinds = {}
        for m, _ in EVENTS:
            kinds[m] = kinds.get(m, 0) + 1
        print(f"  final status: {final!r}")
        print("  lifecycle events reaching a gate-off client:")
        for k in sorted(kinds):
            print(f"    {kinds[k]:2}x {k}")
        print(f"  distinct sessionIds on session/update: {len(SESSIONS)}")
        for s in sorted(SESSIONS):
            print(f"    {s}{'  (parent, gate-off)' if s == sid_off else '  (step peer session)'}")
        verdict[("A-off", "_kiro/workflow/invoke")] = (
            f"OK status={final} events={len(EVENTS)}" if final == "completed"
            else f"PARTIAL status={final} events={len(EVENTS)}"
        )
    else:
        verdict[("A-off", "_kiro/workflow/invoke")] = "ERROR " + json.dumps(r)[:160]

print("\n===== VERDICT =====")
a_ok = all(v.startswith("OK") for (arm, _), v in verdict.items() if arm == "A-off")
b_ok = all(v.startswith("OK") for (arm, _), v in verdict.items() if arm == "B-on")
if not b_ok:
    print("INCONCLUSIVE — the control arm failed; the probe, not the gate, is wrong.")
elif a_ok:
    print("METHODS ROUTE GATE-OFF. cyril can drive _kiro/workflow/* natively without")
    print("registering the five agent-facing tools — no model in the control plane.")
else:
    print("METHODS ARE GATED. Driving workflows requires flipping the gate, which also")
    print("registers run_workflow et al. — the model CAN start runs. Blast radius is real.")

OUT.close()
p.stdin.close()
p.terminate()
