#!/usr/bin/env python3
"""Does the v1/v2 engine implement `session/set_model` on 2.20.1?

Trigger: cyril's CLAUDE.md says `session/set_model` is "behind unstable feature
flag, not advertised in capabilities", so cyril routes every model change
through `_kiro.dev/commands/execute {command:"model"}`. KiroCrew calls
`session/set_model` unconditionally on kiro-cli in production, and their commit
5790395c1 records a raw JSON-RPC probe on 2.15.1: acked in 12.5ms, conversation
carried across the switch INCLUDING ACROSS VENDORS, sticks over later turns,
mid-turn leaves the in-flight turn undisturbed. Two of us cannot both be right.

Second claim under test (same commit family): cyril's CLAUDE.md says
`config_options` is "always null on the v1/v2 engine". KiroCrew parses
configOptions off the session/new response on BOTH backends and derives
/effort levels from it.

Protocol (control-first, per the schema-vs-runtime rule -- schema-accepted is
not functional, so every arm has a control):
  0. auth pre-flight: kiro-cli user whoami --format json
  1. initialize -> record agentCapabilities VERBATIM (is set_model advertised?)
  2. session/new -> record `models` and `configOptions` VERBATIM
  3. CONTROL: _kiro.dev/commands/options {command:"model"} -> advertised list
     (proves this session's model machinery works at all)
  4. ARM A: session/set_model {sessionId, modelId:<served id from step 3>}
     -32601 => cyril's doc is right. result => cyril's doc is stale.
  5. ARM B: session/set_model with a bogus id -> does it validate, or accept
     anything? (KiroCrew: "kiro-cli ACCEPTS the id, only the service rejects
     it mid-prompt" -- if so a bogus id is accepted here and poisons the turn)
  6. ARM C: session/set_config_option {configId:"model"} -> documented as
     "Method not found" on v1/v2; confirm on 2.20.1.
  7. VERIFY (1 tiny paid turn): set_model to a served id != the session default,
     run a 3-token prompt, then read the session sidecar
     ($HOME/.kiro/sessions/cli/<sid>.json) -- the only on-disk source of the
     model actually billed. Wire `_kiro.dev/metadata` frames captured too.

Env: HOME=<tmp> + real XDG_DATA_HOME (house isolation rule; nothing here reads
~/.kiro so no seeding needed). Falls back to real HOME with a loud DEVIATION
line if whoami fails under isolation.
COST: exactly 1 tiny real turn.

    probe-v2-set-model-2.20.1.py <out.jsonl>
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
PROBE_HOME = tempfile.mkdtemp(prefix="setmodel-home-")
CWD = tempfile.mkdtemp(prefix="setmodel-cwd-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

env = dict(os.environ)
env["HOME"] = PROBE_HOME
env.setdefault("XDG_DATA_HOME", os.path.join(REAL_HOME, ".local", "share"))

who = subprocess.run(["kiro-cli", "user", "whoami", "--format", "json"],
                     env=env, capture_output=True, text=True)
if who.returncode != 0:
    print(f"DEVIATION: whoami failed under HOME={PROBE_HOME} "
          f"(rc={who.returncode}); falling back to real HOME")
    env["HOME"] = PROBE_HOME = REAL_HOME
    who = subprocess.run(["kiro-cli", "user", "whoami", "--format", "json"],
                         env=env, capture_output=True, text=True)
    if who.returncode != 0:
        # whoami reports auth state on STDOUT ({"account":null}) with an EMPTY
        # stderr, so print both or the abort reads as a silent failure.
        sys.exit(f"ABORT: not authenticated (rc={who.returncode}) "
                 f"stdout={who.stdout.strip()[:120]!r} "
                 f"stderr={who.stderr.strip()[:120]!r}\n"
                 f"       `kiro-cli acp` refuses to start unauthenticated: it writes\n"
                 f"       'error: You are not logged in, please log in with kiro-cli login'\n"
                 f"       to stderr and emits no ACP frames. Run: kiro-cli login")
ver = subprocess.run(["kiro-cli", "--version"], env=env,
                     capture_output=True, text=True).stdout.strip()
print("binary :", ver)
print("whoami :", who.stdout.strip()[:200])
OUT.write(json.dumps({"probe": "preflight", "version": ver,
                      "whoami": who.stdout.strip()}) + "\n")

p = subprocess.Popen(["kiro-cli", "acp"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
FRAMES = []


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
        FRAMES.append(o)
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


def set_model(model_id, tag, timeout=30):
    """ARM: session/set_model. Returns (verdict, elapsed_ms, frame)."""
    t0 = time.time()
    rid = req("session/set_model", {"sessionId": sid, "modelId": model_id})
    r = pump(rid, timeout, tag=tag)
    ms = (time.time() - t0) * 1000
    if r is None:
        return "NO_RESPONSE", ms, None
    if "error" in r:
        code = (r["error"] or {}).get("code")
        return f"ERROR {code}", ms, r
    return "OK", ms, r


# ---- 1. initialize ---------------------------------------------------------
iid = req("initialize", {"protocolVersion": 1,
                         "clientCapabilities": {},
                         "clientInfo": {"name": "cyril-probe", "version": "0"}})
ini = pump(iid, 25, tag="init")
ires = (ini or {}).get("result") or {}
print("\n=== 1. initialize ===")
print("  protocolVersion echoed:", json.dumps(ires.get("protocolVersion")))
print("  agentCapabilities     :", json.dumps(ires.get("agentCapabilities"))[:600])

# ---- 2. session/new --------------------------------------------------------
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 60, tag="session_new")
sres = (sess or {}).get("result") or {}
sid = sres.get("sessionId")
print("\n=== 2. session/new ===")
print("  sessionId    :", sid)
print("  models       :", json.dumps(sres.get("models"))[:500]
      if "models" in sres else "ABSENT")
print("  configOptions:", json.dumps(sres.get("configOptions"))[:500]
      if "configOptions" in sres else "ABSENT")
print("  modes        :", json.dumps(sres.get("modes"))[:300]
      if "modes" in sres else "ABSENT")
print("  top-level keys:", sorted(sres.keys()))
if not sid:
    sys.exit("ABORT: no sessionId")
pump(-1, 5, tag="settle")

# ---- 3. CONTROL: advertised model list -------------------------------------
oid = req("_kiro.dev/commands/options",
          {"command": "model", "sessionId": sid, "partial": ""})
opts = pump(oid, 45, tag="options")
ores = (opts or {}).get("result") or {}
cand = ores.get("options") or ores.get("Options") or []
served = [o.get("value") for o in cand if isinstance(o, dict) and o.get("value")]
current = [o.get("value") for o in cand
           if isinstance(o, dict) and o.get("current") is True]
print("\n=== 3. CONTROL: commands/options{model} ===")
print(f"  advertised {len(served)}: {served[:14]}")
print("  marked current:", current or "NONE (no `current` field -- cyril-imjx)")
OUT.write(json.dumps({"probe": "advertised", "options": cand}) + "\n")

# ---- 4/5/6. the arms -------------------------------------------------------
print("\n=== 4. ARM A: session/set_model with a SERVED id ===")
target = next((m for m in served if m != "auto"), (served[0] if served else "auto"))
va, msa, fa = set_model(target, "armA_served")
print(f"  modelId={target!r} -> {va} in {msa:.1f}ms")
if fa:
    print("  frame:", json.dumps(fa.get("result", fa.get("error")))[:300])

print("\n=== 5. ARM B: session/set_model with a BOGUS id ===")
vb, msb, fb = set_model("definitely-not-a-model-9x", "armB_bogus")
print(f"  -> {vb} in {msb:.1f}ms")
if fb:
    print("  frame:", json.dumps(fb.get("result", fb.get("error")))[:300])

print("\n=== 6. ARM C: session/set_config_option (documented Method not found) ===")
cid = req("session/set_config_option",
          {"sessionId": sid, "configId": "model", "value": target})
cr = pump(cid, 25, tag="armC_setconfig")
print("  ->", json.dumps((cr or {}).get("error") or (cr or {}).get("result"))[:300]
      if cr else "NO_RESPONSE")

OUT.write(json.dumps({"probe": "arms",
                      "A_served": {"target": target, "verdict": va, "ms": msa},
                      "B_bogus": {"verdict": vb, "ms": msb}}) + "\n")

# ---- 7. VERIFY: does it actually take effect? ------------------------------
print("\n=== 7. VERIFY (1 paid turn) ===")
if va == "OK":
    # re-assert the served target (arm B may have moved it), then one tiny turn
    set_model(target, "armA_reassert")
    print(f"  re-asserted {target!r}; sending a 3-token prompt...")
    pid = req("session/prompt", {"sessionId": sid,
                                 "prompt": [{"type": "text",
                                             "text": "Reply with exactly: OK"}]})
    pr = pump(pid, 180, tag="turn")
    print("  stopReason:", json.dumps((pr or {}).get("result"))[:200]
          if pr else "NO RESPONSE (turn never ended)")
    meta = [f for f in FRAMES if (f.get("method") or "").endswith("metadata")]
    print(f"  _kiro.dev/metadata frames: {len(meta)}")
    time.sleep(2)
    side = os.path.join(PROBE_HOME, ".kiro", "sessions", "cli", f"{sid}.json")
    hits = glob.glob(side) or glob.glob(
        os.path.join(PROBE_HOME, ".kiro", "sessions", "cli", "*.json"))
    print("  sidecar:", hits[0] if hits else "NOT FOUND")
    if hits:
        try:
            blob = json.load(open(hits[0]))
            txt = json.dumps(blob)
            found = sorted({m for m in served if m and m in txt})
            print("  model ids present in sidecar:", found)
            OUT.write(json.dumps({"probe": "sidecar", "models_seen": found}) + "\n")
        except Exception as e:
            print("  sidecar unreadable:", e)
else:
    print("  SKIPPED: ARM A did not succeed, so there is nothing to verify.")

# ---- verdict ---------------------------------------------------------------
print("\n" + "=" * 66)
print("VERDICT")
print("=" * 66)
if va == "ERROR -32601":
    print("  session/set_model: NOT IMPLEMENTED on v2 @", ver)
    print("  => cyril's CLAUDE.md is CORRECT as written.")
elif va == "OK":
    print(f"  session/set_model: IMPLEMENTED and accepted a served id ({msa:.0f}ms) on v2 @", ver)
    print("  => cyril's CLAUDE.md is STALE. KiroCrew's 2.15.1 finding holds at 2.20.1.")
    print(f"  bogus-id handling: {vb} "
          f"({'validates' if vb.startswith('ERROR') else 'ACCEPTS ANYTHING -- rejection lands mid-prompt'})")
else:
    print(f"  session/set_model: INCONCLUSIVE ({va})")
print("  configOptions on session/new:",
      "PRESENT" if sres.get("configOptions") else
      ("null" if "configOptions" in sres else "ABSENT"))
OUT.close()
try:
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 999,
                              "method": "_kiro.dev/session/terminate",
                              "params": {"sessionId": sid}}) + "\n")
    p.stdin.flush()
    time.sleep(1)
except Exception:
    pass
p.terminate()
