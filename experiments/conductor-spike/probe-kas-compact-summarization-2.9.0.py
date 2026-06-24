#!/usr/bin/env python3
"""Trigger + capture the KAS `summarization_completed` session_info_update sub-type.
Builds a few trivial turns of history (compaction strategies need messages.length>=3/5),
then calls `_kiro/session/compact` and dumps any summarization-family session_info_update
payload verbatim. Direct-spawn free path. Costs a few small turns of credits.
Usage: probe-kas-compact-summarization-2.9.0.py <path-to-acp-server.js> <out.jsonl>"""
import json, os, subprocess, threading, queue, time, tempfile, sys

SERVER = sys.argv[1]; OUT = sys.argv[2]
assert os.path.exists(SERVER), SERVER
runtime = os.environ.get("KIRO_AGENT_PATH", "node")
TOKEN = os.path.expanduser("~/.aws/sso/cache/kiro-auth-token.json")
CWD = tempfile.mkdtemp(prefix="kas-compact-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)
log = open(OUT, "w")
def rec(d, o): log.write(json.dumps({"d": d, **o}) + "\n"); log.flush()
proc = subprocess.Popen([runtime, "--experimental-wasm-modules", SERVER, "--transport=stdio"],
                        cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in proc.stdout if l.strip()], daemon=True).start()
i = [0]
def send(o): proc.stdin.write(json.dumps(o) + "\n"); proc.stdin.flush(); rec("C->A", o)
def req(m, p): i[0]+=1; send({"jsonrpc":"2.0","id":i[0],"method":m,"params":p}); return i[0]
def rep(rid, res): send({"jsonrpc":"2.0","id":rid,"result":res})
def read_token():
    try: return json.load(open(TOKEN)).get("accessToken")
    except Exception: return None

SUMM_KINDS = {"summarization_completed","summarization","summarization_separator","summary_message","recap"}
captured = []   # full session_info_update payloads whose _meta.kiro.kind is summarization-family
info_kinds = {}
def on(o):
    m=o.get("method"); p=o.get("params",{}) or {}
    rec("A->C", o)
    if o.get("id") is not None and m:
        if m=="_kiro/auth/getAccessToken": rep(o["id"],{"accessToken":read_token()})
        elif m=="session/request_permission":
            opts=p.get("options") or []
            allow=next((x for x in opts if "allow" in json.dumps(x).lower()), opts[0] if opts else None)
            rep(o["id"], {"outcome":{"outcome":"selected","optionId":(allow or {}).get("optionId")}})
        else: rep(o["id"],{})
        return
    if m=="session/update":
        u=p.get("update") or {}
        if u.get("sessionUpdate")=="session_info_update":
            k=((u.get("_meta") or {}).get("kiro") or {}).get("kind")
            info_kinds[k]=info_kinds.get(k,0)+1
            if k in SUMM_KINDS or "summar" in json.dumps(u).lower():
                captured.append(u)

def pump(until, to):
    end=time.time()+to
    while time.time()<end:
        try: raw=q.get(timeout=2)
        except queue.Empty: continue
        try: o=json.loads(raw)
        except Exception: continue
        if "method" in o: on(o)
        if until is not None and o.get("id")==until and ("result" in o or "error" in o):
            return o
    return None

pump(req("initialize",{"protocolVersion":1,"clientCapabilities":{}}),20)
nid=req("session/new",{"cwd":CWD,"mcpServers":[]}); sn=pump(nid,40)
sid=(sn or {}).get("result",{}).get("sessionId")
print("sessionId:", sid)
for prompt in ["What is 2+2? Answer in one word.",
               "Name one primary color. One word.",
               "What is the capital of France? One word.",
               "Say the word done."]:
    pid=req("session/prompt",{"sessionId":sid,"prompt":[{"type":"text","text":prompt}]})
    r=pump(pid,180); print(f"  turn '{prompt[:24]}...' -> {(r or {}).get('result',{}).get('stopReason')}")

print("\n=== calling _kiro/session/compact ===")
cid=req("_kiro/session/compact",{"sessionId":sid})
cr=pump(cid,120)
print("compact result:", json.dumps((cr or {}).get("result", cr), indent=1)[:500])
pump(None,5)  # drain trailing session_info_update

print("\n=== session_info_update kinds seen ===", json.dumps(info_kinds))
print(f"\n=== summarization-family payloads captured: {len(captured)} ===")
for c in captured:
    print(json.dumps(c, indent=1)[:1500]); print("---")
print(f"\nraw wire -> {OUT}")
proc.stdin.close(); proc.terminate()
