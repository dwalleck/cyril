#!/usr/bin/env python3
"""cyril-ild0 live smoke: drive the REAL cyril binary through `/review` on a
pty and record what the operator would see.

    python3 -m venv venv && venv/bin/pip install pyte
    venv/bin/python smoke-review.py <cyril> <repo> <home> <out> \
        [--until launch|end] [--minutes 80] [--command "/review ..."]

<home> becomes $HOME for cyril (and so for KAS): the review assets install
there and the real ~/.kiro is untouched; XDG_DATA_HOME stays the real one so
kiro-cli's credential store is found. <repo> must be a git repository root.
All four paths may be relative; they are resolved before cyril starts.

The run refuses to start unless the stored Kiro token outlives --minutes: it
only reads the expiry (as .cyril-0qe6/live-sweep.py does) and never renews —
a second renewer racing kiro-cli's own can log the user out.
--skip-token-check skips that read when the login was confirmed another way.

--action drives one interruption: esc-during-check (Esc while "running check…"
is the latest review line; cancelled.txt and processes.txt, cyril's session
right after), cancel-after-launch (`/review cancel` 20 s after the run starts;
cancelled.txt, processes.txt), kill-after-launch (SIGKILL to every process in
cyril's session 60 s after the run starts — a crash, not a quit; killed.txt),
so a later `--command "/review resume"` run can pick the run up, or
chat-after-launch (ask the main agent for a shell command 20 s after the run
starts; chat-approval.txt shows its approval, which must be the main
session's, and the allow-once option is chosen; chat.txt the answer).

Cleanup sends Ctrl+Q, then SIGKILLs every process still in cyril's session:
KAS and step terminals lead their own process groups, so a process-group kill
would leave them running.

--until launch stops once the workflow is invoked (or refused); --until end
waits for this run's "review: run ended" line in cyril.log. A launch that
fails stops both modes. Screens go to <out> as 01-ready.txt, 02-form.txt,
launch.txt / summary.txt, approval.txt (first ordinary permission prompt, if
any) and 04-final.txt, with a timestamped drive.log.
"""
import argparse, fcntl, json, os, pty, select, signal, sqlite3, struct, subprocess, sys
import termios, time
from datetime import datetime, timezone

import pyte

COLS, ROWS = 200, 60
# cyril's own lines for a launch that ends without a run.
LAUNCH_FAILED = ("could not run", "was not created", "did not start", "nothing was started",
                 "nothing to review", "run /review from the repo root",
                 # /review resume refusals
                 "nothing to continue", "is still running — /workflow status", "no active session")
# A launch, or a resume, that started its run.
LAUNCHED = ("is running —", "continues —")
PROMPT_TITLE = " Permission Required "

parser = argparse.ArgumentParser()
for name in ("binary", "repo", "home", "out"):
    parser.add_argument(name)
parser.add_argument("--until", choices=("launch", "end"), default="end")
parser.add_argument("--minutes", type=float, default=80)
parser.add_argument("--command", default="/review")
parser.add_argument("--skip-token-check", action="store_true",
                    help="start without reading the token expiry (the login was checked another way)")
parser.add_argument(
    "--action",
    choices=("none", "esc-during-check", "cancel-after-launch", "kill-after-launch",
             "chat-after-launch"),
    default="none",
    help="esc-during-check: Esc while the check runs; cancel-after-launch: /review cancel "
    "once the run is invoked; kill-after-launch: SIGKILL cyril and KAS once it is invoked; "
    "chat-after-launch: approve one main-session command once while it runs",
)
args = parser.parse_args()
binary, repo, home, out = (os.path.abspath(path) for path in
                           (args.binary, args.repo, args.home, args.out))
os.makedirs(out, exist_ok=True)
log = open(os.path.join(out, "drive.log"), "a")


def note(text):
    log.write(f"[{time.strftime('%H:%M:%S')}] {text}\n")
    log.flush()


def token_seconds_left():
    store = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
    connection = sqlite3.connect(f"file:{store}?mode=ro", uri=True)
    try:
        row = connection.execute(
            "select value from auth_kv where key in "
            "('kirocli:odic:token','kirocli:social:token') order by key desc"
        ).fetchone()
    finally:
        connection.close()
    if row is None:
        return None
    value = row[0].decode() if isinstance(row[0], (bytes, bytearray)) else row[0]
    expires = datetime.fromisoformat(json.loads(value)["expires_at"].replace("Z", "+00:00"))
    return (expires - datetime.now(timezone.utc)).total_seconds()


if args.skip_token_check:
    note("token check skipped")
else:
    left = token_seconds_left()
    if left is None or left < args.minutes * 60:
        shown = "no token" if left is None else f"{round(left / 60)} min left on the token"
        note(f"{shown}; run `kiro-cli login` first (need {args.minutes:g} min)")
        sys.exit(3)

cyril_log = os.path.join(home, ".config", "cyril", "cyril.log")
log_offset = os.path.getsize(cyril_log) if os.path.exists(cyril_log) else 0

master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
env = {**os.environ, "TERM": "xterm-256color", "HOME": home,
       "XDG_DATA_HOME": os.path.expanduser("~/.local/share")}
env.pop("CARGO_TARGET_DIR", None)
proc = subprocess.Popen([binary, "--agent-engine", "kas"], stdin=slave, stdout=slave,
                        stderr=slave, cwd=repo, start_new_session=True, env=env)
os.close(slave)
screen = pyte.Screen(COLS, ROWS)
stream = pyte.ByteStream(screen)


def pump(seconds):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        ready, _, _ = select.select([master], [], [], 0.1)
        if master in ready:
            try:
                data = os.read(master, 65536)
            except OSError:
                return False
            if not data:
                return False
            stream.feed(data)
    return proc.poll() is None


def text():
    return "\n".join(line.rstrip() for line in screen.display)


def snap(name):
    with open(os.path.join(out, name), "w") as f:
        f.write(text())


def send(data):
    os.write(master, data.encode())


def session_processes():
    """Every process in cyril's session (it was started with start_new_session):
    KAS and step terminals lead their own process groups, and an orphan
    reparented away from cyril still keeps the session id."""
    rows = [line.split(None, 2) for line in subprocess.run(
        ["ps", "-e", "-o", "pid=,sid=,args="], capture_output=True, text=True).stdout.splitlines()]
    return [(int(pid), rest[0] if rest else "") for pid, sid, *rest in rows
            if int(sid) == proc.pid]


def processes(name):
    """cyril's session now, so an interruption that leaves a process alive shows."""
    with open(os.path.join(out, name), "w") as f:
        f.writelines(f"{pid} {args}\n" for pid, args in session_processes())


def kill_session():
    for pid, _ in session_processes():
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass


def prompt_options():
    """The approval popup's options, top to bottom, read inside its borders:
    the selected one starts with "▸ ", the rest with two spaces."""
    rows = text().splitlines()
    top = next(n for n, row in enumerate(rows) if PROMPT_TITLE in row)
    left = rows[top].rindex("┌", 0, rows[top].index(PROMPT_TITLE))
    right = rows[top].index("┐", left)
    options = []
    for row in rows[top + 1:]:
        if row[left:left + 1] == "└":
            break
        inner = row[left + 1:right]
        if inner.startswith("▸ ") or (options and inner.startswith("  ") and inner[2:3].strip()):
            options.append(inner[2:].strip())
        elif options:
            break
    return options


def drive():
    for _ in range(120):
        pump(1)
        if "Session created" in text():
            break
    else:
        snap("01-ready.txt")
        note("no session was created within 120 s; stopping")
        return 4
    pump(3)
    snap("01-ready.txt"); note("ready")
    for ch in args.command:
        send(ch); pump(0.05)
    pump(1)
    send("\r")
    pump(15)
    snap("02-form.txt"); note("form shown")
    if " Review " not in text():
        note("form not visible; stopping")
        return 2
    send("\r")
    start = time.monotonic()
    note("confirmed")
    prompted = False
    handle = None
    seen = ""
    try:
        while time.monotonic() - start < args.minutes * 60:
            alive = pump(2)
            screen_text = text()
            if PROMPT_TITLE in screen_text and not prompted:
                prompted = True
                snap("approval.txt")
                note("an ordinary permission prompt appeared during the run")
            review_lines = [line for line in screen_text.splitlines() if "review:" in line]
            if (args.action == "esc-during-check" and review_lines
                    and review_lines[-1].rstrip().endswith("running check…")):
                pump(2); send("\x1b"); pump(15)
                snap("cancelled.txt"); processes("processes.txt"); note("Esc during the check")
                return 0
            failed = next((line for line in LAUNCH_FAILED if line in screen_text), None)
            if failed:
                pump(2); snap("launch.txt"); note(f"launch ended: {failed}")
                return 5
            if any(line in screen_text for line in LAUNCHED):
                if args.action == "cancel-after-launch":
                    pump(20)
                    for ch in "/review cancel":
                        send(ch); pump(0.05)
                    pump(1); send("\r"); pump(30)
                    snap("cancelled.txt"); processes("processes.txt")
                    note("/review cancel after launch")
                    return 0
                if args.action == "chat-after-launch":
                    pump(20)
                    for ch in "Run the shell command `echo cyril-ild0-chat` and tell me its output.":
                        send(ch); pump(0.02)
                    pump(1); send("\r")
                    for _ in range(90):
                        pump(2)
                        if PROMPT_TITLE in text():
                            break
                    else:
                        snap("chat.txt"); note("no approval for the main-session command")
                        return 8
                    snap("chat-approval.txt")
                    # A review step's prompt carries "— <session>" after the title.
                    if f"{PROMPT_TITLE}—" in text():
                        note("the prompt belongs to another session, not the main one")
                        return 9
                    labels = prompt_options()
                    once = next((n for n, line in enumerate(labels) if "once" in line.lower()), None)
                    if once is None:
                        note(f"no allow-once option among {labels!r}")
                        return 10
                    note(f"main-session approval shown during the run; choosing {labels[once].strip()!r}")
                    for _ in range(once):
                        send("\x1b[B"); pump(0.3)
                    send("\r"); pump(30)
                    snap("chat.txt"); note("approved once; chat answered")
                    return 0
                if args.action == "kill-after-launch":
                    pump(60)
                    snap("killed.txt"); kill_session()
                    note("SIGKILL to every process in cyril's session mid-run")
                    return 0
                if args.until == "launch":
                    pump(2); snap("launch.txt"); note("launch ended: running")
                    return 0
            if args.until == "end":
                if handle is None and os.path.exists(cyril_log):
                    handle = open(cyril_log, errors="replace")
                    handle.seek(log_offset)
                if handle is not None:
                    seen += handle.read()
                if "review: run ended" in seen:
                    pump(5); snap("summary.txt"); note("run ended")
                    return 0
            if not alive:
                note("cyril exited")
                return 6
        note("timed out")
        return 7
    finally:
        if handle is not None:
            handle.close()
        note(f"elapsed {round((time.monotonic() - start) / 60, 1)} min")


status = 1
try:
    status = drive()
finally:
    snap("04-final.txt")
    try:
        send("\x11")
        pump(5)
    except OSError:
        pass
    # KAS and step terminals lead their own process groups; kill the session.
    kill_session()
    proc.wait()
sys.exit(status)
