#!/usr/bin/env python3
"""C8 live check for cyril-qo13 (@kiro/agent 0.8.0, kiro-cli 2.11.0) — v2.

Question: does KAS act on the SPECIFIC optionId in a user_input permission
reply — the exact byte shape cyril's fixed converter emits
({"outcome": {"outcome": "selected", "optionId": <options[1].optionId>}},
byte-equality fenced by probe_qo13_reply_shape_matches_reference_bytes)?

v1 lesson: a bare prompt in an empty tempdir never fires user_input — the
clarify questions are a SPEC-MODE behavior. v2 reproduces the reference
trace's recipe (kas-live-session-trace-2.11.0.jsonl): real workspace (a
throwaway local clone of ~/repos/rivets), set_config_option autopilot=on +
mode=spec, then the trace's own spec prompt. Every genuine user_input
question is answered with options[1] (non-first); the spec artifacts the
turn writes into the clone are the behavioral oracle.

Permission classification: standard tool approvals (optionIds accept/
always-accept/reject/always-reject) are auto-accepted; a request counts as
user_input only when ALL options share kind 'allow_once' and optionIds are
non-standard (trace shape: '<toolCallId>-option-N').

Auth: standalone KAS is file-auth-only (~/.aws/sso/cache/kiro-auth-token.json)
and never calls _kiro/auth/getAccessToken — see cyril-dcc6. Seed that file
from the CLI sqlite IDC token first (WITHOUT the refresh token) and run while
it is fresh. Usage: <script> <path-to-acp-server.js>"""
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
SRC_REPO = os.path.expanduser("~/repos/rivets")
runtime = os.environ.get("KIRO_AGENT_PATH", "node")
BASE = tempfile.mkdtemp(prefix="kas-userinput-")
subprocess.run(["git", "clone", "-q", "--local", SRC_REPO, "repo"], cwd=BASE, check=True)
CWD = os.path.join(BASE, "repo")
print("workspace (throwaway clone):", CWD)
proc = subprocess.Popen([runtime, "--experimental-wasm-modules", SERVER, "--transport=stdio"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open(os.path.join(BASE, "kas-stderr.log"), "w"), text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]
PENDING = {}
TEXT = []
QUESTIONS = []   # (request_id, title, options, picked_optionId, picked_name)
APPROVALS = []   # standard tool approvals auto-accepted

STD_APPROVAL_IDS = {"accept", "always-accept", "reject", "always-reject"}


def is_user_input(opts):
    return (len(opts) > 1
            and all(o.get("kind") == "allow_once" for o in opts)
            and not any(o.get("optionId") in STD_APPROVAL_IDS for o in opts))


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
                    p = o.get("params") or {}
                    opts = p.get("options") or []
                    title = (p.get("toolCall") or {}).get("title", "")
                    if not opts:
                        rep(o["id"], {"outcome": {"outcome": "cancelled"}})
                        continue
                    if is_user_input(opts):
                        pick = opts[1]
                        # EXACTLY the bytes cyril's converter emits for a
                        # non-first pick (C2 fence: byte-equal to reference).
                        reply = {"outcome": {"outcome": "selected",
                                             "optionId": pick["optionId"]}}
                        QUESTIONS.append((o["id"], title, opts,
                                          pick["optionId"], pick.get("name")))
                        print(f"[user_input id={o['id']}] Q: {title}")
                        print(f"[user_input id={o['id']}] options: "
                              + json.dumps([x.get("name") for x in opts]))
                        print(f"[user_input id={o['id']}] replying options[1] (cyril bytes): {json.dumps(reply)}")
                        rep(o["id"], reply)
                    else:
                        APPROVALS.append((o["id"], title))
                        print(f"[tool_approval id={o['id']}] {title[:80]} — accepting once")
                        rep(o["id"], {"outcome": {"outcome": "selected",
                                                  "optionId": "accept"}})
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

# Reference-trace recipe: autopilot on, spec mode, fast model.
for cfg, val in (("autopilot", "on"), ("mode", "spec"), ("model", "claude-haiku-4.5")):
    r = wait_resp(req("session/set_config_option",
                      {"sessionId": SID, "configId": cfg, "value": val}), 30)
    print(f"set_config_option {cfg}={val}:", "ok" if r and "result" in r else json.dumps(r)[:150])

pid = req("session/prompt", {"sessionId": SID, "prompt": [{"type": "text", "text":
    'Start a new spec called "multiple-notes". Create the .kiro/specs/multiple-notes/ '
    'directory and draft the initial requirements document.'}]})
turn = wait_resp(pid, 480)
stop = (turn or {}).get("result", {}).get("stopReason") if turn and "result" in turn else turn
text = "".join(TEXT)
print("turn stopReason:", json.dumps(stop)[:200])
print("agent text (tail):", text[-500:])

print("\n--- questions answered (all with options[1]) ---")
for rid, title, opts, oid, name in QUESTIONS:
    print(f"  id={rid} pick={name!r} (of {[x.get('name') for x in opts]})  Q: {title[:100]}")

req_md = os.path.join(CWD, ".kiro", "specs", "multiple-notes", "requirements.md")
doc = open(req_md).read() if os.path.exists(req_md) else ""
print("\n--- requirements.md", "(first 3500 chars) ---" if doc else "NOT WRITTEN ---")
print(doc[:3500])

hits = [(name, name and name.lower() in (doc + text).lower())
        for _, _, _, _, name in QUESTIONS]
print("\nVERDICT: user_input questions:", len(QUESTIONS),
      "| tool approvals:", len(APPROVALS),
      "| picked-name verbatim in output:", hits,
      "\n(semantic steering: judge the doc above against each non-first pick)")
print("workspace kept for inspection:", CWD)
proc.stdin.close()
proc.terminate()
