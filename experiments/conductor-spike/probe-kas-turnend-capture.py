#!/usr/bin/env python3
"""KAS-2a cheapest-falsifier (cyril-j16p): capture a live `turn_end` frame.

Runs ONE trivial KAS turn on the free path and dumps EVERY session/update +
session_info_update verbatim, in order, so we can resolve:
  (1) Is the terminal busy-clear signal `turn_end` or `turn_completion`?
  (2) Where does stopReason live (_meta.kiro.turnEnd.stopReason? .stopReason?)?
  (3) Does the session/prompt response also return (and with what stopReason)?

Designed to FAIL the KAS-2a assumption if the terminal kind is `turn_completion`,
if `turn_end` carries no stopReason, or if `turn_end` fires mid-turn (non-terminal).

Usage: probe-kas-turnend-capture.py <acp-server.js> <out.jsonl>
"""
import json, os, subprocess, threading, queue, time, tempfile, sys

SERVER = sys.argv[1]
OUT = sys.argv[2] if len(sys.argv) > 2 else "/tmp/kas-turnend.jsonl"
assert os.path.exists(SERVER), SERVER
runtime = os.environ.get("KIRO_AGENT_PATH", "node")
TOKEN = os.path.expanduser("~/.aws/sso/cache/kiro-auth-token.json")
CWD = tempfile.mkdtemp(prefix="kas-turnend-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
log = open(OUT, "w")

proc = subprocess.Popen([runtime, "--experimental-wasm-modules", SERVER, "--transport=stdio"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]
def send(o): proc.stdin.write(json.dumps(o) + "\n"); proc.stdin.flush()
def req(m, p):
    i[0] += 1; send({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}); return i[0]
def rep(rid, res): send({"jsonrpc": "2.0", "id": rid, "result": res})
def token():
    try: return json.load(open(TOKEN))
    except Exception: return {}

SIU = []          # ordered list of (kind, full_update_obj)
prompt_stop = ["<none>"]

def handle(o):
    m = o.get("method"); rid = o.get("id"); pr = o.get("params", {}) or {}
    if m:
        log.write(json.dumps({"d": "A->C", "method": m, "params": pr}) + "\n"); log.flush()
        if m == "session/update":
            u = pr.get("update", {})
            if u.get("sessionUpdate") == "session_info_update":
                kind = (((u.get("_meta") or {}).get("kiro")) or {}).get("kind")
                SIU.append((kind, u))
        if rid is not None:  # agent->client request: answer
            if "getAccessToken" in m:
                t = token()
                rep(rid, {"accessToken": t.get("accessToken"), "expiresAt": t.get("expiresAt"),
                          "profileArn": t.get("profileArn"), "provider": t.get("provider")})
            elif "request_permission" in m:
                # auto-approve so a tool turn can proceed (we send a no-tool prompt anyway)
                opts = (pr.get("options") or [])
                allow = next((x.get("optionId") for x in opts if "allow" in str(x.get("kind","")).lower()), None)
                rep(rid, {"outcome": {"outcome": "selected", "optionId": allow}} if allow else {"outcome": {"outcome": "cancelled"}})
            elif "shell_type" in m:
                rep(rid, {"shellType": "bash"})
            else:
                rep(rid, {})

def pump(until=None, to=180):
    end = time.time() + to
    while time.time() < end:
        try: raw = q.get(timeout=2)
        except queue.Empty: continue
        try: o = json.loads(raw)
        except Exception: continue
        if "method" in o: handle(o)
        if until is not None and o.get("id") == until and ("result" in o or "error" in o):
            return o
    return None

r = pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 20)
if r and "error" in r: print("INIT ERROR:", r["error"]); sys.exit(1)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sn = pump(nid, 60)
SID = (sn or {}).get("result", {}).get("sessionId")
print(f"sessionId={SID}")
SIU.clear()  # only count turn-driven frames

pid = req("session/prompt", {"sessionId": SID, "prompt": [{"type": "text", "text": "Reply with exactly: ok"}]})
pr = pump(pid, 180)
if pr and "result" in pr:
    prompt_stop[0] = pr["result"].get("stopReason", "<no stopReason>")
elif pr and "error" in pr:
    prompt_stop[0] = f"<error: {pr['error']}>"
pump(None, 4)  # drain trailing

print("\n==== session_info_update kinds in ORDER (turn) ====")
for k, _ in SIU: print(f"  - {k}")
print(f"\nprompt-response stopReason: {prompt_stop[0]}")
print(f"terminal (last) session_info_update kind: {SIU[-1][0] if SIU else '<none>'}")

def dump(kind):
    for k, u in SIU:
        if k == kind:
            print(f"\n==== FULL `{kind}` frame ====")
            print(json.dumps(u, indent=2))
            return
    print(f"\n(no `{kind}` frame seen)")
dump("turn_end")
dump("turn_completion")
print(f"\nfull jsonl log -> {OUT}")
proc.stdin.close(); proc.terminate()
