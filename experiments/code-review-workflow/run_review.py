#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Drive the `code-review-max` KAS workflow over raw ACP.

    # zero credits: register + validate against the live engine, never invoke
    run_review.py --workspace <ws> --validate-only

    # real review
    run_review.py --workspace <ws> --target '<base>...<head>' --scope crates

The workspace must be checked out at the diff's head (finders read surrounding
code from the working tree) and must contain the recipe, the three cr-* agents
and crtool.py — `--install` copies them in from this repo, which is how a
throwaway review worktree at an old commit gets them.

Unattended-run policy: permission requests are answered by review_policy.decide,
not blanket-approved. Writes are allowed only inside the run directory, and
shell only for this run's exact crtool command with a step subcommand and the
run directory, so a confused step cannot touch the code under review.

HOME is isolated (KAS writes ~/.kiro/{sessions,logs}) while kiro-cli's data
directory stays real (the auth store lives there). A real run uses a STABLE
isolated HOME so a failed run can be retried: `--retry <workflowId>`.

Runs on Linux, macOS and Windows, under plain Python or `uv run --script`.
The workflow's `crtool` input is how KAS launches crtool.py on this machine;
`--runner` picks it (default: uv when it is on PATH, else the platform's python).
"""
import argparse
import calendar
import collections
import json
import os
import pathlib
import queue
import re
import shutil
import signal
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
from typing import NoReturn

IS_WINDOWS = os.name == "nt"
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import review_policy as policy  # noqa: E402  (next to this file; the pure, tested decisions)

policy.utf8_stdio()

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
CACHE = os.path.expanduser("~/.cache/kas-code-review")
INSTALL = [".kiro/agents/cr-finder.md", ".kiro/agents/cr-verifier.md", ".kiro/agents/cr-clerk.md",
           ".kiro/agents/cr-commenter.md",
           ".kiro/code-review/crtool.py", ".kiro/workflows/code-review-max.workflow.json"]
RECIPE_REL = ".kiro/workflows/code-review-max.workflow.json"
TERMINAL = ("completed", "failed", "aborted")

ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
ap.add_argument("--workspace", default=os.getcwd())
ap.add_argument("--target", default="auto", help="git diff target: 'main...HEAD', '<base>...<head>', a commit, or auto")
ap.add_argument("--scope", default=".", help="space-separated git pathspecs")
ap.add_argument("--validate-only", action="store_true", help="_kiro/workflow/new without invoke: zero credits")
ap.add_argument("--install", action="store_true", help="copy recipe/agents/crtool from this repo into the workspace")
ap.add_argument("--recipe", help="alternate recipe JSON, sent inline (e.g. a negative control)")
ap.add_argument("--model", help="workflow-wide modelId override (sent as an inline recipe)")
ap.add_argument("--effort", help="workflow-wide effortLevel override")
ap.add_argument("--retry", metavar="WORKFLOW_ID", help="retry a failed run instead of starting one")
ap.add_argument("--resume", metavar="WORKFLOW_ID", help="resume a paused run instead of starting one")
ap.add_argument("--context", default="", help="free text naming the documents authoritative for this change "
                "(its spec/design docs, protocol references); verifiers consult them before confirming")
ap.add_argument("--context-file", help="read --context from a file")
ap.add_argument("--check-cmd", metavar="CMD",
                help="the repo's check command (e.g. 'cargo clippy --workspace --all-targets --message-format=short "
                     "-- -D warnings'). Run ONCE here, before the workflow, so a slow build never sits inside an "
                     "LLM tool call; the result lands in <rundir>/facts/diagnostics.txt")
ap.add_argument("--direct", action="store_true",
                help="spawn KAS directly (node acp-server.js --transport=stdio --auth=acp-callback), the way "
                     "cyril does, instead of through the `kiro-cli acp --agent-engine kas` launcher")
ap.add_argument("--parent-prompt", metavar="TEXT",
                help="probe: send one prompt on the PARENT session, print the reply, and exit (no workflow)")
ap.add_argument("--kiro-setting", action="append", default=[], metavar="KEY",
                help="enable a KAS AgentSettings gate connection-wide: "
                     "initialize.clientCapabilities._meta.kiro.settings.KEY = {enabled: true} (e.g. "
                     "codeIntelligence). Must be an object; a bare true reads as off")
ap.add_argument("--kiro-setting-at", choices=("initialize", "session", "both"), default="initialize",
                help="where --kiro-setting is sent: initialize._meta (connection-wide), session/new._meta, or both")
ap.add_argument("--allow-cap", action="append", default=[], metavar="CAPABILITY",
                help="also allow this consent capability (for probes; e.g. a code-intelligence tool)")
ap.add_argument("--auto-recover", type=int, default=0, metavar="N",
                help="when the run ends without completing (auth death, KAS stall, idle timeout), start a fresh "
                     "server and --retry/--resume it, up to N times. Completed steps are never redone")
ap.add_argument("--rundir", help="run directory (default <ws>/.code-review/<timestamp>)")
ap.add_argument("--timeout-min", type=float, default=180)
ap.add_argument("--idle-min", type=float, default=20, help="abort when the wire is silent this long")
ap.add_argument("--runner", choices=("auto", "uv", "python"), default=None,
                help="how KAS's shell launches crtool.py: `uv run --script` (no project or venv needed) or the "
                     "platform's python (`python` on Windows, `python3` elsewhere). auto = uv when it is on PATH")
ap.add_argument("--crtool-cmd", help="launch crtool.py with exactly this command (overrides --runner)")
ap.add_argument("--kiro", default=os.environ.get("KIRO_BIN", shutil.which("kiro-cli") or "kiro-cli"))
args = ap.parse_args()

WS = os.path.realpath(args.workspace)
RUN_ID = time.strftime("%Y%m%d-%H%M%S")
# Forward slashes, fixed once here: RUNDIR reaches the recipe as {{rundir}}, and models copy it
# into JSON; a backslash path there becomes an escaping bug on Windows.
RUNDIR = policy.posix_path(args.rundir or os.path.join(WS, ".code-review", RUN_ID))


KIRO_DATA = policy.kiro_data_dir()
AUTH_DB = os.path.join(KIRO_DATA, "data.sqlite3")
REAL_ENV = dict(os.environ)

if args.install:
    for rel in INSTALL:
        dst = os.path.join(WS, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy2(os.path.join(REPO, rel), dst)
    print(f"== installed {len(INSTALL)} files into {WS}")
missing = [rel for rel in INSTALL if not os.path.exists(os.path.join(WS, rel))]
if missing:
    sys.exit(f"workspace is missing {missing}; pass --install")

RESUMING = bool(args.retry or args.resume)
# What the run was started with. KAS keeps a run's inputs, so a retry cannot change them;
# the driver keeps its own copy because its permission policy needs the exact crtool command.
INPUTS_FILE = os.path.join(RUNDIR, "_driver-inputs.json")
TARGET, SCOPE = args.target, args.scope


def python_runs(name):
    exe = shutil.which(name)
    try:
        return bool(exe) and subprocess.run([exe, "-c", "import sys"], capture_output=True, timeout=30).returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


_stored = None
if RESUMING:
    try:
        with open(INPUTS_FILE, encoding="utf-8") as _f:
            _stored = json.load(_f)
        CRTOOL, TARGET, SCOPE = _stored["crtool"], _stored["target"], _stored["scope"]
        if args.crtool_cmd or args.runner:
            print("!! --crtool-cmd/--runner ignored: --retry/--resume reuse the crtool command stored with the run")
    except (OSError, ValueError, KeyError) as e:
        # A run started by an older driver, or --rundir not the run's own. KAS still has the real
        # command; the policy can only allow steps whose command matches the one computed here.
        _stored = None
        print(f"!! {INPUTS_FILE} is unreadable ({e}): the permission policy will allow only the crtool command "
              "computed from --crtool-cmd/--runner. If the run was started with another one, pass it with "
              "--crtool-cmd, and pass --rundir, --target and --scope as the run was started")
if _stored is None:
    runner = args.runner or "auto"
    if runner == "auto" and not shutil.which("uv"):
        runner = "python"
    CRTOOL = args.crtool_cmd or policy.crtool_command(runner, runner == "uv", IS_WINDOWS)
    py = "python" if IS_WINDOWS else "python3"
    if not args.crtool_cmd and runner == "python" and not python_runs(py):
        # On Windows `python` is often the Microsoft Store alias, which exits 9009.
        example = ('& "C:/path/to/python.exe" ' if IS_WINDOWS else "/path/to/python3 ") + policy.CRTOOL_REL
        sys.exit(f"`{py}` does not run here, so KAS's shell could not start crtool. Install uv, or pass "
                 f"--crtool-cmd, e.g. --crtool-cmd '{example}'")
    problem = policy.crtool_command_problem(CRTOOL, IS_WINDOWS)
    if problem and not RESUMING:
        sys.exit(f"crtool command {CRTOOL!r} would not work: {problem}")
if not (RESUMING or args.validate_only or args.parent_prompt):
    for _name, _value in (("rundir", RUNDIR), ("target", args.target), ("scope", args.scope)):
        problem = policy.input_problem(_name, _value)
        if problem:
            sys.exit(f"cannot start the review: {problem}")

CONTEXT = open(args.context_file, encoding="utf-8").read().strip() if args.context_file else args.context.strip()
CONTEXT = CONTEXT or "No extra context was provided; rely on manifest.json `change_docs`."

if args.check_cmd and not (args.validate_only or args.retry or args.resume or args.parent_prompt):
    # Deterministic, outside any model: gather (idempotent - the workflow's setup step
    # will find it done) and then the check command, in the caller's REAL environment.
    crtool = os.path.join(WS, ".kiro", "code-review", "crtool.py")
    steps = [[sys.executable, crtool, "gather", RUNDIR, args.target, args.scope]]
    if not os.path.exists(os.path.join(RUNDIR, "facts", "diagnostics.txt")):
        steps.append([sys.executable, crtool, "diagnostics", RUNDIR, args.check_cmd])
    for argv in steps:
        print(f"== pre-step  {' '.join(argv[2:4])} …")
        r = subprocess.run(argv, cwd=WS, env=REAL_ENV, capture_output=True, text=True,
                           encoding="utf-8", errors="replace")
        print("   " + (r.stdout.strip() or r.stderr.strip()).replace("\n", "\n   "))
        if r.returncode != 0:
            sys.exit(f"pre-step failed (exit {r.returncode})")

if not os.path.exists(AUTH_DB) and not args.validate_only:
    print(f"!! no kiro-cli auth store at {AUTH_DB}; set KIRO_DATA_DIR if kiro-cli keeps it elsewhere")
os.makedirs(os.path.join(CACHE, "traces"), exist_ok=True)
if args.validate_only:
    FAKE_HOME = tempfile.mkdtemp(prefix="kas-cr-validate-home-")
else:
    FAKE_HOME = os.path.join(CACHE, "home")
    os.makedirs(FAKE_HOME, exist_ok=True)
RUNTIME = tempfile.mkdtemp(prefix="kas-cr-runtime-")
TRACE = os.path.join(CACHE, "traces", f"{RUN_ID}{'-validate' if args.validate_only else ''}.jsonl")
STDERR = TRACE.replace(".jsonl", "-stderr.log")


# --- auth -------------------------------------------------------------------

def open_auth_db():
    """Read-only: a plain connect CREATES a missing data.sqlite3, and an empty one would then
    win policy.kiro_data_dir's existence check on macOS for every later run."""
    if not os.path.exists(AUTH_DB):
        raise FileNotFoundError(AUTH_DB)
    return sqlite3.connect(pathlib.Path(os.path.abspath(AUTH_DB)).as_uri() + "?mode=ro", uri=True)


def profile_arn():
    if os.environ.get("KIRO_PROFILE_ARN"):
        return os.environ["KIRO_PROFILE_ARN"]
    # The OIDC token no longer carries profile_arn; kiro-cli persists the active
    # profile separately. A null ARN passes init and dies on the first real turn.
    try:
        c = open_auth_db()
        try:
            row = c.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
        finally:
            c.close()
        if row:
            v = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
            arn = json.loads(v).get("arn")
            if arn:
                return arn
    except (OSError, sqlite3.Error, ValueError, AttributeError):
        pass
    try:
        out = subprocess.run([args.kiro, "user", "whoami"], capture_output=True, text=True,
                             encoding="utf-8", errors="replace",
                             timeout=30, env=REAL_ENV).stdout
    except (OSError, subprocess.SubprocessError):
        out = ""
    m = re.search(r"arn:aws:codewhisperer:\S+", out)
    return m.group(0) if m else None


PROFILE_ARN = profile_arn()


KAS_REFRESH_BUFFER = 180   # KAS rejects a token with less than this many seconds left


def remaining(expires_at):
    """Seconds until an RFC3339-Z timestamp (kiro writes 9 fractional digits)."""
    try:
        when = time.strptime(str(expires_at)[:19], "%Y-%m-%dT%H:%M:%S")
        return calendar.timegm(when) - time.time()
    except ValueError:
        return 0.0


RENEW_LOCK = threading.Lock()
RENEW_STATE = {"last": 0.0}


def renew_store():
    """Renew the stored token - at most ONE attempt at a time, and only for a token that
    has really expired. The OIDC refresh token is SINGLE-USE: two `kiro-cli` processes
    renewing at once race on it, the loser is rejected, and kiro-cli answers a rejected
    refresh by logging the user out. (This driver did exactly that once, with a
    refresher thread and an auth callback both renewing at the moment of expiry.)"""
    with RENEW_LOCK:
        tok = read_token()
        if tok is not None and remaining(tok["expiresAt"]) > 0:
            return  # someone else - another thread, the user's own kiro-cli - already renewed
        if time.time() - RENEW_STATE["last"] < 45:
            return  # one attempt is in the past 45 s; do not hammer a single-use grant
        RENEW_STATE["last"] = time.time()
        time.sleep(4)  # let any other kiro-cli that noticed the expiry go first
        tok = read_token()
        if tok is not None and remaining(tok["expiresAt"]) > 0:
            return
        try:
            subprocess.run([args.kiro, "user", "whoami"], capture_output=True, timeout=60, env=REAL_ENV)
        except (OSError, subprocess.SubprocessError) as e:
            print(f"!! token refresh failed: {e}")


def fresh_token(max_wait=330):
    """A token KAS will accept. kiro-cli renews the stored token only once it has
    EXPIRED, while KAS wants one with >180 s left - so for the last three minutes of
    every hour nothing can supply a valid token. Wait that window out instead of
    handing back one KAS will reject, which fails every in-flight step at once.
    This function only WAITS and READS; renewing is the refresher thread's job alone."""
    deadline = time.time() + max_wait
    warned = False
    while True:
        tok = read_token()
        if tok is not None and remaining(tok["expiresAt"]) > KAS_REFRESH_BUFFER + 15:
            return tok
        if time.time() > deadline:
            if tok is None:
                print("!! no stored token (logged out?) - run `kiro-cli login`, then --retry this run")
            return tok
        if not warned:
            left = f"{remaining(tok['expiresAt']):.0f}s left" if tok else "no token readable"
            print(f"   .. token: {left}, inside KAS's refresh buffer; waiting for renewal")
            warned = True
        time.sleep(3)


def read_token():
    if not os.path.exists(AUTH_DB):
        return None
    try:
        c = open_auth_db()
        try:
            row = c.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
        finally:
            c.close()
        if not row:
            return None
        v = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
        d = json.loads(v)
        return {"accessToken": d["access_token"], "expiresAt": d["expires_at"], "profileArn": PROFILE_ARN}
    except (OSError, sqlite3.Error, KeyError, ValueError) as e:
        print(f"!! token read failed: {e}")
        return None


def refresher(stop):
    """The stored access token lives ~1h and nothing refreshes it for a raw ACP
    client; any authenticated kiro-cli call does. Without this a long run dies
    mid-step with `CodeWhispererStreaming: Access denied`."""
    while True:
        tok = read_token()
        left = remaining(tok["expiresAt"]) if tok else None
        # The ONLY renewer in this process. A renewal before expiry is a no-op, so it
        # acts only once the token has lapsed; near expiry it just watches more closely.
        if stop.wait(10 if left is not None and left < 300 else 120):
            return
        if left is not None and left <= 0:
            renew_store()


# --- wire -------------------------------------------------------------------

SENSITIVE = {"accesstoken", "refreshtoken", "authorization", "clientsecret", "secret", "password",
             "profilearn", "expiresat"}


def scrub(x, key=""):
    if key.lower() in SENSITIVE:
        return "<REDACTED>"
    if isinstance(x, dict):
        return {k: scrub(v, k) for k, v in x.items()}
    if isinstance(x, list):
        return [scrub(v, key) for v in x]
    return x


trace = open(TRACE, "w", buffering=1, encoding="utf-8")


def record(direction, obj):
    if obj.get("method") == "_kiro/auth/getAccessToken" or "accessToken" in json.dumps(obj.get("result") or ""):
        obj = {k: ("<REDACTED>" if k in ("params", "result") else v) for k, v in obj.items()}
    trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": scrub(obj)}) + "\n")


env = dict(os.environ)
if IS_WINDOWS:
    # node's os.homedir() reads USERPROFILE on Windows, so that is what isolates ~/.kiro;
    # kiro-cli's own data is under LOCALAPPDATA, which stays real.
    env.update({"USERPROFILE": FAKE_HOME, "TEMP": RUNTIME, "TMP": RUNTIME})
# HOME is isolated below, but rustup shims (rust-analyzer, cargo) locate their
# toolchain through $HOME: pin them to the real install or a language server
# spawned by KAS's code-intelligence tool fails for sandbox reasons, not KAS ones.
env.setdefault("RUSTUP_HOME", os.path.expanduser("~/.rustup"))
env.setdefault("CARGO_HOME", os.path.expanduser("~/.cargo"))
env["HOME"] = FAKE_HOME
if not IS_WINDOWS:
    env["XDG_RUNTIME_DIR"] = RUNTIME
    # HOME is fake, so every ~/-relative data dir would move with it. uv's managed Pythons and
    # the like live under the real XDG data home: keep pointing there.
    _real_home = os.path.expanduser("~")
    env["XDG_DATA_HOME"] = REAL_ENV.get("XDG_DATA_HOME") or os.path.join(_real_home, ".local", "share")
    _data = os.path.normpath(KIRO_DATA)
    if sys.platform != "darwin" and os.path.basename(_data) == "kiro-cli":
        # Linux kiro-cli reads $XDG_DATA_HOME/kiro-cli, so a non-default data dir needs its parent.
        env["XDG_DATA_HOME"] = os.path.dirname(_data)
    elif sys.platform == "darwin" and not os.path.relpath(_data, _real_home).startswith(".."):
        # macOS kiro-cli resolves its data dir from HOME (~/Library/Application Support) and
        # ignores XDG: link the real one into the fake HOME at the same place.
        _link = os.path.join(FAKE_HOME, os.path.relpath(_data, _real_home))
        os.makedirs(os.path.dirname(_link), exist_ok=True)
        if os.path.islink(_link) and os.readlink(_link) != _data:
            os.unlink(_link)
        if not os.path.lexists(_link):
            os.symlink(_data, _link)
        elif not os.path.islink(_link):
            print(f"!! {_link} is a real directory in the isolated HOME; kiro-cli will read it, not {_data}")
    else:
        print(f"!! kiro data dir {KIRO_DATA} cannot be expressed to the child (not named kiro-cli, or outside "
              "HOME on macOS); it will look under the isolated HOME")
stderr = open(STDERR, "w", encoding="utf-8")
if args.direct:
    # The launcher's own spawn line, minus the launcher: client meta reaches KAS unmediated.
    version = subprocess.run([args.kiro, "--version"], capture_output=True, text=True, encoding="utf-8",
                             errors="replace", env=REAL_ENV).stdout.split()[-1]
    root = KIRO_DATA
    matches = sorted(d for d in os.listdir(os.path.join(root, "kas")) if d.startswith(version + "-") and not d.endswith(".lock"))
    if not matches:
        sys.exit(f"no KAS bundle for kiro-cli {version} under {root}/kas")
    server = os.path.join(root, "kas", matches[-1], "node_modules", "@kiro", "agent", "dist", "server", "acp-server.js")
    SPAWN = [os.path.join(root, "node.exe" if IS_WINDOWS else "node"), "--experimental-wasm-modules", server, "--transport=stdio", "--auth=acp-callback"]
    print(f"== spawn     DIRECT node acp-server.js ({matches[-1][:24]}…)")
else:
    SPAWN = [args.kiro, "acp", "--agent-engine", "kas"]
proc = subprocess.Popen(SPAWN, cwd=WS, env=env,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr,
                        text=True, bufsize=1, encoding="utf-8",
                        # its own process group, so cleanup reaps the node server KAS spawns too
                        **({"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP} if IS_WINDOWS
                           else {"start_new_session": True}))
assert proc.stdin is not None and proc.stdout is not None
STDIN, STDOUT = proc.stdin, proc.stdout
msgs = queue.Queue()


def reader():
    for line in STDOUT:
        if line.strip():
            msgs.put(line.strip())
    msgs.put(None)


threading.Thread(target=reader, daemon=True).start()
_id = [10]


SEND_LOCK = threading.Lock()


def send(obj):
    with SEND_LOCK:
        record("client->agent", obj)
        STDIN.write(json.dumps(obj) + "\n")
        STDIN.flush()


def req(method, params=None):
    _id[0] += 1
    obj = {"jsonrpc": "2.0", "id": _id[0], "method": method}
    if params is not None:
        obj["params"] = params
    send(obj)
    return _id[0]


# --- permission policy ------------------------------------------------------

PERMS = collections.Counter()
DENIED = []


def decide(p):
    return policy.decide(p, WS, RUNDIR, CRTOOL, tuple(args.allow_cap))


def answer_permission(p):
    allow, why = decide(p)
    tool = ((p.get("_meta") or {}).get("kiro") or {}).get("toolId")
    if tool != "fs_write" and not PERMS[f"seen:{tool}"]:
        print(f"   .. first {tool!r} permission request, raw: {json.dumps(scrub(p))[:700]}")
    PERMS[f"seen:{tool}"] += 1
    want = "allow_once" if allow else "reject_once"
    opt = next((o for o in p.get("options") or [] if o.get("kind") == want), None)
    PERMS["allowed" if allow else "DENIED"] += 1
    if not allow:
        DENIED.append(why)
        print(f"   !! DENIED permission: {why}")
    if opt is None:
        return {"outcome": {"outcome": "cancelled"}}
    return {"outcome": {"outcome": "selected", "optionId": opt["optionId"]}}


def callback(obj):
    method, p = obj.get("method", ""), obj.get("params") or {}
    if method == "_kiro/auth/getAccessToken":
        return read_token() or {}
    if method == "_kiro/terminal/shell_type":
        return {"shellType": "powershell" if IS_WINDOWS else "bash"}
    if method == "session/request_permission":
        return answer_permission(p)
    return {}


# --- run state --------------------------------------------------------------

EVENTS = []
SESS = {}       # sessionId -> stats
NODE_OF = {}    # sessionId -> label
T0 = [time.time()]
STATE = {"status": None, "last": time.time(), "nudges": collections.Counter(), "pending_nudge": {}}


def el():
    s = int(time.time() - T0[0])
    return f"{s // 60:3d}:{s % 60:02d}"


def label(p):
    path = p.get("nodePath") or []
    return "/".join(str(x) for x in path[1:]) or str(p.get("nodeId"))


def on_workflow(method, p):
    kind = method.rsplit("/", 1)[-1]
    if kind == "run_start":
        print(f"[{el()}] run_start {p.get('workflowName')}")
    elif kind == "node_start" and p.get("sessionId"):
        NODE_OF[p["sessionId"]] = label(p)
        SESS.setdefault(p["sessionId"], {"node": label(p), "agent": p.get("agentName"), "start": time.time(),
                                        "peak_ctx": 0.0, "tools": 0, "chars": 0})
        print(f"[{el()}]   start   {label(p):42s} {p.get('agentName')}")
    elif kind == "node_complete" and len(p.get("nodePath") or []) > 1:
        extra = f"  <- {p['failureReason']}" if p.get("failureReason") else ""
        print(f"[{el()}]   {str(p.get('status')):9s} {label(p)}{extra}")
        for st in SESS.values():
            if st["node"] == label(p) and "end" not in st:
                st["end"], st["status"] = time.time(), p.get("status")
    elif kind == "loop_iteration":
        print(f"[{el()}]   loop    {p.get('loopId')} iter={p.get('iteration')} stop={p.get('stopConditionMet')}")
    elif kind == "node_paused":
        print(f"[{el()}]   PAUSED  {label(p)} kind={p.get('kind')} reason={str(p.get('reason'))[:140]}")
        if p.get("kind") != "retry-wait":
            nudge(label(p))
    elif kind == "paused":
        print(f"[{el()}] run paused: {p.get('pauseReason')}")
    elif kind == "run_complete":
        STATE["status"] = p.get("status")
        print(f"[{el()}] run_complete status={p.get('status')}")


def nudge(node):
    """BEST-EFFORT, UNTESTED. A step that ends its turn without signalling parks the
    run ("Awaiting next user message on step session"). Send that message, then
    resume the run once the turn answers."""
    sid = next((s for s, n in NODE_OF.items() if n == node), None)
    if not sid or STATE["nudges"][node] >= 2:
        return
    STATE["nudges"][node] += 1
    rid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": (
        "You ended your turn without signalling completion. If your task is finished, signal completion now "
        "with send_message (severity success). If it is not, finish it per your instructions, then signal.")}]})
    STATE["pending_nudge"][rid] = node
    print(f"[{el()}]   nudged  {node} (attempt {STATE['nudges'][node]})")


MODEL_OF = {}   # sessionId -> resolved model; keyed apart from SESS because a step's
                # bootstrap frames arrive BEFORE the node_start that names its session


def on_session_update(p):
    u = p.get("update") or {}
    kind = u.get("sessionUpdate")
    if kind == "config_option_update":
        for opt in u.get("configOptions") or []:
            if opt.get("id") == "model":
                MODEL_OF[p.get("sessionId")] = opt.get("currentValue")
    st = SESS.get(p.get("sessionId"))
    if st is None:
        return
    kiro = (u.get("_meta") or {}).get("kiro") or {}
    if kind == "tool_call":
        st["tools"] += 1
    elif kind == "agent_message_chunk":
        st["chars"] += len(((u.get("content") or {}).get("text")) or "")
    elif kiro.get("kind") == "context_usage":
        pct = kiro.get("usagePercentage")
        if isinstance(pct, (int, float)) and pct >= st["peak_ctx"]:
            st["peak_ctx"] = pct
            st["breakdown"] = {k: v.get("tokens") for k, v in (kiro.get("breakdown") or {}).items()
                               if isinstance(v, dict)}


def pump(until_id=None, timeout=60, stop=None):
    end = time.time() + timeout
    while time.time() < end:
        try:
            raw = msgs.get(timeout=1)
        except queue.Empty:
            if stop and stop():
                return None
            continue
        if raw is None:
            print("!! agent process closed its stdout")
            return None
        try:
            obj = json.loads(raw)
        except json.JSONDecodeError:
            continue
        STATE["last"] = time.time()
        record("agent->client", obj)
        method = obj.get("method")
        if method == "_kiro/auth/getAccessToken" and "id" in obj:
            # Off the pump thread: this answer may have to wait out the refresh window,
            # and every other session's requests must keep flowing meanwhile.
            threading.Thread(target=lambda rid=obj["id"]: send(
                {"jsonrpc": "2.0", "id": rid, "result": fresh_token() or {}}), daemon=True).start()
        elif method and "id" in obj:
            send({"jsonrpc": "2.0", "id": obj["id"], "result": callback(obj)})
        elif method:
            if method.startswith("_kiro/workflow/"):
                EVENTS.append(obj)
                on_workflow(method, obj.get("params") or {})
            elif method == "session/update":
                on_session_update(obj.get("params") or {})
        else:
            node = STATE["pending_nudge"].pop(obj.get("id"), None)
            if node is not None and STATE.get("wid"):
                req("_kiro/workflow/resume", {"workflowId": STATE["wid"]})
                print(f"[{el()}]   resume  after nudging {node}")
            if obj.get("id") == until_id:
                return obj
        if stop and stop():
            return None
    return None


CLEANED = [False]


def cleanup():
    # Runs from fail() and the early exits, then again from `finally`: once is enough.
    if CLEANED[0]:
        return
    CLEANED[0] = True
    if IS_WINDOWS:
        if proc.poll() is None:
            # taskkill /T takes the whole tree: the launcher and the node server under it.
            r = subprocess.run(["taskkill", "/T", "/F", "/PID", str(proc.pid)], capture_output=True,
                               text=True, encoding="utf-8", errors="replace")
            if r.returncode != 0:
                print(f"!! taskkill failed ({r.returncode}): {(r.stdout + r.stderr).strip()[:200]}")
            try:
                proc.wait(timeout=8)
            except subprocess.TimeoutExpired:
                proc.kill()
                print("!! launcher did not exit after taskkill; killed it directly")
        else:
            # Windows has no process group to signal after the leader exits.
            print("!! kiro-cli had already exited; a KAS node server it started may still be running")
    else:
        # The group outlives its leader, so signal it even when kiro-cli itself already
        # exited: node can still be in it. start_new_session made the pgid == our child's pid.
        for sig in (signal.SIGTERM, signal.SIGKILL):
            try:
                os.killpg(proc.pid, sig)
            except ProcessLookupError:
                break
            try:
                proc.wait(timeout=8)
                time.sleep(0.5)
                os.killpg(proc.pid, 0)  # anything left in the group?
            except ProcessLookupError:
                break
            except subprocess.TimeoutExpired:
                continue
    trace.close()
    stderr.close()


def fail(msg, payload=None) -> NoReturn:
    print(f"!! {msg}")
    if payload is not None:
        print(json.dumps(payload, indent=2)[:3000])
    cleanup()
    sys.exit(1)


def tree_stats(nodes, depth=1):
    steps, total, deepest = 0, 0, depth
    for n in nodes or []:
        total += 1
        steps += n.get("type") == "step"
        kids = (n.get("children") or []) + (n.get("steps") or []) + (n.get("branches") or [])
        s, t, d = tree_stats(kids, depth + 1)
        steps, total, deepest = steps + s, total + t, max(deepest, d)
    return steps, total, deepest


# --- main -------------------------------------------------------------------

def _terminate(signum, _frame):
    # Default SIGTERM skips `finally`, which would orphan the KAS server group.
    raise SystemExit(f"signal {signum}")


signal.signal(signal.SIGTERM, _terminate)
if hasattr(signal, "SIGHUP"):  # not on Windows
    signal.signal(signal.SIGHUP, _terminate)

stop_refresh = threading.Event()
print(f"== workspace {WS}\n== rundir    {RUNDIR}\n== trace     {TRACE}")
try:
    init_params = {"protocolVersion": 1, "clientCapabilities": {},
                   "clientInfo": {"name": "cyril-code-review-driver", "version": "0.1"}}
    if args.kiro_setting and args.kiro_setting_at in ("initialize", "both"):
        # The initialize form is connection-wide, so engine-created workflow step
        # sessions inherit it; the session/new form would only reach the parent.
        # KiroClientMeta lives at clientCapabilities._meta.kiro (covenant §2), NOT at
        # params._meta: a top-level _meta is accepted and silently ignored.
        kiro_meta = {"settings": {k: {"enabled": True} for k in args.kiro_setting}}
        init_params["clientCapabilities"] = {"_meta": {"kiro": kiro_meta}}
        print(f"== clientCapabilities._meta.kiro  {kiro_meta}")
    init = pump(req("initialize", init_params), 120)
    if not init or "error" in init:
        fail("initialize failed", init)
    info = init["result"].get("agentInfo") or init["result"].get("serverInfo") or {}
    print(f"== agent     {info.get('name', '?')} {info.get('version', '?')}")
    print(f"== crtool    {CRTOOL}{' (stored with the run)' if _stored else ''}")

    new_params = {"cwd": WS, "mcpServers": []}
    if args.kiro_setting and args.kiro_setting_at in ("session", "both"):
        new_params["_meta"] = {"kiro": {"settings": {k: {"enabled": True} for k in args.kiro_setting}}}
        print(f"== session/new settings  {new_params['_meta']['kiro']['settings']}")
    new = pump(req("session/new", new_params), 180)
    if not new or "error" in new:
        fail("session/new failed", new)
    sid = new["result"]["sessionId"]
    for opt in new["result"].get("configOptions") or []:
        if opt.get("id") == "model":
            vals = [o.get("value") for o in opt.get("options") or []]
            print(f"== model     current={opt.get('currentValue')}  available={vals}")

    if args.parent_prompt:
        PARENT_TEXT = []
        _orig = on_session_update

        def _capture(p):
            u = p.get("update") or {}
            if p.get("sessionId") == sid and u.get("sessionUpdate") == "agent_message_chunk":
                PARENT_TEXT.append((u.get("content") or {}).get("text") or "")
            _orig(p)
        globals()["on_session_update"] = _capture
        pump(req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": args.parent_prompt}]}), 300)
        print("== parent reply:\n" + "".join(PARENT_TEXT))
        cleanup()
        sys.exit(0)

    if args.retry or args.resume:
        wid = args.retry or args.resume
        verb = "retry" if args.retry else "resume"
        T0[0] = time.time()
        # A new server process does not know the run: rehydrate it from disk first and
        # re-attach the notification bridge to THIS session.
        ld = pump(req("_kiro/workflow/load", {"workflowId": wid, "workspacePaths": [WS], "parentSessionId": sid}), 120)
        print(f"== load      {'ok' if ld and 'result' in ld else json.dumps((ld or {}).get('error'))[:200]}")
        r = pump(req(f"_kiro/workflow/{verb}", {"workflowId": wid}), 120)
        if not r or "error" in r:
            fail(f"workflow/{verb} failed", r)
    else:
        params = {"inputs": {"rundir": RUNDIR, "target": TARGET, "scope": SCOPE, "context": CONTEXT,
                             "crtool": CRTOOL},
                  "parentSessionId": sid, "workspacePaths": [WS]}
        if args.recipe or args.model or args.effort:
            recipe = json.load(open(args.recipe or os.path.join(WS, RECIPE_REL), encoding="utf-8"))
            if args.model:
                recipe["modelId"] = args.model
            if args.effort:
                recipe["effortLevel"] = args.effort
            params["workflow"] = recipe
        else:
            params["workflowPath"] = os.path.join(WS, RECIPE_REL)
        wnew = pump(req("_kiro/workflow/new", params), 120)
        if not wnew or "error" in wnew:
            fail("ENGINE REJECTED THE RECIPE (_kiro/workflow/new)", wnew)
        wid = wnew["result"]["workflowId"]
        state = wnew["result"].get("initialState") or {}
        tree = state.get("nodeTree") or state.get("plan") or state.get("root") or {}
        nodes = tree if isinstance(tree, list) else [tree]
        steps, total, depth = tree_stats(nodes)
        print(f"== ENGINE ACCEPTED THE RECIPE  workflowId={wid}")
        print(f"== compiled tree: {steps} step nodes, {total} nodes, depth {depth}; "
              f"initialState keys={sorted(state.keys())}")
        if args.validate_only:
            with open(os.path.join(CACHE, "last-validate-initial-state.json"), "w", encoding="utf-8") as f:
                json.dump(scrub(state), f, indent=2)
            print("== validate-only: not invoking (zero credits)")
            cleanup()
            shutil.rmtree(FAKE_HOME, ignore_errors=True)
            sys.exit(0)
        os.makedirs(RUNDIR, exist_ok=True)
        with open(INPUTS_FILE, "w", encoding="utf-8") as f:
            json.dump({"workflowId": wid, "crtool": CRTOOL, "target": TARGET, "scope": SCOPE}, f, indent=2)
        T0[0] = time.time()
        inv = pump(req("_kiro/workflow/invoke", {"workflowId": wid}), 60)
        if inv and "error" in inv:
            fail("workflow/invoke failed", inv)

    STATE["wid"] = wid
    threading.Thread(target=refresher, args=(stop_refresh,), daemon=True).start()
    deadline = time.time() + args.timeout_min * 60

    def done():
        if STATE["status"] in TERMINAL:
            return True
        if time.time() > deadline:
            print(f"!! overall timeout ({args.timeout_min} min)")
            return True
        if time.time() - STATE["last"] > args.idle_min * 60:
            print(f"!! wire idle for {args.idle_min} min (status={STATE['status']})")
            return True
        return False

    pump(timeout=args.timeout_min * 60 + 60, stop=done)
finally:
    stop_refresh.set()
    cleanup()

# --- summary ----------------------------------------------------------------

elapsed = time.time() - T0[0]
for _sid, _st in SESS.items():
    _st["model"] = MODEL_OF.get(_sid)
rows = sorted(SESS.values(), key=lambda s: s["start"])
print(f"\n== status={STATE['status']}  elapsed={elapsed / 60:.1f} min  step sessions={len(rows)}  "
      f"permissions={dict(PERMS)}")
print(f"{'node':44s} {'agent':12s} {'model':18s} {'sec':>5s} {'peak ctx%':>9s} {'tools':>5s} {'status'}")
for s in rows:
    dur = (s.get("end") or time.time()) - s["start"]
    print(f"{s['node']:44s} {str(s['agent']):12s} {str(s.get('model')):18s} {dur:5.0f} "
          f"{s['peak_ctx']:9.1f} {s['tools']:5d} {s.get('status', '?')}")
if rows:
    peaks = [s["peak_ctx"] for s in rows]
    print(f"peak context per step session: max {max(peaks):.1f}%  mean {sum(peaks) / len(peaks):.1f}%")
summary = {"workflowId": STATE.get("wid"), "status": STATE["status"], "elapsedSec": round(elapsed),
           "target": TARGET, "scope": SCOPE, "sessions": rows, "permissions": dict(PERMS),
           "denied": DENIED, "trace": TRACE}
if os.path.isdir(RUNDIR):
    with open(os.path.join(RUNDIR, "_driver-summary.json"), "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=2)
for name in ("report.md", "findings.json", "comments.md"):
    path = os.path.join(RUNDIR, name)
    print(f"== {name}: {path if os.path.exists(path) else 'NOT WRITTEN'}")
if STATE["status"] not in TERMINAL:
    print(f"== run is not terminal. Retry/resume with: --retry {STATE.get('wid')}  (or --resume)")
if STATE["status"] != "completed" and args.auto_recover > 0 and STATE.get("wid"):
    if read_token() is None:
        print("== not auto-recovering: no stored token (logged out). Run `kiro-cli login`, then --retry.")
    else:
        # A failed run is terminal -> retry; anything else rehydrates as paused -> resume.
        verb = "--retry" if STATE["status"] == "failed" else "--resume"
        argv = [sys.executable, os.path.abspath(__file__), "--workspace", WS, "--rundir", RUNDIR, verb, STATE["wid"],
                "--kiro", args.kiro, "--auto-recover", str(args.auto_recover - 1), "--idle-min", str(args.idle_min),
                "--timeout-min", str(args.timeout_min), "--kiro-setting-at", args.kiro_setting_at,
                "--target", TARGET, "--scope", SCOPE]
        if args.direct:
            argv.append("--direct")
        for k in args.kiro_setting:
            argv += ["--kiro-setting", k]
        for c in args.allow_cap:
            argv += ["--allow-cap", c]
        print(f"\n== AUTO-RECOVER ({args.auto_recover} left): {verb} {STATE['wid']} in a fresh server\n", flush=True)
        time.sleep(20)
        if IS_WINDOWS:
            # execv is not a real exec on Windows (it returns to the caller at once), so wait on a
            # child. Ctrl+C reaches it too (same console): give its own cleanup time to taskkill
            # its KAS tree instead of killing it mid-cleanup, and take the tree down only if it hangs.
            child = subprocess.Popen(argv)
            try:
                sys.exit(child.wait())
            except KeyboardInterrupt:
                print("== waiting up to 30 s for the recovered run to clean up ...", flush=True)
                try:
                    sys.exit(child.wait(timeout=30))
                except (KeyboardInterrupt, subprocess.TimeoutExpired):
                    subprocess.run(["taskkill", "/T", "/F", "/PID", str(child.pid)], capture_output=True)
                    sys.exit(1)
        # POSIX: replace this process, so a SIGTERM reaches the process that owns the new
        # KAS server and its `finally: cleanup()` runs. A child would be SIGKILLed by
        # subprocess.call on our way out and leave its server running.
        sys.stdout.flush()
        os.execv(sys.executable, argv)
sys.exit(0 if STATE["status"] == "completed" else 1)
