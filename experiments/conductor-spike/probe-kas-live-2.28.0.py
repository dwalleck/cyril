#!/usr/bin/env python3
"""KAS (v3) live discovery driver for the 2.28.0 audit: baseline sweep + workflow lane.

Spawns the pinned KAS server DIRECTLY (cyril's "KAS Free" path, also exactly what
the 2.28.0 TUI host runs):

    <node> --experimental-wasm-modules <acp-server.js> --transport=stdio --auth=acp-callback

so the KAS-bundle axis is isolated (0.66.15 from 2.26.0 vs 0.66.26 from 2.28.0)
against one same-day backend, with no Rust host in the path (the 2.23.1+ host
deletes "stale extracted V3 engines" in the shared data dir -- a direct spawn of
the carved trees cannot trigger that).

    KAS=<acp-server.js> LABEL=<tag> SCENARIO=sweep|workflow [GATE=on|off] \
        probe-kas-live-2.28.0.py <out.jsonl>

SCENARIO=sweep     initialize -> session/new -> settle -> prompt (file read) ->
                   prompt (shell, permission) -> read-only method census ->
                   set_model valid/bogus + prompt (2.27.1 model-fallback probe) ->
                   session/close.
SCENARIO=workflow  the `_kiro/workflow/*` control plane over a fixed recipe set
                   (linear+steer+pause/resume, repeat cap pause + extendRepeat +
                   update + cancel + retry + delete, repeat-without-stop, parallel
                   YAML, command watch, interactive `completion` step answered on the
                   step session, failing step + retry nodeId, agent:// run, user-tier
                   recipe, custom agent with run_workflow [gate on]), mid-session
                   recipe add/edit (recipes_changed), method-discovery probes, and a
                   SECOND connection that session/loads the parent and attaches a run.
                   Every terminal run_complete is followed by pumping the parent
                   auto-wake turn (cyril-lki9) to its turn_end.

Frames are recorded as {ts, conn, dir, tag, msg}; auth material is redacted.
HOME is a fresh temp dir (KAS prunes $HOME/.kiro/logs); XDG_DATA_HOME stays real.

Auth: `_kiro/auth/getAccessToken` is answered from the kiro-cli sqlite store
(profileArn from the `state` table). Single-flight, post-expiry-only renewal
(one renewer, lock, 90 s grace so the user's own kiro-cli renews first, 60 s
back-off) via `kiro-cli user whoami`; callbacks only wait-and-re-read (330 s
deadline, off the pump thread). An absent token row = logged out -> abort.
"""
import hashlib, json, os, queue, shlex, signal, sqlite3, subprocess, sys, tempfile, threading, time
from datetime import datetime, timezone

KAS = os.path.abspath(os.environ["KAS"])
LABEL = os.environ.get("LABEL", "kas")
SCENARIO = os.environ.get("SCENARIO", "sweep")
GATE = os.environ.get("GATE", "off")
OUT_PATH = sys.argv[1]
NODE = os.environ.get("KAS_NODE", os.path.expanduser("~/.local/share/kiro-cli/node"))
KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/bin/kiro-cli"))
REAL_HOME = os.path.expanduser("~")
DATA_HOME = os.path.join(REAL_HOME, ".local", "share")
AUTH_DB = os.path.join(DATA_HOME, "kiro-cli", "data.sqlite3")
SCRATCH = os.environ.get("PROBE_SCRATCH") or tempfile.gettempdir()
FAKE_HOME = tempfile.mkdtemp(prefix=f"kas-{LABEL}-home-", dir=SCRATCH)
WS = tempfile.mkdtemp(prefix=f"kas-{LABEL}-ws-", dir=SCRATCH)
RUNTIME = tempfile.mkdtemp(prefix=f"kas-{LABEL}-rt-", dir=SCRATCH)
TMPD = tempfile.mkdtemp(prefix=f"kas-{LABEL}-tmp-", dir=SCRATCH)

OUT = open(OUT_PATH, "w", buffering=1)
OUT_LOCK = threading.Lock()
T0 = time.time()
def log(*a):
    print(f"[{time.time()-T0:7.1f}]", *a, flush=True)

REDACT = ("accesstoken", "refreshtoken", "idtoken", "profilearn", "authorization", "expiresat")
def scrub(o, k=""):
    if str(k).lower() in REDACT:
        return "<REDACTED>"
    if isinstance(o, dict):
        return {kk: scrub(v, kk) for kk, v in o.items()}
    if isinstance(o, list):
        return [scrub(v, k) for v in o]
    return o

def emit(rec):
    with OUT_LOCK:
        OUT.write(json.dumps(scrub(rec)) + "\n")

def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()

# --------------------------------------------------------------------- auth
AUTH_LOCK = threading.Lock()
LAST_RENEW = [0.0]
AUTH_EVENTS = []

def _db():
    return sqlite3.connect(f"file:{AUTH_DB}?mode=ro", uri=True, timeout=10)

def profile_arn():
    c = _db()
    try:
        row = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    finally:
        c.close()
    if not row:
        return None
    v = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
    return json.loads(v).get("arn")

PROFILE_ARN = profile_arn()

def read_token():
    c = _db()
    try:
        row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
    finally:
        c.close()
    if not row:
        return None
    v = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
    return json.loads(v)

def secs_left(tok):
    s = tok["expires_at"]
    if "." in s:
        head, frac = s.rstrip("Z").split(".")
        s = f"{head}.{frac[:6]}+00:00"
    else:
        s = s.replace("Z", "+00:00")
    return datetime.fromisoformat(s).timestamp() - time.time()

def maybe_renew():
    if not AUTH_LOCK.acquire(blocking=False):
        return
    try:
        if time.time() - LAST_RENEW[0] < 60:
            return
        tok = read_token()
        if tok is None or secs_left(tok) > -90:
            return
        LAST_RENEW[0] = time.time()
        env = dict(os.environ, HOME=FAKE_HOME, XDG_DATA_HOME=DATA_HOME)
        r = subprocess.run([KIRO, "user", "whoami"], env=env, capture_output=True, text=True, timeout=60)
        after = read_token()
        ev = {"renew_at": round(time.time() - T0, 1), "rc": r.returncode,
              "secs_left_after": round(secs_left(after)) if after else None}
        AUTH_EVENTS.append(ev)
        emit({"probe": "auth_renew", **ev})
        log("auth renew", ev)
    finally:
        AUTH_LOCK.release()

def access_token_reply():
    deadline = time.time() + 330
    while True:
        tok = read_token()
        if tok is None:
            AUTH_EVENTS.append({"logged_out_at": round(time.time() - T0, 1)})
            log("AUTH: token row absent (logged out)")
            return {}
        left = secs_left(tok)
        if left > 200 or time.time() > deadline:
            return {"accessToken": tok["access_token"], "expiresAt": tok["expires_at"],
                    "profileArn": PROFILE_ARN}
        if left < -90:
            maybe_renew()
        time.sleep(5)

# --------------------------------------------------------------- connection
class Conn:
    def __init__(self, name):
        self.name = name
        self.env = dict(os.environ)
        self.env.update({"HOME": FAKE_HOME, "XDG_DATA_HOME": DATA_HOME,
                         "XDG_RUNTIME_DIR": RUNTIME, "TMPDIR": TMPD})
        self.env.pop("KIRO_KAS_SERVER_PATH", None)
        self.stderr = open(OUT_PATH.replace(".jsonl", f"-{name}-stderr.log"), "w")
        self.proc = subprocess.Popen(
            [NODE, "--experimental-wasm-modules", KAS, "--transport=stdio", "--auth=acp-callback"],
            cwd=WS, env=self.env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=self.stderr, text=True, bufsize=1, start_new_session=True)
        emit({"probe": "proc", "conn": name, "pid": self.proc.pid,
              "exe": os.readlink(f"/proc/{self.proc.pid}/exe"),
              "argv": [a.decode() for a in open(f"/proc/{self.proc.pid}/cmdline", "rb").read().split(b"\0") if a]})
        self.send_lock = threading.Lock()
        self.cv = threading.Condition()
        self.responses = {}
        self.notes = []            # (ts, method, params)
        self.nid = 1000
        self.tag = "boot"
        self.terms = {}
        self.perms = []
        self.callbacks = {}
        threading.Thread(target=self._reader, daemon=True).start()

    def record(self, direction, msg):
        emit({"ts": round(time.time(), 3), "conn": self.name, "dir": direction,
              "tag": self.tag, "msg": msg})

    def send(self, obj):
        self.record("client->agent", obj)
        with self.send_lock:
            self.proc.stdin.write(json.dumps(obj) + "\n")
            self.proc.stdin.flush()

    def req(self, method, params):
        with self.cv:
            self.nid += 1
            i = self.nid
        self.send({"jsonrpc": "2.0", "id": i, "method": method, "params": params})
        return i

    def call(self, method, params, timeout=60):
        i = self.req(method, params)
        end = time.time() + timeout
        with self.cv:
            while i not in self.responses and time.time() < end and self.proc.poll() is None:
                self.cv.wait(timeout=1)
            return self.responses.get(i)

    def _reader(self):
        for line in self.proc.stdout:
            line = line.strip()
            if not line:
                continue
            try:
                o = json.loads(line)
            except Exception:
                self.record("agent->client(raw)", {"raw": line[:4000]})
                continue
            self.record("agent->client", o)
            m = o.get("method")
            if m and "id" in o:
                self.callbacks[m] = self.callbacks.get(m, 0) + 1
                threading.Thread(target=self._answer, args=(o,), daemon=True).start()
            elif m:
                with self.cv:
                    self.notes.append((time.time(), m, o.get("params") or {}))
                    self.cv.notify_all()
            elif "id" in o:
                with self.cv:
                    self.responses[o["id"]] = o
                    self.cv.notify_all()
        with self.cv:
            self.cv.notify_all()

    def _answer(self, o):
        try:
            res = self.callback(o["method"], o.get("params") or {})
        except Exception as e:  # never leave the agent hanging
            res = {}
            emit({"probe": "callback_error", "method": o["method"], "error": repr(e)})
        self.send({"jsonrpc": "2.0", "id": o["id"], "result": res})

    def callback(self, method, p):
        if method == "_kiro/auth/getAccessToken":
            return access_token_reply()
        if method == "_kiro/terminal/shell_type":
            return {"shellType": "bash"}
        if method == "session/request_permission":
            opts = p.get("options") or []
            pick = next((x for x in opts if x.get("kind") == "allow_once"), None) \
                or next((x for x in opts if "allow" in str(x.get("kind", ""))), None) \
                or (opts[0] if opts else None)
            self.perms.append({"tag": self.tag, "sessionId": p.get("sessionId"),
                               "title": (p.get("toolCall") or {}).get("title"),
                               "kinds": [x.get("kind") for x in opts],
                               "picked": pick and pick.get("optionId")})
            return ({"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                    if pick else {"outcome": {"outcome": "cancelled"}})
        path = p.get("path")
        if method in ("fs/read_text_file", "_kiro/fs/read_file") and path:
            try:
                with open(path, encoding="utf-8") as f:
                    lines = f.read().splitlines(keepends=True)
                start = max((p.get("line") or 1) - 1, 0)
                lim = p.get("limit")
                return {"content": "".join(lines[start:start + lim] if lim else lines[start:])}
            except OSError as e:
                return {"content": f"(error: {e})"}
        if method in ("fs/write_text_file", "_kiro/fs/write_file") and path:
            os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
            with open(path, "w", encoding="utf-8") as f:
                f.write(p.get("content", ""))
            return {}
        if method == "terminal/create":
            cmd = p.get("command", "")
            if p.get("args"):
                cmd = cmd + " " + " ".join(shlex.quote(a) for a in p["args"])
            env = dict(self.env)
            for e in p.get("env") or []:
                env[e.get("name")] = e.get("value", "")
            pr = subprocess.Popen(["bash", "-lc", cmd], cwd=p.get("cwd") or WS, env=env,
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.STDOUT, text=True)
            tid = f"term-{len(self.terms)+1}"
            buf = []
            t = threading.Thread(target=lambda: [buf.append(l) for l in pr.stdout], daemon=True)
            t.start()
            self.terms[tid] = (pr, buf, t)
            return {"terminalId": tid}
        if method in ("terminal/output", "terminal/wait_for_exit", "terminal/release",
                      "terminal/kill", "_kiro/terminal/write"):
            ent = self.terms.get(p.get("terminalId"))
            if not ent:
                return {} if method != "terminal/output" else \
                    {"output": "", "truncated": False, "exitStatus": {"exitCode": -1, "signal": None}}
            pr, buf, t = ent
            if method == "terminal/wait_for_exit":
                try:
                    pr.wait(timeout=600)
                except subprocess.TimeoutExpired:
                    pr.kill(); pr.wait()
                t.join(timeout=5)
                rc = pr.returncode
                return {"exitCode": rc if rc >= 0 else None, "signal": None if rc >= 0 else str(-rc)}
            if method == "terminal/output":
                rc = pr.poll()
                if rc is not None:
                    t.join(timeout=5)
                return {"output": "".join(buf), "truncated": False,
                        "exitStatus": None if rc is None else
                        {"exitCode": rc if rc >= 0 else None, "signal": None if rc >= 0 else str(-rc)}}
            if method == "_kiro/terminal/write":
                try:
                    pr.stdin.write(p.get("input", "")); pr.stdin.flush()
                except Exception:
                    pass
                return {}
            if method == "terminal/kill" and pr.poll() is None:
                pr.kill()
            if method == "terminal/release":
                self.terms.pop(p.get("terminalId"), None)
            return {}
        emit({"probe": "unhandled_callback", "conn": self.name, "method": method})
        return {}

    def wait_note(self, pred, timeout, since=0.0):
        """Block until a notification (ts>=since) satisfies pred(method, params)."""
        end = time.time() + timeout
        with self.cv:
            while time.time() < end and self.proc.poll() is None:
                for ts, m, prm in self.notes:
                    if ts >= since and pred(m, prm):
                        return (ts, m, prm)
                self.cv.wait(timeout=1)
        return None

    def settle(self, secs):
        time.sleep(secs)

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        try:
            os.killpg(os.getpgid(self.proc.pid), signal.SIGTERM)
            self.proc.wait(timeout=10)
        except Exception:
            try:
                os.killpg(os.getpgid(self.proc.pid), signal.SIGKILL)
            except Exception:
                pass
        for pr, _, _ in self.terms.values():
            if pr.poll() is None:
                pr.kill()
        self.stderr.close()

def res_or_err(r):
    if r is None:
        return "TIMEOUT"
    if "error" in r:
        e = r["error"]
        return {"error": {"code": e.get("code"), "message": str(e.get("message"))[:300],
                          "data": str(e.get("data"))[:400] if e.get("data") is not None else None}}
    return r.get("result")

def short(x, n=400):
    s = json.dumps(x) if not isinstance(x, str) else x
    return s[:n]

# ------------------------------------------------------------------- shared
def kiro_kind(prm):
    upd = prm.get("update") or {}
    return ((upd.get("_meta") or {}).get("kiro") or {}).get("kind")

def turn_end_on(sid):
    return lambda m, p: m == "session/update" and p.get("sessionId") == sid and \
        (p.get("update") or {}).get("sessionUpdate") == "session_info_update" and kiro_kind(p) == "turn_end"

SETTINGS_ON = {"workflows": {"enabled": True}, "goal": {"enabled": True},
               "workflowNotifications": {"enabled": True, "delivery": "steer"},
               "codeIntelligence": {"enabled": True}, "knowledge": {"enabled": True},
               "thinking": {"enabled": True}, "largeToolOutputHandler": {"enabled": True}}

def handshake(c, settings=None, tag="init"):
    c.tag = tag
    caps = {"fs": {"readTextFile": True, "writeTextFile": True}, "terminal": True,
            "_meta": {"kiro": {"configurationState": True}}}
    params = {"protocolVersion": 1, "clientCapabilities": caps,
              "clientInfo": {"name": "cyril-probe", "version": "audit-2.28.0"}}
    if settings:
        params["_meta"] = {"kiro": {"settings": settings}}
    return c.call("initialize", params, 120)

def new_session(c, settings=None, tag="session_new"):
    c.tag = tag
    params = {"cwd": WS, "mcpServers": []}
    if settings:
        params["_meta"] = {"kiro": {"settings": settings}}
    return c.call("session/new", params, 180)

def prompt(c, sid, text, tag, timeout=600, meta=None):
    c.tag = tag
    t0 = time.time()
    params = {"sessionId": sid, "prompt": [{"type": "text", "text": text}]}
    if meta:
        params["_meta"] = meta
    r = c.call("session/prompt", params, timeout)
    c.settle(4)
    err = None if (r and "result" in r) else res_or_err(r)
    return {"stopReason": ((r or {}).get("result") or {}).get("stopReason"),
            "error": err, "seconds": round(time.time() - t0, 1)}

SUMMARY = {"probe": "result", "label": LABEL, "scenario": SCENARIO, "gate": GATE,
           "kas": KAS, "kas_sha256": sha256(KAS), "node": NODE}
try:
    SUMMARY["kas_version"] = json.load(open(os.path.join(os.path.dirname(KAS), "..", "..", "package.json")))["version"]
except Exception:
    SUMMARY["kas_version"] = None
emit({"probe": "preflight", **{k: SUMMARY[k] for k in ("label", "scenario", "gate", "kas", "kas_sha256", "kas_version", "node")},
      "node_version": subprocess.run([NODE, "--version"], capture_output=True, text=True).stdout.strip()})
log("KAS", SUMMARY["kas_version"], KAS, "scenario", SCENARIO, "gate", GATE)

tok0 = read_token()
if tok0 is None:
    sys.exit("ABORT: kiro-cli logged out (no token row)")
log("token secs left at start:", round(secs_left(tok0)))

# ==================================================================== SWEEP
def run_sweep():
    c = Conn("main")
    try:
        init = handshake(c)
        SUMMARY["init"] = "ok" if init and "result" in init else res_or_err(init)
        new = new_session(c)
        sid = ((new or {}).get("result") or {}).get("sessionId")
        if not sid:
            SUMMARY["abort"] = res_or_err(new); return
        res = new["result"]
        models = (res.get("models") or {}).get("availableModels") or []
        cur = (res.get("models") or {}).get("currentModelId")
        c.tag = "settle"; c.settle(10)
        T = SUMMARY.setdefault("turns", {})
        T["p1_read"] = prompt(c, sid, "Read the file PROBE.txt in the current directory and reply with only the single word it contains. Do not explain.", "p1_read")
        T["p2_shell"] = prompt(c, sid, "Use your shell command tool to run exactly: echo BRAVO > shell.txt && cat shell.txt   -- then reply with only the command's output.", "p2_shell")
        X = SUMMARY.setdefault("methods_called", {})
        for m, prm in [
            ("session/list", {}), ("session/list", {"cwd": WS}),
            ("_kiro/session/list", {}), ("_kiro/session/context", {"sessionId": sid}),
            ("_kiro/session/history", {"sessionId": sid}), ("_kiro/account/getUsage", {}),
            ("_kiro/memory/list", {}), ("_kiro/permissions/list", {"sessionId": sid}),
            ("_kiro/powers/list", {"sessionId": sid}), ("_kiro/hooks/list", {"sessionId": sid}),
            ("_kiro/sourceProviders/list", {}), ("_kiro/config/template", {}),
            ("_kiro/safety/getProperties", {"sessionId": sid}),
            ("_kiro/workflow/list", {"sessionId": sid}), ("_kiro/workflow/listRecipes", {"sessionId": sid}),
            ("_kiro/workflow/listWatchHandlers", {}),
            ("_kiro/spec/getTaskStatuses", {"sessionId": sid}),
            ("_kiro/configuration/contribute", {}),
            ("_kiro/configuration/contribute", {"sessionId": sid, "settings": {}}),
            ("_kiro/session/setWorkflowNotificationDelivery", {"sessionId": sid, "delivery": "steer"}),
            ("_kiro/session/rename", {"sessionId": sid, "title": "audit sweep"}),
            ("session/set_config_option", {"sessionId": sid, "configId": "autopilot", "value": "on"}),
        ]:
            c.tag = "m:" + m
            X[m + ("" if m not in X else "#2")] = short(res_or_err(c.call(m, prm, 45)), 300)
        caps = (((init or {}).get("result") or {}).get("agentCapabilities") or {})
        SUMMARY["sessionCapabilities"] = caps.get("sessionCapabilities")
        # --- model fallback (2.27.1) --------------------------------------
        alt = next((m.get("modelId") for m in models if m.get("modelId") not in (cur, "auto")), None)
        c.tag = "set_model_valid"
        X["set_model_valid"] = {"modelId": alt, "resp": short(res_or_err(c.call("session/set_model", {"sessionId": sid, "modelId": alt}, 30)))}
        c.tag = "set_model_restore"
        X["set_model_restore"] = short(res_or_err(c.call("session/set_model", {"sessionId": sid, "modelId": cur}, 30)))
        c.tag = "set_model_bogus"
        X["set_model_bogus"] = short(res_or_err(c.call("session/set_model", {"sessionId": sid, "modelId": "bogus-model-zz9"}, 30)))
        c.tag = "set_config_model_bogus"
        X["set_config_option_model_bogus"] = short(res_or_err(c.call("session/set_config_option", {"sessionId": sid, "configId": "model", "value": "bogus-model-zz9"}, 30)))
        T["p3_after_bogus_model"] = prompt(c, sid, "Reply with exactly the word CHARLIE.", "p3_bogus", 300)
        c.tag = "session_close"
        X["session/close"] = short(res_or_err(c.call("session/close", {"sessionId": sid}, 30)))
        c.settle(3)
        SUMMARY["permissions"] = c.perms
        SUMMARY["callbacks"] = c.callbacks
    finally:
        c.close()

# ================================================================= WORKFLOW
def write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)

def prepare_workspace():
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=WS)
    write(os.path.join(WS, "PROBE.txt"), "ALPHA\n")
    wf = os.path.join(WS, ".kiro", "workflows")
    J = lambda name, obj: write(os.path.join(wf, name), json.dumps(obj, indent=1))
    J("linear.workflow.json", {
        "name": "linear", "description": "two steps, captured output", "inputs": {"token": "string"},
        "injectOriginalUserRequest": False,
        "steps": [
            {"type": "step", "id": "s1", "agent": "wf-coder",
             "prompt": "Create the file out/s1.txt containing exactly {{token}} (create the out directory if needed). Then reply with exactly {{token}} and nothing else."},
            {"type": "step", "id": "s2", "agent": "wf-coder",
             "prompt": "The previous step reported: {{s1.output}}. Read out/s1.txt and write its content followed by -TWO into out/s2.txt. Then reply with exactly the content of out/s2.txt and nothing else."}]})
    J("loop.workflow.json", {
        "name": "loop", "description": "repeat parked at cap", "injectOriginalUserRequest": False,
        "steps": [
            {"type": "repeat", "id": "loop", "maxIterations": 1, "onMaxIterations": "pause",
             "stopCondition": {"containsText": "NEVER_MATCH_ZZ9"},
             "steps": [{"type": "step", "id": "work", "agent": "wf-coder",
                        "prompt": "Append one line containing tick to the file out/loop.txt (create it if needed). Then reply with exactly TICK."}]},
            {"type": "step", "id": "after", "agent": "wf-coder", "prompt": "Reply with exactly AFTER."}]})
    J("loopnone.workflow.json", {
        "name": "loopnone", "description": "repeat without stop condition", "injectOriginalUserRequest": False,
        "steps": [{"type": "repeat", "id": "ln", "maxIterations": 2, "onMaxIterations": "continue",
                   "steps": [{"type": "step", "id": "tock", "agent": "wf-coder",
                              "prompt": "Append one line containing tock to out/tock.txt (create it if needed). Then reply with exactly TOCK."}]}]})
    write(os.path.join(wf, "branches.workflow.yaml"), "\n".join([
        "name: branches", "description: parallel branches (YAML recipe)", "injectOriginalUserRequest: false", "steps:",
        "  - type: parallel", "    id: par", "    joinPolicy: all", "    branches:",
        "      - type: step", "        id: b1", "        agent: wf-coder",
        "        prompt: Write exactly B1 into out/b1.txt (create out/ if needed). Then reply with exactly B1.",
        "      - type: step", "        id: b2", "        agent: wf-coder",
        "        prompt: Write exactly B2 into out/b2.txt (create out/ if needed). Then reply with exactly B2.",
        "  - type: step", "    id: join", "    agent: wf-coder",
        "    prompt: Read out/b1.txt and out/b2.txt and reply with their contents joined by a plus sign, nothing else.", ""]))
    write(os.path.join(WS, ".kiro", "watch.sh"),
          "#!/usr/bin/env bash\ncat >/dev/null\necho '{\"outcome\":\"terminal-state\",\"cursor\":{\"n\":1},\"payload\":\"WATCHDONE\",\"targetId\":\"audit\"}'\n")
    J("watch.workflow.json", {
        "name": "watch", "description": "command watch handler", "injectOriginalUserRequest": False,
        "steps": [{"type": "repeat", "id": "wl", "maxIterations": 2, "onMaxIterations": "continue",
                   "stopWhen": "w.terminal",
                   "steps": [{"type": "watch", "id": "w", "handler": "command",
                              "config": {"command": "bash .kiro/watch.sh", "pollIntervalSec": 10, "commandTimeoutSec": 30},
                              "idleTimeoutSec": 120},
                             {"type": "step", "id": "resp", "agent": "wf-coder",
                              "prompt": "The watch reported: {{w.output}}. Reply with exactly SEEN."}]}]})
    J("bgwatch.workflow.json", {
        "name": "bgwatch", "description": "background-process watch handler (new in KAS 0.66.26)", "injectOriginalUserRequest": False,
        "steps": [{"type": "watch", "id": "bg", "handler": "background-process",
                   "config": {"command": "sleep 2; echo BGDONE", "waitSec": 60, "outputTailLines": 5},
                   "idleTimeoutSec": 120},
                  {"type": "step", "id": "bgresp", "agent": "wf-coder",
                   "prompt": "The background process reported: {{bg.output}}. Reply with exactly BGSEEN."}]})
    J("park.workflow.json", {
        "name": "park", "description": "interactive completion step", "injectOriginalUserRequest": False,
        "steps": [{"type": "step", "id": "ask", "agent": "wf-coder",
                   "prompt": "Reply with exactly the word WAITING and nothing else. Do not use any tools.",
                   "completion": {"containsText": "FINISHED"}}]})
    J("fail.workflow.json", {
        "name": "fail", "description": "step on an unknown model", "injectOriginalUserRequest": False,
        "steps": [{"type": "step", "id": "bad", "agent": "wf-coder", "modelId": "bogus-model-zz9",
                   "prompt": "Reply with exactly OK."}]})
    J("broken.workflow.json", {"name": "broken", "steps": [{"type": "step", "id": "x", "agent": "wf-coder"}]})
    write(os.path.join(FAKE_HOME, ".kiro", "workflows", "usertier.workflow.yaml"), "\n".join([
        "name: usertier", "description: user-tier recipe", "injectOriginalUserRequest: false", "steps:",
        "  - type: step", "    id: u1", "    agent: wf-coder", "    prompt: Reply with exactly USERTIER.", ""]))
    write(os.path.join(WS, ".kiro", "agents", "wf-launcher.json"), json.dumps({
        "name": "wf-launcher", "description": "Launches workflow recipes on request.",
        "prompt": "You launch workflow recipes. When asked to run a workflow, call the run_workflow tool with the absolute workflowPath given, then reply with exactly STARTED.",
        "tools": ["run_workflow", "inspect_workflow", "read_file"], "permissions": {"rules": []}}, indent=1))
    return wf

RUNS = {}
def wf_events(c, wid, since=0.0):
    with c.cv:
        return [(ts, m, p) for ts, m, p in c.notes if m.startswith("_kiro/workflow/") and p.get("workflowId") == wid and ts >= since]

def wait_run(c, wid, statuses, timeout, since):
    return c.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == wid
                       and p.get("status") in statuses, timeout, since)

def wait_wake(c, parent, since, timeout=300):
    """Pump the parent auto-wake turn (cyril-lki9) after a terminal run_complete."""
    st = c.wait_note(lambda m, p: m == "session/update" and p.get("sessionId") == parent and
                     (p.get("update") or {}).get("sessionUpdate") == "session_info_update" and
                     kiro_kind(p) == "turn_start", 90, since)
    if not st:
        return {"woke": False}
    en = c.wait_note(turn_end_on(parent), timeout, st[0])
    c.settle(3)
    return {"woke": True, "turn_start_after_s": round(st[0] - since, 1),
            "turn_secs": round(en[0] - st[0], 1) if en else None}

def start_run(c, parent, path, tag, inputs=None, label=None):
    c.tag = tag + ":new"
    # `inputs` is REQUIRED by the 0.66.x zod schema even for recipes that declare none.
    prm = {"workflowPath": path, "parentSessionId": parent, "workspacePaths": [WS],
           "inputs": inputs if inputs is not None else {}}
    if label:
        prm["runLabel"] = label
    r = c.call("_kiro/workflow/new", prm, 60)
    wid = ((r or {}).get("result") or {}).get("workflowId")
    RUNS[tag] = {"new": "ok" if wid else short(res_or_err(r), 500), "workflowId": wid, "steps": []}
    if not wid:
        log(tag, "new failed", RUNS[tag]["new"]); return None, None
    since = time.time()
    c.tag = tag + ":invoke"
    RUNS[tag]["invoke"] = short(res_or_err(c.call("_kiro/workflow/invoke", {"workflowId": wid}, 60)))
    log(tag, "invoked", wid)
    return wid, since

def finish_run(c, parent, tag, wid, since, timeout=900):
    c.tag = tag + ":run"
    done = wait_run(c, wid, ("completed", "failed", "aborted"), timeout, since)
    RUNS[tag]["terminal"] = done[2].get("status") if done else "TIMEOUT"
    log(tag, "terminal", RUNS[tag]["terminal"])
    if done:
        c.tag = tag + ":wake"
        RUNS[tag]["wake"] = wait_wake(c, parent, done[0])
        log(tag, "wake", RUNS[tag]["wake"])
    RUNS[tag]["events"] = [m.split("/")[-1] + (":" + str(p.get("status")) if p.get("status") else "")
                           for _, m, p in wf_events(c, wid)]
    return done

def ctl(c, tag, method, params, key=None, timeout=60):
    c.tag = f"{tag}:{method.split('/')[-1]}"
    r = res_or_err(c.call(method, params, timeout))
    RUNS.setdefault(tag, {}).setdefault("steps", []).append({key or method.split("/")[-1]: short(r, 500)})
    log(tag, method.split("/")[-1], short(r, 200))
    return r

def run_workflow_lane():
    wfdir = prepare_workspace()
    settings = SETTINGS_ON if GATE == "on" else None
    c = Conn("main")
    D = SUMMARY.setdefault("discovery", {})
    try:
        init = handshake(c, settings)
        SUMMARY["init"] = "ok" if init and "result" in init else res_or_err(init)
        new = new_session(c, settings)
        res = (new or {}).get("result") or {}
        parent = res.get("sessionId")
        if not parent:
            SUMMARY["abort"] = res_or_err(new); return
        SUMMARY["parent"] = parent
        SUMMARY["workflowsEnabled"] = (res.get("_meta") or {}).get("workflowsEnabled")
        SUMMARY["modes"] = [m.get("id") for m in ((res.get("modes") or {}).get("availableModes") or [])]
        c.tag = "settle"; c.settle(10)
        # ---- catalog + discovery ------------------------------------------
        for m, prm in [("_kiro/workflow/listRecipes", {"sessionId": parent}),
                       ("_kiro/workflow/listRecipes", {"workspacePaths": [WS]}),
                       ("_kiro/workflow/listWatchHandlers", {}),
                       ("_kiro/workflow/list", {"sessionId": parent}),
                       ("_kiro/workflow/list", {"workspacePaths": [WS]}),
                       ("_kiro/workflow/resumeAll", {}),
                       ("_kiro/workflow/steer", {"workflowId": "wf_x", "message": "x"}),
                       ("_kiro/workflow/message", {"workflowId": "wf_x", "message": "x"}),
                       ("_kiro/workflow/status", {"workflowId": "wf_x"}),
                       ("_kiro/workflow/attach", {"workflowId": "wf_x"}),
                       ("_kiro/workflow/validate", {"workflowPath": os.path.join(wfdir, "linear.workflow.json")}),
                       ("_kiro/workflow/notifications", {"sessionId": parent}),
                       ("_kiro/session/setWorkflowNotificationDelivery", {"sessionId": parent, "delivery": "queue"}),
                       ("_kiro/configuration/contribute", {}),
                       ("_kiro/config/template", {}),
                       ("_kiro/workflow/inspect", {"workflowId": "wf_doesnotexist"})]:
            c.tag = "disc:" + m
            k = m if m not in D else m + "#2"
            D[k] = short(res_or_err(c.call(m, prm, 45)), 700)
        # ---- recipes_changed: add + edit + broken --------------------------
        t_add = time.time()
        c.tag = "recipes:add"
        write(os.path.join(wfdir, "late.workflow.json"), json.dumps({"name": "late", "description": "added mid-session",
              "steps": [{"type": "step", "id": "x", "agent": "wf-coder", "prompt": "Reply LATE."}]}))
        c.settle(10)
        c.tag = "recipes:edit"
        write(os.path.join(wfdir, "late.workflow.json"), json.dumps({"name": "late", "description": "EDITED mid-session",
              "steps": [{"type": "step", "id": "x", "agent": "wf-coder", "prompt": "Reply LATE2."}]}))
        c.settle(10)
        with c.cv:
            rc = [p for ts, m, p in c.notes if m == "_kiro/workflow/recipes_changed"]
        D["recipes_changed_total"] = len(rc)
        D["recipes_changed_after_add"] = len([1 for ts, m, p in c.notes if m == "_kiro/workflow/recipes_changed" and ts >= t_add])
        c.tag = "recipes:list"
        lr = res_or_err(c.call("_kiro/workflow/listRecipes", {"sessionId": parent}, 45))
        if isinstance(lr, dict) and "recipes" in lr:
            D["recipes"] = [{k: r.get(k) for k in ("name", "source", "tier", "builtIn", "validationError", "format") if k in r}
                            for r in lr["recipes"]]
            D["recipe_row_keys"] = sorted({k for r in lr["recipes"] for k in r})
        # ---- 1. linear: steer + pause/inspect/load/list + resume ----------
        wid, since = start_run(c, parent, os.path.join(wfdir, "linear.workflow.json"), "linear", {"token": "ALPHA"}, "audit-linear")
        if wid:
            ns = c.wait_note(lambda m, p: m == "_kiro/workflow/node_start" and p.get("workflowId") == wid and p.get("sessionId"), 180, since)
            if ns:
                ctl(c, "linear", "_session/steer", {"sessionId": ns[2]["sessionId"], "message": "When you reply, also include the word STEERED."}, "steer_s1")
            ctl(c, "linear", "_kiro/workflow/pause", {"workflowId": wid, "initiator": "user", "reason": "audit pause"})
            pz = wait_run(c, wid, ("paused",), 300, since)
            RUNS["linear"]["paused_seen"] = bool(pz)
            ctl(c, "linear", "_kiro/workflow/inspect", {"workflowId": wid})
            ctl(c, "linear", "_kiro/workflow/load", {"workflowId": wid})
            ctl(c, "linear", "_kiro/workflow/list", {"sessionId": parent})
            ctl(c, "linear", "_kiro/workflow/resume", {"workflowId": wid, "initiator": "user", "reason": "audit resume"})
            finish_run(c, parent, "linear", wid, since)
        # ---- 2. loop cap pause -> update -> extendRepeat -> cancel -> retry -> delete
        wid, since = start_run(c, parent, os.path.join(wfdir, "loop.workflow.json"), "loop")
        if wid:
            p1 = wait_run(c, wid, ("paused",), 600, since)
            RUNS["loop"]["cap_paused"] = bool(p1)
            ctl(c, "loop", "_kiro/workflow/inspect", {"workflowId": wid})
            ctl(c, "loop", "_kiro/workflow/update", {"workflowId": wid, "action": "replace_remaining",
                "remainingSteps": [{"type": "step", "id": "after2", "agent": "wf-coder", "prompt": "Reply with exactly REPLACED."}]})
            t1 = time.time()
            ctl(c, "loop", "_kiro/workflow/resume", {"workflowId": wid, "extendRepeat": {"nodeId": "loop", "additionalIterations": 1}}, "resume_extendRepeat")
            p2 = c.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == wid, 600, t1)
            RUNS["loop"]["after_extend"] = p2[2].get("status") if p2 else "TIMEOUT"
            if p2 and p2[2].get("status") in ("completed", "failed", "aborted"):
                RUNS["loop"]["wake_after_extend"] = wait_wake(c, parent, p2[0])
            else:
                t2 = time.time()
                ctl(c, "loop", "_kiro/workflow/cancel", {"workflowId": wid, "initiator": "user", "reason": "audit cancel"})
                ab = wait_run(c, wid, ("aborted", "failed", "completed"), 120, t2)
                RUNS["loop"]["cancel_terminal"] = ab[2].get("status") if ab else "TIMEOUT"
                if ab:
                    RUNS["loop"]["wake_after_cancel"] = wait_wake(c, parent, ab[0])
                t3 = time.time()
                ctl(c, "loop", "_kiro/workflow/retry", {"workflowId": wid})
                rr = c.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == wid, 600, t3)
                RUNS["loop"]["after_retry"] = rr[2].get("status") if rr else "TIMEOUT"
                if rr and rr[2].get("status") == "paused":
                    t4 = time.time()
                    ctl(c, "loop", "_kiro/workflow/cancel", {"workflowId": wid, "targetStatus": "aborted"}, "cancel2")
                    ab2 = wait_run(c, wid, ("aborted", "failed", "completed"), 120, t4)
                    if ab2:
                        RUNS["loop"]["wake_after_cancel2"] = wait_wake(c, parent, ab2[0])
                elif rr:
                    RUNS["loop"]["wake_after_retry"] = wait_wake(c, parent, rr[0])
            ctl(c, "loop", "_kiro/workflow/delete", {"workflowId": wid, "initiator": "user", "reason": "audit delete"})
            ctl(c, "loop", "_kiro/workflow/inspect", {"workflowId": wid}, "inspect_after_delete")
            RUNS["loop"]["events"] = [m.split("/")[-1] + (":" + str(p.get("status")) if p.get("status") else "") for _, m, p in wf_events(c, wid)]
        # ---- 3. watch (command handler; permission routing) ---------------
        nperm = len(c.perms)
        wid, since = start_run(c, parent, os.path.join(wfdir, "watch.workflow.json"), "watch")
        if wid:
            finish_run(c, parent, "watch", wid, since)
            RUNS["watch"]["permissions"] = c.perms[nperm:]
        # ---- 3b. background-process watch (handler new in 0.66.26) -------
        nperm = len(c.perms)
        wid, since = start_run(c, parent, os.path.join(wfdir, "bgwatch.workflow.json"), "bgwatch")
        if wid:
            finish_run(c, parent, "bgwatch", wid, since, 600)
            RUNS["bgwatch"]["permissions"] = c.perms[nperm:]
        # ---- 4. park (interactive completion) answered on the step session
        wid, since = start_run(c, parent, os.path.join(wfdir, "park.workflow.json"), "park")
        if wid:
            ns = c.wait_note(lambda m, p: m == "_kiro/workflow/node_start" and p.get("workflowId") == wid and p.get("sessionId"), 180, since)
            if ns:
                step_sid = ns[2]["sessionId"]
                te = c.wait_note(turn_end_on(step_sid), 300, ns[0])
                c.settle(5)
                ctl(c, "park", "_kiro/workflow/inspect", {"workflowId": wid}, "inspect_parked")
                RUNS["park"]["reply"] = prompt(c, step_sid, "Now reply with exactly the word FINISHED.", "park:reply_step", 300)
            finish_run(c, parent, "park", wid, since, 600)
        if GATE == "on" or os.environ.get("FULL") == "1":
            # ---- 5. loopnone -------------------------------------------------
            wid, since = start_run(c, parent, os.path.join(wfdir, "loopnone.workflow.json"), "loopnone")
            if wid:
                finish_run(c, parent, "loopnone", wid, since)
            # ---- 6. branches (YAML) ------------------------------------------
            wid, since = start_run(c, parent, os.path.join(wfdir, "branches.workflow.yaml"), "branches")
            if wid:
                finish_run(c, parent, "branches", wid, since)
            # ---- 7. fail + retry nodeId (model-fallback evidence on KAS steps)
            wid, since = start_run(c, parent, os.path.join(wfdir, "fail.workflow.json"), "fail")
            if wid:
                d = finish_run(c, parent, "fail", wid, since, 400)
                if d:
                    t5 = time.time()
                    ctl(c, "fail", "_kiro/workflow/retry", {"workflowId": wid, "nodeId": "bad"}, "retry_node")
                    rr = c.wait_note(lambda m, p: m == "_kiro/workflow/run_complete" and p.get("workflowId") == wid, 400, t5)
                    RUNS["fail"]["after_retry"] = rr[2].get("status") if rr else "TIMEOUT"
                    if rr and rr[2].get("status") in ("completed", "failed", "aborted"):
                        RUNS["fail"]["wake_after_retry"] = wait_wake(c, parent, rr[0])
            # ---- 8. user-tier recipe -----------------------------------------
            wid, since = start_run(c, parent, os.path.join(FAKE_HOME, ".kiro", "workflows", "usertier.workflow.yaml"), "usertier")
            if wid:
                finish_run(c, parent, "usertier", wid, since)
        # ---- 9. agent:// ----------------------------------------------------
        wid, since = start_run(c, parent, "agent://wf-coder", "agenturi", {"prompt": "Reply with exactly AGENTRUN."})
        if wid:
            finish_run(c, parent, "agenturi", wid, since)
        # ---- 10. custom agent with run_workflow (model-launched) ------------
        if GATE == "on":
            t6 = time.time()
            RUNS["launcher"] = {"turn": prompt(c, parent,
                f"Use the run_workflow tool to start the workflow at {os.path.join(wfdir, 'linear.workflow.json')} with inputs {{\"token\": \"GAMMA\"}}. Then reply with exactly STARTED.",
                "launcher:turn", 600, meta={"kiro": {"modeId": "wf-launcher"}})}
            rs = c.wait_note(lambda m, p: m == "_kiro/workflow/run_start", 120, t6)
            if rs:
                lw = rs[2].get("workflowId")
                RUNS["launcher"]["workflowId"] = lw
                RUNS["launcher"]["runLabel"] = rs[2].get("runLabel")
                d = wait_run(c, lw, ("completed", "failed", "aborted"), 900, t6)
                RUNS["launcher"]["terminal"] = d[2].get("status") if d else "TIMEOUT"
                if d:
                    RUNS["launcher"]["wake"] = wait_wake(c, parent, d[0])
        # ---- census after runs ----------------------------------------------
        for m, prm in [("_kiro/workflow/list", {"sessionId": parent}), ("session/list", {}),
                       ("_kiro/session/list", {}), ("_kiro/account/getUsage", {}),
                       ("_kiro/session/context", {"sessionId": parent})]:
            c.tag = "post:" + m
            D["post:" + m] = short(res_or_err(c.call(m, prm, 45)), 1500)
        SUMMARY["permissions"] = c.perms
        SUMMARY["callbacks"] = c.callbacks
        with c.cv:
            SUMMARY["note_methods"] = {}
            for _, m, p in c.notes:
                k = m if m != "session/update" else "session/update:" + str((p.get("update") or {}).get("sessionUpdate"))
                SUMMARY["note_methods"][k] = SUMMARY["note_methods"].get(k, 0) + 1
    finally:
        c.close()
    # ---- second connection: session/load parent + attach a run -------------
    c2 = Conn("reload")
    try:
        handshake(c2, settings, "reload:init")
        c2.tag = "reload:session_load"
        R = SUMMARY.setdefault("reload", {})
        R["session/load"] = short(res_or_err(c2.call("session/load", {"sessionId": parent, "cwd": WS, "mcpServers": []}, 180)), 800)
        c2.settle(8)
        for m, prm in [("_kiro/workflow/list", {"sessionId": parent}), ("session/list", {}),
                       ("_kiro/workflow/resumeAll", {})]:
            c2.tag = "reload:" + m
            R[m] = short(res_or_err(c2.call(m, prm, 60)), 1500)
        lw = (RUNS.get("linear") or {}).get("workflowId")
        if lw:
            c2.tag = "reload:load"
            R["workflow/load"] = short(res_or_err(c2.call("_kiro/workflow/load", {"workflowId": lw}, 60)), 1500)
        c2.settle(5)
        with c2.cv:
            R["note_methods"] = {}
            for _, m, p in c2.notes:
                k = m if m != "session/update" else "session/update:" + str((p.get("update") or {}).get("sessionUpdate"))
                R["note_methods"][k] = R["note_methods"].get(k, 0) + 1
    finally:
        c2.close()

if SCENARIO == "lib":       # imported by probe-kas-leads-2.28.0.py for Conn/auth plumbing
    pass
else:
  try:
    if SCENARIO == "sweep":
        run_sweep()
    else:
        run_workflow_lane()
  finally:
      SUMMARY["runs"] = RUNS
      SUMMARY["auth_events"] = AUTH_EVENTS
      SUMMARY["elapsed_s"] = round(time.time() - T0, 1)
      tok = read_token()
      SUMMARY["token_secs_left_at_end"] = round(secs_left(tok)) if tok else "LOGGED_OUT"
      emit(SUMMARY)
      print(json.dumps(scrub(SUMMARY), indent=1)[:12000])
