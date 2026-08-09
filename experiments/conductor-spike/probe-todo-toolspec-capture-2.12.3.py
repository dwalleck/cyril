#!/usr/bin/env python3
"""Capture the EXACT toolSpecification KAS sends to the backend for todo_list.

Direct-spawns acp-server.js (0.17.2) with --endpoint pointed at a local mock
that logs every HTTP request body and returns 500. The first session/prompt
dies at the mock — zero model tokens — but the request body carries
userInputMessageContext.tools[], which is the client-side ground truth for
the todo-schema A/B investigation (does `tasks` leave the client as
anyOf[array,null] or already record-ified?).

    probe-todo-toolspec-capture-2.12.3.py
"""
import glob
import json
import os
import queue
import signal
import sqlite3
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer

KAS_DIR = glob.glob(os.path.expanduser("~/.local/share/kiro-cli/kas/2.12.3-*/"))[0]
ENTRY = os.path.join(KAS_DIR, "node_modules/@kiro/agent/dist/server/acp-server.js")
NODE = os.path.expanduser("~/.local/share/kiro-cli/node")
if not os.path.exists(NODE):
    NODE = "node"
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "logs",
                   "todo-toolspec-capture-2.12.3-20260715.json")


def load_auth():
    db = sqlite3.connect(os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3"))
    row = db.execute("select value from auth_kv where key='kirocli:odic:token'").fetchone()
    if not row:
        sys.exit("no auth token — run kiro-cli login")
    tok = json.loads(row[0])
    prow = db.execute("select value from state where key='api.codewhisperer.profile'").fetchone()
    arn = json.loads(prow[0]).get("arn") if prow else None
    return {"accessToken": tok["access_token"], "expiresAt": tok["expires_at"], "profileArn": arn}


AUTH = load_auth()
CAPTURED = []


class Mock(BaseHTTPRequestHandler):
    def _handle(self):
        ln = int(self.headers.get("content-length") or 0)
        body = self.rfile.read(ln) if ln else b""
        CAPTURED.append({"path": self.path, "len": len(body),
                         "body": body.decode(errors="replace")})
        print(f"    [mock] {self.command} {self.path} body={len(body)}B")
        self.send_response(500)
        self.send_header("content-type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"message":"mock"}')

    do_GET = do_POST = do_PUT = _handle

    def log_message(self, *a):
        pass


srv = HTTPServer(("127.0.0.1", 0), Mock)
PORT = srv.server_address[1]
threading.Thread(target=srv.serve_forever, daemon=True).start()

cwd = tempfile.mkdtemp(prefix="toolspec-")
subprocess.run("git init -q -b main", cwd=cwd, shell=True)
p = subprocess.Popen([NODE, "--experimental-wasm-modules", ENTRY, "--transport=stdio",
                      "--auth=acp-callback", f"--endpoint=http://127.0.0.1:{PORT}"],
                     cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1,
                     start_new_session=True)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = 0


def send(obj):
    p.stdin.write(json.dumps(obj) + "\n")
    p.stdin.flush()


def req(m, pr):
    global i
    i += 1
    send({"jsonrpc": "2.0", "id": i, "method": m, "params": pr})
    return i


def pump(until, to=60):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = q.get(timeout=2)
        except queue.Empty:
            if CAPTURED and until is None:
                return None
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        if o.get("id") is not None and o.get("method"):
            res = AUTH if o["method"] == "_kiro/auth/getAccessToken" else {}
            send({"jsonrpc": "2.0", "id": o["id"], "result": res})
            continue
        if until is not None and o.get("id") == until and ("result" in o or "error" in o):
            return o
    return None


try:
    rid = req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
    r = pump(rid, 60)
    print("initialize:", "ok" if r else "NO RESPONSE")
    rid = req("session/new", {"cwd": cwd, "mcpServers": []})
    r = pump(rid, 60)
    sid = ((r or {}).get("result") or {}).get("sessionId")
    print("sessionId:", sid)
    rid = req("session/prompt", {"sessionId": sid,
                                 "prompt": [{"type": "text", "text": "hi"}]})
    end = time.time() + 90
    while time.time() < end and not CAPTURED:
        pump(None, 5)
    pump(rid, 10)
finally:
    try:
        os.killpg(os.getpgid(p.pid), signal.SIGKILL)
    except Exception:
        pass

print(f"\ncaptured {len(CAPTURED)} request(s)")
for c in CAPTURED:
    try:
        body = json.loads(c["body"])
    except Exception:
        print(f"  {c['path']}: non-JSON body ({c['len']}B), head: {c['body'][:200]!r}")
        continue
    ctx = (((body.get("conversationState") or {}).get("currentMessage") or {})
           .get("userInputMessage") or {}).get("userInputMessageContext") or {}
    tools = ctx.get("tools") or []
    print(f"  {c['path']}: {len(tools)} tools in request")
    for t in tools:
        spec = t.get("toolSpecification") or {}
        if spec.get("name") == "todo_list":
            with open(OUT, "w") as fh:
                json.dump(spec, fh, indent=1)
            js = (spec.get("inputSchema") or {}).get("json") or {}
            props = js.get("properties") or {}
            print("  >>> todo_list inputSchema.json.properties.tasks:")
            print(json.dumps(props.get("tasks"), indent=1))
            print("  >>> full spec saved to", OUT)
