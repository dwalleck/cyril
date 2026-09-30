#!/usr/bin/env python3
"""Live probe of the 2.26.0 TUI `/workflow new <description>` authoring flow over ACP.

The TUI implements `/workflow new` as a canned session/prompt (tui.js `jCn`, bundle
sha256 df77898749d0f3fb...), extracted verbatim to tui-workflow-new-prompt-2.26.0.txt.
This probe replays that exact prompt on a KAS session and records whether the model
delegates to `wf-workflow-creator`, which workflow tools it calls, and whether a valid
`.kiro/workflows/<name>.workflow.json` lands.

WF_GATE=on  (TUI shape: session/new _meta.kiro.settings.workflows.enabled=true)
WF_GATE=off (cyril ADR-0011 shape: no settings -> wf-* agents and tools unregistered)
Temp HOME + workspace, real XDG_DATA_HOME, auth redacted. Run legs serially.
"""
import json, os, queue, re, signal, sqlite3, subprocess, tempfile, threading, time

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.environ.get("PROBE_OUT", HERE)
GATE = os.environ.get("WF_GATE", "on")
LABEL = os.environ.get("LABEL", "06615")
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.environ.get("KIRO_XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
AUTH_DB = os.environ.get("KIRO_AUTH_DB", os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3"))
PIN = os.environ.get("KIRO_KAS_SERVER_PATH")
GOAL = os.environ.get("WF_GOAL", "for every Markdown file in the notes/ directory, write a one-paragraph summary, then combine the summaries into notes/SUMMARY.md")
TAG = f"{LABEL}-gate{GATE}-2.26.0"
TRACE = os.path.join(OUT, f"kas-workflow-new-{TAG}.jsonl")
VERDICT = os.path.join(OUT, f"kas-workflow-new-{TAG}-verdict.json")
STDERR = os.path.join(OUT, f"kas-workflow-new-{TAG}-stderr.log")
TEMPLATE = open(os.path.join(HERE, "tui-workflow-new-prompt-2.26.0.txt"), encoding="utf-8").read()
PROMPT = TEMPLATE.replace("__GOAL__", GOAL)

FAKE_HOME = tempfile.mkdtemp(prefix=f"wfnew-{TAG}-home-")
WS = tempfile.mkdtemp(prefix=f"wfnew-{TAG}-ws-")
RUNTIME = tempfile.mkdtemp(prefix=f"wfnew-{TAG}-runtime-")
TMP = tempfile.mkdtemp(prefix=f"wfnew-{TAG}-tmp-")
os.makedirs(os.path.join(WS, ".kiro", "workflows"), exist_ok=True)
os.makedirs(os.path.join(WS, "notes"), exist_ok=True)
for n, body in (("alpha.md", "# Alpha\nAlpha covers onboarding."), ("beta.md", "# Beta\nBeta covers billing.")):
    open(os.path.join(WS, "notes", n), "w").write(body)

env = dict(os.environ)
env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME, "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP})
if PIN:
    env["KIRO_KAS_SERVER_PATH"] = PIN
print(f"== gate={GATE} launcher={KIRO} workspace={WS}")

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



verdict = {"gate": GATE, "label": LABEL, "goal": GOAL, "workspace": WS}
try:
    iid = req("initialize", {"protocolVersion": 1, "clientInfo": {"name": "cyril-2.26.0-workflow-new-audit", "version": "2.26.0"}, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}})
    init = pump(iid, 90) or {}
    snp = {"cwd": WS, "mcpServers": []}
    if GATE == "on": snp["_meta"] = {"kiro": {"settings": {"workflows": {"enabled": True}}}}
    nid = req("session/new", snp)
    new = pump(nid, 120) or {}; sid = (new.get("result") or {}).get("sessionId")
    verdict["workflowsEnabled"] = ((new.get("result") or {}).get("_meta") or {}).get("workflowsEnabled")
    modes = ((new.get("result") or {}).get("modes") or {}).get("availableModes") or []
    verdict["wfAgentsAdvertised"] = sorted(m.get("id") for m in modes if str(m.get("id", "")).startswith("wf-"))
    t0 = time.time()
    pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
    resp = pump(pid, 900) or {}
    verdict["firstTurnSec"] = round(time.time() - t0, 1)
    verdict["promptResult"] = resp.get("result") or resp.get("error")
    # Follow-through: with the gate on, the model delegates via run_workflow(agent://wf-workflow-creator),
    # which is ASYNC -- the creator reports back later via send_message relayed to this parent session.
    # Keep pumping until every launched run is terminal, then wait for any wake-up turn(s) on the main
    # session to finish (turn_start/turn_end session_info_update kinds), bounded by idle + hard limits.
    MAIN_TURNS = {"start": 0, "end": 0}
    def main_turn_counts():
        c = {"start": 0, "end": 0}
        with open(TRACE, encoding="utf-8") as tf:
            for line in tf:
                if sid and sid in line and '"session_info_update"' in line:
                    k = ((json.loads(line)["msg"].get("params") or {}).get("update") or {}).get("_meta", {}).get("kiro", {}).get("kind")
                    if (json.loads(line)["msg"].get("params") or {}).get("sessionId") == sid and k in ("turn_start", "turn_end"):
                        c["start" if k == "turn_start" else "end"] += 1
        return c
    def runs_terminal():
        started = {(e.get("params") or {}).get("workflowId") for e in EVENTS if e.get("method", "").endswith("run_start")}
        done = {(e.get("params") or {}).get("workflowId") for e in EVENTS if e.get("method", "").endswith("run_complete") and (e.get("params") or {}).get("status") in ("completed", "failed", "aborted")}
        return started <= done  # vacuously true when no run was launched (gate-off: nothing to wait for)
    hard = time.time() + 1200
    pump(timeout=900, stop=runs_terminal)
    verdict["runsTerminalSec"] = round(time.time() - t0, 1)
    idle_until = time.time() + 90
    last = main_turn_counts()
    while time.time() < min(hard, idle_until):
        pump(timeout=10)
        cur = main_turn_counts()
        if cur != last:
            last = cur; idle_until = time.time() + 90
        if cur["start"] > 1 and cur["start"] == cur["end"] and time.time() > idle_until - 75:
            break
    verdict["mainTurns"] = last
    verdict["elapsedSec"] = round(time.time() - t0, 1)
    verdict["workflowEvents"] = [{"method": e.get("method"), "workflowId": (e.get("params") or {}).get("workflowId"), "status": (e.get("params") or {}).get("status")} for e in EVENTS]
    wfdir = os.path.join(WS, ".kiro", "workflows")
    files = sorted(os.listdir(wfdir))
    verdict["workflowFiles"] = files
    parsed = {}
    for f in files:
        try: parsed[f] = json.load(open(os.path.join(wfdir, f), encoding="utf-8"))
        except Exception as e: parsed[f] = {"_parse_error": str(e)}
    verdict["workflowJson"] = parsed
    lr = req("_kiro/workflow/listRecipes", {"sessionId": sid, "workspacePaths": [WS]})
    lrr = pump(lr, 30) or {}
    verdict["workspaceRecipes"] = [r for r in ((lrr.get("result") or {}).get("recipes") or []) if not r.get("builtIn")]
    verdict["requests"] = REQUESTS
    verdict["mainSessionId"] = sid
finally:
    cleanup()
with open(VERDICT, "w") as f: json.dump(scrub(verdict), f, indent=2, sort_keys=True)
print("== firstTurn", verdict.get("firstTurnSec"), "elapsed", verdict.get("elapsedSec"), "mainTurns", verdict.get("mainTurns"), "files", verdict.get("workflowFiles"), "stop", json.dumps(verdict.get("promptResult"))[:200])
print("== trace:", TRACE); print("== verdict:", VERDICT)
