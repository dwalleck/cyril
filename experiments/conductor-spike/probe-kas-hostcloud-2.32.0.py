#!/usr/bin/env python3
"""HOST leads (2.32.0 live lane): `kiro-cli chat --no-interactive --agent-engine v3` cloud/repo gates, flag validation,
closed-stdout exit and SIGINT text. Archived 2.32.0 bin/ FIRST on PATH (trap 8); fresh temp HOME + real XDG_DATA_HOME (trap 7);
every descendant's /proc/<pid>/exe + argv is recorded (polled every 0.1 s).

  PROBE_SCRATCH=<dir> python3 -I probe-kas-hostcloud-2.32.0.py <arm[,arm...]> <out.jsonl>
arms: listmodels | cloud | cloud_resume | cloud_sandbox | repo | stdout_closed | stdout_file | sigint
"""
import json, os, signal, subprocess, sys, tempfile, threading, time
ARMS = sys.argv[1].split(","); OUT = open(sys.argv[2], "a", buffering=1)
SCRATCH = os.environ["PROBE_SCRATCH"]
BIN = os.path.expanduser("~/.local/share/kiro-research/binaries/2.32.0/extracted/kirocli/bin")
REDACT = ("accesstoken", "refreshtoken", "idtoken", "profilearn", "authorization", "expiresat")
def scrub(o, k=""):
    if str(k).lower() in REDACT: return "<REDACTED>"
    if isinstance(o, dict): return {kk: scrub(v, kk) for kk, v in o.items()}
    if isinstance(o, list): return [scrub(v, k) for v in o]
    return o
def emit(r): OUT.write(json.dumps(scrub(r)) + "\n")
def descendants(pid):
    out = {}
    try:
        kids = subprocess.run(["pgrep", "-P", str(pid)], capture_output=True, text=True).stdout.split()
    except Exception:
        return out
    for k in kids:
        try:
            out[k] = {"exe": os.readlink(f"/proc/{k}/exe"),
                      "argv": [a.decode(errors="replace") for a in open(f"/proc/{k}/cmdline", "rb").read().split(b"\0") if a][:8]}
        except OSError:
            continue
        out.update(descendants(int(k)))
    return out
def run(arm, args, shell_pipe=None, sigint_after=None, timeout=240):
    home = tempfile.mkdtemp(prefix=f"host-{arm}-home-", dir=SCRATCH)
    ws = tempfile.mkdtemp(prefix=f"host-{arm}-ws-", dir=SCRATCH)
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=ws)
    env = dict(os.environ, HOME=home, XDG_DATA_HOME=os.path.expanduser("~/.local/share"), PATH=BIN + ":" + os.environ["PATH"])
    env.pop("KIRO_KAS_SERVER_PATH", None); env.pop("KIRO_HOME", None)
    argv = [os.path.join(BIN, "kiro-cli")] + args
    seen = {}
    t0 = time.time()
    if shell_pipe:
        cmd = " ".join("'" + a.replace("'", "'\\''") + "'" for a in argv) + shell_pipe + "; echo \"PIPESTATUS=${PIPESTATUS[*]}\" >&2"
        p = subprocess.Popen(["bash", "-c", cmd], cwd=ws, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
    else:
        p = subprocess.Popen(argv, cwd=ws, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
    done = threading.Event()
    def poll():
        while not done.is_set():
            seen.update(descendants(p.pid)); time.sleep(0.1)
    threading.Thread(target=poll, daemon=True).start()
    if sigint_after:
        def fire():
            time.sleep(sigint_after)
            if p.poll() is None:
                emit({"arm": arm, "event": "SIGINT", "at_s": round(time.time() - t0, 1)})
                tgt = p.pid
                if shell_pipe is None: os.killpg(tgt, signal.SIGINT)  # terminal Ctrl+C hits the whole foreground group
        threading.Thread(target=fire, daemon=True).start()
    try:
        so, se = p.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(os.getpgid(p.pid), signal.SIGKILL); so, se = p.communicate()
    done.set()
    rec = {"arm": arm, "argv": argv[1:], "pipe": shell_pipe, "rc": p.returncode, "secs": round(time.time() - t0, 1),
           "stderr": se[-4000:], "stdout_lines": len(so.splitlines()), "children": seen,
           "kas_child_spawned": any("acp-server.js" in " ".join(v["argv"]) for v in seen.values())}
    frames = []
    for line in so.splitlines():
        try: frames.append(json.loads(line))
        except Exception: frames.append({"raw": line[:2000]})
    rec["stdout_head"] = so[:3000]
    emit(rec)
    for i, f in enumerate(frames):
        emit({"arm": arm, "frame": i, "data": f})
    print(json.dumps({k: rec[k] for k in ("arm", "rc", "secs", "kas_child_spawned")}), "|", se.strip().splitlines()[-3:] if se.strip() else "", flush=True)
V3 = ["chat", "--no-interactive", "--agent-engine", "v3"]
for arm in ARMS:
    if arm == "listmodels": run(arm, ["chat", "--list-models", "--agent-engine", "v3"])
    elif arm == "cloud": run(arm, V3 + ["--cloud", "--output-format", "stream-json", "reply ok"])
    elif arm == "cloud_resume": run(arm, V3 + ["--cloud", "--resume-id", "00000000-0000-0000-0000-000000000000", "x"])
    elif arm == "cloud_sandbox": run(arm, V3 + ["--cloud", "--sandbox", "x"])
    elif arm == "repo": run(arm, V3 + ["--cloud", "--repo", "owner/name", "--output-format", "stream-json", "x"])
    elif arm == "stdout_closed": run(arm, V3 + ["reply with exactly ok"], shell_pipe=" | head -c 0")
    elif arm == "stdout_file": run(arm, V3 + ["reply with exactly ok"], shell_pipe=" > out.txt")
    elif arm == "sigint": run(arm, V3 + ["Count slowly from 1 to 200, one number per line, with a short sentence about each number."], sigint_after=8)
