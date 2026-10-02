#!/usr/bin/env python3
"""cyril-ild0 live smoke: drive the REAL cyril binary through `/review` on a
pty and record what the operator would see.

    python3 -m venv venv && venv/bin/pip install pyte
    venv/bin/python smoke-review.py <cyril> <repo> <home> <out> \
        [--until launch|end] [--minutes 80] [--command "/review ..."]

<home> becomes $HOME for cyril (and so for KAS): the review assets install
there and the real ~/.kiro is untouched; XDG_DATA_HOME stays the real one so
kiro-cli's credential store is found. <repo> must be a git repository root.

--until launch stops once the workflow is invoked (or refused); --until end
waits for cyril.log's "review: run ended" line. Screens are written to <out>
as 01-ready.txt, 02-form.txt, launch.txt / summary.txt and 04-final.txt, with
a timestamped drive.log.
"""
import argparse, fcntl, os, pty, select, struct, subprocess, sys, termios, time
import pyte

COLS, ROWS = 200, 60
parser = argparse.ArgumentParser()
for name in ("binary", "repo", "home", "out"):
    parser.add_argument(name)
parser.add_argument("--until", choices=("launch", "end"), default="end")
parser.add_argument("--minutes", type=float, default=80)
parser.add_argument("--command", default="/review")
args = parser.parse_args()

master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
env = {**os.environ, "TERM": "xterm-256color", "HOME": args.home,
       "XDG_DATA_HOME": os.path.expanduser("~/.local/share")}
env.pop("CARGO_TARGET_DIR", None)
proc = subprocess.Popen([args.binary, "--agent-engine", "kas"], stdin=slave, stdout=slave,
                        stderr=slave, cwd=args.repo, start_new_session=True, env=env)
os.close(slave)
screen = pyte.Screen(COLS, ROWS)
stream = pyte.ByteStream(screen)
log = open(os.path.join(args.out, "drive.log"), "a")


def note(text):
    log.write(f"[{time.strftime('%H:%M:%S')}] {text}\n")
    log.flush()


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
    with open(os.path.join(args.out, name), "w") as f:
        f.write(text())


def send(data):
    os.write(master, data.encode())


for _ in range(120):
    pump(1)
    if "Session created" in text():
        break
pump(3)
snap("01-ready.txt"); note("ready")
for ch in args.command:
    send(ch); pump(0.05)
pump(1)
send("\r")
pump(15)
snap("02-form.txt"); note("form shown")
if "Review" not in text():
    note("form not visible; aborting"); send("\x11"); pump(3); sys.exit(2)
send("\r")
start = time.monotonic()
note("confirmed")
log_path = os.path.join(args.home, ".config", "cyril", "cyril.log")
stops = ("is running —", "could not run", "was not created", "did not start",
         "auth failed", "nothing was started", "nothing to review")
while time.monotonic() - start < args.minutes * 60:
    alive = pump(2)
    screen_text = text()
    if "Allow" in screen_text and "Reject" in screen_text:
        snap("approval.txt"); note("an ordinary approval prompt appeared during the run")
    if args.until == "launch":
        hit = next((stop for stop in stops if stop in screen_text), None)
        if hit:
            pump(2); snap("launch.txt"); note(f"launch ended: {hit}"); break
    else:
        try:
            cyril_log = open(log_path, errors="replace").read()
        except OSError:
            cyril_log = ""
        if "review: run ended" in cyril_log:
            pump(5); snap("summary.txt"); note("run ended"); break
    if not alive:
        note("cyril exited"); break
snap("04-final.txt")
send("\x11"); pump(5)
if proc.poll() is None:
    proc.terminate()
note(f"elapsed {round((time.monotonic() - start) / 60, 1)} min")
