#!/usr/bin/env python3
"""Paired launcher-path workflow capture/artifact probe for the 2.21.1 audit.

Set LABEL=2.21.0 or 2.21.1, KIRO_BIN to that version's CLI, and optionally
KIRO_KAS_SERVER_PATH to a pinned acp-server.js.  WF_STYLE=restate|terse.
Auth values are redacted before capture; all sessions/workspaces are temporary.
"""
import json, os, queue, re, signal, sqlite3, subprocess, tempfile, threading, time

OUT = os.environ.get("PROBE_OUT", os.path.dirname(os.path.abspath(__file__)))
LABEL = os.environ.get("LABEL", "2.21.1")
STYLE = os.environ.get("WF_STYLE", "restate")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.environ.get("KIRO_XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
AUTH_DB = os.environ.get("KIRO_AUTH_DB", os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3"))
PIN = os.environ.get("KIRO_KAS_SERVER_PATH")
TAG = f"{LABEL}-{STYLE}-2.21.1"
TRACE = os.path.join(OUT, f"kas-workflow-channels-{TAG}.jsonl")
VERDICT = os.path.join(OUT, f"kas-workflow-channels-{TAG}-verdict.json")
STDERR = os.path.join(OUT, f"kas-workflow-channels-{TAG}-stderr.log")
os.makedirs(OUT, exist_ok=True)

FAKE_HOME = tempfile.mkdtemp(prefix=f"wf-{TAG}-home-")
WS = tempfile.mkdtemp(prefix=f"wf-{TAG}-ws-")
RUNTIME = tempfile.mkdtemp(prefix=f"wf-{TAG}-runtime-")
TMP = tempfile.mkdtemp(prefix=f"wf-{TAG}-tmp-")
os.makedirs(os.path.join(WS, ".kiro", "workflows"), exist_ok=True)
WORKDIR = os.path.join(WS, "run-" + str(int(time.time())))
RESULT = os.path.join(WORKDIR, "result.json")
RECIPE = os.path.join(WS, ".kiro", "workflows", "audit-channels.workflow.json")

with open(RECIPE, "w", encoding="utf-8") as f:
    json.dump({
        "name": "audit-channels-2.21.1",
        "description": "paired template capture and artifact channel audit",
        "inputs": {"token": "string", "workdir": "string"},
        "steps": [
            {"type": "step", "id": "s1", "agent": "wf-coder", "effortLevel": "low",
             "artifacts": {"value": "{{workdir}}/value.txt"},
             "prompt": (("Create the directory {{workdir}} if it does not exist. "
                         "Write exactly the single word {{token}} to the file "
                         "{{workdir}}/value.txt with no other text. "
                         "Then reply with exactly this single word and nothing else: {{token}}")
                        if STYLE == "restate" else
                        ("Create the directory {{workdir}} if it does not exist. "
                         "Write exactly the single word {{token}} to the file "
                         "{{workdir}}/value.txt with no other text. "
                         "That is the entire task. Do not write any summary, explanation or "
                         "closing message; signal completion and stop."))},
            {"type": "step", "id": "s2", "agent": "wf-coder", "effortLevel": "low",
             "prompt": ("Two independent channels are under test; report what you ACTUALLY see.\n"
                        "CHANNEL A (template): between the markers here is A_BEGIN{{s1.output}}A_END. "
                        "If there is nothing between the markers, channelA is the empty string.\n"
                        "CHANNEL B (file): read the file {{artifacts.value}} and note its exact contents.\n"
                        "Write a JSON file to {{workdir}}/result.json containing exactly the keys "
                        "channelA and channelB, whose values are the two things you saw, verbatim. "
                        "Do not guess or copy one channel into the other. Then reply with exactly: DONE")},
        ],
    }, f, indent=1)

env = dict(os.environ)
env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME, "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP})
if PIN:
    env["KIRO_KAS_SERVER_PATH"] = PIN
print(f"== label={LABEL} style={STYLE} launcher={KIRO} kas_pin={PIN or '<launcher-resolved>'}")
print(f"== workspace={WS} workdir={WORKDIR}")

def profile_arn():
    explicit = os.environ.get("KIRO_PROFILE_ARN")
    if explicit:
        return explicit
    try:
        out = subprocess.run([KIRO, "user", "whoami"], capture_output=True, text=True, timeout=15).stdout
    except Exception:
        out = ""
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None
PROFILE_ARN = profile_arn()

def read_token():
    try:
        c = sqlite3.connect(AUTH_DB)
        try: row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        finally: c.close()
        if not row: return None
        v = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
        d = json.loads(v)
        return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": PROFILE_ARN}
    except Exception:
        return None

SENSITIVE = {"accesstoken", "refreshtoken", "authorization", "clientsecret", "secret", "password", "profilearn", "expiresat"}
def scrub(x, key=""):
    kl = key.lower()
    if kl in SENSITIVE or "token" in kl or kl.endswith("credential"):
        return "<REDACTED>"
    if isinstance(x, dict): return {k: scrub(v, k) for k, v in x.items()}
    if isinstance(x, list): return [scrub(v, key) for v in x]
    return x

trace = open(TRACE, "w", buffering=1)
def record(direction, obj):
    if obj.get("method") == "_kiro/auth/getAccessToken":
        obj = dict(obj); obj["params"] = "<REDACTED>"
    trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": scrub(obj)}, sort_keys=True) + "\n")

stderr = open(STDERR, "w")
proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=WS, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr,
                        text=True, bufsize=1, start_new_session=True)
msgs = queue.Queue()
def reader():
    for line in proc.stdout:
        if line.strip(): msgs.put(line.strip())
    msgs.put(None)
threading.Thread(target=reader, daemon=True).start()
_id = [10]
def send(obj):
    record("client->agent", obj); proc.stdin.write(json.dumps(obj) + "\n"); proc.stdin.flush()
def req(method, params=None):
    _id[0] += 1; obj = {"jsonrpc": "2.0", "id": _id[0], "method": method}
    if params is not None: obj["params"] = params
    send(obj); return _id[0]

EVENTS, REQUESTS = [], {}
def within_workspace(path):
    try: return os.path.commonpath([WS, os.path.abspath(path)]) == os.path.abspath(WS)
    except (TypeError, ValueError): return False

TERMS = {}
def callback(obj):
    method = obj.get("method", ""); p = obj.get("params") or {}
    REQUESTS[method] = REQUESTS.get(method, 0) + 1
    if method == "_kiro/auth/getAccessToken": return read_token() or {}
    if method == "_kiro/terminal/shell_type": return {"shellType": "bash"}
    if method == "session/request_permission":
        opts = p.get("options") or []; pick = next((x for x in opts if "allow" in (str(x.get("kind", "")) + str(x.get("optionId", "")).lower())), opts[0] if opts else None)
        return {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}
    path = p.get("path")
    if path and not within_workspace(path): return {"error": "audit path rejected"}
    if method in ("fs/read_text_file", "fs/read_file", "_kiro/fs/read_file") and path:
        try: return {"content": open(path, encoding="utf-8").read()}
        except OSError as e: return {"content": f"(err {e})"}
    if method in ("fs/write_text_file", "fs/write_file", "_kiro/fs/write_file") and path:
        try:
            os.makedirs(os.path.dirname(path), exist_ok=True); open(path, "w", encoding="utf-8").write(p.get("content", p.get("text", ""))); return {}
        except OSError as e: return {"error": str(e)}
    if method == "terminal/create":
        cwd = p.get("cwd") or WS
        try:
            q = subprocess.Popen(["bash", "-lc", p.get("command", "")], cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            tid = f"term-{len(TERMS) + 1}"; TERMS[tid] = q; return {"terminalId": tid}
        except OSError: return {"terminalId": "term-rejected"}
    if method == "terminal/wait_for_exit":
        q = TERMS.get(p.get("terminalId"))
        if not q: return {"exitCode": -1, "signal": None}
        try: q.wait(timeout=60)
        except subprocess.TimeoutExpired: q.kill(); q.wait()
        return {"exitCode": q.returncode if q.returncode >= 0 else None, "signal": -q.returncode if q.returncode < 0 else None}
    if method == "terminal/output":
        q = TERMS.get(p.get("terminalId"))
        if not q or q.stdout is None: return {"output": "", "truncated": False, "exitStatus": {"exitCode": -1, "signal": None}}
        return {"output": q.stdout.read() if q.poll() is not None else "", "truncated": False, "exitStatus": {"exitCode": q.returncode if q.returncode is not None and q.returncode >= 0 else None, "signal": -q.returncode if q.returncode is not None and q.returncode < 0 else None}}
    if method in ("terminal/release", "terminal/kill"):
        q = TERMS.pop(p.get("terminalId"), None)
        if method == "terminal/kill" and q and q.poll() is None: q.kill()
        return {}
    return {}

def pump(until_id=None, timeout=60, stop=None):
    end = time.time() + timeout
    while time.time() < end:
        try: raw = msgs.get(timeout=1)
        except queue.Empty:
            if stop and stop(): return None
            continue
        if raw is None: return None
        try: obj = json.loads(raw)
        except json.JSONDecodeError: continue
        record("agent->client", obj)
        if obj.get("method") and "id" in obj:
            send({"jsonrpc": "2.0", "id": obj["id"], "result": callback(obj)})
        elif obj.get("method"):
            if obj["method"].startswith("_kiro/workflow/"): EVENTS.append(obj)
        elif obj.get("id") == until_id: return obj
        if stop and stop(): return None
    return None

def cleanup():
    if proc.poll() is None:
        try: os.killpg(os.getpgid(proc.pid), signal.SIGTERM); proc.wait(timeout=5)
        except Exception:
            try: os.killpg(os.getpgid(proc.pid), signal.SIGKILL); proc.wait(timeout=5)
            except Exception: pass
    trace.close(); stderr.close()

verdict = {"label": LABEL, "style": STYLE, "launcher": KIRO, "kasPin": PIN, "workspace": WS, "workdir": WORKDIR}
try:
    iid = req("initialize", {"protocolVersion": 1, "clientInfo": {"name": "cyril-2.21.1-workflow-audit", "version": "2.21.1"}, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}})
    init = pump(iid, 90) or {}
    nid = req("session/new", {"cwd": WS, "mcpServers": []})
    new = pump(nid, 120) or {}; sid = (new.get("result") or {}).get("sessionId")
    wreq = req("_kiro/workflow/new", {"workflowPath": RECIPE, "inputs": {"token": "ALPHA", "workdir": WORKDIR}, "parentSessionId": sid, "workspacePaths": [WS]})
    wnew = pump(wreq, 90) or {}
    if "error" in wnew:
        verdict.update({"initialize": init.get("result"), "sessionNew": new.get("result"), "workflowNew": wnew, "events": EVENTS})
        raise RuntimeError("workflow/new failed")
    wid = (wnew.get("result") or {}).get("workflowId")
    inv = req("_kiro/workflow/invoke", {"workflowId": wid})
    t0 = time.time(); pump(inv, 30)
    def done(): return any(e.get("method", "").endswith("run_complete") and (e.get("params") or {}).get("status") in ("completed", "failed", "aborted") for e in EVENTS)
    pump(timeout=720, stop=done)
    rc = next((e for e in EVENTS if e.get("method", "").endswith("run_complete")), None)
    status = (rc.get("params") or {}).get("status") if rc else None
    captured = ((rc.get("params") or {}).get("finalState") or {}).get("capturedOutputs") if rc else None
    observed = None
    if os.path.exists(RESULT):
        try: observed = json.load(open(RESULT, encoding="utf-8"))
        except Exception as e: observed = {"_parse_error": str(e), "_raw": open(RESULT, encoding="utf-8").read()[:400]}
    disk_value = open(os.path.join(WORKDIR, "value.txt"), encoding="utf-8").read() if os.path.exists(os.path.join(WORKDIR, "value.txt")) else None
    verdict.update({"initialize": init.get("result"), "sessionNew": new.get("result"), "workflowNew": wnew.get("result"), "workflowId": wid,
                    "status": status, "elapsedSec": round(time.time() - t0, 2), "capturedOutputs": captured,
                    "observedResult": observed, "diskValue": disk_value, "events": EVENTS, "requests": REQUESTS,
                    "channelA": (observed or {}).get("channelA"), "channelB": (observed or {}).get("channelB"),
                    "channelA_CORRECT": (observed or {}).get("channelA", "").strip() == "ALPHA",
                    "channelB_CORRECT": (observed or {}).get("channelB", "").strip() == "ALPHA"})
    print("== workflow status:", status, "elapsed:", verdict["elapsedSec"], "events:", len(EVENTS))
    print("== captured:", json.dumps(captured), "observed:", json.dumps(observed))
finally:
    cleanup()
with open(VERDICT, "w") as f: json.dump(scrub(verdict), f, indent=2, sort_keys=True)
print("== trace:", TRACE); print("== verdict:", VERDICT)
