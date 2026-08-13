#!/usr/bin/env python3
"""Windows live probe: /voice over ACP on kiro-cli 2.18.0 — the open question
from cyril's voice-subsystem research.

Linux builds have no voice engine (every ACP invocation returns the structured
"not supported on this platform" refusal), so cyril has NEVER observed a
successful /voice session over ACP. Windows builds compile the full engine
(onnxruntime/Whisper, voice.rs/voice_serve.rs — verified statically on the
2.16.0 MSI), and 2.18.0 lifted the commands/available advertise gate. This
script answers, on a real Windows host:

  Q1  Is /voice advertised in _kiro.dev/commands/available?
  Q2  What does `voice status` return (shape, fields)?
  Q3  Does `voice start` succeed? How does the one-time model download
      surface over ACP (permission request? command response? silence)?
  Q4  Does a voice subprocess appear while recording?
  Q5  What arrives on the wire DURING recording — any notification stream
      (partial transcripts? levels?), or nothing?
  Q6  What does `voice status` say mid-recording?
  Q7  Does `voice stop` return the final transcript in its response?
  Q8  Does transcribed text arrive as any session/update kind afterward?
  Q9  Is the second start/stop cycle (post-download) different/faster?

ZERO CREDITS: no session/prompt is ever sent — voice commands are local.
Audio stays on the machine (local Whisper); nothing is uploaded by this probe.

Usage (PowerShell):
    py -3 probe_v2_voice_win.py [path-to-kiro-cli] [out.jsonl]
    # defaults: "kiro-cli" on PATH, out = v2-voice-win-2.18.0.jsonl

YOU MUST SPEAK during the marked windows — say something distinctive like
"cyril wire probe testing one two three" so the transcript is recognizable.
"""
import json, queue, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1] if len(sys.argv) > 1 else "kiro-cli"
OUTPATH = sys.argv[2] if len(sys.argv) > 2 else "v2-voice-win-2.18.0.jsonl"
OUT = open(OUTPATH, "w", encoding="utf-8")

RECORD_SECONDS = 25          # how long phase C records before `voice stop`
DOWNLOAD_WAIT = 300          # first `voice start` may block on model download


def log_line(tag, obj):
    OUT.write(json.dumps({"tag": tag, "line": obj}) + "\n")
    OUT.flush()


def ps_list_kiro_processes():
    """Command lines of every kiro/voice-ish process (PowerShell CIM query)."""
    try:
        r = subprocess.run(
            ["powershell", "-NoProfile", "-Command",
             "Get-CimInstance Win32_Process | "
             "Where-Object { $_.Name -like '*kiro*' -or $_.CommandLine -like '*voice*' } | "
             "Select-Object ProcessId,CommandLine | ConvertTo-Json -Compress"],
            capture_output=True, text=True, timeout=30)
        return (r.stdout or "").strip() or "(none)"
    except Exception as e:  # PowerShell missing/locked down — report, don't die
        return f"(process listing failed: {e})"


print(f"== kiro binary: {KIRO}")
try:
    v = subprocess.run([KIRO, "--version"], capture_output=True, text=True, timeout=30)
    print("== version:", (v.stdout or v.stderr).strip())
except FileNotFoundError:
    sys.exit(f"FATAL: {KIRO!r} not found. Pass the full path to kiro-cli(.exe).")

# Native subcommand parse check — Linux says "unrecognized subcommand 'voice'";
# the Windows monolithic exe should parse it. Cheap, answers engine presence.
h = subprocess.run([KIRO, "voice", "--help"], capture_output=True, text=True, timeout=30)
print("== `kiro-cli voice --help` exit:", h.returncode)
print((h.stdout or h.stderr)[:600])
log_line("voice-help", {"exit": h.returncode, "out": (h.stdout or h.stderr)[:2000]})

CWD = tempfile.mkdtemp(prefix="v2voicewin-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)   # best-effort, ignore failure

STDERR = open(OUTPATH + ".stderr", "w", encoding="utf-8")
p = subprocess.Popen([KIRO, "acp"], cwd=CWD, stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=STDERR, text=True,
                     encoding="utf-8", bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
NOTIF_KINDS = {}
REQUESTS_SEEN = []           # every server->client request (download confirm?)


def req(m, pr):
    i[0] += 1
    msg = {"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}
    p.stdin.write(json.dumps(msg) + "\n")
    p.stdin.flush()
    log_line("send", msg)
    return i[0]


def pump(until, to=30, tag=""):
    """Drain frames; auto-reply to requests (allow permissions); return response."""
    end = time.time() + to
    while time.time() < end:
        try:
            raw = q.get(timeout=1)
        except queue.Empty:
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        log_line(tag or "recv", o)
        m, rid, pr = o.get("method"), o.get("id"), o.get("params") or {}
        if m and rid is None:
            key = m
            if m.endswith("session/update"):
                u = pr.get("update") or {}
                key = f"{m}:{u.get('sessionUpdate')}"
            NOTIF_KINDS.setdefault(f"{tag}|{key}", 0)
            NOTIF_KINDS[f"{tag}|{key}"] += 1
            continue
        if rid is not None and m:
            # A server->client REQUEST. The model-download confirmation, if it
            # rides ACP at all, should land here — print it loudly, then allow.
            REQUESTS_SEEN.append((tag, m, pr))
            print(f"  >> SERVER REQUEST [{m}]: {json.dumps(pr)[:300]}")
            if m == "session/request_permission":
                opts = pr.get("options", [])
                pick = next((x for x in opts
                             if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                            opts[0] if opts else None)
                res = ({"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                       if pick else {"outcome": {"outcome": "cancelled"}})
            else:
                res = {}
            reply = {"jsonrpc": "2.0", "id": rid, "result": res}
            p.stdin.write(json.dumps(reply) + "\n")
            p.stdin.flush()
            log_line("reply", reply)
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


def brief(r):
    if r is None:
        return "TIMEOUT (no response)"
    if "error" in r:
        e = r["error"]
        return f"ERR {e.get('code')}: {str(e.get('message'))[:200]}"
    return "ok: " + json.dumps(r.get("result"))[:300]


SID = [None]


def execute(args_obj, to=30, tag=""):
    rid = req("_kiro.dev/commands/execute",
              {"sessionId": SID[0], "command": {"command": "voice", "args": args_obj}})
    return pump(rid, to, tag=tag)


ACCEPTED_SHAPE = []          # remembers the first args shape the engine accepts


def voice(sub, to=30, tag=""):
    """Try the known arg shapes until one is accepted; remember the winner."""
    if ACCEPTED_SHAPE:
        return execute(ACCEPTED_SHAPE[0](sub), to, tag=tag)
    for label, mk in [("subcommand", lambda s: {"subcommand": s}),
                      ("value", lambda s: {"value": s}),
                      ("bare", lambda _s: {})]:
        r = execute(mk(sub), to, tag=tag)
        serr = json.dumps(r or {})
        if r is not None and "error" not in r and "unknown" not in serr.lower():
            print(f"  (accepted args shape: {label})")
            ACCEPTED_SHAPE.append(mk)
            return r
        print(f"  shape {label} -> {brief(r)}")
    return None

print("\n== Phase A: handshake ==")
req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump(1, 30, tag="init")
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 60, tag="new")
SID[0] = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", SID[0])
pump(-1, 6, tag="settle")

# Q1 — advertise check, from the recorded commands/available notification
adv = None
OUT.flush()
with open(OUTPATH, encoding="utf-8") as fh:
    for line in fh:
        rec = json.loads(line)
        o = rec.get("line")
        if isinstance(o, dict) and o.get("method") == "_kiro.dev/commands/available":
            for c in (o.get("params") or {}).get("commands", []):
                if "voice" in c.get("name", ""):
                    adv = c
print("Q1 /voice advertised:", json.dumps(adv) if adv else "NOT ADVERTISED")

print("\n== Phase B: voice status ==")
r = voice("status", tag="status")
print("Q2 status:", brief(r))

print("\n== Phase C: voice start — FIRST cycle ==")
print(">>> If a download prompt appears anywhere (this console, a window, the")
print(">>> wire), note it. First start may take minutes (model download).")
t0 = time.time()
r = voice("start", to=DOWNLOAD_WAIT, tag="start1")
print(f"Q3 start (after {time.time() - t0:.0f}s):", brief(r))
print("Q4 processes:", ps_list_kiro_processes())
print(f"\n>>> SPEAK NOW for ~{RECORD_SECONDS}s: 'cyril wire probe testing one two three' <<<")
pump(-1, RECORD_SECONDS, tag="recording1")          # capture ANY stream traffic
r = voice("status", tag="status-mid")
print("Q6 status mid-recording:", brief(r))
r = voice("stop", to=60, tag="stop1")
print("Q7 stop:", brief(r))
pump(-1, 10, tag="post1")                           # late transcript delivery?

print("\n== Phase D: second start/stop cycle (post-download) ==")
t0 = time.time()
r = voice("start", to=60, tag="start2")
print(f"Q9 start#2 (after {time.time() - t0:.0f}s):", brief(r))
print(f">>> SPEAK AGAIN for ~{RECORD_SECONDS}s <<<")
pump(-1, RECORD_SECONDS, tag="recording2")
r = voice("stop", to=60, tag="stop2")
print("   stop#2:", brief(r))
pump(-1, 10, tag="post2")

print("\n== SUMMARY ==")
print("notification kinds per phase:")
for k in sorted(NOTIF_KINDS):
    print(f"  {NOTIF_KINDS[k]:3}x {k}")
print(f"server->client requests seen: {len(REQUESTS_SEEN)}")
for tag, m, pr in REQUESTS_SEEN:
    print(f"  [{tag}] {m}: {json.dumps(pr)[:200]}")
print(f"\nCapture written to {OUTPATH} (+ .stderr). Send both files back.")

OUT.close()
p.stdin.close()
p.terminate()
STDERR.close()
