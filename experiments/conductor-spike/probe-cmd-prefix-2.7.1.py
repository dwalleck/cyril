#!/usr/bin/env python3
"""
Resolve the command-execute prefix anomaly (possible live cyril bug).

cyril sends UNprefixed `kiro.dev/commands/execute`; an earlier probe saw that 404
(-32601) while `_kiro.dev/commands/execute` worked — but that probe (a) only looked
for the command list under `availableCommands` (cyril actually reads `commands` first),
and (b) may not have waited for the registry to populate past MCP init.

This dumps the FULL commands/available payload (all keys), waits up to 90s for a
non-empty registry, then tests BOTH execute prefixes (and both options prefixes).
v2 engine (cyril's path); v2 self-authenticates. Run `kiro-cli whoami` first if idle.
"""
import json, os, subprocess, threading, queue, time, tempfile, sys

KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat"))
CWD = tempfile.mkdtemp(prefix="cmdprefix-")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-cmd-prefix-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s=" ".join(str(x) for x in a); print(s); logf.write(s+"\n"); logf.flush()

proc = subprocess.Popen([KIRO,"acp"], cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs=queue.Queue()
threading.Thread(target=lambda:([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id=[10]
def req(method, params):
    _id[0]+=1
    PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":method,"params":params})+"\n"); PIN.flush(); return _id[0]
def reply(rid,res): PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); PIN.flush()
NOTIFS=[]
def pump(until_id, timeout=40):
    end=time.time()+timeout
    while time.time()<end:
        try: raw=msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: return None
        try: o=json.loads(raw)
        except: continue
        if "method" in o and "id" in o: reply(o["id"], {})
        elif "method" in o: NOTIFS.append(o)
        elif "id" in o and o["id"]==until_id: return o
    return None

def cmd_list():
    """Mirror cyril's convert: commands -> availableCommands -> [] ; return (key, names)."""
    av=[n for n in NOTIFS if "commands/available" in n.get("method","")]
    if not av: return (None, None)
    p=av[-1].get("params",{})
    for key in ("commands","availableCommands","available_commands"):
        v=p.get(key)
        if isinstance(v,list):
            return (key, [c.get("name") if isinstance(c,dict) else c for c in v])
    if isinstance(p,list): return ("<root-array>", [c.get("name") for c in p])
    return ("<none>", None)

req("initialize", {"protocolVersion":1,"clientCapabilities":{}})
if pump(11,30) is None: log("FATAL: no init"); sys.exit(1)
nid=req("session/new", {"cwd":CWD,"mcpServers":[]})
sid=pump(nid,40)["result"]["sessionId"]
log("sessionId:", sid)

# wait up to 90s for a non-empty registry under ANY key
deadline=time.time()+90
while time.time()<deadline:
    try: raw=msgs.get(timeout=2)
    except queue.Empty:
        k,names=cmd_list()
        if names: break
        continue
    if raw is None: break
    try: o=json.loads(raw)
    except: continue
    if "method" in o and "id" in o: reply(o["id"], {})
    elif "method" in o: NOTIFS.append(o)

# dump EVERY commands/available payload we saw (raw, all keys)
av=[n for n in NOTIFS if "commands/available" in n.get("method","")]
log(f"\n# commands/available notifications seen: {len(av)}")
for n in av:
    log("  method:", n["method"], "| param keys:", list(n.get("params",{}).keys()))
    log("  raw params:", json.dumps(n.get("params"))[:700])
k,names=cmd_list()
log(f"\n# resolved registry: key={k!r} count={len(names) if names else 0}")
log("  names sample:", names[:20] if names else names)

# test both prefixes for execute + options
for method in ("kiro.dev/commands/execute","_kiro.dev/commands/execute"):
    rid=req(method, {"sessionId":sid,"command":{"command":"stats","args":{}}})
    r=pump(rid,40)
    verdict = "no response" if r is None else ("ERROR "+json.dumps(r.get("error"))[:120] if "error" in r else "OK "+json.dumps(r.get("result"))[:120])
    log(f"\nexecute via {method}: {verdict}")

PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
