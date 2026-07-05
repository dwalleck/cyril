#!/usr/bin/env python3
"""
First live run of `_kiro/spec/invoke {operation:"runAllTasks"}` (2.11.0,
@kiro/agent 0.8.0) — the last unexercised spec-execution op (covenant: "only
runAllTasks (looped executeTask) is unexercised"; bundle-read 2026-07-05: one
long spec-mode turn prompted "Run all tasks for this spec" with a
checkpoint/resume system, ready-order progression).

To fit a token window, the spec is HAND-SEEDED (no createSpec/generateDocument
turns): buggy calc.py + .kiro/specs/calc-fix/{requirements,design,tasks}.md
with two tiny leaf tasks (fix add(); add a regression test). Format matches
spec-sample-2.7.1. getTaskStatuses validates the hand-written tasks.md parses
BEFORE burning the run-all turn.

Arc: initialize -> spec/resolveSession {featureName, strategy:fresh} ->
spec/getTaskStatuses (parse check) -> spec/invoke runAllTasks -> pump to
turn_end -> oracle: calc.py fixed, test file exists AND passes when run,
tasks.md checkboxes flipped, getTaskStatuses shows completed. Permissions
auto-approved and counted (2.10.0 finding: spec-flow writes fire permissions
even with autopilot on).
"""
import json, os, pathlib, queue, sqlite3, subprocess, tempfile, threading, time

KIRO = "kiro-cli-chat"
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
HERE = os.path.dirname(os.path.abspath(__file__))
OUTDIR = os.path.join(HERE, "kas-runalltasks-dumps")
os.makedirs(OUTDIR, exist_ok=True)

BUGGY = "def add(a, b):\n    return a - b  # BUG: should be a + b\n\n\nif __name__ == \"__main__\":\n    print(add(2, 3))\n"

REQUIREMENTS = """# Requirements: calc-fix

## 1. Correct addition

- 1.1 The `add(a, b)` function in `calc.py` SHALL return the sum `a + b`.
- 1.2 A regression test SHALL exist that fails if `add` regresses.
"""

DESIGN = """# Design: calc-fix

Single-file fix. `calc.py` keeps its current shape; only the return
expression changes. The regression test is a plain-`assert` script,
`test_calc.py`, runnable with `python test_calc.py` (no framework).
"""

TASKS = """# Implementation Plan: calc-fix

## Overview

Fix the addition bug in calc.py and fence it with a minimal regression test.

## Tasks

- [ ] 1. Fix the add function
  - In `calc.py`, change `return a - b` to `return a + b`
  - _Requirements: 1.1_

- [ ] 2. Add a regression test
  - Create `test_calc.py` containing `from calc import add` and
    `assert add(2, 3) == 5`, then a print of "ok"
  - Verify it passes by running `python test_calc.py`
  - _Requirements: 1.2_
"""


def log(*a):
    print(" ".join(str(x) for x in a), flush=True)


def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        prow = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        c.close()
    if row is None:
        raise SystemExit("logged out; run `kiro-cli login`")
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    profile_arn = d.get("profile_arn")
    if not profile_arn and prow:
        pv = prow[0]; pv = pv.decode() if isinstance(pv, (bytes, bytearray)) else pv
        try: profile_arn = json.loads(pv).get("arn")
        except Exception: pass
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": profile_arn}


def main():
    v = subprocess.run([KIRO, "--version"], capture_output=True, text=True).stdout.strip()
    log(f"# binary={v}")
    cwd = tempfile.mkdtemp(prefix="kas-runall-")
    subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
                   cwd=cwd, shell=True)
    pathlib.Path(cwd, "calc.py").write_text(BUGGY)
    spec = pathlib.Path(cwd, ".kiro", "specs", "calc-fix")
    spec.mkdir(parents=True)
    (spec / "requirements.md").write_text(REQUIREMENTS)
    (spec / "design.md").write_text(DESIGN)
    (spec / "tasks.md").write_text(TASKS)
    subprocess.run("git add -A && git commit -qm baseline", cwd=cwd, shell=True)
    tasks_md = str(spec / "tasks.md")
    docs = [str(spec / "requirements.md"), str(spec / "design.md"), tasks_md]

    proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "v3"], cwd=cwd,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, text=True, bufsize=1)
    assert proc.stdin and proc.stdout
    stdin, stdout = proc.stdin, proc.stdout
    msgs = queue.Queue()
    threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in stdout if l.strip()],
                                     msgs.put(None)), daemon=True).start()
    state = {"id": 0, "turn_ends": 0}
    frames, perms, agent_text, tool_titles = [], [], [], []

    def req(m, p):
        state["id"] += 1
        stdin.write(json.dumps({"jsonrpc": "2.0", "id": state["id"], "method": m, "params": p}) + "\n")
        stdin.flush()
        return state["id"]

    def reply(rid, res):
        stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
        stdin.flush()

    def handle(o):
        m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
        frames.append({"dir": "in", "method": m, "id": rid, "params": p})
        if rid is None:
            if m and "session/update" in m:
                u = (p.get("update") or {}) if isinstance(p, dict) else {}
                if isinstance(u, dict):
                    kind = u.get("sessionUpdate")
                    meta = ((u.get("_meta") or {}).get("kiro") or {})
                    if kind == "agent_message_chunk":
                        agent_text.append(((u.get("content") or {}).get("text") or ""))
                    if kind == "tool_call":
                        tool_titles.append((u.get("title"), meta.get("kind")))
                    if kind == "session_info_update" and meta.get("kind") == "turn_end":
                        state["turn_ends"] += 1
            return
        if m == "_kiro/auth/getAccessToken": reply(rid, read_token()); return
        if m == "_kiro/terminal/shell_type": reply(rid, {"shellType": "bash"}); return
        if m == "_kiro/userInput":
            opts = p.get("options", [])
            pick = next((o2 for o2 in opts if isinstance(o2, dict) and o2.get("recommended")),
                        opts[0] if opts else None)
            ans = (pick.get("title") if isinstance(pick, dict) else pick) if pick else "yes"
            log(f"  -> userInput {str(p.get('question'))[:70]!r} answering {ans!r}")
            reply(rid, {"action": "answered", "answer": ans}); return
        if m == "session/request_permission":
            t = (p.get("toolCall", {}) or {}).get("title") or "?"
            perms.append(t)
            log(f"  -> permission: {t!r} (allowing)")
            opts = p.get("options", [])
            pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                        opts[0] if opts else None)
            reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                  if pick else {"outcome": {"outcome": "cancelled"}})
            return
        reply(rid, {})

    def call_sync(method, params, to=60):
        rid = req(method, params); end = time.time() + to
        while time.time() < end:
            try: raw = msgs.get(timeout=1)
            except queue.Empty: continue
            if raw is None: return None
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: handle(o)
            elif o.get("id") == rid: return o
        return None

    def pump_turn(to):
        before = state["turn_ends"]; end = time.time() + to
        while time.time() < end and state["turn_ends"] == before:
            try: raw = msgs.get(timeout=2)
            except queue.Empty: continue
            if raw is None: break
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: handle(o)
        g = time.time() + 6
        while time.time() < g:
            try: raw = msgs.get(timeout=1)
            except queue.Empty: continue
            if raw is None: break
            try: o = json.loads(raw)
            except Exception: continue
            if "method" in o: handle(o)
        return state["turn_ends"] > before

    call_sync("initialize", {"protocolVersion": 1,
                             "clientCapabilities": {"_meta": {"kiro": {"userInput": True}}}})
    rr = call_sync("_kiro/spec/resolveSession",
                   {"featureName": "calc-fix", "strategy": "fresh", "workspacePaths": [cwd]})
    sid = ((rr or {}).get("result") or {}).get("sessionId")
    log(f"resolveSession -> {sid} (error={json.dumps((rr or {}).get('error'))[:150]})")
    if not sid:
        raise SystemExit("no spec session")

    ts = call_sync("_kiro/spec/getTaskStatuses",
                   {"tasksFilePath": tasks_md, "featureName": "calc-fix", "workspacePaths": [cwd]})
    ts_res = (ts or {}).get("result")
    log(f"getTaskStatuses (pre) -> {json.dumps(ts_res)[:300]}")
    if not ts_res:
        raise SystemExit("hand-written tasks.md did not parse — fix format before burning the turn")

    ir = call_sync("_kiro/spec/invoke",
                   {"operation": "runAllTasks", "sessionId": sid, "featureName": "calc-fix",
                    "tasksFilePath": tasks_md, "specDocuments": docs})
    log(f"runAllTasks invoke -> result={json.dumps((ir or {}).get('result'))[:200]} "
        f"error={json.dumps((ir or {}).get('error'))[:200]}")
    ended = pump_turn(900)
    log(f"turn ended: {ended}")

    calc = pathlib.Path(cwd, "calc.py").read_text()
    fixed = "return a + b" in calc
    test_path = pathlib.Path(cwd, "test_calc.py")
    test_exists = test_path.is_file()
    test_passes = False
    if test_exists:
        test_passes = subprocess.run(["python3", "test_calc.py"], cwd=cwd,
                                     capture_output=True).returncode == 0
    tasks_now = pathlib.Path(tasks_md).read_text()
    boxes_flipped = tasks_now.count("[x]")
    porcelain = subprocess.run("git status --porcelain", cwd=cwd, shell=True,
                               capture_output=True, text=True).stdout.strip()
    ts2 = call_sync("_kiro/spec/getTaskStatuses",
                    {"tasksFilePath": tasks_md, "featureName": "calc-fix", "workspacePaths": [cwd]})
    log("\n===== ORACLE =====")
    log(f"  calc fixed: {fixed}")
    log(f"  test_calc.py exists: {test_exists}  passes: {test_passes}")
    log(f"  tasks.md [x] count: {boxes_flipped}")
    log(f"  porcelain: {porcelain!r}")
    log(f"  permissions fired: {perms}")
    log(f"  tool calls: {tool_titles[:12]}")
    log(f"  getTaskStatuses (post): {json.dumps((ts2 or {}).get('result'))[:400]}")

    pathlib.Path(OUTDIR, "runalltasks.json").write_text(json.dumps({
        "cwd": cwd, "resolve": (rr or {}).get("result"),
        "statuses_pre": ts_res, "invoke_result": (ir or {}).get("result"),
        "invoke_error": (ir or {}).get("error"), "turn_ended": ended,
        "oracle": {"calc_fixed": fixed, "test_exists": test_exists,
                   "test_passes": test_passes, "boxes_flipped": boxes_flipped,
                   "porcelain": porcelain},
        "permissions": perms, "tool_calls": tool_titles,
        "statuses_post": (ts2 or {}).get("result"),
        "agent_text": "".join(agent_text), "frames": frames}, indent=2) + "\n")
    log(f"  dump: {OUTDIR}/runalltasks.json")
    try: stdin.close()
    except Exception: pass
    proc.terminate()


if __name__ == "__main__":
    main()
