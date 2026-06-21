#!/usr/bin/env python3
"""
prove-it probe (cyril-7z7u): does a steer QUEUED MID-TURN that turn 1 ends
WITHOUT consuming (turn 1 = long, NO tool boundary) DRAIN on the next turn, or
get DROPPED at turn-end? Raw ACP against `kiro-cli acp` (v2) — deliberately
bypasses cyril's bridge to isolate kiro's own backend behavior.

Turn 1: long NO-TOOL prompt -> on the first agent chunk (turn is in flight),
        fire `_session/steer` (raw wire form) with a marker. No tool boundary
        in turn 1 -> the steer cannot be consumed there, so it ends queued.
Turn 2: a TOOL prompt (guarantees a tool boundary). Observe whether
        steering_consumed fires + whether the model honored it (ZEBRA marker).

VERDICT: steering_consumed after turn-1-end -> SURVIVES (the chip must NOT reset
at TurnCompleted); never -> DROPPED (reset is fine, just finalize the echo).
"""
import json, os, subprocess, threading, queue, time, tempfile

KIRO = "kiro-cli"
CWD = tempfile.mkdtemp(prefix="cyril-7z7u-")
LOG = os.path.join(os.path.dirname(os.path.abspath(__file__)), "probe-cross-turn-steer.log")
logf = open(LOG, "w")
def log(*a):
    s = " ".join(str(x) for x in a); print(s); logf.write(s + "\n"); logf.flush()

STEER_MSG = "STEER-PROBE-MARKER: from now on, end every reply with the word ZEBRA."

env = dict(os.environ, KIRO_LOG_LEVEL="debug")  # richer kiro-chat.log for the oracle
proc = subprocess.Popen([KIRO, "acp"], cwd=CWD, stdin=subprocess.PIPE,
                        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1, env=env)
PIN, POUT = proc.stdin, proc.stdout
assert PIN and POUT
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()

_id = [0]
def req(method, params):
    _id[0] += 1
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": _id[0], "method": method, "params": params}) + "\n"); PIN.flush()
    return _id[0]
def reply(rid, res):
    PIN.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n"); PIN.flush()

steer_events = []          # (turn, variant)
agent_text = {"1": "", "2": ""}

def pump_turn(turn, until_id, on_first_chunk=None, timeout=180):
    fired = [False]
    end = time.time() + timeout
    while time.time() < end:
        try:
            raw = msgs.get(timeout=2)
        except queue.Empty:
            continue
        if raw is None:
            return None
        try:
            o = json.loads(raw)
        except Exception:
            continue
        if "method" in o and "id" in o:                    # server->client request
            if o["method"] == "session/request_permission":
                opts = o["params"].get("options", [])
                pick = next((x for x in opts if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                            opts[0] if opts else None)
                reply(o["id"], {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                      if pick else {"outcome": {"outcome": "cancelled"}})
            else:
                reply(o["id"], {})
            continue
        if "method" in o and "id" not in o:                # notification
            u = o.get("params", {}).get("update", {}) or {}
            kind = u.get("sessionUpdate")
            if kind in ("steering_queued", "steering_consumed", "steering_cleared"):
                steer_events.append((turn, kind))
                log(f"  [turn {turn}] {kind}: {u.get('message') or u.get('content')}")
            elif kind == "agent_message_chunk" and turn in agent_text:
                c = u.get("content")
                agent_text[turn] += c.get("text", "") if isinstance(c, dict) else (c or "")
                if on_first_chunk and not fired[0]:
                    fired[0] = True; on_first_chunk()
            continue
        if "id" in o and o.get("id") == until_id:          # response to our request
            return o
    log(f"  [turn {turn}] TIMEOUT waiting id={until_id}")
    return None

try:
    req("initialize", {"protocolVersion": 1, "clientCapabilities": {}, "clientInfo": {"name": "cyril-7z7u-probe", "version": "0"}})
    pump_turn("init", 1, timeout=30)
    nid = req("session/new", {"cwd": CWD, "mcpServers": []})
    nr = pump_turn("new", nid, timeout=30)
    assert nr and "result" in nr, f"session/new failed: {nr}"
    SID = nr["result"]["sessionId"]; log("sessionId:", SID)

    log("\n# TURN 1 (long, no-tool) — steer fired on first chunk (mid-turn)")
    def fire_steer():
        sid = req("_session/steer", {"message": STEER_MSG, "sessionId": SID})
        log(f"  -> sent _session/steer (id={sid}) mid-turn-1")
    turn1_prompt = os.environ.get("TURN1", "Write about 300 words on the history of the bicycle. Do not use any tools.")
    log(f"  turn-1 prompt: {turn1_prompt!r}")
    p1 = req("session/prompt", {"sessionId": SID, "prompt": [{"type": "text", "text": turn1_prompt}]})
    r1 = pump_turn("1", p1, on_first_chunk=fire_steer, timeout=180)
    log(f"  turn 1 stopReason: {r1.get('result', {}).get('stopReason') if r1 and 'result' in r1 else r1}")

    log("\n# TURN 2 (tool boundary) — does the queued steer drain?")
    p2 = req("session/prompt", {"sessionId": SID, "prompt": [{"type": "text", "text": "Use your tools to list the files in the current working directory, then reply."}]})
    r2 = pump_turn("2", p2, timeout=180)
    log(f"  turn 2 stopReason: {r2.get('result', {}).get('stopReason') if r2 and 'result' in r2 else r2}")
finally:
    try:
        PIN.close()
    except Exception:
        pass
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except Exception:
        proc.kill()

log("\n" + "=" * 60 + "\nVERDICT\n" + "=" * 60)
t1 = [k for (t, k) in steer_events if t == "1"]
t2 = [k for (t, k) in steer_events if t == "2"]
log(f"turn-1 steering events : {t1}")
log(f"turn-2 steering events : {t2}")
log(f"queued mid-turn-1, consumed IN turn-1 : {'steering_consumed' in t1}")
log(f"SURVIVES turn-end (consumed in turn 2): {'steering_consumed' in t2}")
log(f"corroboration — turn-2 reply contains ZEBRA: {'ZEBRA' in agent_text['2'].upper()}")
log(f"\n# cwd: {CWD}\n# log: {LOG}")
