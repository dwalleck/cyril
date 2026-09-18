#!/usr/bin/env python3
"""Does KIRO_ROLLOUT_FORCE_INTERNAL (and _NIGHTLY) actually flip the compiled-in
Rust host rollout registry? (kiro-cli 2.22.0)

Static: both names sit in kiro-cli-chat's `chat_cli::rollout` module (the
`kiro-cli` launcher has zero hits). Rollout::enabled_features is exported to
the child as `KIRO_ENABLED_FEATURES` (a JSON array the embedded TUI parses) and
`KIRO_INTERNAL=1` (`process.env.KIRO_INTERNAL==="1"` in tui.js); per-feature
exports named in the registry descriptions: KIRO_INFRA_SAFETY_ROLLOUT_ENABLED,
KIRO_C2S_ROLLOUT_ENABLED. So the observable is the CHILD ENVIRONMENT.

    MODE=acp-kas|acp-v2|tui LABEL=<tag> [FORCE_INTERNAL=1] [FORCE_NIGHTLY=1] \
        probe-rollout-force-internal-2.22.0.py <out.json>

acp-* legs do initialize + session/new (no prompt), then walk the process tree
under the spawned host and dump every KIRO_*/CLOUD_CONFIG*/AB_* variable of
each process, plus the handshake frames (available commands, _kiro/* frames).
tui spawns `kiro-cli chat` on a pty, waits, dumps the tree, kills it.
Auth values are redacted. COST: zero real turns.
"""
import json, os, pty, queue, re, signal, sqlite3, subprocess, sys, tempfile, threading, time

MODE = os.environ.get("MODE", "acp-kas")
LABEL = os.environ.get("LABEL", MODE)
OUTP = sys.argv[1]
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.path.expanduser("~/.local/share")
AUTH_DB = os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3")
FAKE_HOME = tempfile.mkdtemp(prefix=f"rf-{LABEL}-home-")
CWD = tempfile.mkdtemp(prefix=f"rf-{LABEL}-cwd-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

def profile_arn():
    e = os.environ.get("KIRO_PROFILE_ARN")
    if e: return e
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True, timeout=20).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

def read_token():
    try:
        c = sqlite3.connect(AUTH_DB)
        try: row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        finally: c.close()
        if not row: return None
        v = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
        d = json.loads(v)
        return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": PROFILE_ARN}
    except Exception as e:
        print("auth unavailable:", type(e).__name__); return None

env = dict(os.environ)
for k in list(env):
    if k.startswith("KIRO_ROLLOUT"): del env[k]
env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME, "TERM": "xterm-256color"})
if os.environ.get("FORCE_INTERNAL"): env["KIRO_ROLLOUT_FORCE_INTERNAL"] = os.environ["FORCE_INTERNAL"]
if os.environ.get("FORCE_NIGHTLY"): env["KIRO_ROLLOUT_FORCE_NIGHTLY"] = os.environ["FORCE_NIGHTLY"]
INJECTED = {k: v for k, v in env.items() if k.startswith("KIRO_ROLLOUT")}

REDACT = ("accessToken", "refreshToken", "idToken", "profileArn", "expiresAt")
def redact(o):
    if isinstance(o, dict): return {k: ("<REDACTED>" if k in REDACT else redact(v)) for k, v in o.items()}
    if isinstance(o, list): return [redact(v) for v in o]
    return o

def descendants(root):
    kids = {}
    for p in os.listdir("/proc"):
        if not p.isdigit(): continue
        try:
            with open(f"/proc/{p}/status") as f:
                pp = next((l.split()[1] for l in f if l.startswith("PPid:")), None)
        except OSError: continue
        if pp: kids.setdefault(int(pp), []).append(int(p))
    out, stack = [], [root]
    while stack:
        n = stack.pop(); out.append(n); stack.extend(kids.get(n, []))
    return out

KEYS = re.compile(r"^(KIRO_|CLOUD_CONFIG|AB_|KAS_|Q_CLI)")
SENSITIVE = re.compile(r"TOKEN|SECRET|KEY$|PASSWORD|ARN", re.I)
def tree_env(root):
    rows = []
    for pid in descendants(root):
        try:
            cmd = open(f"/proc/{pid}/cmdline", "rb").read().replace(b"\0", b" ").decode(errors="replace").strip()
            raw = open(f"/proc/{pid}/environ", "rb").read().split(b"\0")
        except OSError: continue
        ev = {}
        for kv in raw:
            if b"=" not in kv: continue
            k, v = kv.split(b"=", 1); k = k.decode(errors="replace"); v = v.decode(errors="replace")
            if KEYS.match(k): ev[k] = "<REDACTED>" if SENSITIVE.search(k) else v[:400]
        rows.append({"pid": pid, "cmd": cmd[:160], "env": dict(sorted(ev.items()))})
    return rows

PROFILE_ARN = profile_arn()
result = {"probe": "rollout-force-internal", "label": LABEL, "mode": MODE, "injected": INJECTED,
          "fake_home": FAKE_HOME, "frames": [], "tree": [], "commands": None, "notes": []}

if MODE.startswith("acp"):
    argv = [KIRO, "acp"] + (["--agent-engine", "kas"] if MODE == "acp-kas" else [])
    proc = subprocess.Popen(argv, cwd=CWD, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=open(OUTP.replace(".json", "-stderr.log"), "w"), text=True, bufsize=1,
                            start_new_session=True)
    msgs = queue.Queue()
    threading.Thread(target=lambda: [msgs.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
    i = [0]
    def send(o): proc.stdin.write(json.dumps(o) + "\n"); proc.stdin.flush()
    def req(m, pr):
        i[0] += 1; send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}); return i[0]
    def cb(method, params):
        if method == "_kiro/auth/getAccessToken": return read_token() or {}
        if method == "_kiro/terminal/shell_type": return {"shellType": "bash"}
        return {}
    def pump(until, to):
        end = time.time() + to
        while time.time() < end:
            try: raw = msgs.get(timeout=1)
            except queue.Empty:
                if proc.poll() is not None: return None
                continue
            try: o = json.loads(raw)
            except Exception: continue
            result["frames"].append(redact(o))
            m, rid = o.get("method"), o.get("id")
            if rid is not None and m:
                send({"jsonrpc": "2.0", "id": rid, "result": cb(m, o.get("params") or {})}); continue
            if rid == until and ("result" in o or "error" in o): return o
        return None
    init = pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {"terminal": True, "fs": {"readTextFile": True, "writeTextFile": True}}, "clientInfo": {"name": "cyril-probe", "version": "0"}}), 90)
    new = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 150)
    pump(-1, 4)
    result["tree"] = tree_env(proc.pid)
    cmds = set()
    for f in result["frames"]:
        if f.get("method") == "_kiro.dev/commands/available":
            cmds.update(c.get("name") for c in (f.get("params") or {}).get("commands", []) if isinstance(c, dict))
        if f.get("method") == "session/update":
            u = (f.get("params") or {}).get("update") or {}
            if u.get("sessionUpdate") == "available_commands_update":
                cmds.update(c.get("name") for c in u.get("availableCommands", []) if isinstance(c, dict))
    result["commands"] = sorted(c for c in cmds if c)
    result["initialize_result"] = redact((init or {}).get("result")); result["session_new_result"] = redact((new or {}).get("result"))
    try: os.killpg(os.getpgid(proc.pid), signal.SIGTERM); proc.wait(timeout=10)
    except Exception:
        try: os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
        except Exception: pass
else:
    master, slave = pty.openpty()
    proc = subprocess.Popen([KIRO, "chat"], cwd=CWD, env=env, stdin=slave, stdout=slave, stderr=slave,
                            start_new_session=True, close_fds=True)
    os.close(slave)
    buf = []
    def drain():
        while True:
            try: b = os.read(master, 4096)
            except OSError: return
            if not b: return
            buf.append(b)
    threading.Thread(target=drain, daemon=True).start()
    time.sleep(float(os.environ.get("TUI_WAIT", "14")))
    result["tree"] = tree_env(proc.pid)
    screen = re.sub(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07]*\x07|\x1b[=>]", "", b"".join(buf).decode(errors="replace"))
    result["tui_screen_tail"] = screen[-1500:]
    try: os.killpg(os.getpgid(proc.pid), signal.SIGTERM); proc.wait(timeout=10)
    except Exception:
        try: os.killpg(os.getpgid(proc.pid), signal.SIGKILL)
        except Exception: pass

json.dump(result, open(OUTP, "w"), indent=1)
print(f"== {LABEL} mode={MODE} injected={INJECTED} procs={len(result['tree'])} frames={len(result['frames'])} commands={len(result['commands'] or [])}")
for row in result["tree"]:
    interesting = {k: v for k, v in row["env"].items() if k in ("KIRO_INTERNAL", "KIRO_ENABLED_FEATURES", "KIRO_INFRA_SAFETY_ROLLOUT_ENABLED", "KIRO_C2S_ROLLOUT_ENABLED", "KIRO_AGENT_ENGINE", "CLOUD_CONFIG_ENDPOINT", "KIRO_ROLLOUT_FORCE_INTERNAL", "KIRO_ROLLOUT_FORCE_NIGHTLY") or k.endswith("_ROLLOUT_ENABLED")}
    print(f"   pid={row['pid']} {row['cmd'][:70]!r}\n      {json.dumps(interesting)}")
