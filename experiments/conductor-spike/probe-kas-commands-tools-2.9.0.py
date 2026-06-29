#!/usr/bin/env python3
"""Capture the KAS slash-command set + tool definitions (deterministic, no turn).
Direct-spawn free path. Dumps every inbound method, any available_commands_update
payload, and any tool list (hunts for orchestrate_subagent + its input schema).
Usage: probe-kas-commands-tools-2.9.0.py <path-to-acp-server.js>"""
import json, os, subprocess, threading, queue, time, tempfile, sys

SERVER = sys.argv[1]
assert os.path.exists(SERVER), SERVER
runtime = os.environ.get("KIRO_AGENT_PATH", "node")
CWD = tempfile.mkdtemp(prefix="kas-cmds-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
proc = subprocess.Popen([runtime, "--experimental-wasm-modules", SERVER, "--transport=stdio"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]
def req(m, p):
    i[0] += 1
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": p}) + "\n")
    proc.stdin.flush(); return i[0]
def rep(rid, res):
    proc.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n"); proc.stdin.flush()

inbound = {}
commands = set()
tools = {}
def harvest(o):
    m = o.get("method")
    if m:
        inbound[m] = inbound.get(m, 0) + 1
        p = o.get("params", {}) or {}
        # ACP available_commands_update arrives as session/update
        upd = p.get("update") or {}
        kind = upd.get("sessionUpdate")
        blob = json.dumps(p)
        # commands can live in update.availableCommands or params.availableCommands
        for src in (upd.get("availableCommands"), p.get("availableCommands"), p.get("commands")):
            if isinstance(src, list):
                for c in src:
                    n = c.get("name") if isinstance(c, dict) else c
                    if n: commands.add(n)
        # tools: hunt any object that has name + inputSchema
        def walk(x):
            if isinstance(x, dict):
                if "name" in x and ("inputSchema" in x or "input_schema" in x):
                    tools[x["name"]] = x.get("inputSchema") or x.get("input_schema")
                for v in x.values(): walk(v)
            elif isinstance(x, list):
                for v in x: walk(v)
        walk(p)
        if o.get("id") is not None:
            rep(o["id"], {})

def pump(until, to):
    end = time.time() + to
    while time.time() < end:
        try: raw = q.get(timeout=2)
        except queue.Empty:
            if until is None: continue
            else: continue
        try: o = json.loads(raw)
        except Exception: continue
        harvest(o)
        if until is not None and o.get("id") == until and ("result" in o or "error" in o):
            return o
    return None

pump(req("initialize", {"protocolVersion": 1, "clientCapabilities": {}}), 20)
pump(req("session/new", {"cwd": CWD, "mcpServers": []}), 40)
pump(None, 8)  # drain trailing notifications (available_commands_update etc.)

print(f"INBOUND methods: {json.dumps(inbound, indent=0)}")
print(f"\nCOMMANDS ({len(commands)}): {' '.join(sorted(commands))}")
print(f"\n/stats present? {'YES' if 'stats' in commands or '/stats' in commands else 'NO'}")
print(f"\nTOOLS ({len(tools)}): {' '.join(sorted(tools))}")
orch = [t for t in tools if 'orchestr' in t.lower() or 'subagent' in t.lower() or 'sub_agent' in t.lower()]
for t in orch:
    print(f"\n=== {t} inputSchema ===")
    print(json.dumps(tools[t], indent=1)[:3000])
proc.stdin.close(); proc.terminate()
