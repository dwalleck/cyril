#!/usr/bin/env python3
"""Same-day A/B of EVERY advertised v2 command response — 2.16.0 audit gap-closure.

probe-v2-surface-ab-2.11.0.py only captures session SETTLE (initialize,
session/new, and the notifications that arrive before the first turn). The
2.16.0 audit then executed exactly one command, `/context`, because the nm diff
pointed there — and found an additive `groups[]` the settle capture could not
see. That leaves 23 other command responses unchecked, all of which cyril's
`format_command_response` parses.

This executes every read-only command on both binaries and structurally diffs
the responses. Mutating / terminal commands are deliberately skipped (see
SKIP below) so the session stays comparable across the run.

No prompt turn, so zero credits.

    probe-v2-all-commands-ab-2.16.0.py <path-to-kiro-cli-chat> <out.jsonl>
"""
import json, subprocess, threading, queue, time, tempfile, sys

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")

# Read-only commands, safe to execute against a scratch session.
CMDS = ["agent", "code", "context", "effort", "goal", "guide", "help", "hooks",
        "knowledge", "mcp", "model", "plan", "prompts", "stats", "tools", "usage"]
# Skipped and why: quit (kills the session), clear/compact/rewind (mutate history),
# paste (needs an image), reply/chat (need a prior turn), feedback (outbound).
SKIP = ["quit", "clear", "compact", "rewind", "paste", "reply", "chat", "feedback"]

CWD = tempfile.mkdtemp(prefix="v2cmdab-")
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


def pump(until, to=45):
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
        OUT.flush()
        if o.get("id") is not None and o.get("method"):
            rep(o["id"], {})
            continue
        if o.get("id") == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 40)
sid = (sess or {}).get("result", {}).get("sessionId")
pump(-1, 5)
print("sessionId:", sid)

results = {}
for c in CMDS:
    rid = req("_kiro.dev/commands/execute",
              {"sessionId": sid, "command": {"command": c, "args": {}}})
    r = pump(rid, 45)
    results[c] = r
    tag = "ERR " if (r or {}).get("error") else "ok  "
    print(f"  {tag}{c}")

# also snapshot the options surface for the selection commands
for c in ("model", "effort", "agent"):
    rid = req("_kiro.dev/commands/options", {"sessionId": sid, "command": c})
    r = pump(rid, 30)
    results[f"options:{c}"] = r
    tag = "ERR " if (r or {}).get("error") else "ok  "
    print(f"  {tag}options:{c}")

print(f"\nskipped (unsafe/stateful): {' '.join(SKIP)}")
OUT.write(json.dumps({"_probe_results": {k: v for k, v in results.items()}}) + "\n")
OUT.close()
p.stdin.close()
p.terminate()
