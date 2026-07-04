#!/usr/bin/env python3
"""cyril-dcc6 oracle (independent of both probes): run the same tiny prompt
through `kiro-cli acp --agent-engine v3` — the product's own KAS spawn +
callback-auth implementation, none of cyril's/our code.

Yields two ground truths:
  - AUTH: does the same sqlite credential complete a turn via the product's
    own acp-callback responder? (oracle for probe B)
  - DISCOVERY: which acp-server.js path does kiro-cli itself resolve and
    spawn? Read from /proc/<child>/cmdline. (oracle for probe A)
"""
import json, os, queue, sqlite3, subprocess, tempfile, threading, time

db = sqlite3.connect(os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3"))
TOK = json.loads(db.execute("SELECT value FROM auth_kv WHERE key='kirocli:odic:token'").fetchone()[0])
PROFILE = json.loads(db.execute("SELECT value FROM state WHERE key='api.codewhisperer.profile'").fetchone()[0])
CALLS = []

CWD = tempfile.mkdtemp(prefix="dcc6-oracle-")
proc = subprocess.Popen(["kiro-cli", "acp", "--agent-engine", "v3"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l) for l in proc.stdout if l.strip()], daemon=True).start()
i, PENDING, TEXT = [0], {}, []

def req(m, p):
    i[0] += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}) + "\n")
    proc.stdin.flush()
    return i[0]

def kas_children():
    out = subprocess.run(["pgrep", "-f", "acp-server.js"], capture_output=True, text=True).stdout.split()
    hits = []
    for pid in out:
        try:
            argv = open(f"/proc/{pid}/cmdline").read().split("\0")
            ppid = open(f"/proc/{pid}/stat").read().split()[3]
        except OSError:
            continue
        hits.append((pid, ppid, [a for a in argv if a]))
    return hits

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
                CALLS.append(m)
                print(f"[forwarded callback #{len(CALLS)}] getAccessToken — replying from sqlite")
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

before = {pid for pid, _, _ in kas_children()}
print("initialize:", "ok" if wait(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 25) else "TIMEOUT")
sid = ((wait(req("session/new", {"cwd": CWD, "mcpServers": []}), 60) or {}).get("result") or {}).get("sessionId")
print("session/new:", sid or "FAILED")
for pid, ppid, argv in kas_children():
    if pid not in before:
        entry = next((a for a in argv if a.endswith("acp-server.js")), "?")
        flags = [a for a in argv if a.startswith("--")]
        print(f"SPAWNED CHILD pid={pid} ppid={ppid}")
        print(f"  entry: {entry}")
        print(f"  flags: {flags}")
r = wait(req("session/prompt", {"sessionId": sid, "prompt": [
    {"type": "text", "text": "Reply with exactly the text KAS_AUTH_OK and nothing else. Do not use any tools."}]}), 240)
res = (r or {}).get("result") or {}
err = (r or {}).get("error") or {}
text = "".join(TEXT)
print("turn:", json.dumps(res or err)[:200])
print("agent text:", text[:200])
print("\nORACLE VERDICT: turn_ok:", res.get("stopReason") == "end_turn",
      "| auth_ok:", "KAS_AUTH_OK" in text)
proc.stdin.close()
proc.terminate()
try:
    proc.wait(timeout=5)
except Exception:
    proc.kill()
# leave no orphans (cyril-0pms lesson)
for pid, _, _ in kas_children():
    if pid not in before:
        subprocess.run(["kill", pid], check=False)
        print("reaped child", pid)
