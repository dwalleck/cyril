#!/usr/bin/env python3
"""v2 reasoning-surface probe (2.24.0 audit): model -> claude-sonnet-4.6 via
session/set_model, commands/options for `reasoning` and `effort`, one tiny turn,
then commands/execute effort=max and a second tiny turn. Captures every
_kiro.dev/metadata frame so the new `reasoning{support,effortLevels}` block can
be read on a reasoning-capable model, paired against 2.22.0.

    probe-v2-reasoning-2.24.0.py <kiro-cli-binary> <out.jsonl>

(orig docstring follows)

WHY: the 2.21.2 audit's first live lane never sent a session/prompt, so it
captured ZERO turn frames -- no agent_message_chunk, tool_call,
tool_call_update, plan, usage, or turn-end. That is exactly the family cyril
renders, and sweep-new-fields.py's own docstring warns that "targeted probes
answer their own hypothesis and will not notice a field that nobody asked
about". This probe exists to be swept, not to test a hypothesis.

Workload (identical on both binaries, so the path sets are comparable):
  1. initialize / session/new
  2. one prompt that forces a FILE READ tool call -- reads need no permission
     on v2, so the turn completes without approval friction
  3. drain to the session/prompt response (stop_reason), then settle

A permission request is answered by selecting the first allow-shaped option
rather than the template's `{}`, which these requests reject.

COST: exactly one tiny real turn per binary.

    probe-v2-turn-sweep-2.24.0.py <kiro-cli-binary> <out.jsonl>
"""
import json, os, queue, subprocess, sys, tempfile, threading, time

BIN = sys.argv[1]
OUT = open(sys.argv[2], "w")
REAL_HOME = os.path.expanduser("~")
PROBE_HOME = tempfile.mkdtemp(prefix="turnsweep-home-")
CWD = tempfile.mkdtemp(prefix="turnsweep-cwd-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
# Deterministic, tiny read target -- identical on both legs so the tool call
# and its content frames are comparable.
with open(os.path.join(CWD, "PROBE.txt"), "w") as fh:
    fh.write("ALPHA\n")

env = dict(os.environ)
# The kiro-cli launcher resolves `kiro-cli-chat` via PATH, NOT as its own
# sibling: an archived launcher otherwise silently spawns the INSTALLED chat
# binary (found in the 2.24.0 audit; the 2.22.0 audit's v2 pairing was void).
env["PATH"] = os.path.dirname(os.path.abspath(BIN)) + os.pathsep + env.get("PATH", "")
env["HOME"] = PROBE_HOME
env.setdefault("XDG_DATA_HOME", os.path.join(REAL_HOME, ".local", "share"))

who = subprocess.run([BIN, "user", "whoami", "--format", "json"],
                     env=env, capture_output=True, text=True)
if who.returncode != 0:
    print(f"DEVIATION: whoami failed under HOME={PROBE_HOME}; falling back to real HOME")
    env["HOME"] = REAL_HOME
    who = subprocess.run([BIN, "user", "whoami", "--format", "json"],
                         env=env, capture_output=True, text=True)
    if who.returncode != 0:
        sys.exit(f"ABORT: not authenticated (rc={who.returncode})")

ver = subprocess.run([BIN, "--version"], env=env, capture_output=True, text=True).stdout.strip()
print("binary :", BIN, "|", ver)
OUT.write(json.dumps({"probe": "preflight", "binary": BIN, "version": ver}) + "\n")

p = subprocess.Popen([BIN, "acp"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
CHILD_EXE = None
for _ in range(50):
    kids = subprocess.run(["ps", "-o", "pid=", "--ppid", str(p.pid)],
                          capture_output=True, text=True).stdout.split()
    if kids:
        try:
            CHILD_EXE = os.readlink(f"/proc/{kids[0]}/exe")
        except OSError:
            pass
        break
    time.sleep(0.1)
print("chat   :", CHILD_EXE)
OUT.write(json.dumps({"probe": "child_exe", "exe": CHILD_EXE}) + "\n")
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
METHODS = {}
FRAMES = []
PERMS = [0]

def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]

def answer_permission(rid, params):
    """Pick an allow-shaped option; `{}` is rejected by request_permission."""
    opts = (params or {}).get("options") or []
    pick = None
    for o in opts:
        k = (o.get("kind") or "").lower()
        if "allow" in k:
            pick = o.get("optionId") or o.get("id")
            if "once" in k:
                break
    if pick is None and opts:
        pick = opts[0].get("optionId") or opts[0].get("id")
    PERMS[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid,
                              "result": {"outcome": {"outcome": "selected",
                                                     "optionId": pick}}}) + "\n")
    p.stdin.flush()

def pump(until, to=120, tag=""):
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
        o["_tag"] = tag
        OUT.write(json.dumps(o) + "\n"); OUT.flush()
        m, rid = o.get("method"), o.get("id")
        if m:
            key = m
            if m == "session/update":
                key = "session/update:" + str(((o.get("params") or {}).get("update") or {}).get("sessionUpdate"))
            METHODS[key] = METHODS.get(key, 0) + 1
            if m == "_kiro.dev/metadata":
                FRAMES.append((tag, o.get("params") or {}))
        if rid is not None and m:                       # server->client request
            if "request_permission" in m:
                answer_permission(rid, o.get("params"))
            else:
                p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {}}) + "\n")
                p.stdin.flush()
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None

pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {},
                        "clientInfo": {"name": "cyril-probe", "version": "0"}}), 30, "init")
r = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 60, "session_new")
sid = ((r or {}).get("result") or {}).get("sessionId")
if not sid:
    sys.exit("ABORT: no sessionId")
RES = {"probe": "v2-reasoning", "version": ver}
def brief(o):
    if o is None: return {"timeout": True}
    return {"error": o["error"]} if "error" in o else {"result": o.get("result")}
RES["set_model"] = brief(pump(req("session/set_model", {"sessionId": sid, "modelId": "claude-sonnet-4.6"}), 30, "set_model"))
for c in ("reasoning", "effort"):
    RES[f"options_{c}"] = brief(pump(req("_kiro.dev/commands/options", {"sessionId": sid, "command": c, "partial": ""}), 30, f"opt_{c}"))
def meta_frames(tag):
    return [x for x in FRAMES if x[0] == tag]
def turn(tag):
    rid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "Reply with exactly the word OK and nothing else."}]})
    resp = pump(rid, 180, tag); pump(-1, 5, tag + "_settle")
    return ((resp or {}).get("result") or {}).get("stopReason")
RES["turn1_stop"] = turn("turn1")
RES["exec_effort_max"] = brief(pump(req("_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": "effort", "args": {"value": "max"}}}), 30, "exec_effort"))
RES["turn2_stop"] = turn("turn2")
# unknown-variant probe: the serde error lists every TuiCommand variant, so it
# shows whether `reasoning` is executable even though commands/options rejects it
RES["exec_reasoning_probe"] = brief(pump(req("_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": "reasoning", "args": {}}}), 30, "exec_reasoning"))
RES["exec_variant_list"] = brief(pump(req("_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": "zz_not_a_command", "args": {}}}), 30, "exec_bogus"))
for tag, args in tuple((t, a) for t, a in json.loads(os.environ.get("RARGS", "[]"))) or (("rargs_bogus", {"zz": 1}), ("rargs_thinking_off", {"thinking": "off"}),
                  ("rargs_thinking_false", {"thinking": False}), ("rargs_value_off", {"value": "off"})):
    RES[tag] = brief(pump(req("_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": "reasoning", "args": args}}), 30, tag))
RES["turn3_stop"] = turn("turn3")
RES["metadata"] = [(t, {k: v for k, v in pr.items() if k in ("effort", "reasoning", "contextUsagePercentage")}) for t, pr in FRAMES]
OUT.write(json.dumps(RES) + "\n"); OUT.flush()
print(json.dumps(RES, indent=1)[:6000])
try:
    p.stdin.close(); p.terminate(); p.wait(timeout=10)
except Exception:
    p.kill()
