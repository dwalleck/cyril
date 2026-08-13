#!/usr/bin/env python3
"""LIVE v2 turn probe for the 2.18.0 audit — real backend, PAID (2 turns).

The mock A/B (probe-v2-turn-traffic-ab-2.16.0.py) already showed zero
structural delta 2.17.0→2.18.0, but the mock backend cannot produce thought
chunks, tool calls, permission requests, or metering frames. This drives the
real backend for the two behaviors 2.18.0's changelog puts on the v2 wire:

  turn 1 (reasoning) — model stays Auto; prompt asks for stepwise reasoning.
      2.18.0 ships `ReasoningContentForHistory` (reasoning replayed to the
      server under Auto, "enabling thinking with Auto"). Question: does an
      Auto-model turn now emit `agent_thought_chunk` on the ACP wire? Prior
      state (2.5.0 audit): thinking chunks were Anthropic-model-only.
  turn 2 (tool) — exact shell command → session/request_permission +
      tool_call/tool_call_update lifecycle + `_kiro.dev/metadata` in its
      POST-TURN form. Watch: the 2.17.0 Rust strings added overage metering
      vocabulary (subscription_tier, overage_*); does the backend emit any of
      it on metadata yet?

HOME-isolated per feedback_isolate_kiro_probes_with_home (real XDG_DATA_HOME
keeps v2 auth reachable).

    probe-v2-live-turn-2.18.0.py <path-to-kiro-cli-chat> <out.jsonl>
"""
import json, os, queue, subprocess, sys, tempfile, threading, time

KIRO = sys.argv[1]
OUT = open(sys.argv[2], "w")

CWD = tempfile.mkdtemp(prefix="v2live-")
subprocess.run("git init -q -b main", cwd=CWD, shell=True)

env = dict(os.environ)
env["HOME"] = tempfile.mkdtemp(prefix="v2livehome-")
env.setdefault("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))

p = subprocess.Popen([KIRO, "acp"], cwd=CWD, env=env, stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     text=True, bufsize=1)
q = queue.Queue()
threading.Thread(target=lambda: [q.put(l.strip()) for l in p.stdout if l.strip()],
                 daemon=True).start()
i = [0]
KINDS = {}
THOUGHT = []
METADATA = []
PERMS = []


def req(m, pr):
    i[0] += 1
    p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": i[0], "method": m, "params": pr}) + "\n")
    p.stdin.flush()
    return i[0]


def pump(until, to=120, tag=""):
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
                key = f"{m}:{u.get('sessionUpdate')}"
                if u.get("sessionUpdate") == "agent_thought_chunk":
                    THOUGHT.append((tag, (u.get("content") or {}).get("text", "")))
            if m == "_kiro.dev/metadata":
                METADATA.append((tag, pr))
            KINDS[f"{tag}|{key}"] = KINDS.get(f"{tag}|{key}", 0) + 1
        if rid is not None and m:
            if m == "session/request_permission":
                PERMS.append((tag, pr))
                opts = pr.get("options", [])
                pick = next((x for x in opts
                             if "allow" in (x.get("kind", "") + x.get("optionId", "")).lower()),
                            opts[0] if opts else None)
                res = ({"outcome": {"outcome": "selected", "optionId": pick["optionId"]}}
                       if pick else {"outcome": {"outcome": "cancelled"}})
                p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": res}) + "\n")
            else:
                p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": rid, "result": {}}) + "\n")
            p.stdin.flush()
            continue
        if rid == until and ("result" in o or "error" in o):
            return o
    return None


req("initialize", {"protocolVersion": 1, "clientCapabilities": {}})
pump(1, 20)
nid = req("session/new", {"cwd": CWD, "mcpServers": []})
sess = pump(nid, 60)
sid = (sess or {}).get("result", {}).get("sessionId")
print("sessionId:", sid)
pump(-1, 5)

TURNS = [
    ("reasoning",
     "What is 17*23 minus 12*9? Think through the arithmetic step by step "
     "carefully before giving the final answer."),
    ("tool",
     "Run this exact shell command and show me its output: echo cyril-wire-check-2.18.0"),
]
for tag, text in TURNS:
    print(f"--- turn: {tag}")
    rid = req("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": text}]})
    r = pump(rid, 300, tag=tag)
    print(f"    stopReason={((r or {}).get('result') or {}).get('stopReason')!r}")
    pump(-1, 8, tag=f"{tag}post")

print("\n=== notification kinds ===")
for k in sorted(KINDS):
    print(f"  {KINDS[k]:3}x {k}")
print(f"\n=== agent_thought_chunk under Auto: {len(THOUGHT)} chunks ===")
if THOUGHT:
    print("  first:", THOUGHT[0][1][:200])
print("\n=== permission requests ===")
for tag, pr in PERMS:
    print(f"  [{tag}] toolCall title={((pr.get('toolCall') or {}).get('title') or '')!r} "
          f"options={[o.get('optionId') for o in pr.get('options', [])]}")
print("\n=== metadata frames (post-turn form) ===")
for tag, pr in METADATA[-3:]:
    print(f"  [{tag}] keys={sorted(pr)}")
    mu = pr.get("meteringUsage")
    if mu is not None:
        print(f"        meteringUsage={json.dumps(mu)[:300]}")
    for k in ("subscriptionTier", "overageEnabled", "overageCap", "creditsUsed",
              "overageCreditsUsed", "subscription_tier", "overage_enabled"):
        if k in pr:
            print(f"        OVERAGE FIELD PRESENT: {k}={pr[k]}")

OUT.close()
p.stdin.close()
p.terminate()
