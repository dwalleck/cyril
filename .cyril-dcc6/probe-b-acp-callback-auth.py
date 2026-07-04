#!/usr/bin/env python3
"""cyril-dcc6 probe B: does `--auth=acp-callback` + a sqlite-backed
_kiro/auth/getAccessToken responder yield a working authenticated turn?

Smallest question: with the callback auth provider selected, (a) does KAS
actually CALL the callback (file mode never does), and (b) does relaying the
CLI sqlite IDC token + state-table profileArn complete a real turn?

Oracle (independent): `kiro-cli acp --agent-engine v3` running the same tiny
prompt — the product's own callback responder implementation, not ours.
Usage: <script> <path-to-acp-server.js>
"""
import json, os, queue, sqlite3, subprocess, sys, tempfile, threading, time

db = sqlite3.connect(os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3"))
TOK = json.loads(db.execute("SELECT value FROM auth_kv WHERE key='kirocli:odic:token'").fetchone()[0])
PROFILE = json.loads(db.execute("SELECT value FROM state WHERE key='api.codewhisperer.profile'").fetchone()[0])
print("token expires_at:", TOK.get("expires_at"))

CWD = tempfile.mkdtemp(prefix="dcc6-probe-b-")
proc = subprocess.Popen(
    ["node", "--experimental-wasm-modules", sys.argv[1], "--transport=stdio", "--auth=acp-callback"],
    cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
    stderr=open(os.path.join(CWD, "stderr.log"), "w"), text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l) for l in proc.stdout if l.strip()], daemon=True).start()
i, PENDING, TEXT, CALLS = [0], {}, [], []

def req(m, p):
    i[0] += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}) + "\n")
    proc.stdin.flush()
    return i[0]

def drain(deadline):
    while time.time() < deadline:
        try:
            o = json.loads(q.get(timeout=0.5))
        except queue.Empty:
            continue
        except Exception:
            continue
        m = o.get("method")
        if m and o.get("id") is not None:
            if m == "_kiro/auth/getAccessToken":
                CALLS.append(time.time())
                print(f"[callback #{len(CALLS)}] getAccessToken — replying from sqlite")
                proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": {
                    "accessToken": TOK["access_token"], "expiresAt": TOK.get("expires_at"),
                    "profileArn": PROFILE.get("arn")}}) + "\n")
            else:
                proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": {}}) + "\n")
            proc.stdin.flush()
        elif m:
            upd = (o.get("params") or {}).get("update") or {}
            c = upd.get("content") or {}
            if upd.get("sessionUpdate") == "agent_message_chunk" and c.get("type") == "text":
                TEXT.append(c.get("text", ""))
        elif o.get("id") is not None:
            PENDING[o["id"]] = o

def wait(rid, to):
    end = time.time() + to
    while time.time() < end:
        if rid in PENDING:
            return PENDING.pop(rid)
        drain(time.time() + 0.5)
    return None

print("initialize:", "ok" if wait(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 20) else "TIMEOUT")
sid = ((wait(req("session/new", {"cwd": CWD, "mcpServers": []}), 40) or {}).get("result") or {}).get("sessionId")
print("session/new:", sid or "FAILED")
r = wait(req("session/prompt", {"sessionId": sid, "prompt": [
    {"type": "text", "text": "Reply with exactly the text KAS_AUTH_OK and nothing else. Do not use any tools."}]}), 240)
res = (r or {}).get("result") or {}
err = (r or {}).get("error") or {}
text = "".join(TEXT)
print("turn:", json.dumps(res or err)[:200])
print("agent text:", text[:200])
print("\nPROBE B VERDICT: callback_called:", len(CALLS),
      "| turn_ok:", res.get("stopReason") == "end_turn",
      "| auth_ok:", "KAS_AUTH_OK" in text)
proc.stdin.close()
proc.terminate()
