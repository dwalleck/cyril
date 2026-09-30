#!/usr/bin/env python3
"""cyril-lki9 live probe for P4 (cancel), P5 (steer / prompt) and P6 (busy parent)
during a KAS workflow wake turn. kiro-cli 2.26.0 / KAS 0.66.15, gate OFF (cyril's
ADR-0011 shape: session/new without workflow settings, _kiro/workflow/new with
parentSessionId = main session, then invoke).

LEG=cancel  at the wake's first agentInitiated frame on main: send session/cancel
LEG=steer   at that frame: send _session/steer {sessionId, message} (cyril's steer path)
LEG=prompt  at that frame: send session/prompt (what cyril sends today while it
            believes it is idle)
LEG=busy    start a long no-tool operator turn (essay) FIRST, then new+invoke so the
            run completes while the parent is mid-turn; look for any wire marker.

Temp HOME + workspace, real XDG_DATA_HOME, access token read-only (no renewal),
auth redacted. Run legs SERIALLY.
"""
import json, os, queue, re, signal, sqlite3, subprocess, tempfile, threading, time

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.environ.get("PROBE_OUT", HERE)
LEG = os.environ["LEG"]
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
DATA_HOME = os.environ.get("KIRO_XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
AUTH_DB = os.environ.get("KIRO_AUTH_DB", os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3"))
PIN = os.environ.get("KIRO_KAS_SERVER_PATH")
TAG = f"{LEG}-06615-2.26.0"
TRACE = os.path.join(OUT, f"lki9-live-{TAG}.jsonl")
VERDICT = os.path.join(OUT, f"lki9-live-{TAG}-verdict.json")
STDERR = os.path.join(OUT, f"lki9-live-{TAG}-stderr.log")

FAKE_HOME = tempfile.mkdtemp(prefix=f"lki9-{TAG}-home-")
WS = tempfile.mkdtemp(prefix=f"lki9-{TAG}-ws-")
RUNTIME = tempfile.mkdtemp(prefix=f"lki9-{TAG}-runtime-")
TMP = tempfile.mkdtemp(prefix=f"lki9-{TAG}-tmp-")
os.makedirs(os.path.join(WS, ".kiro", "workflows"), exist_ok=True)
RECIPE = os.path.join(WS, ".kiro", "workflows", "lki9-quick.workflow.json")
with open(RECIPE, "w", encoding="utf-8") as f:
    json.dump({"name": "lki9-quick", "description": "one fast step for wake probes",
               "steps": [{"type": "step", "id": "s1", "agent": "wf-coder", "effortLevel": "low",
                          "prompt": "Reply with exactly the single word OK and nothing else."}]}, f, indent=1)

env = dict(os.environ)
env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME, "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMP})
if PIN:
    env["KIRO_KAS_SERVER_PATH"] = PIN
print(f"== leg={LEG} launcher={KIRO} workspace={WS}")

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


def notify(method, params):
    send({"jsonrpc": "2.0", "method": method, "params": params})

def main_frames():
    out = []
    with open(TRACE, encoding="utf-8") as tf:
        for line in tf:
            f = json.loads(line); m = f["msg"]; p = m.get("params") if isinstance(m.get("params"), dict) else {}
            out.append((f["ts"], f["dir"], m, p))
    return out

def main_kinds(sid):
    ks = []
    for ts, d, m, p in main_frames():
        if m.get("method") == "session/update" and p.get("sessionId") == sid:
            u = p.get("update") or {}; k = ((u.get("_meta") or {}).get("kiro") or {})
            if u.get("sessionUpdate") == "session_info_update" and k.get("kind") in ("turn_start", "turn_end", "steering_queued", "steering_injected", "steering_cleared"):
                ks.append({"t": ts, "kind": k.get("kind"), "stopReason": k.get("stopReason")})
    return ks

def tagged_seen(sid):
    for ts, d, m, p in main_frames():
        if m.get("method") == "session/update" and p.get("sessionId") == sid:
            k = (((p.get("update") or {}).get("_meta") or {}).get("kiro") or {})
            if k.get("agentInitiated"):
                return True
    return False

def run_done():
    return any(e.get("method", "").endswith("run_complete") and (e.get("params") or {}).get("status") in ("completed", "failed", "aborted") for e in EVENTS)

verdict = {"leg": LEG, "workspace": WS}
try:
    iid = req("initialize", {"protocolVersion": 1, "clientInfo": {"name": "cyril-lki9-probe", "version": "2.26.0"}, "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True}})
    pump(iid, 90)
    nid = req("session/new", {"cwd": WS, "mcpServers": []})
    new = pump(nid, 120) or {}; sid = (new.get("result") or {}).get("sessionId")
    verdict["mainSessionId"] = sid
    t0 = time.time()
    busy_pid = None
    if LEG == "busy":
        # No tools: a tool-using busy turn (e.g. `sleep 30`) blocks this probe's own
        # loop, because the harness answers terminal/wait_for_exit synchronously
        # (first busy run launched the workflow only after the sleep -> not busy).
        busy_pid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "Without using any tools, write a detailed essay of about 1200 words on the history of the printing press. Then end with the single line: ESSAY-DONE"}]})
        pump(timeout=3)
    wreq = req("_kiro/workflow/new", {"workflowPath": RECIPE, "inputs": {}, "parentSessionId": sid, "workspacePaths": [WS]})
    wnew = pump(wreq, 90) or {}
    wid = (wnew.get("result") or {}).get("workflowId"); verdict["workflowId"] = wid
    inv = req("_kiro/workflow/invoke", {"workflowId": wid}); pump(inv, 30)
    if LEG == "busy":
        busy_resp = pump(busy_pid, 300) or {}
        verdict["busyPromptResult"] = busy_resp.get("result") or busy_resp.get("error")
        verdict["runCompleteSeen"] = run_done()
        pump(timeout=60)  # any post-turn wake?
    else:
        pump(timeout=300, stop=run_done)
        verdict["runCompleteSeen"] = run_done()
        pump(timeout=60, stop=lambda: tagged_seen(sid))
        verdict["wakeTaggedSeen"] = tagged_seen(sid)
        verdict["actionAt"] = round(time.time() - t0, 2)
        if LEG == "cancel":
            notify("session/cancel", {"sessionId": sid})
            pump(timeout=60, stop=lambda: any(k["kind"] == "turn_end" for k in main_kinds(sid)))
            pump(timeout=20)
        elif LEG == "steer":
            st = req("_session/steer", {"sessionId": sid, "message": "Also include the exact word BANANA somewhere in your reply."})
            verdict["steerResponse"] = (pump(st, 60) or {})
            pump(timeout=90, stop=lambda: any(k["kind"] == "turn_end" for k in main_kinds(sid)))
            pump(timeout=20)
        elif LEG == "prompt":
            pr = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": "Reply with exactly: PINEAPPLE"}]})
            presp = pump(pr, 180) or {}
            verdict["promptResponse"] = presp.get("result") or presp.get("error")
            verdict["promptResponseAt"] = round(time.time() - t0, 2)
            pump(timeout=20)
    # per-turn text on main, tag reasons, steering kinds
    turns, cur = [], None
    for ts, d, m, p in main_frames():
        if m.get("method") == "session/update" and p.get("sessionId") == sid:
            u = p.get("update") or {}; k = ((u.get("_meta") or {}).get("kiro") or {})
            if u.get("sessionUpdate") == "session_info_update" and k.get("kind") == "turn_start":
                cur = {"start": round(ts - t0, 2), "text": "", "reasons": set(), "tagged": 0, "untagged": 0}; turns.append(cur)
            elif u.get("sessionUpdate") == "session_info_update" and k.get("kind") == "turn_end" and cur:
                cur["end"] = round(ts - t0, 2); cur["stopReason"] = k.get("stopReason"); cur = None
            elif cur is not None and u.get("sessionUpdate") in ("agent_message_chunk", "tool_call", "tool_call_update"):
                if k.get("agentInitiated"): cur["tagged"] += 1; cur["reasons"].add(k.get("agentInitiatedReason"))
                else: cur["untagged"] += 1
                if u.get("sessionUpdate") == "agent_message_chunk": cur["text"] += (u.get("content") or {}).get("text", "")
        if d == "agent->client" and "id" in m and ("result" in m or "error" in m):
            pass
    for t in turns: t["reasons"] = sorted(r for r in t["reasons"] if r); t["text"] = t["text"][:400]
    verdict["mainTurns"] = turns
    verdict["mainKinds"] = [{**k, "t": round(k["t"] - t0, 2)} for k in main_kinds(sid)]
    verdict["workflowEvents"] = [{"method": e.get("method", "").split("/")[-1], "status": (e.get("params") or {}).get("status")} for e in EVENTS]
    verdict["requests"] = REQUESTS
finally:
    cleanup()
with open(VERDICT, "w") as f: json.dump(scrub(verdict), f, indent=2, sort_keys=True, default=list)
print(json.dumps({k: verdict.get(k) for k in ("leg", "runCompleteSeen", "wakeTaggedSeen", "actionAt", "steerResponse", "promptResponse", "busyPromptResult")}, default=str)[:600])
for t in verdict.get("mainTurns", []): print("  TURN", json.dumps(t, default=list)[:500])
print("  KINDS", json.dumps(verdict.get("mainKinds"))[:600])
print("== trace:", TRACE)
