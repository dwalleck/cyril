#!/usr/bin/env python3
"""2.10.0 hot-reload wire probe (v2 default engine, self-auth).

Question: when `mcp.json` / `.kiro/agents/*.json` change on disk mid-session, does
kiro RE-EMIT anything on the ACP wire (commands/available re-advertise, McpServer
init notifications), and does the `/agent` options query reflect a newly-added
agent file? Determines whether cyril reflects hot-reload for free.

Usage: probe-hotreload-2.10.0.py <path-to-kiro-cli-chat>
"""
import json, subprocess, threading, queue, time, tempfile, sys, pathlib

KIRO = sys.argv[1]
CWD = tempfile.mkdtemp(prefix="hotreload-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

settings = pathlib.Path(CWD, ".kiro", "settings"); settings.mkdir(parents=True, exist_ok=True)
agents = pathlib.Path(CWD, ".kiro", "agents"); agents.mkdir(parents=True, exist_ok=True)
MCP = settings / "mcp.json"
# baseline: empty MCP config + one custom agent file
MCP.write_text(json.dumps({"mcpServers": {}}, indent=2))
(agents / "probe-agent.json").write_text(json.dumps({
    "name": "probe-agent", "description": "probe baseline agent",
    "prompt": "You are a probe agent.", "tools": ["fs_read"]
}, indent=2))

p = subprocess.Popen([KIRO, "acp"], cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()], daemon=True).start()

PHASE = ["baseline"]
INBOUND = []  # (phase, ts, method, brief)
i = [0]; t0 = time.time()
def req(m, pr):
    i[0] += 1; p.stdin.write(json.dumps({"jsonrpc":"2.0","id":i[0],"method":m,"params":pr})+"\n"); p.stdin.flush(); return i[0]
def rep(rid, res):
    p.stdin.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); p.stdin.flush()

def brief(m, pr):
    if "commands/available" in m:
        tools=[t.get("name") for t in (pr.get("tools") or []) if isinstance(t,dict)]
        cmds=[(c.get("name") or c.get("command")) for c in (pr.get("commands") or []) if isinstance(c,dict)]
        srv=pr.get("mcpServers") or pr.get("servers")
        return f"tools={len(tools)} cmds={len(cmds)} mcpServers={srv}"
    if "mcp" in m.lower() or "Mcp" in m:
        return json.dumps(pr)[:200]
    if m=="session/update":
        u=pr.get("update",{}); return u.get("sessionUpdate") or list(u.keys())
    return json.dumps(pr)[:120]

def handle(o):
    m=o.get("method"); rid=o.get("id"); pr=o.get("params",{}) or {}
    if m:
        INBOUND.append((PHASE[0], round(time.time()-t0,1), m, brief(m,pr)))
        if rid is not None:  # agent->client request: ack (reject permissions)
            if "request_permission" in m:
                rep(rid, {"outcome":{"outcome":"cancelled"}})
            else:
                rep(rid, {})

def pump(until=None, to=10):
    end=time.time()+to
    while time.time()<end:
        try: raw=q.get(timeout=1)
        except queue.Empty: continue
        try: o=json.loads(raw)
        except: continue
        if "method" in o: handle(o)
        if until is not None and o.get("id")==until and ("result" in o or "error" in o):
            return o
    return None

def agent_options(sid):
    rid=req("_kiro.dev/commands/options", {"command":"agent","sessionId":sid,"partial":""})
    r=pump(rid, 15)
    if not r or "result" not in r: return None
    res=r["result"]
    opts=res.get("options") or (res.get("0",{}) if isinstance(res,dict) else None) or res
    # options may be nested; try common shapes
    if isinstance(res,dict) and "options" in res: opts=res["options"]
    names=[]
    if isinstance(opts,list):
        for o in opts:
            if isinstance(o,dict): names.append(o.get("value") or o.get("label"))
    return names

# --- baseline ---
req("initialize", {"protocolVersion":1,"clientCapabilities":{}}); pump(1, 15)
nid=req("session/new", {"cwd":CWD,"mcpServers":[]}); sn=pump(nid, 30)
SID=(sn or {}).get("result",{}).get("sessionId") if sn else None
pump(None, 3)  # drain trailing commands/available
print(f"sessionId={SID}")
base_agents=agent_options(SID)
print(f"[baseline] /agent options: {base_agents}")

# --- MUTATE on disk mid-session ---
PHASE[0]="after-mutate"
MCP.write_text(json.dumps({"mcpServers":{
    "probe_srv":{"command":"kiro_probe_nonexistent_cmd_xyz","args":[]}
}}, indent=2))
(agents / "probe-agent-2.json").write_text(json.dumps({
    "name":"probe-agent-2","description":"added mid-session",
    "prompt":"second probe agent.","tools":["fs_read"]
}, indent=2))
print("\n[mutated] added mcp server 'probe_srv' + agent file 'probe-agent-2'; watching idle 10s...")
pump(None, 10)  # idle: does the watcher fire without a turn?

# --- cross an idle boundary with a trivial turn ---
PHASE[0]="during-turn"
print("[turn] sending trivial prompt to cross idle boundary...")
pid=req("session/prompt", {"sessionId":SID,"prompt":[{"type":"text","text":"Reply with exactly: ok"}]})
pump(pid, 120)
PHASE[0]="after-turn"
pump(None, 4)

# --- re-query agent options for freshness ---
fresh_agents=agent_options(SID)
print(f"[after] /agent options: {fresh_agents}")

# --- report ---
print("\n==== INBOUND TIMELINE (method arrivals by phase) ====")
for ph,ts,m,b in INBOUND:
    if m=="session/update" and b=="agent_message_chunk": continue  # noise
    print(f"  [{ph:12}] +{ts:5}s  {m}   {b}")
print("\n==== SUMMARY ====")
def fired(substr, phase=None):
    return [x for x in INBOUND if substr in x[2] and (phase is None or x[0]==phase)]
print(f"commands/available total: {len(fired('commands/available'))} "
      f"(baseline {len(fired('commands/available','baseline'))}, "
      f"after-mutate {len(fired('commands/available','after-mutate'))}, "
      f"during/after-turn {len(fired('commands/available','during-turn'))+len(fired('commands/available','after-turn'))})")
print(f"any mcp-related inbound: {[ (x[0],x[2]) for x in INBOUND if 'mcp' in x[2].lower() ]}")
print(f"agent list changed baseline->after: {base_agents} -> {fresh_agents}")
p.stdin.close(); p.terminate()
