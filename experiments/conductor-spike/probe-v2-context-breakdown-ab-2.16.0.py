#!/usr/bin/env python3
"""Same-day A/B of the v2 `/context` command response for the 2.16.0 audit.

2.16.0's changelog adds "per-tool token breakdown in /context, grouped by
source (built-in, MCP server, agent)". The nm diff shows three NEW types under
`chat_cli_v2::agent::acp::commands::context` (ToolBreakdownItem,
ToolCategoryBreakdown, ToolGroupBreakdown) — i.e. on the ACP commands path
cyril drives, not just the TUI. The session-settle A/B cannot see this because
the shape only materialises when `/context` is actually executed.

    probe-v2-context-breakdown-ab-2.16.0.py <path-to-kiro-cli-chat> <out.jsonl>

Executes `kiro.dev/commands/execute` for `context` as a JSON-RPC *request*
(with id) and awaits the response — per the project rule that commands/execute
must be a request, not a notification. No prompt turn, so zero credits.
"""
import json, subprocess, threading, queue, time, tempfile, sys

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")
CWD = tempfile.mkdtemp(prefix="v2ctxab-")
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
        # answer any server->client request so the agent never blocks
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
pump(-1, 5)  # drain settle notifications

sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid)

# /context — the adjacently-tagged TuiCommand object form
cid = req("_kiro.dev/commands/execute",
          {"sessionId": sid, "command": {"command": "context", "args": {}}})
resp = pump(cid, 60)
print("CONTEXT RESPONSE:", json.dumps(resp, indent=2)[:4000] if resp else "NONE")

OUT.close()
p.stdin.close()
p.terminate()
