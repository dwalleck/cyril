#!/usr/bin/env python3
"""
Fire the fs + terminal HOST CALLBACKS — the KAS-5 proxy-stage interception point.

By default KAS runs file I/O and shell IN-PROCESS. If the client advertises the
standard ACP capabilities (`clientCapabilities.fs` / `.terminal`), KAS instead
DELEGATES every read/write/exec back to the client. This probe advertises both,
implements REAL responders (reads/writes files on disk, spawns processes), and runs
one turn that forces a file read + a file write + a shell command — then confirms
each routed THROUGH us (we logged the callback + performed the op).

fs callbacks (per @kiro/acp-type-covenant): `fs/read_text_file`, `fs/write_text_file`
(bare ACP), `_kiro/fs/{read_file,write_file,delete,stat,read_directory}` (Kiro extras).
terminal: `terminal/{create,output,wait_for_exit,release,kill}` (bare ACP) +
`_kiro/terminal/shell_type`.

Auth token self-sourced; never logged. Throwaway repo — safe to read/write/exec.
"""
import json, os, subprocess, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
AUTH = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-kas-fs-terminal-host-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

CWD = tempfile.mkdtemp(prefix="kas-fsterm-")
subprocess.run("git init -q -b main && git config user.email p@p && git config user.name p", cwd=CWD, shell=True)
pathlib.Path(CWD, "README.md").write_text("# csv2json\nThe magic number is 4242.\n")
subprocess.run("git add -A && git commit -qm baseline", cwd=CWD, shell=True)
log(f"# CWD={CWD}")

def read_token():
    c = sqlite3.connect(AUTH)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    v = row[0]; v = v.decode() if isinstance(v, (bytes, bytearray)) else v; d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": d.get("profile_arn")}

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [0]
def req(m, p):
    _id[0] += 1; PIN.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p}) + "\n"); PIN.flush(); return _id[0]
def reply(rid, res):
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n"); PIN.flush()
def err(rid, msg):
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "error": {"code": -32000, "message": msg}}) + "\n"); PIN.flush()

CALLBACKS = []        # (method, short summary)  — proof of routing through us
TERMS = {}            # terminalId -> {out, code}
TURN_ENDS = [0]
AGENT = []

def abspath(p):
    return p if os.path.isabs(p) else os.path.join(CWD, p)

def handle(o):
    m = o.get("method"); rid = o.get("id"); p = o.get("params", {}) or {}
    if rid is None:
        # notifications
        if m and "session/update" in m:
            u = (p.get("update") or {}) if isinstance(p, dict) else {}
            if isinstance(u, dict):
                v = u.get("sessionUpdate")
                if v == "session_info_update" and (((u.get("_meta") or {}).get("kiro") or {}).get("kind")) == "turn_end":
                    TURN_ENDS[0] += 1
                elif v == "agent_message_chunk":
                    AGENT.append(u.get("content", {}).get("text", ""))
        return

    # ---- server -> client REQUESTS ----
    if m == "_kiro/auth/getAccessToken":
        reply(rid, read_token()); return
    if m == "_kiro/terminal/shell_type":
        CALLBACKS.append(("_kiro/terminal/shell_type", "")); reply(rid, {"shellType": "bash"}); return

    # ---- fs callbacks (we perform the real op on disk) ----
    if m in ("fs/read_text_file", "_kiro/fs/read_file"):
        path = p.get("path", "");
        try:
            content = pathlib.Path(abspath(path)).read_text()
        except Exception as e:
            CALLBACKS.append((m, f"path={path} ERR={e}")); err(rid, f"read failed: {e}"); return
        CALLBACKS.append((m, f"path={path} ({len(content)}b)")); reply(rid, {"content": content}); return
    if m in ("fs/write_text_file", "_kiro/fs/write_file"):
        path = p.get("path", ""); content = p.get("content", "")
        try:
            ap = pathlib.Path(abspath(path)); ap.parent.mkdir(parents=True, exist_ok=True); ap.write_text(content)
        except Exception as e:
            CALLBACKS.append((m, f"path={path} ERR={e}")); err(rid, f"write failed: {e}"); return
        CALLBACKS.append((m, f"path={path} ({len(content)}b)")); reply(rid, {}); return
    if m == "_kiro/fs/read_directory":
        path = p.get("path", "")
        try:
            entries = [{"name": e.name, "type": ("directory" if e.is_dir() else "file")} for e in pathlib.Path(abspath(path)).iterdir()]
        except Exception as e:
            CALLBACKS.append((m, f"path={path} ERR={e}")); err(rid, str(e)); return
        CALLBACKS.append((m, f"path={path} ({len(entries)} entries)")); reply(rid, {"entries": entries}); return
    if m == "_kiro/fs/stat":
        path = p.get("path", ""); ap = pathlib.Path(abspath(path))
        if not ap.exists():
            CALLBACKS.append((m, f"path={path} MISSING")); err(rid, "not found"); return
        CALLBACKS.append((m, f"path={path}")); reply(rid, {"type": ("directory" if ap.is_dir() else "file"), "size": ap.stat().st_size}); return
    if m == "_kiro/fs/delete":
        path = p.get("path", "")
        try:
            pathlib.Path(abspath(path)).unlink()
        except Exception as e:
            CALLBACKS.append((m, f"path={path} ERR={e}")); err(rid, str(e)); return
        CALLBACKS.append((m, f"path={path}")); reply(rid, {}); return

    # ---- terminal callbacks (we spawn the real process) ----
    if m == "terminal/create":
        cmd = p.get("command", ""); args = p.get("args") or []
        tid = f"term-{len(TERMS)+1}"
        CALLBACKS.append((m, f"{cmd} {' '.join(args)}"))
        try:
            r = subprocess.run([cmd, *args], cwd=p.get("cwd") or CWD, capture_output=True, text=True, timeout=60)
            TERMS[tid] = {"out": (r.stdout + r.stderr), "code": r.returncode}
        except Exception as e:
            TERMS[tid] = {"out": f"(host error: {e})", "code": 1}
        reply(rid, {"terminalId": tid}); return
    if m == "terminal/output":
        t = TERMS.get(p.get("terminalId"), {"out": "", "code": 0})
        CALLBACKS.append((m, p.get("terminalId", ""))); reply(rid, {"output": t["out"], "truncated": False, "exitStatus": {"exitCode": t["code"]}}); return
    if m == "terminal/wait_for_exit":
        t = TERMS.get(p.get("terminalId"), {"code": 0})
        CALLBACKS.append((m, p.get("terminalId", ""))); reply(rid, {"exitStatus": {"exitCode": t["code"], "signal": None}}); return
    if m in ("terminal/release", "terminal/kill"):
        CALLBACKS.append((m, p.get("terminalId", ""))); reply(rid, {}); return

    if m == "session/request_permission":
        opts = p.get("options", [])
        pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()), opts[0] if opts else None)
        reply(rid, {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}); return
    reply(rid, {})

def pump_once(to=10):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = msgs.get(timeout=2)
        except queue.Empty:
            continue
        if raw is None:
            return False
        try:
            o = json.loads(raw)
        except Exception:
            continue
        if "method" in o:
            handle(o)
    return True

def call_sync(method, params, to=40):
    rid = req(method, params)
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
        if "method" in o:
            handle(o)
        elif o.get("id") == rid:
            return o
    return None

# advertise fs + terminal so KAS delegates to us
CAPS = {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}
ir = call_sync("initialize", {"protocolVersion": 1, "clientCapabilities": CAPS})
log("# initialize agentCapabilities:", json.dumps((ir.get("result") or {}).get("agentCapabilities", {}))[:200] if ir and "result" in ir else "?")
nr = call_sync("session/new", {"cwd": CWD, "mcpServers": []})
sid = (nr.get("result") or {}).get("sessionId") if nr and "result" in nr else None
log("# sessionId:", sid)

PROMPT = ("Do these three steps in order, using your tools: "
          "1) read the file README.md and tell me the magic number in it; "
          "2) create a new file notes.txt containing exactly the line `hello from kas`; "
          "3) run the shell command `echo done-42` and report its output verbatim.")
pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
before = TURN_ENDS[0]
end = time.time() + 240
while time.time() < end and TURN_ENDS[0] <= before:
    pump_once(10)
log("# turn complete:", TURN_ENDS[0] > before)
pump_once(4)

log("\n===== HOST CALLBACKS received (proof KAS routed I/O through us) =====")
for m, s in CALLBACKS:
    log(f"  {m:28} {s}")
if not CALLBACKS:
    log("  (NONE — KAS ran everything in-process; capability not honored?)")

log("\n===== verification =====")
notes = pathlib.Path(CWD, "notes.txt")
log("  notes.txt exists on disk (written via our fs callback):", notes.exists(),
    "->", repr(notes.read_text()) if notes.exists() else "")
fs_cbs = [m for m, _ in CALLBACKS if "fs/" in m]
term_cbs = [m for m, _ in CALLBACKS if m.startswith("terminal/")]
log("  fs callbacks fired:", fs_cbs or "(none)")
log("  terminal callbacks fired:", term_cbs or "(none)")
log("\n===== agent final message (head) =====")
log("".join(AGENT)[:600] or "(none)")
PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
