#!/usr/bin/env python3
"""Paired v2 probe for the two host-side ACP lines in the kiro-cli 2.21.2 notes.

Changelog under test (both are v2/host changes -- KAS 0.58.7 is byte-frozen
across 2.21.1->2.21.2, so neither line can be a KAS change):

  1. "ACP session creation no longer advertises a hardcoded model list when the
     model-list fetch fails; it retries briefly, then reports the list as
     unknown"
  2. "Announce agent switches made through the ACP protocol so the TUI stops
     showing the previous agent"

Static evidence already gathered (strings new in 2.21.2, absent in 2.21.1):
  "Failed to fetch available models (attempt {}): {}; retrying in {}"
  "Failed to fetch available models after {} attempts: {}; the model list is unknown"
  "no model list was fetched for this session"
  "mock model list failure armed by test"

This probe establishes the HEALTHY-path baseline on both binaries (the failure
path is not safely inducible without the test hook) and answers (2) directly:
does an ACP-ORIGINATED agent switch emit `_kiro.dev/agent/switched`?

Arms (no paid turns -- no session/prompt is ever sent):
  1. initialize                -> agentCapabilities verbatim
  2. session/new               -> models / modes / configOptions verbatim + key set
  3. CONTROL commands/options {command:"model"} -> the advertised model list
  4. CONTROL commands/options {command:"agent"} -> the available agents
  5. ARM     commands/execute {command:"agent", args:{value:<other agent>}}
     then drain frames and report every notification method seen, flagging
     `_kiro.dev/agent/switched` and any current_mode_update.

commands/execute is sent as a REQUEST (with id) and awaited -- as a
notification it is silently dropped for these commands.

Env: HOME=<tmp> + real XDG_DATA_HOME (house isolation rule), falling back to
real HOME with a loud DEVIATION line if whoami fails under isolation.

    probe-v2-models-agentswitch-2.21.2.py <kiro-cli-binary> <out.jsonl>
"""
import json, os, queue, subprocess, sys, tempfile, threading, time

BIN = sys.argv[1]
OUT = open(sys.argv[2], "w")
REAL_HOME = os.path.expanduser("~")
PROBE_HOME = tempfile.mkdtemp(prefix="mas-home-")
CWD = tempfile.mkdtemp(prefix="mas-cwd-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

env = dict(os.environ)
env["HOME"] = PROBE_HOME
env.setdefault("XDG_DATA_HOME", os.path.join(REAL_HOME, ".local", "share"))

who = subprocess.run([BIN, "user", "whoami", "--format", "json"],
                     env=env, capture_output=True, text=True)
if who.returncode != 0:
    print(f"DEVIATION: whoami failed under HOME={PROBE_HOME} (rc={who.returncode}); "
          f"falling back to real HOME")
    env["HOME"] = PROBE_HOME = REAL_HOME
    who = subprocess.run([BIN, "user", "whoami", "--format", "json"],
                         env=env, capture_output=True, text=True)
    if who.returncode != 0:
        sys.exit(f"ABORT: not authenticated (rc={who.returncode}) "
                 f"stdout={who.stdout.strip()[:120]!r}")

ver = subprocess.run([BIN, "--version"], env=env,
                     capture_output=True, text=True).stdout.strip()
print("binary :", BIN)
print("version:", ver)
OUT.write(json.dumps({"probe": "preflight", "binary": BIN, "version": ver}) + "\n")

p = subprocess.Popen([BIN, "acp"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
NOTIFS = []

def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]

def pump(until, to=45, tag=""):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = q.get(timeout=1.5)
        except queue.Empty:
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        o["_tag"] = tag
        OUT.write(json.dumps(o) + "\n"); OUT.flush()
        m, rid = o.get("method"), o.get("id")
        if m and rid is None:
            NOTIFS.append((tag, m, o.get("params")))
        if rid is not None and m:          # server->client request: always answer
            p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {}}) + "\n")
            p.stdin.flush()
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None

R = {"binary": BIN, "version": ver}

# 1. initialize
r = pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {},
                            "clientInfo": {"name": "cyril-probe", "version": "0"}}), 25, "init")
ires = (r or {}).get("result") or {}
R["agentCapabilities"] = ires.get("agentCapabilities")

# 2. session/new
r = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 60, "session_new")
sres = (r or {}).get("result") or {}
sid = sres.get("sessionId")
R["session_new_keys"] = sorted(sres.keys())
R["models_present"] = "models" in sres
R["models"] = sres.get("models")
R["configOptions_present"] = "configOptions" in sres
R["modes"] = sres.get("modes")
if not sid:
    R["abort"] = "no sessionId"
    OUT.write(json.dumps({"probe": "result", **R}) + "\n"); sys.exit("ABORT: no sessionId")
mc = (sres.get("models") or {})
R["model_count"] = len(mc.get("availableModels") or []) if isinstance(mc, dict) else None
R["current_model"] = mc.get("currentModelId") if isinstance(mc, dict) else None
pump(-1, 4, "settle")

# 3. CONTROL: model options
r = pump(req("_kiro.dev/commands/options", {"command": "model", "sessionId": sid, "partial": ""}), 30, "model_options")
mo = ((r or {}).get("result") or {})
R["model_options_error"] = (r or {}).get("error")
opts = mo.get("options") or mo.get("data", {}).get("options") or []
R["model_options_count"] = len(opts)

# 4. CONTROL: agent options
r = pump(req("_kiro.dev/commands/options", {"command": "agent", "sessionId": sid, "partial": ""}), 30, "agent_options")
ao = ((r or {}).get("result") or {})
R["agent_options_error"] = (r or {}).get("error")
aopts = ao.get("options") or ao.get("data", {}).get("options") or []
R["agents"] = [o.get("value") or o.get("label") for o in aopts]
# commands/options carries NO `current` flag on v2 (cyril-imjx), so the live
# agent must come from session/new's modes.currentModeId. Using the absent
# flag picks the CURRENT agent as the switch target, making the switch a
# no-op that announces nothing -- a false negative.
R["agent_current"] = next((o.get("value") for o in aopts if o.get("current")), None) \
    or ((sres.get("modes") or {}).get("currentModeId"))

# 5. ARM: switch agent over ACP, then watch what is announced
target = next((o.get("value") for o in aopts
               if o.get("value") and o.get("value") != R["agent_current"]), None)
assert target != R["agent_current"], "switch target must differ from the live agent"
R["switch_target"] = target
before = len(NOTIFS)
if target:
    t0 = time.time()
    r = pump(req("_kiro.dev/commands/execute",
                 {"sessionId": sid,
                  "command": {"command": "agent", "args": {"value": target}}}), 45, "agent_switch")
    R["switch_ms"] = round((time.time() - t0) * 1000, 1)
    R["switch_error"] = (r or {}).get("error")
    R["switch_result"] = (r or {}).get("result")
    pump(-1, 8, "post_switch")     # drain trailing notifications
after = NOTIFS[before:]
R["notifs_after_switch"] = [m for _, m, _ in after]
R["agent_switched_seen"] = any(m.endswith("agent/switched") for _, m, _ in after)
R["agent_switched_params"] = next((pr for _, m, pr in after if m.endswith("agent/switched")), None)
R["mode_update_seen"] = any("session/update" in m for _, m, _ in after)

OUT.write(json.dumps({"probe": "result", **R}) + "\n")
OUT.flush()
print(json.dumps({k: v for k, v in R.items() if k not in ("models", "agentCapabilities")},
                 indent=2)[:2000])
try:
    p.stdin.close(); p.terminate(); p.wait(timeout=10)
except Exception:
    p.kill()
