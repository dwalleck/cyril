#!/usr/bin/env python3
"""Does an exit-2 `preToolUse` hook BLOCK the tool on current KAS? (cyril, 2.20.1)

Ports the HOOK_BLOCK arm of probe-kas-hooks-host-2.7.1.py onto the auth +
fs/terminal responders of probe-kas-v2hooks-2.16.0.py. Two open questions,
per docs/adr/0010 and .cyril-jiyn/findings.md caveat 1 (the exit-2 claim rests
on a 2026-06-16 2.7.1 capture that is not in the repo, plus source continuity):

  Q1. Does `{"exitCode": 2}` from `_kiro/hooks/executeHook` still stop the tool?
  Q2. Does the hook's `output` string reach the MODEL as the denial reason?
      (Decides whether a redirect hook can teach "use the grep tool instead"
      or can only deny.)

HOST mode only: advertise `_meta.kiro.hooks = {enabled: true}` (object, no v2 —
v2 hands execution to KAS wholesale and the host gate disappears). The probe
owns the registry, serves `_kiro/hooks/list`, and answers `executeHook`.

Oracles for "the tool ran", strongest first:
  1. `terminal/create` carrying MARK  — the host IS the shell; nothing runs without it
  2. the marker file on disk          — side effect survives the process
  3. `postToolUse` queried            — KAS only asks after the tool executed
  4. the agent's own final message

  HOOK_BLOCK=1  deny preToolUse with exit 2 + a DENY message
  (unset)       benign arm: exit 0, tool should run

Costs credits (one short turn per arm). Auth self-sourced; never logged.

    probe-kas-hooks-block-2.20.1.py [kiro-cli] [log-suffix]
"""
import json, os, pathlib, queue, sqlite3, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/.local/bin/kiro-cli")
ENGINE = os.environ.get("ENGINE", "v3")  # 2.8.0 renamed `kas` -> `v3`
ARM = "block" if os.environ.get("HOOK_BLOCK") else "observe"
SUFFIX = sys.argv[2] if len(sys.argv) > 2 else ARM
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(os.path.abspath(__file__)), "logs",
                   f"probe-kas-hooks-block-2.20.1-{SUFFIX}.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")


def log(*a):
    s = " ".join(str(x) for x in a)
    print(s)
    logf.write(s + "\n")
    logf.flush()


# A token the model cannot produce by accident, so "the tool ran" is unambiguous.
MARK = "TOOLRAN-9f3c1d"
DENY_MSG = "DENY: shell blocked by probe policy - use the read tool instead of cat"

CWD = tempfile.mkdtemp(prefix="kas-hooks-block-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p",
               cwd=CWD, shell=True)
MARKER_FILE = os.path.join(CWD, "tool-ran.txt")


def read_token():
    """Current IdC shape: kirocli:odic:token + profileArn from state (findings.md)."""
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


def fresh_token():
    """Re-read per callback; `kiro-cli whoami` refreshes in place when near expiry."""
    import datetime
    try:
        t = read_token()
        exp = datetime.datetime.fromisoformat(t["expiresAt"].replace("Z", "+00:00"))
        if (exp - datetime.datetime.now(datetime.timezone.utc)).total_seconds() > 120:
            return t
    except Exception:
        pass
    subprocess.run([KIRO, "whoami"], capture_output=True, text=True, timeout=60)
    return read_token()


TOK = fresh_token()

# Host mode: object with enabled, and NO v2.
META = {"kiro": {"clientName": "cyril-audit", "checkpoints": True, "hooks": {"enabled": True}}}

TMPH = tempfile.mkdtemp(prefix="kas-hooks-block-home-")
env = dict(os.environ)
if os.environ.get("TEMP_HOME"):
    env["HOME"] = TMPH
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", ENGINE], cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open(LOG + ".stderr", "w"), text=True, bufsize=1)
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()],
                                 msgs.put(None)), daemon=True).start()
_id = [0]

LIST_CALLS = []    # (trigger, toolId, toolTags)
EXEC_CALLS = []    # (hookName, command, userPrompt[:300], exitCode)
TERM_CMDS = []     # every command the agent asked the HOST to run
TERMS = {}
AGENT = []
TOOL_FRAMES = []


def req(m, p):
    _id[0] += 1
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p}) + "\n")
    PIN.flush()
    return _id[0]


def reply(rid, res):
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    PIN.flush()


def ap(x):
    return x if os.path.isabs(x or "") else os.path.join(CWD, x or "")


def hook_for(trigger):
    """One runCommand hook per trigger. Command is host-defined, so running it is safe."""
    return {"id": f"probe:{trigger}", "hookId": f"probe:{trigger}",
            "name": f"probe-{trigger}", "hookName": f"probe-{trigger}",
            "trigger": trigger, "enabled": True,
            "action": {"type": "runCommand", "command": f"echo HOOKFIRED-{trigger}"},
            "command": f"echo HOOKFIRED-{trigger}", "timeout": 30}


def answer(m, rid, p):
    if m == "_kiro/auth/getAccessToken":
        return reply(rid, fresh_token())
    if m == "_kiro/terminal/shell_type":
        return reply(rid, {"shellType": "bash"})
    if m == "_kiro/hooks/list":
        trig, tid, tags = p.get("trigger"), p.get("toolId"), p.get("toolTags")
        LIST_CALLS.append((trig, tid, tags))
        return reply(rid, {"hooks": [hook_for(trig)]})
    if m == "_kiro/hooks/executeHook":
        cmd, up, name = p.get("command", ""), p.get("userPrompt", ""), p.get("hookName", "")
        hid = p.get("hookId", "")
        is_pre = "preToolUse" in (name or "") or "preToolUse" in (hid or "")
        if os.environ.get("HOOK_BLOCK") and is_pre:
            EXEC_CALLS.append((name, cmd, up[:300], 2))
            return reply(rid, {"output": DENY_MSG, "exitCode": 2, "cancelled": False})
        try:
            r = subprocess.run(cmd, shell=True, cwd=CWD, capture_output=True,
                               text=True, timeout=p.get("timeout") or 30)
            out, code = (r.stdout + r.stderr).strip(), r.returncode
        except Exception as e:
            out, code = f"(host error: {e})", 1
        EXEC_CALLS.append((name, cmd, up[:300], code))
        return reply(rid, {"output": out, "exitCode": code, "cancelled": False})
    if m == "_kiro/hooks/sessionStart":
        return reply(rid, {"results": []})
    if m in ("fs/read_text_file", "_kiro/fs/read_file"):
        try:
            return reply(rid, {"content": pathlib.Path(ap(p.get("path"))).read_text()})
        except Exception as e:
            return reply(rid, {"content": f"(err {e})"})
    if m in ("fs/write_text_file", "_kiro/fs/write_file"):
        try:
            f = pathlib.Path(ap(p.get("path")))
            f.parent.mkdir(parents=True, exist_ok=True)
            f.write_text(p.get("content", ""))
        except Exception:
            pass
        return reply(rid, {})
    if m == "_kiro/fs/stat":
        f = pathlib.Path(ap(p.get("path")))
        return reply(rid, {"type": "directory" if f.is_dir() else "file",
                           "size": f.stat().st_size} if f.exists() else {})
    if m == "_kiro/fs/read_directory":
        try:
            return reply(rid, {"entries": [
                {"name": e.name, "type": "directory" if e.is_dir() else "file"}
                for e in pathlib.Path(ap(p.get("path"))).iterdir()]})
        except Exception:
            return reply(rid, {"entries": []})
    if m == "session/request_permission":
        opts = p.get("options", [])
        pick = next((x for x in opts if "allow" in
                     (x.get("kind", "") + x.get("optionId", "")).lower()),
                    opts[0] if opts else None)
        return reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                     if pick else {"outcome": {"outcome": "cancelled"}})
    if m == "terminal/create":
        cmd, args = p.get("command", ""), p.get("args") or []
        TERM_CMDS.append(" ".join([cmd, *args]) if args else cmd)
        tid = f"term-{len(TERMS)+1}"
        try:
            r = (subprocess.run([cmd, *args], cwd=p.get("cwd") or CWD, capture_output=True,
                                text=True, timeout=60) if args else
                 subprocess.run(cmd, shell=True, cwd=p.get("cwd") or CWD,
                                capture_output=True, text=True, timeout=60))
            TERMS[tid] = {"out": r.stdout + r.stderr, "code": r.returncode}
        except Exception as e:
            TERMS[tid] = {"out": f"(host error: {e})", "code": 127}
        return reply(rid, {"terminalId": tid})
    if m == "terminal/output":
        t = TERMS.get(p.get("terminalId"), {"out": "", "code": 0})
        return reply(rid, {"output": t["out"], "truncated": False,
                           "exitStatus": {"exitCode": t["code"], "signal": None}})
    if m == "terminal/wait_for_exit":
        t = TERMS.get(p.get("terminalId"), {"code": 0})
        return reply(rid, {"exitCode": t["code"], "signal": None})
    if m in ("terminal/release", "terminal/kill"):
        return reply(rid, {})
    return reply(rid, {})


def pump(until, to=180):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = msgs.get(timeout=2)
        except queue.Empty:
            continue
        if raw is None:
            return None
        try:
            o = json.loads(raw)
        except Exception:
            continue
        m, rid = o.get("method"), o.get("id")
        if m and rid is not None:
            answer(m, rid, o.get("params") or {})
            continue
        if m:
            u = (o.get("params") or {}).get("update") or {}
            if isinstance(u, dict):
                k = u.get("sessionUpdate")
                if k == "agent_message_chunk":
                    AGENT.append((u.get("content") or {}).get("text", ""))
                elif k in ("tool_call", "tool_call_update"):
                    TOOL_FRAMES.append(json.dumps(u)[:300])
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {"protocolVersion": 1,
                   "clientCapabilities": {
                       "fs": {"readTextFile": True, "writeTextFile": True,
                              "_meta": {"kiro": {"readFile": True, "writeFile": True,
                                                 "stat": True, "readDirectory": True}}},
                       "terminal": True,
                       "_meta": META},
                   "_meta": META})
ir = pump(1, 40)
log("  initialize _meta.kiro:", json.dumps((((ir or {}).get("result") or {})
    .get("agentCapabilities") or {}).get("_meta", {}))[:700])
nid = req("session/new", {"cwd": CWD, "mcpServers": [], "_meta": META})
nr = pump(nid, 90)
assert nr and "result" in nr, f"session/new failed: {nr}"
sid = nr["result"]["sessionId"]
log(f"# sessionId: {sid} | HOST hooks registry | arm={ARM} | kiro={KIRO}")

PROMPT = (f"Run the shell command `echo {MARK} > tool-ran.txt` using your shell tool, "
          f"then tell me whether it succeeded.")
pid = req("session/prompt", {"sessionId": sid,
                             "prompt": [{"type": "text", "text": PROMPT}]})
pr = pump(pid, 420)
log("  raw prompt response:", json.dumps(pr)[:600] if pr else "(None - pump timed out)")
log("  stopReason:", ((pr or {}).get("result") or {}).get("stopReason"))
time.sleep(2)

log("\n===== _kiro/hooks/list callbacks (agent -> host) =====")
for trig, tid, tags in LIST_CALLS:
    log(f"  trigger={trig!r:18} toolId={tid!r} toolTags={tags!r}")
log(f"  ({len(LIST_CALLS)} list calls)")

log("\n===== _kiro/hooks/executeHook callbacks =====")
for name, cmd, up, code in EXEC_CALLS:
    log(f"  hook={name!r} exit={code} cmd={cmd!r}")
    log(f"     userPrompt[:300]={up!r}")
log(f"  ({len(EXEC_CALLS)} execute calls)")

log("\n===== terminal/create commands the agent asked the HOST to run =====")
for c in TERM_CMDS:
    log(f"  {c!r}")
log(f"  ({len(TERM_CMDS)} terminal/create calls)")

review = "".join(AGENT)
marker_exists = os.path.exists(MARKER_FILE)
term_ran = any(MARK in c for c in TERM_CMDS)
triggers = [t for t, _, _ in LIST_CALLS]
post_fired = "postToolUse" in triggers
tool_ran = term_ran or marker_exists or post_fired

log("\n===== agent final message =====")
log("  ", (review[:900] or "(nothing)"))

log("\n===== Q2: did the DENY text reach the model? =====")
log("  agent message contains 'DENY':", "DENY" in review)
log("  agent message mentions 'read tool':", "read tool" in review.lower())
log("  agent message contains hook stdout 'HOOKFIRED':", "HOOKFIRED" in review)

log("\n===== VERDICT =====")
log("  arm:", "BLOCK (preToolUse denied, exit 2)" if ARM == "block" else "observe (benign, exit 0)")
log("  hooks/list called:", bool(LIST_CALLS), triggers)
log("  executeHook called:", bool(EXEC_CALLS))
log("  ORACLE 1 terminal/create carrying MARK:", term_ran)
log("  ORACLE 2 marker file on disk:", marker_exists)
log("  ORACLE 3 postToolUse queried:", post_fired)
log("  => TOOL RAN:", tool_ran)
if ARM == "block":
    log("  => Q1 ANSWER:", "exit-2 BLOCKED the tool" if not tool_ran
        else "NOT BLOCKED - tool ran despite exit 2")
log(f"\n# log: {LOG}")
PIN.close()
proc.terminate()
