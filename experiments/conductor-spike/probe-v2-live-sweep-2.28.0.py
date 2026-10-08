#!/usr/bin/env python3
"""v2 (Rust engine) live discovery sweep for the 2.28.0 audit.

Same-day paired capture of an archived kiro-cli (2.26.0 baseline vs 2.28.0)
over an IDENTICAL workload, recorded as {ts, dir, msg} JSONL so
sweep-new-fields.py can diff the field-path sets.

    probe-v2-live-sweep-2.28.0.py <archive-bin-dir> <out.jsonl>

PATH trap (2.24.0 audit): the launcher resolves `kiro-cli-chat` via PATH, so
the archive bin/ is PREPENDED to PATH and every descendant's /proc/<pid>/exe
is recorded (`probe: proc_tree`). An "identical" pairing is a claim about
which binaries ran -- the capture carries the proof.

Workload:
  initialize -> session/new -> settle (ext notifications)
  prompt 1: file read (PROBE.txt)            -> tool_call, no permission
  prompt 2: shell command                    -> session/request_permission (allow once)
  _kiro.dev/commands/options  model | agent | context
  _kiro.dev/commands/execute  context | tools | usage
  session/list, session/close (if advertised), session/set_config_option
  session/set_model <served id> (control) then <bogus id> + prompt 3
  (2.27.1 "Model fallback" -- does a refused turn now retry another model?)

HOME is a fresh tmp dir; XDG_DATA_HOME stays real (auth store). v2 renews its
own token; this script never runs login/logout/whoami.
"""
import hashlib, json, os, queue, subprocess, sys, tempfile, threading, time

BIN_DIR = os.path.abspath(sys.argv[1])
OUT_PATH = sys.argv[2]
OUT = open(OUT_PATH, "w", buffering=1)
REAL_HOME = os.path.expanduser("~")
SCRATCH = os.environ.get("PROBE_SCRATCH") or tempfile.gettempdir()
PROBE_HOME = tempfile.mkdtemp(prefix="v2sweep-home-", dir=SCRATCH)
CWD = tempfile.mkdtemp(prefix="v2sweep-cwd-", dir=SCRATCH)
RUNTIME = tempfile.mkdtemp(prefix="v2sweep-rt-", dir=SCRATCH)
subprocess.run(["git", "init", "-q", "-b", "main"], cwd=CWD)
with open(os.path.join(CWD, "PROBE.txt"), "w") as fh:
    fh.write("ALPHA\n")

env = dict(os.environ)
env["PATH"] = BIN_DIR + os.pathsep + env.get("PATH", "")
env["HOME"] = PROBE_HOME
env["XDG_DATA_HOME"] = os.path.join(REAL_HOME, ".local", "share")
env["XDG_RUNTIME_DIR"] = RUNTIME

REDACT = ("accesstoken", "refreshtoken", "idtoken", "profilearn", "authorization")
def scrub(o, k=""):
    if k.lower() in REDACT:
        return "<REDACTED>"
    if isinstance(o, dict):
        return {kk: scrub(v, kk) for kk, v in o.items()}
    if isinstance(o, list):
        return [scrub(v, k) for v in o]
    return o

def record(direction, msg, tag=""):
    OUT.write(json.dumps({"ts": round(time.time(), 3), "dir": direction, "tag": tag,
                          "msg": scrub(msg)}) + "\n")

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
            argv = open(f"/proc/{pid}/cmdline", "rb").read().split(b"\0")
            argv = [a.decode(errors="replace") for a in argv if a]
        except OSError:
            continue
        out.append({"pid": pid, "exe": exe, "argv": argv[:6],
                    "sha256": sha256(exe) if os.path.exists(exe) else None})
        kids = subprocess.run(["ps", "-o", "pid=", "--ppid", str(pid)],
                              capture_output=True, text=True).stdout.split()
        todo.extend(int(k) for k in kids)
    return out

ver = subprocess.run([os.path.join(BIN_DIR, "kiro-cli"), "--version"], env=env,
                     capture_output=True, text=True).stdout.strip()
OUT.write(json.dumps({"probe": "preflight", "bin_dir": BIN_DIR, "version": ver,
                      "archive_chat_sha256": sha256(os.path.join(BIN_DIR, "kiro-cli-chat"))}) + "\n")
print("launcher:", ver)

p = subprocess.Popen([os.path.join(BIN_DIR, "kiro-cli"), "acp"], cwd=CWD, env=env,
                     stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=open(OUT_PATH.replace(".jsonl", "-stderr.log"), "w"),
                     text=True, bufsize=1, start_new_session=True)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
nid = [0]
METHODS, PERMS, ERRORS = {}, [], {}

def send(o, tag=""):
    record("client->agent", o, tag)
    p.stdin.write(json.dumps(o) + "\n"); p.stdin.flush()

def req(m, pr, tag=""):
    nid[0] += 1
    send({"jsonrpc": "2.0", "id": nid[0], "method": m, "params": pr}, tag)
    return nid[0]

def answer_permission(rid, params, tag):
    opts = (params or {}).get("options") or []
    pick = next((o for o in opts if (o.get("kind") or "") == "allow_once"), None) \
        or next((o for o in opts if "allow" in (o.get("kind") or "")), None) \
        or (opts[0] if opts else None)
    PERMS.append({"tag": tag, "options": [(o.get("optionId"), o.get("kind")) for o in opts],
                  "picked": pick and pick.get("optionId")})
    send({"jsonrpc": "2.0", "id": rid,
          "result": {"outcome": {"outcome": "selected", "optionId": pick.get("optionId")}}
          if pick else {"outcome": {"outcome": "cancelled"}}}, tag)

def pump(until, to=180, tag=""):
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
            record("agent->client(raw)", {"raw": raw[:2000]}, tag)
            continue
        record("agent->client", o, tag)
        m, rid = o.get("method"), o.get("id")
        if m:
            key = m
            if m == "session/update":
                key += ":" + str(((o.get("params") or {}).get("update") or {}).get("sessionUpdate"))
            METHODS[key] = METHODS.get(key, 0) + 1
            if rid is not None:
                if "request_permission" in m:
                    answer_permission(rid, o.get("params"), tag)
                else:
                    send({"jsonrpc": "2.0", "id": rid, "result": {}}, tag)
            continue
        if rid == until and ("result" in o or "error" in o):
            if "error" in o:
                ERRORS[tag] = o["error"]
            return o
    return None

time.sleep(1.5)
TREE = proc_tree(p.pid)
OUT.write(json.dumps({"probe": "proc_tree", "tree": TREE}) + "\n")
for t in TREE:
    print("  proc:", t["exe"], t["sha256"][:12] if t["sha256"] else None)

init = pump(req("initialize", {"protocolVersion": 1,
                               "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True},
                                                      "terminal": True},
                               "clientInfo": {"name": "cyril-probe", "version": "audit-2.28.0"}},
                "init"), 60, "init")
new = pump(req("session/new", {"cwd": CWD, "mcpServers": []}, "session_new"), 90, "session_new")
res = (new or {}).get("result") or {}
sid = res.get("sessionId")
if not sid:
    sys.exit("ABORT: no sessionId")
models = ((res.get("models") or {}).get("availableModels") or [])
cur_model = (res.get("models") or {}).get("currentModelId")
pump(-1, 8, "settle")

def prompt(text, tag, to=300):
    t0 = time.time()
    r = pump(req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": text}]}, tag), to, tag)
    pump(-1, 5, tag + "_settle")
    return {"stopReason": ((r or {}).get("result") or {}).get("stopReason"),
            "error": (r or {}).get("error"), "seconds": round(time.time() - t0, 1)}

TURNS = {}
TURNS["p1_read"] = prompt("Read the file PROBE.txt in the current directory and reply with only the single word it contains. Do not explain.", "p1_read")
TURNS["p2_shell"] = prompt("Use your shell command tool to run exactly: echo BRAVO > shell.txt && cat shell.txt   -- then reply with only the command's output.", "p2_shell")

EXTRA = {}
for cmd in ("model", "agent", "context"):
    r = pump(req("_kiro.dev/commands/options", {"command": cmd, "sessionId": sid, "partial": ""}, f"opts_{cmd}"), 30, f"opts_{cmd}")
    EXTRA[f"options:{cmd}"] = "error" if (r or {}).get("error") else ("ok" if r else "timeout")
for cmd in ("context", "tools", "usage"):
    r = pump(req("_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": cmd, "args": {}}}, f"exec_{cmd}"), 30, f"exec_{cmd}")
    EXTRA[f"execute:{cmd}"] = "error" if (r or {}).get("error") else ("ok" if r else "timeout")
for m, pr in (("session/list", {}),
              ("session/set_config_option", {"sessionId": sid, "configId": "mode", "value": "x"})):
    r = pump(req(m, pr, m), 20, m)
    EXTRA[m] = (r or {}).get("error") or ("ok" if r else "timeout")

caps = (((init or {}).get("result") or {}).get("agentCapabilities") or {})
if "close" in (caps.get("sessionCapabilities") or {}):
    EXTRA["close_advertised"] = True

# --- model fallback (2.27.1) -------------------------------------------------
alt = next((m.get("modelId") for m in models if m.get("modelId") != cur_model), None)
if alt:
    r = pump(req("session/set_model", {"sessionId": sid, "modelId": alt}, "set_model_valid"), 20, "set_model_valid")
    EXTRA["set_model_valid"] = {"modelId": alt, "resp": (r or {}).get("result", (r or {}).get("error"))}
    r = pump(req("session/set_model", {"sessionId": sid, "modelId": cur_model}, "set_model_restore"), 20, "set_model_restore")
r = pump(req("session/set_model", {"sessionId": sid, "modelId": "bogus-model-zz9"}, "set_model_bogus"), 20, "set_model_bogus")
EXTRA["set_model_bogus"] = (r or {}).get("result", (r or {}).get("error"))
TURNS["p3_after_bogus_model"] = prompt("Reply with exactly the word CHARLIE.", "p3_bogus", 240)

# session/close last (if the method exists it ends the session)
r = pump(req("session/close", {"sessionId": sid}, "session_close"), 20, "session_close")
EXTRA["session/close"] = (r or {}).get("result", (r or {}).get("error"))

summary = {"probe": "result", "bin_dir": BIN_DIR, "version": ver,
           "chat_exes": sorted({t["exe"] for t in TREE}),
           "turns": TURNS, "extra": EXTRA, "permissions": PERMS, "errors": ERRORS,
           "methods": dict(sorted(METHODS.items())),
           "models_count": len(models), "current_model": cur_model}
OUT.write(json.dumps(scrub(summary)) + "\n")
print(json.dumps(scrub(summary), indent=1)[:5000])
try:
    p.stdin.close()
    os.killpg(os.getpgid(p.pid), 15); p.wait(timeout=10)
except Exception:
    try: os.killpg(os.getpgid(p.pid), 9)
    except Exception: pass
