#!/usr/bin/env python3
"""What IS the dark-launched v2 `/voice` ACP command? — 2.16.0 audit follow-up.

The KIRO_TEST_MODE sweep in the 2.16.0 audit found `/voice` advertised as a
25th command in `_kiro.dev/commands/available` ("Voice input mode for
hands-free interaction", subcommands start|stop|status) — identically on
2.15.0, so pre-existing, and it was parked without executing it. This probe
closes that thread:

  Phase A (KIRO_TEST_MODE=true):  verify /voice advertised, then execute
    voice status / start / stop through `kiro.dev/commands/execute`, trying
    several args shapes (the subcommand key is undocumented; serde error
    messages will teach us the expected shape). After `start`, pump 10s and
    record EVERY notification — looking for a transcription stream — and
    check whether a `kiro-cli voice` subprocess appears.
  Phase B (test mode unset):      execute voice status again — is the gate
    advertise-only or enforced at execute time?

No prompt turn, zero credits. HOME-isolated per
feedback_isolate_kiro_probes_with_home (real XDG_DATA_HOME keeps v2 auth).

    probe-v2-voice-acp-2.16.0.py <path-to-kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")


class Engine:
    def __init__(self, label, test_mode):
        self.label = label
        cwd = tempfile.mkdtemp(prefix=f"v2voice-{label}-")
        subprocess.run("git init -q -b main", cwd=cwd, shell=True)
        env = dict(os.environ)
        env["HOME"] = tempfile.mkdtemp(prefix=f"v2voicehome-{label}-")
        env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
        if test_mode:
            env["KIRO_TEST_MODE"] = "true"
        else:
            env.pop("KIRO_TEST_MODE", None)
        self.stderr = open(f"{OUT.name}.{label}.stderr", "w")
        self.p = subprocess.Popen([KIRO, "acp"], cwd=cwd, env=env,
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=self.stderr, text=True, bufsize=1)
        self.q = queue.Queue()
        threading.Thread(target=lambda: [self.q.put(l.strip())
                                         for l in self.p.stdout if l.strip()],
                         daemon=True).start()
        self.i = 0
        self.cwd = cwd

    def req(self, m, pr):
        self.i += 1
        self.p.stdin.write(json.dumps(
            {"jsonrpc": "2.0", "id": self.i, "method": m, "params": pr}) + "\n")
        self.p.stdin.flush()
        return self.i

    def log(self, raw):
        OUT.write(json.dumps({"engine": self.label, "line": raw}) + "\n")
        OUT.flush()

    def pump(self, until, to=30):
        end = time.time() + to
        while time.time() < end:
            try:
                raw = self.q.get(timeout=2)
            except queue.Empty:
                continue
            try:
                o = json.loads(raw)
            except Exception:
                continue
            self.log(raw)
            if o.get("id") is not None and o.get("method"):
                self.p.stdin.write(json.dumps(
                    {"jsonrpc": "2.0", "id": o["id"], "result": {}}) + "\n")
                self.p.stdin.flush()
                continue
            if o.get("id") == until and ("result" in o or "error" in o):
                return o
        return None

    def execute(self, command_obj, to=30):
        rid = self.req("_kiro.dev/commands/execute",
                       {"sessionId": self.sid, "command": command_obj})
        return self.pump(rid, to)

    def start_session(self):
        self.req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
        self.pump(1, 20)
        nid = self.req("session/new", {"cwd": self.cwd, "mcpServers": []})
        sess = self.pump(nid, 40)
        self.sid = (sess or {}).get("result", {}).get("sessionId")
        self.pump(-1, 5)  # drain commands/available etc.
        return self.sid

    def close(self):
        try:
            self.p.stdin.close()
        except Exception:
            pass
        self.p.terminate()
        self.stderr.close()


def brief(r):
    if r is None:
        return "TIMEOUT"
    if "error" in r:
        e = r["error"]
        return f"ERR {e.get('code')}: {str(e.get('message'))[:160]}"
    return "ok: " + json.dumps(r.get("result"))[:200]


SHAPES = [
    ("bare", {"command": "voice", "args": {}}),
    ("sub-status", {"command": "voice", "args": {"subcommand": "status"}}),
    ("value-status", {"command": "voice", "args": {"value": "status"}}),
]

print("=== Phase A: KIRO_TEST_MODE=true ===")
a = Engine("testmode", test_mode=True)
sid = a.start_session()
print("sessionId:", sid)

for label, shape in SHAPES:
    r = a.execute(shape)
    print(f"  {label}: {brief(r)}")

# whichever shape worked (or bare), try start → watch → stop
for sub in ("start", "stop"):
    r = a.execute({"command": "voice", "args": {"subcommand": sub}}, to=15)
    print(f"  start/stop [{sub}]: {brief(r)}")
    if sub == "start":
        # look for a voice subprocess + stream notifications for 10s
        time.sleep(2)
        ps = subprocess.run("pgrep -af 'kiro.*voice' || true", shell=True,
                            capture_output=True, text=True).stdout.strip()
        print(f"  voice subprocess: {ps or 'none'}")
        a.pump(-1, 10)  # record any notification traffic
a.close()

print("=== Phase B: no test mode ===")
b = Engine("notest", test_mode=False)
sid = b.start_session()
print("sessionId:", sid)
r = b.execute({"command": "voice", "args": {"subcommand": "status"}})
print(f"  voice status without flag: {brief(r)}")
b.close()
OUT.close()
