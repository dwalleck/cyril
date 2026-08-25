#!/usr/bin/env python3
"""
`_message/send` (v2 engine) param-shape A/B, 2.19.1 vs 2.19.2 — follow-up to
probe-v2-ext-methods-ab-2.19.2.py, which found 2.19.2 rejects the 2.19.1-era
`{sessionId, message}` body with -32700 "missing field `content`".

Sends four candidate shapes to a BOGUS session id (no model call, no cost) on
each binary and records the response class, so the accepted 2.19.2 shape and
the 2.19.1 backward-compat story are both pinned:
  A {sessionId, message: "probe"}                        (old cyril-era shape)
  B {sessionId, content: "probe"}                        (string content)
  C {sessionId, content: [{type:"text", text:"probe"}]}  (ACP content blocks)
  D {sessionId, content: {type:"text", text:"probe"}}    (single block)
Then repeats B/C against the REAL session to see whether a real target changes
the response (still no prompt turn; the injected message just queues).

    probe-v2-message-send-shape-ab-2.19.2.py <kiro-cli-chat-OLD> <kiro-cli-chat-NEW>
HOME-isolated; real XDG_DATA_HOME.
"""
import json, os, queue, subprocess, sys, tempfile, threading, time

def run(binpath, label):
    CWD = tempfile.mkdtemp(prefix=f"msgsend-{label}-")
    env = dict(os.environ)
    env["HOME"] = tempfile.mkdtemp(prefix=f"msgsendhome-{label}-")
    env["XDG_DATA_HOME"] = os.path.expanduser("~/.local/share")
    p = subprocess.Popen([binpath, "acp", "--agent-engine", "v2"], cwd=CWD, env=env,
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                         text=True, bufsize=1)
    q = queue.Queue()
    threading.Thread(target=lambda: ([q.put(l.strip()) for l in p.stdout if l.strip()], q.put(None)), daemon=True).start()
    ids = [0]

    def send(m):
        try:
            p.stdin.write(json.dumps(m) + "\n"); p.stdin.flush()
        except BrokenPipeError:
            pass

    def req(method, params):
        ids[0] += 1
        send({"jsonrpc": "2.0", "id": ids[0], "method": method, "params": params}); return ids[0]

    def pump(until, to=60):
        end = time.time() + to
        while time.time() < end:
            try:
                raw = q.get(timeout=1)
            except queue.Empty:
                continue
            if raw is None:
                return "DIED"
            try:
                m = json.loads(raw)
            except Exception:
                continue
            if "method" in m and "id" in m:
                send({"jsonrpc": "2.0", "id": m["id"], "result": {}})
            elif m.get("id") == until:
                return m
        return None

    def classify(r):
        if r == "DIED" or p.poll() is not None:
            return "AGENT DIED"
        if r is None:
            return "TIMEOUT"
        if "error" in r:
            e = r["error"]
            return f"ERROR {e.get('code')} {str(e.get('message'))[:40]!r} data={json.dumps(e.get('data'))[:140]}"
        return "RESULT " + json.dumps(r.get("result"))[:120]

    print(f"\n######## {label}: {binpath}")
    pump(req("initialize", {"protocolVersion": 1, "clientInfo": {"name": "cyril-audit-probe", "version": "0.0.1"}, "clientCapabilities": {}}))
    new = pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 90)
    sid = ((new or {}).get("result") or {}).get("sessionId") if isinstance(new, dict) else None
    bogus = "00000000-0000-4000-8000-00000000dead"
    shapes = [
        ("A message:str      ", lambda s: {"sessionId": s, "message": "probe"}),
        ("B content:str      ", lambda s: {"sessionId": s, "content": "probe"}),
        ("C content:[block]  ", lambda s: {"sessionId": s, "content": [{"type": "text", "text": "probe"}]}),
        ("D content:block    ", lambda s: {"sessionId": s, "content": {"type": "text", "text": "probe"}}),
    ]
    for target_label, target in (("bogus", bogus), ("real", sid)):
        for name, mk in shapes:
            if target_label == "real" and name.startswith(("A", "D")):
                continue
            r = pump(req("_message/send", mk(target)), 30)
            print(f"  {target_label:5} {name} -> {classify(r)}")
            if p.poll() is not None:
                print("     agent died; stopping this binary")
                return
    p.terminate()

run(sys.argv[1], "old-2.19.1")
run(sys.argv[2], "new-2.19.2")
