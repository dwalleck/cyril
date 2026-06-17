#!/usr/bin/env python3
"""Capture a v2 (default Rust engine) ACP surface: slash commands + tools from
kiro.dev/commands/available. Usage: probe-v2-surface-2.8.0.py <path-to-kiro-cli-chat>.
v2 self-authenticates (no _kiro/auth callback). Used to confirm use_aws is alive on v2
in 2.8.0 and that the v2 surface is identical 2.7.1->2.8.0. See docs/kiro-2.8.0-wire-audit.md."""
import json, os, subprocess, threading, queue, time, tempfile, sys
KIRO=sys.argv[1]
CWD=tempfile.mkdtemp(prefix="v2surf-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
p=subprocess.Popen([KIRO,"acp"],cwd=CWD,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,bufsize=1)
q=queue.Queue(); threading.Thread(target=lambda:[q.put(l.strip()) for l in p.stdout if l.strip()],daemon=True).start()
i=[0]
def req(m,pr): i[0]+=1; p.stdin.write(json.dumps({"jsonrpc":"2.0","id":i[0],"method":m,"params":pr})+"\n"); p.stdin.flush(); return i[0]
def rep(rid,res): p.stdin.write(json.dumps({"jsonrpc":"2.0","id":rid,"result":res})+"\n"); p.stdin.flush()
CMDS=set(); TOOLS=set()
def handle(o):
    m=o.get("method"); rid=o.get("id"); pr=o.get("params",{}) or {}
    if rid is not None: rep(rid,{}); return
    if m and "commands/available" in m:
        for c in (pr.get("commands") or []):
            n=c.get("name") if isinstance(c,dict) else c
            if n: CMDS.add(n.lstrip("/"))
        for t in (pr.get("tools") or []):
            n=t.get("name") if isinstance(t,dict) else t
            if n: TOOLS.add(n)
def pump(until,to=40):
    end=time.time()+to
    while time.time()<end:
        try: raw=q.get(timeout=2)
        except queue.Empty: continue
        try: o=json.loads(raw)
        except: continue
        if "method" in o: handle(o)
        if o.get("id")==until and "result" in o: return o
    return None
req("initialize",{"protocolVersion":1,"clientCapabilities":{}}); pump(1,20)
nid=req("session/new",{"cwd":CWD,"mcpServers":[]}); pump(nid,40); pump(-1,4)
print("COMMANDS(%d):"%len(CMDS), " ".join(sorted(CMDS)))
print("TOOLS(%d):"%len(TOOLS), " ".join(sorted(TOOLS)))
p.stdin.close(); p.terminate()
