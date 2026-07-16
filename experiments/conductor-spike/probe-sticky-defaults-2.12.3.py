#!/usr/bin/env python3
"""Runtime probe for 2.12.3 "sticky defaults": does selecting /model + /effort
INSIDE AN ACP SESSION auto-persist as the user-level default (chat.defaultModel /
chat.modelDefaults), and does a fresh session/new come up with it applied?

Run once per binary (same day, same settings store):

    probe-sticky-defaults-2.12.3.py <path-to-kiro-cli-chat> <model-to-select>

Reports settings before/after the execute, and the currentModelId a FRESH
process+session comes up with afterwards. The caller restores chat.defaultModel
manually afterwards (this script mutates real user settings on purpose — that
is the behavior under test)."""
import json, subprocess, threading, queue, time, tempfile, sys

KIRO = sys.argv[1]
SELECT = sys.argv[2]
CWD = tempfile.mkdtemp(prefix="stickyprobe-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)


def setting(key):
    r = subprocess.run(["kiro-cli", "settings", key], capture_output=True, text=True)
    return r.stdout.strip() or r.stderr.strip()


class Acp:
    def __init__(self):
        self.p = subprocess.Popen([KIRO, "acp"], cwd=CWD, stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                  text=True, bufsize=1)
        self.q = queue.Queue()
        threading.Thread(target=lambda: [self.q.put(l.strip()) for l in self.p.stdout if l.strip()],
                         daemon=True).start()
        self.i = 0

    def req(self, m, pr):
        self.i += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": m, "params": pr}) + "\n")
        self.p.stdin.flush()
        return self.i

    def pump(self, until, to=40):
        end = time.time() + to
        while time.time() < end:
            try:
                raw = self.q.get(timeout=2)
            except queue.Empty:
                continue
            try:
                o = json.loads(raw)
            except Exception:
                continue
            # answer server->client requests with {} so nothing wedges
            if o.get("id") is not None and o.get("method"):
                self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": o["id"], "result": {}}) + "\n")
                self.p.stdin.flush()
                continue
            if o.get("id") == until and ("result" in o or "error" in o):
                return o
        return None

    def close(self):
        try:
            self.p.stdin.close()
        except Exception:
            pass
        self.p.terminate()


print("== settings BEFORE:")
print("  chat.defaultModel =", setting("chat.defaultModel"))
print("  chat.modelDefaults =", setting("chat.modelDefaults"))

a = Acp()
a.req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
a.pump(1, 20)
nid = a.req("session/new", {"cwd": CWD, "mcpServers": []})
r = a.pump(nid, 40)
res = (r or {}).get("result", {})
sid = res.get("sessionId")
print("== session A: currentModelId =", (res.get("models") or {}).get("currentModelId"), "sid =", sid)

ok_method = None
for method in ("kiro.dev/commands/execute", "_kiro.dev/commands/execute"):
    rid = a.req(method, {"sessionId": sid, "command": {"command": "model", "args": {"value": SELECT}}})
    r = a.pump(rid, 40)
    if r is not None and "error" not in r:
        ok_method = method
        print("== model select via", method, "->", json.dumps(r.get("result"))[:200])
        break
    print("== model select via", method, "failed:", None if r is None else json.dumps(r.get("error"))[:150])

if ok_method:
    rid = a.req(ok_method, {"sessionId": sid, "command": {"command": "effort", "args": {"value": "high"}}})
    r = a.pump(rid, 40)
    print("== effort select ->", "no response" if r is None else json.dumps(r.get("result") or r.get("error"))[:200])

time.sleep(2)
a.close()
time.sleep(1)

print("== settings AFTER select:")
print("  chat.defaultModel =", setting("chat.defaultModel"))
print("  chat.modelDefaults =", setting("chat.modelDefaults"))

b = Acp()
b.req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
b.pump(1, 20)
nid = b.req("session/new", {"cwd": CWD, "mcpServers": []})
r = b.pump(nid, 40)
res = (r or {}).get("result", {})
print("== FRESH process session B: currentModelId =", (res.get("models") or {}).get("currentModelId"))
b.close()
