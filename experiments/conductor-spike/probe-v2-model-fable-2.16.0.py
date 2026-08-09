#!/usr/bin/env python3
"""Does the v2 backend accept models the /model picker doesn't advertise?

Trigger: KiroCrew's model_registry.json maps its canonical `fable-5-1m` to acp
id `claude-fable-5`, and kiro's own bundled KAS workflow recipes pin
`modelId: "claude-fable-5"` — yet the v2 /model picker (reportedly) doesn't
list it. Hypothesis: the advertised options list is a FILTERED view of what
the backend actually accepts (model catalog is backend-served; the picker list
and the accept-set can drift independently).

Protocol (control-first, per the schema-vs-runtime rule):
  1. auth pre-flight: `kiro-cli user whoami --format json` under the probe env
  2. initialize → session/new (capture `models` field if present)
  3. _kiro.dev/commands/options {command:"model", partial:""} → advertised list
  4. CONTROL: switch to a model FROM the list via _kiro.dev/commands/execute
     (proves the switch mechanism + response shape work in this session)
  5. EXPERIMENT: switch to "claude-fable-5" (not expected in the list);
     fallbacks: "fable-5-1m", "global.anthropic.claude-fable-5[1m]"
  6. VERIFY: one tiny real turn, then read the session sidecar
     ($HOME/.kiro/sessions/cli/<sid>.json — the only on-disk source of the
     model actually billed) + all _kiro.dev/metadata frames.

Env: HOME=<tmp> + real XDG_DATA_HOME (house isolation rule). Falls back to the
real HOME with a loud DEVIATION line if whoami fails under isolation.
COST: 1-2 tiny real turns (non-zero credits).

    probe-v2-model-fable-2.16.0.py <out.jsonl>
"""
import glob
import json
import os
import queue
import subprocess
import sys
import tempfile
import threading
import time

OUT = open(sys.argv[1], "w")
REAL_HOME = os.path.expanduser("~")
PROBE_HOME = tempfile.mkdtemp(prefix="mprobe-home-")
CWD = tempfile.mkdtemp(prefix="mprobe-cwd-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

env = dict(os.environ)
env["HOME"] = PROBE_HOME
env.setdefault("XDG_DATA_HOME", os.path.join(REAL_HOME, ".local", "share"))

who = subprocess.run(["kiro-cli", "user", "whoami", "--format", "json"],
                     env=env, capture_output=True, text=True)
if who.returncode != 0:
    print(f"DEVIATION: whoami failed under HOME={PROBE_HOME} "
          f"(rc={who.returncode}); falling back to real HOME")
    env["HOME"] = REAL_HOME
    who = subprocess.run(["kiro-cli", "user", "whoami", "--format", "json"],
                         env=env, capture_output=True, text=True)
    if who.returncode != 0:
        sys.exit(f"ABORT: whoami failed under real HOME too: {who.stderr[:200]}")
print("whoami:", who.stdout.strip()[:200])
OUT.write(json.dumps({"probe": "whoami", "out": who.stdout.strip()}) + "\n")

p = subprocess.Popen(["kiro-cli", "acp"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps(
        {"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def pump(until, to=60, tag=""):
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
        OUT.write(json.dumps(o) + "\n")
        OUT.flush()
        m, rid = o.get("method"), o.get("id")
        if rid is not None and m:  # server->client request: answer, never drop
            p.stdin.write(json.dumps(
                {"jsonrpc": "2.0", "id": rid, "result": {}}) + "\n")
            p.stdin.flush()
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


def switch_model(value, tag):
    rid = req("_kiro.dev/commands/execute",
              {"sessionId": sid,
               "command": {"command": "model", "args": {"value": value}}})
    r = pump(rid, 45, tag=tag)
    res = (r or {}).get("result") or {}
    ok = res.get("success") is True  # kiro rejects via success:false, NOT a JSON-RPC error
    print(f"  switch[{tag}] value={value!r} -> "
          f"{'OK ' + json.dumps(r['result'])[:160] if ok else 'ERR ' + json.dumps((r or {}).get('error'))[:160]}")
    return ok, r


req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump(1, 20, tag="init")
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 40, tag="session_new")
res = (sess or {}).get("result", {})
sid = res.get("sessionId")
print("sessionId:", sid)
print("session/new models field:",
      json.dumps(res.get("models"))[:400] if "models" in res else "ABSENT")
pump(-1, 5, tag="settle")

oid = req("_kiro.dev/commands/options",
          {"command": "model", "sessionId": sid, "partial": ""})
opts = pump(oid, 45, tag="options_before")
raw_opts = (opts or {}).get("result") or {}
olist = raw_opts.get("options") if isinstance(raw_opts, dict) else raw_opts
olist = olist or []
values = [o.get("value") for o in olist if isinstance(o, dict)]
print(f"advertised /model options ({len(values)}): {values}")
current = [o.get("value") for o in olist
           if isinstance(o, dict) and o.get("current")]
print("current-flagged:", current or "none (2.14.2 quirk: no current bit)")

FABLE_IN_LIST = any("fable" in (v or "") for v in values)
print("fable in advertised list:", FABLE_IN_LIST)

# CONTROL: a listed model that isn't the current one.
cur_id = res_cur if (res_cur := (res.get("models") or {}).get("currentModelId")) else None
control = next((v for v in values
                if v and v != cur_id and v not in current
                and "fable" not in v and v != "auto"), None)
if control:
    switch_model(control, "control")
else:
    print("  no control candidate found in options list")

# EXPERIMENT: fable ids, most-likely first (KiroCrew acp mapping, canonical
# key, bedrock-style id).
accepted = None
for cand in ("claude-fable-5", "fable-5-1m", "claude-fable-5[1m]", "fable",
             "global.anthropic.claude-fable-5[1m]"):
    ok, _ = switch_model(cand, f"experiment:{cand}")
    if ok:
        accepted = cand
        break

# VERIFY with a real turn regardless of accept/reject (a lying OK response
# that silently keeps the old model must be caught — sidecar is the oracle).
tid = req("session/prompt",
          {"sessionId": sid,
           "prompt": [{"type": "text", "text": "Reply with exactly: OK"}]})
tr = pump(tid, 180, tag="turn")
print("turn stopReason:",
      ((tr or {}).get("result") or {}).get("stopReason"),
      "error:", bool((tr or {}).get("error")))
pump(-1, 8, tag="turn_post")  # metadata et al.

for sc in glob.glob(os.path.join(env["HOME"], ".kiro", "sessions", "cli", "*.json")):
    try:
        with open(sc) as fh:
            side = json.load(fh)
    except Exception:
        continue
    OUT.write(json.dumps({"probe": "sidecar", "path": sc, "data": side}) + "\n")
    blob = json.dumps(side)
    models_seen = sorted({m for m in
                          ("fable", control or "\x00")
                          if m and m in blob})
    print(f"sidecar {os.path.basename(sc)}: model-ish hits={models_seen}")

req("session/cancel", {"sessionId": sid})
p.stdin.close()
time.sleep(2)
p.terminate()
print("\nSUMMARY: advertised_has_fable=%s accepted_id=%s (sidecar is the oracle — read the jsonl)"
      % (FABLE_IN_LIST, accepted))
