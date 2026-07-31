#!/usr/bin/env python3
"""LIVE execution of a CLIENT-AUTHORED workflow DAG — kiro-cli 2.16.0 / KAS 0.27.8.

The 2.16.0 audit established the `_kiro/workflow/*` surface statically (bundle
literals + read-only list/listRecipes/listWatchHandlers calls). Per
feedback_kiro_schema_vs_runtime, schema-accepted != functional, so this probe
actually RUNS a DAG that the *client* authored inline — no model involvement in
constructing it — and captures the real lifecycle notifications.

COSTS CREDITS: the steps are real agent sessions. Kept minimal on purpose —
two trivial no-tool steps.

What it verifies:
  1. `_kiro/workflow/new` accepts an inline `workflow` object from a client
  2. `_kiro/workflow/invoke` executes it
  3. the 9 lifecycle payload shapes documented in docs/kiro-2.16.0-wire-audit.md
  4. the claimed double `node_start` per step node (pre-session, then with sessionId)
  5. `parallel` fan-out + branchId, and that steps are peer SESSIONS

Safety: temp git workspace, HOME-isolated (feedback_isolate_kiro_probes_with_home),
prompts explicitly forbid tool use so the blast radius is an empty temp dir.

    probe-kas-custom-dag-live-2.16.0.py <kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

# A DAG the CLIENT wrote. Exercises: parallel fan-out, branchId, template vars,
# workflow-level inputs. Both steps are told not to touch anything.
DAG = {
    "name": "cyril-audit-probe",
    "description": "Minimal client-authored DAG verifying the 2.16.0 workflow engine.",
    "inputs": {"token": "string"},
    "steps": [
        {
            "type": "parallel",
            "id": "fan",
            "joinPolicy": "all",
            "branches": [
                {
                    "type": "step", "id": "alpha", "agent": "wf-coder",
                    "prompt": ("Do NOT use any tools. Do NOT read, write or modify any file. "
                               "Immediately signal completion, reporting exactly: alpha-{{token}}"),
                },
                {
                    "type": "step", "id": "beta", "agent": "wf-coder",
                    "prompt": ("Do NOT use any tools. Do NOT read, write or modify any file. "
                               "Immediately signal completion, reporting exactly: beta-{{token}}"),
                },
            ],
        }
    ],
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
CWD = tempfile.mkdtemp(prefix="kas-dag-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
TMPH = tempfile.mkdtemp(prefix="kas-daghome-")
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
EVENTS = []          # every _kiro/workflow/* notification, in arrival order
SESSIONS = set()     # every sessionId seen on a session/update


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def rep(rid, res):
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    p.stdin.flush()


def pump(until, to=60, stop_on_run_complete=False):
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
            print(f"  <- {m}")
        if m == "session/update" and pr.get("sessionId"):
            SESSIONS.add(pr["sessionId"])

        if rid is not None and m:                      # server -> client request
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

        if stop_on_run_complete and m == "_kiro/workflow/run_complete":
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
print("parent sessionId:", sid,
      "| workflowsEnabled:", (sess or {}).get("result", {}).get("_meta", {}).get("workflowsEnabled"))
pump(-1, 5)

# 1) client-authored inline DAG
nwid = req("_kiro/workflow/new", {
    "workflow": DAG,
    "inputs": {"token": "OK42"},
    "parentSessionId": sid,
    "workspacePaths": [CWD],
})
created = pump(nwid, 60)
print("\n_kiro/workflow/new ->", json.dumps(created)[:600])
wf = (created or {}).get("result", {}).get("workflowId")
if not wf:
    print("FAILED to create — stopping"); OUT.close(); p.terminate(); sys.exit(1)

# 2) invoke and follow the run to completion
iid = req("_kiro/workflow/invoke", {"workflowId": wf})
print("\n--- lifecycle events ---")
pump(iid, 45)
pump(-1, 420, stop_on_run_complete=True)

print("\n=== EVENT SUMMARY ===")
kinds = {}
for m, _ in EVENTS:
    kinds[m] = kinds.get(m, 0) + 1
for k in sorted(kinds):
    print(f"  {kinds[k]:2}x {k}")

print("\n=== node_start emissions (double-emit check) ===")
for m, pr in EVENTS:
    if m.endswith("/node_start"):
        print(f"  nodeId={pr.get('nodeId'):<8} type={pr.get('type'):<9} "
              f"sessionId={'YES' if pr.get('sessionId') else '--'} "
              f"branchId={pr.get('branchId')} path={pr.get('nodePath')}")

print("\n=== full payloads ===")
for m, pr in EVENTS:
    body = json.dumps(pr)
    print(f"\n{m}\n  {body[:1500]}")

print("\n=== distinct sessionIds seen on session/update ===")
for s in sorted(SESSIONS):
    print("  ", s, "(parent)" if s == sid else "")

OUT.close()
p.stdin.close()
p.terminate()
