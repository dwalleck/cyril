#!/usr/bin/env python3
"""
cyril-gfkm design-first probe: capture the LIVE `usage_update` wire shape from
`omp acp` (standard ACP). Gates the UsageRecord schema — confirms field names,
whether cost rides the wire or must be computed, granularity (per-turn vs
per-message), and model/provider correlation. omp handles its own provider auth
from ~/.omp, so NO auth callback and NO HOME isolation (real env).
"""
import json, os, subprocess, threading, queue, time, tempfile, signal

SCRATCH = os.path.dirname(os.path.abspath(__file__))
TRACE = os.path.join(SCRATCH, "omp-usage-update-trace.jsonl")
OMP = os.path.expanduser("~/.local/bin/omp")
MODEL = os.environ.get("OMP_MODEL", "deepseek-v4-flash")  # cheapest
CWD = tempfile.mkdtemp(prefix="omp-usage-")

proc = subprocess.Popen([OMP, "acp"], cwd=CWD,
                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                        stderr=open(os.path.join(SCRATCH, "omp-usage-stderr.log"), "w"),
                        text=True, bufsize=1, start_new_session=True)
assert proc.stdin and proc.stdout
PIN, POUT = proc.stdin, proc.stdout
msgs = queue.Queue()
trace = open(TRACE, "w")

def record(direction, obj):
    trace.write(json.dumps({"ts": time.time(), "dir": direction, "msg": obj}) + "\n")
    trace.flush()

threading.Thread(target=lambda: ([msgs.put(l.strip()) for l in POUT if l.strip()], msgs.put(None)), daemon=True).start()
_id = [10]

def send(o):
    record("client->agent", o)
    PIN.write(json.dumps(o) + "\n"); PIN.flush()

def req(m, p):
    _id[0] += 1
    send({"jsonrpc": "2.0", "id": _id[0], "method": m, "params": p})
    return _id[0]

UPDATES = []
def handle(o):
    m = o["method"]
    if m == "session/request_permission":
        opts = o.get("params", {}).get("options", [])
        pick = next((x for x in opts if "allow" in (str(x.get("kind", "")) + str(x.get("optionId", ""))).lower()), opts[0] if opts else None)
        send({"jsonrpc": "2.0", "id": o["id"], "result": {"outcome": {"outcome": "selected", "optionId": pick["optionId"]}} if pick else {"outcome": {"outcome": "cancelled"}}})
    else:
        send({"jsonrpc": "2.0", "id": o["id"], "result": {}})

def pump(until_id=None, timeout=120, idle_exit=None):
    end = time.time() + timeout
    last = time.time()
    while time.time() < end:
        try:
            raw = msgs.get(timeout=1)
        except queue.Empty:
            if idle_exit and time.time() - last > idle_exit:
                return None
            continue
        if raw is None:
            return None
        last = time.time()
        try:
            o = json.loads(raw)
        except Exception:
            continue
        record("agent->client", o)
        if "method" in o and "id" in o:
            handle(o)
        elif o.get("method") == "session/update":
            UPDATES.append(o["params"])
        elif "id" in o and until_id is not None and o["id"] == until_id:
            return o
    return None

iid = req("initialize", {"protocolVersion": 1,
                         "clientInfo": {"name": "cyril-usage-probe", "version": "0.0.1"},
                         "clientCapabilities": {"fs": {"readTextFile": True, "writeTextFile": True}}})
init = pump(iid, 30)
print("== initialize agentInfo:", json.dumps((init or {}).get("result", {}).get("agentInfo")))

nid = req("session/new", {"cwd": CWD, "mcpServers": []})
new = pump(nid, 30)
res = (new or {}).get("result", {})
sid = res.get("sessionId")
print("== session/new keys:", sorted(res.keys()))
# try to pin the cheap model if omp exposes a model configOption / mode
cfg = res.get("configOptions") or res.get("modes")
print("== modes/config:", json.dumps(cfg)[:300] if cfg else "none")

# small prompt that forces at least a tool call + text (exercise both message kinds)
t0 = time.time()
pid = req("session/prompt", {"sessionId": sid,
          "prompt": [{"type": "text", "text": "Reply with exactly one word: pong"}]})
resp = pump(pid, 180)
print("== prompt response:", json.dumps((resp or {}).get("result") or (resp or {}).get("error"))[:200], f"({time.time()-t0:.1f}s)")
pump(timeout=6, idle_exit=4)

trace.close()
try:
    os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
except Exception:
    proc.terminate()

# --- characterize ---
kinds = {}
for u in UPDATES:
    upd = u.get("update", u)
    k = upd.get("sessionUpdate", "?")
    kinds[k] = kinds.get(k, 0) + 1
print("\n== session/update kinds seen:", kinds)

usage = [u.get("update", u) for u in UPDATES if (u.get("update", u).get("sessionUpdate") == "usage_update")]
print(f"\n== usage_update frames: {len(usage)}")
for i, u in enumerate(usage):
    print(f"  [{i}] full payload:\n{json.dumps(u, indent=2)}")
if not usage:
    print("  NONE on session/update — dumping any frame containing 'usage' or 'token' or 'cost':")
    for u in UPDATES:
        s = json.dumps(u)
        if any(w in s.lower() for w in ("usage", "token", "cost")):
            print("   ", json.dumps(u.get("update", u))[:400])

print("\nTRACE:", TRACE, f"({sum(1 for _ in open(TRACE))} frames)")
