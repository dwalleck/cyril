#!/usr/bin/env python3
"""
Probe: does KAS (2.7.1) implement session/set_config_option as a working SET?

On the v2 engine this returned "Method not found" and configOptions was always null.
KAS populates configOptions (mode/autopilot/contentCollection); this checks whether
SETTING one actually takes effect on the wire. No prompt turn needed (fast, no model cost).

Sequence: initialize -> session/new (capture currentValues) -> set autopilot on->off,
mode vibe->spec -> inspect each response (KAS returns rebuilt configOptions) and any
config_option_update notification. Self-sources the bearer token (never logged).
"""
import json, os, subprocess, sys, threading, queue, time, tempfile, sqlite3, pathlib

KIRO = os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat")
CWD = tempfile.mkdtemp(prefix="kas-setcfg-")
AUTH_DB = os.path.expanduser("~/.local/share/kiro-cli/data.sqlite3")

def read_token():
    c = sqlite3.connect(AUTH_DB)
    try:
        row = c.execute("select value from auth_kv where key='kirocli:social:token'").fetchone()
    finally:
        c.close()
    if not row: return None
    v = row[0]
    if isinstance(v, (bytes, bytearray)): v = v.decode("utf-8", "replace")
    d = json.loads(v)
    return {"accessToken": d["access_token"], "expiresAt": d["expires_at"],
            "profileArn": d.get("profile_arn"), "provider": d.get("provider")}

proc = subprocess.Popen([KIRO, "acp", "--agent-engine", "kas"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()

_id = [10]
def req(method, params):
    _id[0] += 1
    PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":method,"params":params})+"\n"); PIN.flush()
    return _id[0]
def reply(rid, result):
    PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":result})+"\n"); PIN.flush()

NOTIFS = []
def pump(until_id, timeout=40):
    end = time.time()+timeout
    while time.time() < end:
        try: raw = msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return None
        try: o = json.loads(raw)
        except: continue
        if "method" in o and "id" in o:          # server->client request
            if o["method"] == "_kiro/auth/getAccessToken":
                reply(o["id"], read_token() or {})
            else:
                reply(o["id"], {})
        elif "method" in o:                        # notification
            NOTIFS.append(o)
        elif "id" in o and o["id"] == until_id:
            return o
    return None

def cfg_summary(configOptions):
    return {c.get("id") or c.get("configId"): c.get("currentValue") for c in (configOptions or [])}

req("initialize", {"protocolVersion":1, "clientCapabilities":{}})
pump(11)
nid = req("session/new", {"cwd":CWD, "mcpServers":[]})
resp = pump(nid)
sid = resp["result"]["sessionId"]
print("sessionId:", sid)
print("INITIAL configOptions:", json.dumps(cfg_summary(resp["result"].get("configOptions"))))

def try_set(configId, value, boolean=False):
    NOTIFS.clear()
    params = {"sessionId": sid, "configId": configId}
    params.update({"type":"boolean","value":value} if boolean else {"value":value})
    rid = req("session/set_config_option", params)
    r = pump(rid)
    print(f"\n--- set {configId} = {value!r} ---")
    if r is None: print("  (no response / timeout)"); return
    if "error" in r:
        print("  ERROR:", json.dumps(r["error"])[:300]); return
    print("  response configOptions:", json.dumps(cfg_summary(r["result"].get("configOptions"))))
    cou = [n for n in NOTIFS if (n.get("params",{}).get("update",{}) or {}).get("sessionUpdate")=="config_option_update"]
    print(f"  config_option_update notifications: {len(cou)}")
    for n in cou:
        print("    ->", json.dumps(cfg_summary(n["params"]["update"].get("configOptions"))))

try_set("autopilot", "off")
try_set("mode", "spec")
try_set("autopilot", "on")          # flip back
# also test a bad value to see validation behavior
try_set("autopilot", "bogus")

PIN.close(); proc.terminate()
print("\n(done)")
