#!/usr/bin/env python3
"""PROBE round 2 (prove-it-prototype, cyril-vgcm). Behavior + busy-turn echo shapes.

Round 1 (probe-steer-clear-live-2.12.0.py) found the issue's premise stale:
v2 2.12.0 ACCEPTS _session/steer/clear, and idle echoes on v2 arrive as
AgentExecutionUserMessageQueued/Cleared (camelCase fields) — NOT the
steering_queued/steering_cleared literals convert/kiro.rs handles. Static
strings dumps date the rename to 2.10.0 (the serde rename table pooled string
present in 2.8.1 is gone in 2.12.0) and show a new AgentExecutionSteeringInjected
variant where steering_consumed used to reconcile the chip.

This probe answers, per engine (outer spawn `kiro-cli acp [--agent-engine kas]`):
  Q5: is clear FUNCTIONAL (steer w/ marker instruction, clear, turn ends
      WITHOUT marker) — with a no-clear control turn where the marker lands?
  Q6: what echo frames (verbatim) does a BUSY-turn steer/consume/clear cycle
      emit on 2.12.0 — i.e. what does "consumed" look like now on each engine?

Turn 1 (clear):   prompt story; sleep; steer "end with <MARKER1>"; clear; await end.
                  Clear-first ordering keeps the queue empty for turn 2.
Turn 2 (control): steer MARKER2, NO clear. A fast turn may end before the steer
                  is consumed (cyril-7z7u: it defers to the next turn) — so:
Turn 3 (drain):   trivial prompt. A deferred MARKER2 steer consumes HERE; its
                  consumption frame is the "consumed" shape 2.12.0 emits.

Usage: probe-steer-clear-behavior-2.12.0.py [kas|v2]
"""
import json, os, sqlite3, subprocess, threading, queue, sys, tempfile, time

MODE = sys.argv[1] if len(sys.argv) > 1 else "v2"

def auth_reply():
    """Mimic cyril's KAS auth responder (kas/auth.rs): sqlite auth_kv token +
    state profile arn -> {accessToken, expiresAt, profileArn}."""
    db = sqlite3.connect(os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3"))
    tok = json.loads(db.execute(
        "SELECT value FROM auth_kv WHERE key='kirocli:odic:token'").fetchone()[0])
    prof = json.loads(db.execute(
        "SELECT value FROM state WHERE key='api.codewhisperer.profile'").fetchone()[0])
    db.close()
    return {"accessToken": tok["access_token"], "expiresAt": tok["expires_at"],
            "profileArn": prof["arn"]}
CMD = ["kiro-cli", "acp"] + (["--agent-engine", "kas"] if MODE == "kas" else [])
CWD = tempfile.mkdtemp(prefix=f"vgcm-beh-{MODE}-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
proc = subprocess.Popen(CMD, cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()],
                 daemon=True).start()
i, PENDING, FRAMES, TEXT = [0], {}, [], []

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
        if o.get("method") and o.get("id") is not None:
            res = auth_reply() if "getAccessToken" in o["method"] else {}
            proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": res}) + "\n")
            proc.stdin.flush()
        elif o.get("method"):
            upd = (o.get("params") or {}).get("update") or {}
            if upd.get("sessionUpdate") == "agent_message_chunk":
                c = upd.get("content") or {}
                if c.get("type") == "text":
                    TEXT.append(c.get("text", ""))
            # steering echoes: match BOTH literal families, exclude file-steering noise
            low = raw.lower()
            if ("steering_" in low or "usermessage" in low or "steeringinjected" in low) \
                    and "documents_changed" not in low and "steering/documents" not in low:
                FRAMES.append(o)
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

def run_turn(label, prompt, marker=None, do_clear=False):
    TEXT.clear(); FRAMES.clear()
    pid = req("session/prompt", {"sessionId": SID, "prompt": [{"type": "text", "text": prompt}]})
    if marker:
        time.sleep(2.5)
        sr = wait_resp(req("_session/steer", {"sessionId": SID,
            "message": f"IMPORTANT: end your reply with the single word {marker}"}), 15)
        print(f"[{label}] steer resp: {json.dumps(sr.get('result', sr.get('error')) if sr else None)}")
        if do_clear:
            cr = wait_resp(req("_session/steer/clear", {"sessionId": SID}), 15)
            print(f"[{label}] clear resp: {json.dumps(cr.get('result', cr.get('error')) if cr else None)}")
    turn = wait_resp(pid, 300)
    stop = (turn or {}).get("result", {}).get("stopReason") if turn and "result" in turn else turn
    text = "".join(TEXT)
    drain(time.time() + 2)  # trailing echoes
    print(f"[{label}] stop: {json.dumps(stop)}; textLen: {len(text)}")
    for f in FRAMES:
        print(f"[{label}] FRAME: {json.dumps(f)}")
    return text

STORY = "Write a 600-word story about a lighthouse keeper. Prose only, no lists."
t1 = run_turn("1 steer+clear", STORY, "STEERMARK_KILO", do_clear=True)
t2 = run_turn("2 steer-only", STORY, "STEERMARK_LIMA", do_clear=False)
t3 = run_turn("3 drain", "Reply with exactly one word: done")
print(f"\n[{MODE}] VERDICT: cleared KILO suppressed everywhere: "
      f"{'STEERMARK_KILO' not in t1 and 'STEERMARK_KILO' not in t2 and 'STEERMARK_KILO' not in t3}")
print(f"[{MODE}] VERDICT: LIMA landed turn2: {'STEERMARK_LIMA' in t2} | deferred to turn3: {'STEERMARK_LIMA' in t3}")
proc.stdin.close(); proc.terminate()
