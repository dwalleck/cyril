#!/usr/bin/env python3
"""v3/KAS turn-lifecycle wire sweep -- the paired lane the 2.21.2 audit lacked.

The audit's field sweeps (188 = 188 paths) covered the **v2** engine only: both
sweep probes spawned `<binary> acp` with no engine flag. v3 was exercised only
by the content_chunk gate probe, and all three of its legs pinned the SAME KAS
0.58.7 -- so v3 had no version-paired sweep anywhere.

The strict 2.21.1 vs 2.21.2 pairing cannot supply one: KAS is byte-frozen at
0.58.7 across that pair (acp-server.js hashes identically), so diffing them is
a null experiment by construction. This probe instead pins the two DISTINCT
KAS builds in the research archive:

    KAS 0.58.7  (shipped by 2.21.1 and 2.21.2)   sha e5bd... -> 7e102d15
    KAS 0.54.8  (shipped by 2.21.0)              sha e5bd0a95

which spans a real binary delta and isolates the binary axis by running both
same-day against the same backend.

Workload is deliberately IDENTICAL to probe-v2-turn-sweep-2.21.2.py -- prompt,
file-read tool call, end_turn -- so the v3 and v2 path sets are directly
comparable rather than measuring two different workloads.

    KAS=<acp-server.js> LABEL=<tag> probe-kas-turn-sweep-2.21.2.py <out.jsonl>

COST: one tiny real turn per leg.
"""
import json, os, queue, re, sqlite3, subprocess, sys, tempfile, threading, time

KAS = os.environ["KAS"]
LABEL = os.environ.get("LABEL", "kas")
OUT = open(sys.argv[1], "w")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.path.expanduser("~/.local/share")
AUTH_DB = os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3")
FAKE_HOME = tempfile.mkdtemp(prefix=f"kts-{LABEL}-home-")
CWD = tempfile.mkdtemp(prefix=f"kts-{LABEL}-cwd-")
RUNTIME = tempfile.mkdtemp(prefix=f"kts-{LABEL}-rt-")
TMP = tempfile.mkdtemp(prefix=f"kts-{LABEL}-tmp-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
with open(os.path.join(CWD, "PROBE.txt"), "w") as fh:
    fh.write("ALPHA\n")

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
            "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP,
            "KIRO_KAS_SERVER_PATH": KAS})
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
METHODS = {}
TERMS = {}

def send(o):
    proc.stdin.write(json.dumps(o) + "\n"); proc.stdin.flush()

def req(m, pr):
    i[0] += 1
    send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr})
    return i[0]

def callback_result(method, params):
    if method == "_kiro/auth/getAccessToken":
        return read_token() or {}
    if method == "_kiro/terminal/shell_type":
        return {"shellType": "bash"}
    path = params.get("path")
    if method in ("fs/read_text_file", "_kiro/fs/read_file") and path:
        try:
            with open(path, encoding="utf-8") as f:
                return {"content": f.read()}
        except OSError as e:
            return {"content": f"(err {e})"}
    if method in ("fs/write_text_file", "_kiro/fs/write_file") and path:
        try:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as f:
                f.write(params.get("content", params.get("text", "")))
            return {}
        except OSError as e:
            return {"error": str(e)}
    if method in ("fs/stat", "_kiro/fs/stat") and path:
        try:
            st = os.stat(path)
            return {"type": "directory" if os.path.isdir(path) else "file", "size": st.st_size}
        except OSError:
            return {}
    if method in ("fs/read_directory", "_kiro/fs/read_directory") and path:
        try:
            return {"entries": [{"name": n,
                                 "type": "directory" if os.path.isdir(os.path.join(path, n)) else "file"}
                                for n in sorted(os.listdir(path))]}
        except OSError as e:
            return {"error": str(e)}
    if method == "terminal/create":
        try:
            p = subprocess.Popen(["bash", "-lc", params.get("command", "")],
                                 cwd=params.get("cwd") or CWD, stdout=subprocess.PIPE,
                                 stderr=subprocess.STDOUT, text=True)
        except OSError:
            return {"terminalId": "term-rejected"}
        tid = f"term-{len(TERMS)+1}"; TERMS[tid] = p
        return {"terminalId": tid}
    if method == "terminal/output":
        p = TERMS.get(params.get("terminalId"))
        if not p:
            return {"output": "", "truncated": False,
                    "exitStatus": {"exitCode": -1, "signal": None}}
        out = p.stdout.read() if p.poll() is not None else ""
        rc = p.returncode
        return {"output": out, "truncated": False,
                "exitStatus": None if rc is None else
                              {"exitCode": rc if rc >= 0 else None,
                               "signal": -rc if rc < 0 else None}}
    if method == "terminal/wait_for_exit":
        p = TERMS.get(params.get("terminalId"))
        if not p:
            return {"exitCode": -1, "signal": None}
        try:
            p.wait(timeout=60)
        except subprocess.TimeoutExpired:
            p.kill(); p.wait()
        rc = p.returncode
        return {"exitCode": rc if rc >= 0 else None, "signal": -rc if rc < 0 else None}
    if method in ("terminal/release", "terminal/kill"):
        p = TERMS.pop(params.get("terminalId"), None)
        if method == "terminal/kill" and p and p.poll() is None:
            p.kill()
        return {}
    if method == "session/request_permission":
        opts = params.get("options") or []
        pick = next((o for o in opts if "allow" in str(o.get("kind", "")).lower()),
                    opts[0] if opts else None)
        return ({"outcome": {"outcome": "selected", "optionId": pick.get("optionId")}}
                if pick else {"outcome": {"outcome": "cancelled"}})
    return {}

def pump(until, to=240, tag=""):
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
            key = m
            if m == "session/update":
                key = "session/update:" + str(((o.get("params") or {}).get("update") or {}).get("sessionUpdate"))
            METHODS[key] = METHODS.get(key, 0) + 1
        if rid is not None and m:
            send({"jsonrpc": "2.0", "id": rid, "result": callback_result(m, o.get("params") or {})})
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None

kas_ver = "unknown"
try:
    pkg = os.path.join(os.path.dirname(KAS), "..", "..", "package.json")
    kas_ver = json.load(open(os.path.normpath(pkg)))["version"]
except Exception:
    pass
print(f"== LABEL={LABEL} KAS={kas_ver} pin={KAS}")

pump(req("initialize", {"protocolVersion": 1,
                        "clientCapabilities": {"terminal": True,
                                               "fs": {"readTextFile": True, "writeTextFile": True}},
                        "clientInfo": {"name": "cyril-probe", "version": "0"}}), 90, "init")
r = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 150, "session_new")
sid = ((r or {}).get("result") or {}).get("sessionId")
if not sid:
    print("ABORT: no sessionId"); sys.exit(1)

t0 = time.time()
rid = req("session/prompt", {"sessionId": sid, "prompt": [{
    "type": "text",
    "text": "Read the file PROBE.txt in the current directory and reply with only the single word it contains. Do not explain."}]})
resp = pump(rid, 300, "turn")
pump(-1, 8, "settle")

summary = {"probe": "result", "label": LABEL, "kas_version": kas_ver, "kas_pin": KAS,
           "stop_reason": ((resp or {}).get("result") or {}).get("stopReason"),
           "turn_seconds": round(time.time() - t0, 1),
           "methods": dict(sorted(METHODS.items()))}
OUT.write(json.dumps(summary) + "\n"); OUT.flush()
print(json.dumps(summary, indent=2)[:1600])
try:
    proc.stdin.close(); proc.terminate(); proc.wait(timeout=15)
except Exception:
    proc.kill()
