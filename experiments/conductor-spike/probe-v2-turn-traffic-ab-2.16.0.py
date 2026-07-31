#!/usr/bin/env python3
"""Same-day A/B of v2 TURN traffic via the mock backend — 2.16.0 audit gap-closure.

Every other probe in this audit stops at settle or at a command response, so
`_kiro.dev/metadata` was only ever observed in its EMPTY pre-turn form (2 field
paths) and no `session/update` turn variant was seen at all. This drives real
turns with `KIRO_MOCK_CHAT_RESPONSE` — a free, deterministic, offline backend —
and captures everything the turn emits, on both binaries.

Contract (see reference_kiro_env_vars): the var is a FILE PATH to a JSON array
of arrays; each outer element serves one turn, inner strings stream as
agent_message_chunks. STRINGS ONLY — any object entry panics kiro-cli at
initialize, which is why this cannot produce tool calls, permission prompts or
metering frames. Prompting past the end of the script returns
-32603 "Kiro failed to generate a response" rather than falling through to the
real backend, so overrun is safe but must be expected.

Also verifies 2.16.0's third changelog line — "context usage percentage now
recalculates when switching models via /model" — by sampling /context, doing a
turn, switching model, and sampling /context again.

ZERO CREDITS: no network, no real model call.

    probe-v2-turn-traffic-ab-2.16.0.py <path-to-kiro-cli-chat> <out.jsonl>
"""
import json, os, subprocess, threading, queue, time, tempfile, sys

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")

CWD = tempfile.mkdtemp(prefix="v2turnab-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

# One outer element per turn; inner strings stream as chunks and concatenate.
MOCK = os.path.join(CWD, "mock.json")
with open(MOCK, "w") as fh:
    json.dump([["MOCK", "-ONE"], ["MOCK", "-TWO"], ["MOCK", "-THREE"]], fh)

env = dict(os.environ)
env["KIRO_MOCK_CHAT_RESPONSE"] = MOCK

p = subprocess.Popen([KIRO, "acp"], cwd=CWD, env=env, stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
SEEN = []          # (method_or_response_tag, params_or_result)


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def rep(rid, res):
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
    p.stdin.flush()


def pump(until, to=60, tag=None):
    end = time.time() + to
    while time.time() < end:
        try:
            raw = q.get(timeout=2)
        except queue.Empty:
            continue
        try:
            o = json.loads(raw)
        except Exception:
            continue
        OUT.write(raw + "\n")
        OUT.flush()
        m, rid, pr = o.get("method"), o.get("id"), o.get("params") or {}
        if m and rid is None:
            key = m
            if m.endswith("session/update"):
                u = pr.get("update") or {}
                key = f"{m}:{u.get('sessionUpdate') or sorted(u)[:1]}"
            SEEN.append((f"{tag}|{key}" if tag else key, pr))
        if rid is not None and m:
            rep(rid, {})
            continue
        if rid == until and ("result" in o or "error" in o):
            if tag:
                SEEN.append((f"{tag}|R:{until}", o.get("result") or o.get("error")))
            return o
    return None


req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 40)
sid = (sess or {}).get("result", {}).get("sessionId")
pump(-1, 5)
print("sessionId:", sid)


def ctx():
    rid = req("_kiro.dev/commands/execute",
              {"sessionId": sid, "command": {"command": "context", "args": {}}})
    r = pump(rid, 45)
    return ((r or {}).get("result") or {}).get("data", {})


def turn(n, text):
    rid = req("session/prompt",
              {"sessionId": sid, "prompt": [{"type": "text", "text": text}]})
    r = pump(rid, 90, tag=f"turn{n}")
    print(f"  turn{n} stopReason={((r or {}).get('result') or {}).get('stopReason')!r} "
          f"error={bool((r or {}).get('error'))}")
    pump(-1, 6, tag=f"turn{n}post")   # drain post-turn notifications (metadata et al.)
    return r


c0 = ctx()
print("pre-turn  ctx%:", c0.get("contextUsagePercentage"), "model:", c0.get("model"))

turn(1, "first probe prompt")
c1 = ctx()
print("post-turn ctx%:", c1.get("contextUsagePercentage"), "model:", c1.get("model"))

# 2.16.0 changelog: "context usage percentage now recalculates when switching models"
oid = req("_kiro.dev/commands/options", {"sessionId": sid, "command": "model"})
opts = ((pump(oid, 30) or {}).get("result") or {}).get("options") or []
cur = c1.get("model")
target = next((o for o in opts if o.get("value") and o.get("value") != cur), None)
print("model options:", len(opts), "-> switching to:", (target or {}).get("value"))
if target:
    mid = req("_kiro.dev/commands/execute",
              {"sessionId": sid,
               "command": {"command": "model", "args": {"value": target["value"]}}})
    pump(mid, 45, tag="modelswitch")
    pump(-1, 5, tag="modelswitchpost")
    c2 = ctx()
    print("post-switch ctx%:", c2.get("contextUsagePercentage"), "model:", c2.get("model"))
    print("RECOMPUTED:", c2.get("contextUsagePercentage") != c1.get("contextUsagePercentage"))

turn(2, "second probe prompt")

print("\n=== notification kinds seen (turn-scoped) ===")
kinds = {}
for k, _ in SEEN:
    base = k.split("|", 1)[1] if "|" in k else k
    kinds[base] = kinds.get(base, 0) + 1
for k in sorted(kinds):
    print(f"  {kinds[k]:3}x {k}")

OUT.write(json.dumps({"_probe_seen": [[k, v] for k, v in SEEN]}) + "\n")
OUT.close()
p.stdin.close()
p.terminate()
