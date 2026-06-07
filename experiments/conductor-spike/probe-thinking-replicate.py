#!/usr/bin/env python3
"""
Replicate the kiro-tui sequence that produces agent_thought_chunk on the ACP wire,
to confirm the real precondition (Opus active -> effort engaged) and test whether
clientInfo.name matters.

Sequence (mirrors /tmp/trace.jsonl):
  initialize -> settings/list -> session/new
  -> commands/options(model) -> commands/execute(model=opus) -> wait metadata
  -> commands/options(effort) -> commands/execute(effort=high) -> wait metadata.effort
  -> session/prompt -> count agent_thought_chunk

Env: CLIENT_NAME (default kiro-tui), KIRO_BIN, MODEL (default claude-opus-4.8).
"""
import json, os, subprocess, threading, time
from collections import Counter

KIRO = os.path.expanduser(os.environ.get("KIRO_BIN", "~/.local/bin/kiro-cli-chat"))
CWD = "/home/dwalleck/repos/cyril"
NAME = os.environ.get("CLIENT_NAME", "kiro-tui")
MODEL = os.environ.get("MODEL", "claude-opus-4.8")
LOG = f"/tmp/cyril-probe-replicate-{NAME}.log"
PROMPT = ("Think step by step. A snail climbs 3 feet up a 30-foot well each day and "
          "slips back 2 feet each night. On which day does it reach the top? Reason carefully.")


def main():
    log = open(LOG, "w")
    proc = subprocess.Popen([KIRO, "acp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, cwd=CWD)
    inc, lock, nid = [], threading.Lock(), [1]

    def reader():
        while True:
            ln = proc.stdout.readline()
            if not ln:
                return
            t = ln.decode(errors="replace").rstrip("\n")
            log.write(f"S->C {t}\n"); log.flush()
            try:
                with lock:
                    inc.append(json.loads(t))
            except json.JSONDecodeError:
                pass
    threading.Thread(target=reader, daemon=True).start()

    def send(method, params, resp=True):
        m = {"jsonrpc": "2.0", "method": method, "params": params}
        if resp:
            m["id"] = nid[0]; nid[0] += 1
        log.write(f"C->S {json.dumps(m)}\n"); log.flush()
        proc.stdin.write((json.dumps(m) + "\n").encode()); proc.stdin.flush()
        return m.get("id")

    def waitfor(rid, t=15):
        d = time.time() + t
        while time.time() < d:
            with lock:
                for f in inc:
                    if f.get("id") == rid and ("result" in f or "error" in f):
                        return f
            time.sleep(0.05)
        return None

    def latest_metadata():
        with lock:
            for f in reversed(inc):
                if f.get("method") == "_kiro.dev/metadata":
                    return f.get("params", {})
        return {}

    send("initialize", {"protocolVersion": 1, "clientCapabilities": {},
                        "clientInfo": {"name": NAME, "version": "0.0.0-dev"}})
    time.sleep(0.3)
    send("_kiro.dev/settings/list", {})
    time.sleep(0.3)
    rid = send("session/new", {"cwd": CWD, "mcpServers": []})
    sid = waitfor(rid, 20)["result"]["sessionId"]
    print(f"[{NAME}] session {sid}")

    # model: query options then execute, then wait for switch to register
    send("_kiro.dev/commands/options", {"sessionId": sid, "command": "model", "partial": ""})
    time.sleep(0.3)
    mr = waitfor(send("_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": "model", "args": {"value": MODEL}}}), 12)
    print(f"[{NAME}] model switch: {(mr or {}).get('result', {}).get('message')}")

    # effort: query options (should be non-empty now that Opus is active)
    rid = send("_kiro.dev/commands/options", {"sessionId": sid, "command": "effort", "partial": ""})
    r = waitfor(rid, 10)
    eff_opts = (r or {}).get("result", [])
    print(f"[{NAME}] effort options: {json.dumps(eff_opts)[:200]}")
    er = waitfor(send("_kiro.dev/commands/execute", {"sessionId": sid, "command": {"command": "effort", "args": {"value": "high"}}}), 12)
    print(f"[{NAME}] effort set: {(er or {}).get('result', {}).get('message')}")
    time.sleep(1.0)
    print(f"[{NAME}] metadata after effort set: {json.dumps(latest_metadata())[:200]}")

    with lock:
        cutoff = len(inc)
    rid = send("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
    r = waitfor(rid, 120)
    print(f"[{NAME}] stop={ (r or {}).get('result',{}).get('stopReason') }")
    time.sleep(0.5)

    with lock:
        turn = inc[cutoff:]
    variants = Counter()
    for f in turn:
        v = f.get("params", {}).get("update", {}).get("sessionUpdate")
        if v:
            variants[v] += 1
    print(f"[{NAME}] >>> sessionUpdate variants this turn: {dict(variants)}")
    print(f"[{NAME}] >>> agent_thought_chunk count: {variants.get('agent_thought_chunk', 0)}")
    proc.terminate()


if __name__ == "__main__":
    main()
