#!/usr/bin/env python3
"""Same-day A/B capture of the v2 (default Rust engine) ACP surface for the
2.11.0 audit: initialize + session/new responses and every notification that
arrives through session settle (commands/available, modes, etc.), dumped raw
as JSONL for structural field diffing. Run once per binary:

    probe-v2-surface-ab-2.11.0.py <path-to-kiro-cli-chat> <out.jsonl>

Compare runs with the inline differ in the 2.11.0 audit notes (field-path set
diff over init/session-new results + commands/available name sets).
Derived from probe-v2-surface-2.8.0.py. v2 self-authenticates (no _kiro/auth)."""
import json, subprocess, threading, queue, time, tempfile, sys

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
CWD = tempfile.mkdtemp(prefix="v2surfab-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
p = subprocess.Popen([KIRO, "acp"], cwd=CWD, stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]

def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]

def rep(rid, res):
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    p.stdin.flush()

CMDS = set(); TOOLS = set()

def handle(o):
    m = o.get("method"); rid = o.get("id"); pr = o.get("params", {}) or {}
    if rid is not None and m:
        rep(rid, {})
        return
    if m and "commands/available" in m:
        for c in (pr.get("commands") or []):
            n = c.get("name") if isinstance(c, dict) else c
            if n: CMDS.add(n.lstrip("/"))
        for t in (pr.get("tools") or []):
            n = t.get("name") if isinstance(t, dict) else t
            if n: TOOLS.add(n)

def pump(until, to=40):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = q.get(timeout=2)
        except queue.Empty:
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        OUT.write(raw + "\n")
        if "method" in o:
            handle(o)
        if o.get("id") == until and "result" in o:
            return o

req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
pump(nid, 40)
pump(-1, 6)  # drain post-session notifications (commands/available et al.)
print("COMMANDS(%d):" % len(CMDS), " ".join(sorted(CMDS)))
print("TOOLS(%d):" % len(TOOLS), " ".join(sorted(TOOLS)))
OUT.close()
p.stdin.close()
p.terminate()
