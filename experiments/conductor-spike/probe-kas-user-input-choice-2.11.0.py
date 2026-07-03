#!/usr/bin/env python3
"""C8 live check for cyril-qo13 (@kiro/agent 0.8.0, kiro-cli 2.11.0).

Question: does KAS act on the SPECIFIC optionId in a user_input permission
reply — the exact byte shape cyril's fixed converter emits
({"outcome": {"outcome": "selected", "optionId": <options[1].optionId>}},
byte-equality fenced by probe_qo13_reply_shape_matches_reference_bytes)?

Design: prompt the agent to ask a 3-option user_input question (Red/Green/
Blue) and echo the answer back. The probe always picks options[1] (Green).
  - Under the fixed behavior, the agent's echo names option 1 ("Green").
  - Under the pre-fix bug (always option-0), the echo would name "Red".
The agent's own echo is the behavioral oracle, mirroring the reference-trace
evidence (kas-live-session-trace-2.11.0.jsonl ids 3/4/5).

Direct-spawn free path (default file auth via ~/.aws/sso/cache/
kiro-auth-token.json — run only while that token is fresh).
Usage: <script> <path-to-acp-server.js>"""
import json
import os
import queue
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time

DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")
db = sqlite3.connect(DB)
TOK = json.loads(db.execute("SELECT value FROM auth_kv WHERE key='kirocli:odic:token'").fetchone()[0])
PROFILE = json.loads(db.execute("SELECT value FROM state WHERE key='api.codewhisperer.profile'").fetchone()[0])
print("sqlite token expires_at:", TOK.get("expires_at"), "| profileArn:", PROFILE.get("arn"))

SERVER = sys.argv[1]
runtime = os.environ.get("KIRO_AGENT_PATH", "node")
CWD = tempfile.mkdtemp(prefix="kas-userinput-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
proc = subprocess.Popen([runtime, "--experimental-wasm-modules", SERVER, "--transport=stdio"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open("/tmp/kas-probe-stderr.log", "w"), text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]
PENDING = {}
TEXT = []
QUESTIONS = []  # (request_id, options, replied_option_id)
FOLLOWUPS = []


def req(m, p):
    i[0] += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}) + "\n")
    proc.stdin.flush()
    return i[0]


def rep(rid, res):
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    proc.stdin.flush()


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
        m = o.get("method")
        if m:
            if o.get("id") is not None:
                if m == "_kiro/auth/getAccessToken":
                    print("[auth] getAccessToken CALLED — replying with sqlite token")
                    rep(o["id"], {"accessToken": TOK["access_token"],
                                  "expiresAt": TOK.get("expires_at"),
                                  "profileArn": PROFILE.get("arn")})
                    continue
                if m == "session/request_permission":
                    opts = (o.get("params") or {}).get("options") or []
                    if QUESTIONS:  # follow-ups: evidence already gathered
                        print(f"[permission id={o['id']}] follow-up arrived ({len(opts)} options) — cancelling")
                        FOLLOWUPS.append(opts)
                        rep(o["id"], {"outcome": {"outcome": "cancelled"}})
                        continue
                    pick = opts[1] if len(opts) > 1 else (opts[0] if opts else None)
                    if pick is None:
                        rep(o["id"], {"outcome": {"outcome": "cancelled"}})
                        continue
                    # EXACTLY the bytes cyril's converter emits for a
                    # non-first pick (C2 fence: byte-equal to reference).
                    reply = {"outcome": {"outcome": "selected",
                                         "optionId": pick["optionId"]}}
                    QUESTIONS.append((o["id"], opts, pick["optionId"]))
                    print(f"[permission id={o['id']}] options: "
                          + json.dumps([(x.get('optionId'), x.get('name')) for x in opts]))
                    print(f"[permission id={o['id']}] replying (cyril bytes): {json.dumps(reply)}")
                    rep(o["id"], reply)
                else:
                    rep(o["id"], {})
                continue
            upd = (o.get("params") or {}).get("update") or {}
            if upd.get("sessionUpdate") == "agent_message_chunk":
                c = upd.get("content") or {}
                if c.get("type") == "text":
                    TEXT.append(c.get("text", ""))
        elif o.get("id") is not None:
            PENDING[o["id"]] = o


def wait_resp(rid, to):
    end = time.time() + to
    while time.time() < end:
        if rid in PENDING:
            return PENDING.pop(rid)
        drain(time.time() + 0.5)
    return None


init = req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
wait_resp(init, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
SID = (wait_resp(nid, 40) or {}).get("result", {}).get("sessionId")
print("sessionId:", SID)

pid = req("session/prompt", {"sessionId": SID, "prompt": [{"type": "text", "text":
    "I want to add support for multiple notes per issue in this project."}]})
turn = wait_resp(pid, 300)
stop = (turn or {}).get("result", {}).get("stopReason") if turn and "result" in turn else turn
text = "".join(TEXT)
print("turn stopReason:", json.dumps(stop)[:200])
print("agent text:", text[:400])

picked = QUESTIONS[0][1][1].get("name") if QUESTIONS and len(QUESTIONS[0][1]) > 1 else None
first = QUESTIONS[0][1][0].get("name") if QUESTIONS else None
lower = text.lower()
acted_on_pick = picked is not None and ("bug" in lower or (picked.lower() in lower))
acted_on_first = first is not None and picked is not None and \
    ("feature" in lower and "bug" not in lower)
print("\nVERDICT: user_input fired:", bool(QUESTIONS),
      "| picked (non-first):", picked,
      "| continuation reflects the pick:", acted_on_pick,
      "| continuation reflects option-0 instead (bug behavior):", acted_on_first,
      "| follow-up questions:", len(FOLLOWUPS))
proc.stdin.close()
proc.terminate()
