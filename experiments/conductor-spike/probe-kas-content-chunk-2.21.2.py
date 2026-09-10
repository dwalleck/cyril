#!/usr/bin/env python3
"""Flip the streamingShellContent gate and capture real _kiro/tools/content_chunk.

The 2.21.2 audit recovered the content_chunk contract statically (from the
shipped bundle). This probe proves it live and paired:

  LEG=on   -> clientCapabilities._meta.kiro.streamingShellContent = true
  LEG=off  -> the key omitted entirely (CONTROL)

Same KAS build, same prompt, same command. If chunks appear only under LEG=on,
the gate is established as the cause rather than inferred from source.

KEY DESIGN POINT: KAS does not run shells itself -- it drives them through ACP
`terminal/*` host callbacks. The existing KAS probes return terminal/output
only AFTER the process exits, which means KAS has no partial output to stream
and content_chunk could never fire. This probe drains stdout on a background
thread and returns whatever has accumulated SO FAR on each terminal/output
poll, which is what makes incremental streaming observable at all.

The workload prints a line per second so the flush buffer (interval + maxBytes)
has reason to emit more than once.

    LEG=on|off KIRO_KAS_SERVER_PATH=<acp-server.js> probe-kas-content-chunk-2.21.2.py <out.jsonl>
"""
import json, os, queue, re, sqlite3, subprocess, sys, tempfile, threading, time

LEG = os.environ.get("LEG", "on")
OUT = open(sys.argv[1], "w")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.environ.get("KIRO_XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
AUTH_DB = os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3")
PIN = os.environ.get("KIRO_KAS_SERVER_PATH")
FAKE_HOME = tempfile.mkdtemp(prefix=f"cc-{LEG}-home-")
CWD = tempfile.mkdtemp(prefix=f"cc-{LEG}-cwd-")
RUNTIME = tempfile.mkdtemp(prefix=f"cc-{LEG}-rt-")
TMP = tempfile.mkdtemp(prefix=f"cc-{LEG}-tmp-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

COMMAND = 'for i in 1 2 3 4 5 6; do echo "chunk-line-$i"; sleep 1; done'

def profile_arn():
    e = os.environ.get("KIRO_PROFILE_ARN")
    if e:
        return e
    out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True, timeout=20).stdout
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None

PROFILE_ARN = profile_arn()

def read_token():
    try:
        c = sqlite3.connect(AUTH_DB)
        try:
            row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        finally:
            c.close()
        if not row:
            return None
        v = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
        d = json.loads(v)
        return {"accessToken": d["access_token"], "expiresAt": d["expires_at"],
                "profileArn": PROFILE_ARN}
    except Exception as e:
        print("auth unavailable:", type(e).__name__)
        return None

env = dict(os.environ)
env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME,
            "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP})
if PIN:
    env["KIRO_KAS_SERVER_PATH"] = PIN

STDERR = open(sys.argv[1].replace(".jsonl", "-stderr.log"), "w")
proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=STDERR,
                        text=True, bufsize=1, start_new_session=True)
msgs = queue.Queue()
threading.Thread(target=lambda: [msgs.put(l.strip()) for l in proc.stdout if l.strip()],
                 daemon=True).start()

REDACT = ("accessToken", "refreshToken", "idToken", "profileArn", "expiresAt")
def redact(o):
    if isinstance(o, dict):
        return {k: ("<REDACTED>" if k in REDACT else redact(v)) for k, v in o.items()}
    if isinstance(o, list):
        return [redact(v) for v in o]
    return o

i = [0]
CHUNKS = []
METHODS = {}

def send(o):
    proc.stdin.write(json.dumps(o) + "\n"); proc.stdin.flush()

def req(m, pr):
    i[0] += 1
    send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr})
    return i[0]

# --- incremental terminal: the piece that makes streaming observable ---
TERMS = {}
class Term:
    def __init__(self, cmd, cwd):
        self.p = subprocess.Popen(["bash", "-lc", cmd], cwd=cwd,
                                  stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                  text=True, bufsize=1)
        self.buf = []
        self.lock = threading.Lock()
        threading.Thread(target=self._drain, daemon=True).start()
    def _drain(self):
        for line in self.p.stdout:
            with self.lock:
                self.buf.append(line)
    def so_far(self):
        with self.lock:
            return "".join(self.buf)

def callback_result(method, params):
    if method == "_kiro/auth/getAccessToken":
        return read_token() or {}
    if method == "_kiro/terminal/shell_type":
        return {"shellType": "bash"}
    if method == "terminal/create":
        try:
            t = Term(params.get("command", ""), params.get("cwd") or CWD)
        except OSError:
            return {"terminalId": "term-rejected"}
        tid = f"term-{len(TERMS)+1}"
        TERMS[tid] = t
        return {"terminalId": tid}
    if method == "terminal/output":
        t = TERMS.get(params.get("terminalId"))
        if not t:
            return {"output": "", "truncated": False,
                    "exitStatus": {"exitCode": -1, "signal": None}}
        rc = t.p.poll()
        # PARTIAL output while still running -- this is the whole point.
        return {"output": t.so_far(), "truncated": False,
                "exitStatus": None if rc is None else
                              {"exitCode": rc if rc >= 0 else None,
                               "signal": -rc if rc < 0 else None}}
    if method == "terminal/wait_for_exit":
        t = TERMS.get(params.get("terminalId"))
        if not t:
            return {"exitCode": -1, "signal": None}
        try:
            t.p.wait(timeout=90)
        except subprocess.TimeoutExpired:
            t.p.kill(); t.p.wait()
        rc = t.p.returncode
        return {"exitCode": rc if rc >= 0 else None, "signal": -rc if rc < 0 else None}
    if method in ("terminal/release", "terminal/kill"):
        t = TERMS.pop(params.get("terminalId"), None)
        if method == "terminal/kill" and t and t.p.poll() is None:
            t.p.kill()
        return {}
    if method == "session/request_permission":
        opts = params.get("options") or []
        pick = next((o for o in opts if "allow" in str(o.get("kind", "")).lower()), opts[0] if opts else None)
        return ({"outcome": {"outcome": "selected", "optionId": pick.get("optionId")}}
                if pick else {"outcome": {"outcome": "cancelled"}})
    return {}

def pump(until, to=180, tag=""):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = msgs.get(timeout=2)
        except queue.Empty:
            if proc.poll() is not None:
                return None
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        o["_tag"] = tag
        OUT.write(json.dumps(redact(o)) + "\n"); OUT.flush()
        m, rid = o.get("method"), o.get("id")
        if m:
            METHODS[m] = METHODS.get(m, 0) + 1
        if m and "content_chunk" in m:
            CHUNKS.append({"t": round(time.time() - T0, 2), "params": o.get("params")})
        if rid is not None and m:
            send({"jsonrpc": "2.0", "id": rid, "result": callback_result(m, o.get("params") or {})})
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None

T0 = time.time()
kiro_meta = {}
if LEG == "on":
    kiro_meta["streamingShellContent"] = True
# TERM=off omits the terminal capability, which forces KAS to use its
# in-process DefaultTerminal instead of delegating shells back to us over
# ACP terminal/*. That matters: runBashCommand wires the streaming sink only
# when the terminal object implements onOutputChunk
# (`t.onOutputChunk && n.onOutputChunk && ...`), and DefaultTerminal is the
# implementation that does.
TERM = os.environ.get("TERM_CAP", "on")
caps = {"fs": {"readTextFile": True, "writeTextFile": True}}
if TERM == "on":
    caps["terminal"] = True
if kiro_meta:
    caps["_meta"] = {"kiro": kiro_meta}

print(f"== LEG={LEG} TERM_CAP={TERM}  advertising _meta.kiro={json.dumps(kiro_meta) or '<absent>'}")
pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": caps,
                        "clientInfo": {"name": "cyril-probe", "version": "0"}}), 60, "init")
r = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 120, "session_new")
sid = ((r or {}).get("result") or {}).get("sessionId")
if not sid:
    print("ABORT: no sessionId"); sys.exit(1)

rid = req("session/prompt", {"sessionId": sid, "prompt": [{
    "type": "text",
    "text": f"Run exactly this shell command and then reply with only the word DONE:\n{COMMAND}"}]})
resp = pump(rid, 300, "turn")
pump(-1, 8, "settle")

texts = []
for c in CHUNKS:
    p = c.get("params") or {}
    inner = ((p.get("content") or {}).get("content") or {})
    texts.append({"t": c["t"], "toolCallId": p.get("toolCallId"),
                  "text": inner.get("text"), "keys": sorted(p.keys())})
summary = {"probe": "result", "leg": LEG, "term_cap": TERM, "kas_pin": PIN,
           "advertised": kiro_meta or None,
           "stop_reason": ((resp or {}).get("result") or {}).get("stopReason"),
           "content_chunk_count": len(CHUNKS),
           "chunks": texts,
           "methods": dict(sorted(METHODS.items()))}
OUT.write(json.dumps(summary) + "\n"); OUT.flush()
print(json.dumps({k: v for k, v in summary.items() if k != "chunks"}, indent=2)[:1500])
print(f"\ncontent_chunk frames: {len(CHUNKS)}")
for t in texts[:8]:
    print(f"   t+{t['t']:>5}s  {t['toolCallId']}  text={json.dumps(t['text'])[:90]}")
try:
    proc.stdin.close(); proc.terminate(); proc.wait(timeout=15)
except Exception:
    proc.kill()
