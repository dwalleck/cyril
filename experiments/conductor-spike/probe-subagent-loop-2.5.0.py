#!/usr/bin/env python3
"""
Probe: does 2.5.0's subagent REVIEW-LOOP feature add fields to the
`_kiro.dev/subagent/list_update` notification that cyril's SubagentTracker parses?

2.4.1 baseline per-entry keys: agentName, dependsOn, group, initialQuery, role,
                               sessionId, sessionName, status   (no loop field)
2.5.0 binary adds types: orchestration::types::{LoopConfig, LoopTriggerData}.

Strategy: prompt the agent to run a tiny 2-stage subagent pipeline with an explicit
loop_to (checker -> writer on trigger "NEEDS_CHANGES", max_iterations 2), pure text,
no file I/O. Capture every subagent/list_update + inbox + spawn frame and dump the
union of field keys, so we can diff against the 2.4.1 baseline.

Runs in an isolated empty temp cwd so spawned children never touch the cyril repo.
"""
import json, os, subprocess, tempfile, threading, time
from collections import Counter

KIRO = os.path.expanduser(os.environ.get("KIRO_BIN", "~/.local/bin/kiro-cli-chat"))
TAG = os.environ.get("PROBE_TAG", "2.5.0")
CWD = tempfile.mkdtemp(prefix="cyril-subagent-loop-")
LOG_PATH = f"/tmp/cyril-probe-subagent-loop-{TAG}.log"

PROMPT = (
    "Use the `subagent` tool to run a minimal 2-stage blocking pipeline. Do NOT read or "
    "write any files; this is a pure-text exercise.\n"
    "  - Stage 1, name 'writer': output exactly the line `DRAFT v{n}` where n is the attempt number.\n"
    "  - Stage 2, name 'checker', depends_on 'writer': on the FIRST run output exactly "
    "`NEEDS_CHANGES: add a version note`; on any later run output exactly `APPROVED`.\n"
    "Configure the checker stage with loop_to='writer', trigger='NEEDS_CHANGES', max_iterations=2 "
    "so it loops back to the writer once. Then report the final checker output."
)


def main():
    log = open(LOG_PATH, "w")
    print(f"[setup] binary={KIRO}\n[setup] cwd={CWD}\n[setup] log={LOG_PATH}\n")
    proc = subprocess.Popen([KIRO, "acp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, cwd=CWD)
    incoming, lock, nid = [], threading.Lock(), [1]

    def reader():
        while True:
            line = proc.stdout.readline()
            if not line:
                return
            t = line.decode(errors="replace").rstrip("\n")
            log.write(f"S->C {t}\n"); log.flush()
            try:
                with lock:
                    incoming.append(json.loads(t))
            except json.JSONDecodeError:
                pass
    threading.Thread(target=reader, daemon=True).start()

    def send(method, params, resp=True):
        m = {"jsonrpc": "2.0", "method": method, "params": params}
        if resp:
            m["id"] = nid[0]; nid[0] += 1
        log.write(f"C->S {json.dumps(m)}\n"); log.flush()
        proc.stdin.write((json.dumps(m) + "\n").encode()); proc.stdin.flush()
        return m.get("id")

    def send_resp(rid, result):
        m = {"jsonrpc": "2.0", "id": rid, "result": result}
        proc.stdin.write((json.dumps(m) + "\n").encode()); proc.stdin.flush()
        log.write(f"C->S {json.dumps(m)}\n"); log.flush()

    def auto_approve():
        seen = set()
        while True:
            with lock:
                frames = list(incoming)
            for f in frames:
                if f.get("method") == "session/request_permission" and f.get("id") not in seen:
                    seen.add(f["id"])
                    opts = f.get("params", {}).get("options", [])
                    allow = next((o for o in opts if o.get("kind") in ("allow_once", "allow_always")), opts[0] if opts else None)
                    if allow:
                        send_resp(f["id"], {"outcome": {"outcome": "selected", "optionId": allow["optionId"]}})
            time.sleep(0.1)
    threading.Thread(target=auto_approve, daemon=True).start()

    def waitfor(rid, t):
        d = time.time() + t
        while time.time() < d:
            with lock:
                for f in incoming:
                    if f.get("id") == rid and ("result" in f or "error" in f):
                        return f
            time.sleep(0.1)
        return None

    send("initialize", {"protocolVersion": 1, "clientCapabilities": {"fs": {"readTextFile": False, "writeTextFile": False}, "terminal": False}, "clientInfo": {"name": "cyril-probe", "version": "0.0.1"}})
    time.sleep(0.5)
    rid = send("session/new", {"cwd": CWD, "mcpServers": []})
    r = waitfor(rid, 20)
    sid = r["result"]["sessionId"]
    # use a strong model so it follows the orchestration instructions
    send("_kiro.dev/commands/execute", {"command": {"command": "model", "args": {"value": "claude-opus-4.8"}}}, resp=False)
    time.sleep(1.5)
    print(f"[*] session {sid}, sending subagent-loop prompt (long timeout)...")
    rid = send("session/prompt", {"sessionId": sid, "prompt": [{"type": "text", "text": PROMPT}]})
    r = waitfor(rid, 300)
    print(f"[*] prompt stop_reason={(r or {}).get('result', {}).get('stopReason')}")
    time.sleep(1.0)

    with lock:
        frames = list(incoming)
    methods = Counter(f.get("method") for f in frames if f.get("method"))
    print("\n=== subagent/orchestration methods seen ===")
    for k, v in methods.most_common():
        if k and ("subagent" in k or "spawn" in k or "inbox" in k or "message" in k):
            print(f"   {v:4d}  {k}")

    # union of list_update entry keys
    top_keys, entry_keys, loop_frames = set(), set(), []
    for f in frames:
        if f.get("method") == "_kiro.dev/subagent/list_update":
            p = f.get("params", {})
            top_keys |= set(p.keys())
            for v in p.values():
                if isinstance(v, list):
                    for it in v:
                        if isinstance(it, dict):
                            entry_keys |= set(it.keys())
        blob = json.dumps(f)
        if "loop" in blob.lower() or "iteration" in blob.lower() or "NEEDS_CHANGES" in blob:
            loop_frames.append(f)
    print("\n=== 2.5.0 subagent/list_update schema ===")
    print("  top-level keys:", sorted(top_keys))
    print("  per-entry keys:", sorted(entry_keys))
    print("  >> NEW vs 2.4.1 baseline {agentName,dependsOn,group,initialQuery,role,sessionId,sessionName,status}:")
    base = {"agentName", "dependsOn", "group", "initialQuery", "role", "sessionId", "sessionName", "status"}
    print("     entry NEW:", sorted(entry_keys - base) or "(none)")
    print("     top   NEW:", sorted(top_keys - {"pendingStages", "subagents"}) or "(none)")
    print(f"\n=== frames mentioning loop/iteration/NEEDS_CHANGES: {len(loop_frames)} ===")
    for f in loop_frames[:8]:
        print("   " + json.dumps(f)[:300])
    print("\nfull log:", LOG_PATH)
    proc.terminate()


if __name__ == "__main__":
    main()
