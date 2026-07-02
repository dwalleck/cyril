#!/usr/bin/env python3
"""Behavioral probe for _session/steer/clear (@kiro/agent 0.8.0, kiro-cli 2.11.0).

Design (falsifiable, with control):
  Turn A (test):    prompt a short story; while busy, _session/steer an
                    instruction to append the marker word STEERMARK_ALPHA,
                    then immediately _session/steer/clear. Expect:
                    clear returns the steer's messageId, a session_info_update
                    kind=steering_cleared arrives, and the final text does NOT
                    contain the marker.
  Turn B (control): same, marker STEERMARK_BRAVO, NO clear. Expect the marker
                    (or at least evidence the steer reached the model).

Direct-spawn free path (default file auth via ~/.aws/sso/cache/kiro-auth-token.json —
run only while that token is fresh). Usage: <script> <path-to-acp-server.js>"""
import json, os, subprocess, threading, queue, time, tempfile, sys

SERVER = sys.argv[1]
runtime = os.environ.get("KIRO_AGENT_PATH", "node")
CWD = tempfile.mkdtemp(prefix="kas-steerbeh-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
proc = subprocess.Popen([runtime, "--experimental-wasm-modules", SERVER, "--transport=stdio"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]
PENDING = {}   # id -> response
EVENTS = []    # (tag, payload) in arrival order
TEXT = []      # agent_message_chunk text for the current turn

def req(m, p):
    i[0] += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}) + "\n")
    proc.stdin.flush()
    return i[0]

def rep(rid, res):
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    proc.stdin.flush()

def drain(deadline):
    """Process inbound until deadline; returns on deadline only."""
    while time.time() < deadline:
        try:
            raw = q.get(timeout=0.5)
        except queue.Empty:
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        m = o.get("method")
        if m:
            if o.get("id") is not None:
                rep(o["id"], {})
                continue
            p = o.get("params", {}) or {}
            upd = p.get("update") or {}
            kind = upd.get("sessionUpdate")
            if kind == "agent_message_chunk":
                c = upd.get("content") or {}
                if c.get("type") == "text":
                    TEXT.append(c.get("text", ""))
            info = upd.get("_meta", {}) if isinstance(upd, dict) else {}
            blob = json.dumps(o)
            if "steering" in blob:
                EVENTS.append(("steer-related-update", upd))
        elif o.get("id") is not None:
            PENDING[o["id"]] = o

def wait_resp(rid, to):
    end = time.time() + to
    while time.time() < end:
        if rid in PENDING:
            return PENDING.pop(rid)
        drain(time.time() + 0.5)
    return None

drain_init = req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
wait_resp(drain_init, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
SID = (wait_resp(nid, 40) or {}).get("result", {}).get("sessionId")
print("sessionId:", SID)

def run_turn(label, marker, do_clear):
    TEXT.clear(); EVENTS.clear()
    pid = req("session/prompt", {"sessionId": SID, "prompt": [{"type": "text",
        "text": "Write a short 150-word story about a lighthouse keeper. Prose only, no lists."}]})
    time.sleep(2.5)  # let the turn spin up so the steer lands mid-turn
    sid_ = req("_session/steer", {"sessionId": SID,
        "message": f"IMPORTANT CHANGE: end your reply with the single word {marker}"})
    steer_resp = wait_resp(sid_, 15)
    print(f"[{label}] steer resp:", json.dumps(steer_resp.get("result") if steer_resp and "result" in steer_resp else steer_resp)[:200])
    if do_clear:
        cid = req("_session/steer/clear", {"sessionId": SID})
        clear_resp = wait_resp(cid, 15)
        print(f"[{label}] clear resp:", json.dumps(clear_resp.get("result") if clear_resp and "result" in clear_resp else clear_resp)[:200])
    turn = wait_resp(pid, 300)
    stop = (turn or {}).get("result", {}).get("stopReason") if turn and "result" in turn else turn
    text = "".join(TEXT)
    print(f"[{label}] turn: {json.dumps(stop)[:200]}; textLen: {len(text)}; marker {marker} present: {marker in text}")
    print(f"[{label}] text: {text[:300]}")
    for tag, payload in EVENTS:
        print(f"[{label}] EVENT {tag}: {json.dumps(payload)[:300]}")
    return marker in text

a_leaked = run_turn("A test steer+clear", "STEERMARK_ALPHA", do_clear=True)
b_landed = run_turn("B control steer only", "STEERMARK_BRAVO", do_clear=False)
print("\nVERDICT: cleared steer suppressed:", (not a_leaked), "| control steer landed:", b_landed)
proc.stdin.close()
proc.terminate()
