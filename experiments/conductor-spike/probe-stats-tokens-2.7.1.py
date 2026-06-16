#!/usr/bin/env python3
"""
Probe: does /stats (and _kiro.dev/metadata) carry token usage on the CURRENT backend?

Background: through 2.4.1 the /stats `input_tokens`/`output_tokens` were schema-present
but null (a staged BACKEND rollout, not a binary issue). This re-tests on the v2 engine
(cyril's default path) against today's backend, after one real turn so usage accrues.

Captures: (1) the `_kiro.dev/metadata` notification (camelCase inputTokens/outputTokens —
cyril's parse path), and (2) the `/stats` command output via kiro.dev/commands/execute
(snake_case input_tokens/output_tokens — the user-facing command).

v2 self-authenticates from the store; run `kiro-cli whoami` first if idle. No KAS flag.
"""
import json, os, subprocess, threading, queue, time, tempfile

KIRO = os.environ.get("KIRO_BIN", os.path.expanduser("~/.local/share/kiro-research/binaries/2.7.1/kiro-cli-chat"))
CWD = tempfile.mkdtemp(prefix="stats-")
LOG = os.path.join(os.path.dirname(__file__), "logs", "probe-stats-tokens-2.7.1.log")
os.makedirs(os.path.dirname(LOG), exist_ok=True)
logf = open(LOG, "w")
def log(*a):
    s=" ".join(str(x) for x in a); print(s); logf.write(s+"\n"); logf.flush()

proc = subprocess.Popen([KIRO,"acp"], cwd=CWD,           # no --agent-engine => v2 (cyril default)
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs=queue.Queue()
threading.Thread(target=lambda:([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()

_id=[10]
def req(method, params):
    _id[0]+=1
    PIN.write(json.dumps({"jsonrpc":"2.0","id":_id[0],"method":method,"params":params})+"\n"); PIN.flush()
    return _id[0]
def reply(rid,res):
    PIN.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); PIN.flush()

NOTIFS=[]
def pump(until_id, timeout=120):
    end=time.time()+timeout
    while time.time()<end:
        try: raw=msgs.get(timeout=2)
        except queue.Empty: continue
        if raw is None: log("[closed]"); return None
        try: o=json.loads(raw)
        except: continue
        if "method" in o and "id" in o:
            if o["method"]=="session/request_permission":
                opts=o["params"].get("options",[])
                pick=next((x for x in opts if "allow" in (x.get("kind","")+x.get("optionId","")).lower()), opts[0] if opts else None)
                reply(o["id"], {"outcome":{"outcome":"selected","optionId":pick["optionId"]}} if pick else {"outcome":{"outcome":"cancelled"}})
            else: reply(o["id"], {})
        elif "method" in o:
            NOTIFS.append(o)
        elif "id" in o and o["id"]==until_id:
            return o
    log("[timeout]"); return None

req("initialize", {"protocolVersion":1,"clientCapabilities":{}})
if pump(11, timeout=30) is None: log("FATAL: v2 init no response"); raise SystemExit(1)
nid=req("session/new", {"cwd":CWD,"mcpServers":[]})
nr=pump(nid, timeout=40)
assert nr and "result" in nr
sid=nr["result"]["sessionId"]
log("sessionId:", sid)

# drain until commands/available has a NON-EMPTY list (re-sent once MCP/skills load), up to 35s
def latest_cmd_names():
    av=[n for n in NOTIFS if "commands/available" in n.get("method","")]
    if not av: return None
    cmds=av[-1]["params"].get("availableCommands") or av[-1]["params"].get("available_commands") or []
    return [c.get("name") for c in cmds]
def drain_until_cmds(seconds):
    end=time.time()+seconds
    while time.time()<end:
        try: raw=msgs.get(timeout=0.5)
        except queue.Empty:
            n=latest_cmd_names()
            if n: return
            continue
        if raw is None: break
        try: o=json.loads(raw)
        except: continue
        if "method" in o and "id" in o: reply(o["id"], {})
        elif "method" in o: NOTIFS.append(o)
drain_until_cmds(35)
log("notification methods after session/new:", sorted({n.get("method","") for n in NOTIFS}))
names=latest_cmd_names()
log("commands/available count:", len(names) if names is not None else "NONE",
    "| stats present?:", ("stats" in names) if names else False)
log("command names:", names[:40] if names else names)

# one real turn so usage accrues
log("\n# running one turn...")
NOTIFS.clear()
pid=req("session/prompt", {"sessionId":sid,"prompt":[{"type":"text","text":"Reply with exactly: ok"}]})
pr=pump(pid, timeout=120)
log("# prompt response:", json.dumps(pr.get("result") if pr else None)[:200])

# the metadata notification cyril parses (camelCase inputTokens/outputTokens)
meta=[n for n in NOTIFS if "metadata" in n.get("method","")]
log(f"\n# _kiro.dev/metadata notifications: {len(meta)}")
for n in meta:
    p=n["params"]
    log("  method:", n["method"])
    log("  inputTokens/outputTokens/cachedTokens:",
        p.get("inputTokens"), p.get("outputTokens"), p.get("cachedTokens"))
    log("  full:", json.dumps(p)[:500])

# the /stats command itself — try both the unprefixed (cyril's) and prefixed method names
for method in ("kiro.dev/commands/execute", "_kiro.dev/commands/execute"):
    log(f"\n# executing /stats via {method} ...")
    rid=req(method, {"sessionId":sid, "command":{"command":"stats","args":{}}})
    sr=pump(rid, timeout=40)
    if sr is None: log("  (no response)")
    elif "error" in sr: log("  ERROR:", json.dumps(sr["error"])[:300])
    else:
        log("  stats result:", json.dumps(sr.get("result"))[:1600]); break
# any stats-bearing notification too
for n in NOTIFS:
    s=json.dumps(n)
    if "input_tokens" in s or "output_tokens" in s or ("stats" in n.get("method","").lower()):
        log("  stats-notif:", s[:600])

PIN.close(); proc.terminate()
log(f"\n# log: {LOG}")
