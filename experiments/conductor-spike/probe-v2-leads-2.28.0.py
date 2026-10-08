#!/usr/bin/env python3
"""v2 (Rust engine) targeted leads for the 2.28.0 audit (TUI-lane T-L2 model fallback, T-L3 trust shadow).

    probe-v2-leads-2.28.0.py <archive-bin-dir> <out.jsonl> [mode]

modes: main (default: T-L3/H-L4 trust shadow + T-L2/H-L2 model fallback),
       agentnf (H-L3: `acp --agent nonexistent` + session-level agent switch -> `_kiro.dev/agent/not_found`),
       dropend (H-L5: KIRO_TEST_DROP_FIRST_END_TURN=1 + one prompt),
       sandbox (H-L6: .kiro/sandbox.json {"enabled":true} in workspace AND fake HOME, initialize + session/new only;
                stderr kept beside the capture).

Same plumbing as probe-v2-live-sweep-2.28.0.py (archive bin/ PREPENDED to PATH,
/proc child-exe proof, HOME=tmp, real XDG_DATA_HOME). Workload:

  T-L3  env KIRO_TRUST_CLASSIFIER_SHADOW=1 + KIRO_TRUST_CLASSIFIER_SHADOW_LOG=<file>;
        shell prompt #1 answered with result `_meta:{provenance:"human"}`, shell prompt #2
        answered without `_meta`; the shadow log is copied into the summary.
  T-L2  _kiro.dev/commands/options model (fallback block) ->
        commands/execute fallback {targetModelId} / {targetModelId, fallbackModelId} ->
        options again (configured?) -> fallback configured for a BOGUS target ->
        session/set_model bogus -> prompt: does the turn now fall back (`model_fallback`)?
"""
import hashlib, json, os, queue, subprocess, sys, tempfile, threading, time

BIN_DIR = os.path.abspath(sys.argv[1])
OUT_PATH = sys.argv[2]
MODE = sys.argv[3] if len(sys.argv) > 3 else "main"
OUT = open(OUT_PATH, "w", buffering=1)
REAL_HOME = os.path.expanduser("~")
SCRATCH = os.environ.get("PROBE_SCRATCH") or tempfile.gettempdir()
PROBE_HOME = tempfile.mkdtemp(prefix="v2leads-home-", dir=SCRATCH)
CWD = tempfile.mkdtemp(prefix="v2leads-cwd-", dir=SCRATCH)
RUNTIME = tempfile.mkdtemp(prefix="v2leads-rt-", dir=SCRATCH)
SHADOW_LOG = os.path.join(SCRATCH, f"trust-shadow-{os.path.basename(OUT_PATH)}.log")
subprocess.run(["git", "init", "-q", "-b", "main"], cwd=CWD)
if MODE == "sandbox":
    for root in (CWD, PROBE_HOME):
        os.makedirs(os.path.join(root, ".kiro"), exist_ok=True)
        with open(os.path.join(root, ".kiro", "sandbox.json"), "w") as fh:
            json.dump({"enabled": True}, fh)

env = dict(os.environ)
env.update({"PATH": BIN_DIR + os.pathsep + env.get("PATH", ""), "HOME": PROBE_HOME,
            "XDG_DATA_HOME": os.path.join(REAL_HOME, ".local", "share"), "XDG_RUNTIME_DIR": RUNTIME,
            "KIRO_TRUST_CLASSIFIER_SHADOW": "1", "KIRO_TRUST_CLASSIFIER_SHADOW_LOG": SHADOW_LOG})
if MODE != "main":
    env.pop("KIRO_TRUST_CLASSIFIER_SHADOW"); env.pop("KIRO_TRUST_CLASSIFIER_SHADOW_LOG")
if MODE == "dropend":
    env["KIRO_TEST_DROP_FIRST_END_TURN"] = "1"
ACP_ARGS = ["acp", "--agent", "nonexistent-agent-zz9"] if MODE == "agentnf" else ["acp"]

REDACT = ("accesstoken", "refreshtoken", "idtoken", "profilearn", "authorization")
def scrub(o, k=""):
    if k.lower() in REDACT:
        return "<REDACTED>"
    if isinstance(o, dict):
        return {kk: scrub(v, kk) for kk, v in o.items()}
    if isinstance(o, list):
        return [scrub(v, k) for v in o]
    return o

TAG = ["boot"]
def record(direction, msg):
    OUT.write(json.dumps({"ts": round(time.time(), 3), "dir": direction, "tag": TAG[0], "msg": scrub(msg)}) + "\n")

def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()

def proc_tree(root):
    out, todo = [], [root]
    while todo:
        pid = todo.pop()
        try:
            exe = os.readlink(f"/proc/{pid}/exe")
        except OSError:
            continue
        out.append({"pid": pid, "exe": exe, "sha256": sha256(exe)})
        todo.extend(int(k) for k in subprocess.run(["ps", "-o", "pid=", "--ppid", str(pid)],
                                                   capture_output=True, text=True).stdout.split())
    return out

p = subprocess.Popen([os.path.join(BIN_DIR, "kiro-cli")] + ACP_ARGS, cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=open(OUT_PATH.replace(".jsonl", "-stderr.log"), "w"),
                     text=True, bufsize=1, start_new_session=True)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()], daemon=True).start()
nid = [0]
PERM_MODE = ["human"]
PERMS, NOTES = [], []

def send(o):
    record("client->agent", o)
    p.stdin.write(json.dumps(o) + "\n"); p.stdin.flush()

def req(m, pr):
    nid[0] += 1
    send({"jsonrpc": "2.0", "id": nid[0], "method": m, "params": pr})
    return nid[0]

def pump(until, to=180):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = q.get(timeout=1)
        except queue.Empty:
            if p.poll() is not None:
                return None
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        record("agent->client", o)
        m, rid = o.get("method"), o.get("id")
        if m:
            NOTES.append((TAG[0], m, o.get("params") or {}))
            if rid is not None:
                if "request_permission" in m:
                    opts = (o.get("params") or {}).get("options") or []
                    pick = next((x for x in opts if x.get("kind") == "allow_once"), opts[0] if opts else None)
                    res = {"outcome": {"outcome": "selected", "optionId": pick.get("optionId")}}
                    if PERM_MODE[0] == "human":
                        res["_meta"] = {"provenance": "human"}
                    PERMS.append({"tag": TAG[0], "mode": PERM_MODE[0], "picked": pick.get("optionId"),
                                  "request_meta": (o.get("params") or {}).get("_meta")})
                    send({"jsonrpc": "2.0", "id": rid, "result": res})
                else:
                    send({"jsonrpc": "2.0", "id": rid, "result": {}})
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None

def call(tag, m, pr, to=60):
    TAG[0] = tag
    r = pump(req(m, pr), to)
    if r is None:
        return "TIMEOUT"
    return r.get("result", {"error": r.get("error")})

def prompt(tag, text, to=300):
    TAG[0] = tag
    t0 = time.time()
    r = pump(req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": text}]}), to)
    pump(-1, 5)
    return {"stopReason": ((r or {}).get("result") or {}).get("stopReason"), "error": (r or {}).get("error"),
            "seconds": round(time.time() - t0, 1)}

time.sleep(1.5)
TREE = proc_tree(p.pid)
OUT.write(json.dumps({"probe": "proc_tree", "tree": TREE}) + "\n")
S = {"probe": "lead-result", "mode": MODE, "bin_dir": BIN_DIR, "argv": ACP_ARGS,
     "exes": [(t["exe"], t["sha256"][:16]) for t in TREE]}
call("init", "initialize", {"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True},
                            "clientInfo": {"name": "cyril-probe", "version": "audit-2.28.0"}}, 60)
new = call("session_new", "session/new", {"cwd": CWD, "mcpServers": []}, 90)
sid = new.get("sessionId") if isinstance(new, dict) else None
S["session_new"] = new if not isinstance(new, dict) else {k: (v if k != "models" else "<models>") for k, v in new.items()}

def finish():
    S["notes"] = [(t, m, (pr.get("update") or {}).get("sessionUpdate") if isinstance(pr, dict) and isinstance(pr.get("update"), dict) else None)
                  for t, m, pr in NOTES]
    S["agent_not_found"] = [pr for t, m, pr in NOTES if "not_found" in m]
    try:
        S["stderr_tail"] = open(OUT_PATH.replace(".jsonl", "-stderr.log"), errors="replace").read()[-3000:]
    except OSError:
        pass
    OUT.write(json.dumps(scrub(S)) + "\n")
    print(json.dumps(scrub(S), indent=1)[:15000])
    try:
        p.stdin.close(); os.killpg(os.getpgid(p.pid), 15); p.wait(timeout=10)
    except Exception:
        try: os.killpg(os.getpgid(p.pid), 9)
        except Exception: pass
    sys.exit(0)

if MODE == "sandbox":
    pump(-1, 8); finish()
if not sid:
    S["abort"] = "no session"; finish()
if MODE == "agentnf":
    pump(-1, 6)
    S["exec_agent_switch"] = call("agent_switch", "_kiro.dev/commands/execute",
                                  {"sessionId": sid, "command": {"command": "agent", "args": {"value": "another-missing-agent"}}})
    pump(-1, 6)
    S["turn"] = prompt("agentnf_turn", "Reply with exactly the word DELTA.")
    finish()
if MODE == "dropend":
    pump(-1, 4)
    S["turn1"] = prompt("dropend_t1", "Reply with exactly the word ECHO.", 240)
    S["turn2"] = prompt("dropend_t2", "Reply with exactly the word FOXTROT.", 240)
    finish()
cur = (new.get("models") or {}).get("currentModelId")
pump(-1, 6)

# ---- T-L3 trust classifier shadow ----------------------------------------------
PERM_MODE[0] = "human"
S["shell1_human"] = prompt("tl3_human", "Use your shell tool to run exactly: echo SHADOW1 > s1.txt && cat s1.txt  -- then reply with only the output.")
PERM_MODE[0] = "none"
S["shell2_none"] = prompt("tl3_none", "Use your shell tool to run exactly: echo SHADOW2 > s2.txt && cat s2.txt  -- then reply with only the output.")
S["permissions"] = PERMS
time.sleep(2)
try:
    S["shadow_log"] = open(SHADOW_LOG, encoding="utf-8", errors="replace").read()[-6000:]
except OSError as e:
    S["shadow_log"] = f"(absent: {e.strerror})"

# ---- T-L2 model fallback --------------------------------------------------------
def fb_rows(res):
    if not isinstance(res, dict):
        return res
    return {o.get("value"): o.get("fallback") for o in res.get("options") or []}
S["opts_before"] = fb_rows(call("opts1", "_kiro.dev/commands/options", {"command": "model", "sessionId": sid, "partial": ""}))
S["exec_fallback_target_only"] = call("fb1", "_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": "fallback", "args": {"targetModelId": "claude-sonnet-5"}}})
S["exec_fallback_pair"] = call("fb2", "_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": "fallback", "args": {"targetModelId": "claude-sonnet-5", "fallbackModelId": "claude-haiku-4.5"}}})
S["opts_after"] = fb_rows(call("opts2", "_kiro.dev/commands/options", {"command": "model", "sessionId": sid, "partial": ""}))
S["exec_fallback_bogus_target"] = call("fb3", "_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": "fallback", "args": {"targetModelId": "bogus-model-zz9", "fallbackModelId": "claude-haiku-4.5"}}})
S["exec_fallback_noargs"] = call("fb4", "_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": "fallback", "args": {}}})
S["set_model_bogus"] = call("bogus", "session/set_model", {"sessionId": sid, "modelId": "bogus-model-zz9"})
n0 = len(NOTES)
S["turn_bogus"] = prompt("bogus_turn", "Reply with exactly the word CHARLIE.")
S["notes_during_bogus"] = [(t, m, (pr.get("update") or {}).get("sessionUpdate") if isinstance(pr, dict) else None) for t, m, pr in NOTES[n0:]]
S["model_fallback_frames"] = [pr for t, m, pr in NOTES if "model_fallback" in json.dumps(pr)]
S["opts_final"] = fb_rows(call("opts3", "_kiro.dev/commands/options", {"command": "model", "sessionId": sid, "partial": ""}))
finish()
