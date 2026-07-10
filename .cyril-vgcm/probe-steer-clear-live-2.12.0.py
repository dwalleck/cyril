#!/usr/bin/env python3
"""PROBE (prove-it-prototype, cyril-vgcm). Ugly-but-honest, throwaway.

Q1: does the recorded _session/steer -> _session/steer/clear contract (probed
    2026-07-02 on kiro-cli 2.11.0 KAS) hold on the CURRENT binary, 2.12.0,
    through cyril's actual spawn path (`kiro-cli acp --agent-engine kas`)?
    Expect: steer -> {queued:true, messageId:'steer-<uuid>'}; clear ->
    {cleared:true, messageIds:[that id]} + a broadcast steering_cleared frame
    (captured VERBATIM — its envelope shape is load-bearing for convert/kas.rs);
    clear-on-empty -> no-op [].
Q2: what EXACTLY does the v2 engine (`kiro-cli acp`) return for
    _session/steer/clear, given _session/steer itself works there?
    Expect -32601 (binary lacks the literal) — the exact code matters because
    bridge.rs marks the session steering-unsupported on -32601, which would
    poison WORKING steer-append on v2.

Idle steers only (no prompt turn) — the queue lifecycle is the question here,
not model compliance (that was 2026-07-02's behavior probe). No auth-sensitive
turn is issued, so a stale token can't 403 this probe.

Oracles (independent, static): (a) sha256 of acp-server.js 2.12.0 vs archived
2.11.0 extraction — byte-identical means the recorded live contract carries
over; (b) grep of the v2 kiro-cli-chat 2.12.0 binary for the full
'_session/steer/clear' literal — absence predicts -32601.

Usage: probe-steer-clear-live-2.12.0.py [kas|v2]
"""
import json, subprocess, threading, queue, sys, tempfile, time

MODE = sys.argv[1] if len(sys.argv) > 1 else "kas"
CMD = ["kiro-cli", "acp"] + (["--agent-engine", "kas"] if MODE == "kas" else [])
CWD = tempfile.mkdtemp(prefix=f"vgcm-{MODE}-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
proc = subprocess.Popen(CMD, cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()],
                 daemon=True).start()
i, PENDING, FRAMES = [0], {}, []

def req(m, p):
    i[0] += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}) + "\n")
    proc.stdin.flush()
    return i[0]

def drain(deadline):
    while time.time() < deadline:
        try:
            raw = q.get(timeout=0.5)
        except queue.Empty:
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        if o.get("method") and o.get("id") is not None:      # server->client request
            proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": {}}) + "\n")
            proc.stdin.flush()
        elif o.get("method"):                                 # notification
            if "steer" in raw.lower():
                FRAMES.append(o)                              # verbatim, envelope included
        else:
            PENDING[o["id"]] = o

def wait_resp(rid, to=20):
    end = time.time() + to
    while time.time() < end:
        if rid in PENDING:
            return PENDING.pop(rid)
        drain(time.time() + 0.5)
    return None

wait_resp(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 30)
r = wait_resp(req("session/new", {"cwd": CWD, "mcpServers": []}), 60)
SID = (r or {}).get("result", {}).get("sessionId")
print(f"[{MODE}] sessionId: {SID}")

sr = wait_resp(req("_session/steer", {"sessionId": SID, "message": "probe steer, will be cleared"}))
print(f"[{MODE}] steer resp: {json.dumps(sr.get('result', sr.get('error')) if sr else None)}")
cr = wait_resp(req("_session/steer/clear", {"sessionId": SID}))
print(f"[{MODE}] clear resp: {json.dumps(cr.get('result', cr.get('error')) if cr else None)}")
cr2 = wait_resp(req("_session/steer/clear", {"sessionId": SID}))
print(f"[{MODE}] clear-on-empty resp: {json.dumps(cr2.get('result', cr2.get('error')) if cr2 else None)}")
drain(time.time() + 3)  # let trailing broadcasts arrive
for f in FRAMES:
    print(f"[{MODE}] FRAME: {json.dumps(f)}")
proc.stdin.close(); proc.terminate()
