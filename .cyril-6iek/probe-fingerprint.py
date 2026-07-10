#!/usr/bin/env python3
"""Probe (cyril-6iek): what does the wire actually expose, at handshake time,
that distinguishes a KAS-speaking subprocess from a v2 one?

Claimed fingerprints (from the 2026-07-02 audit + issue text):
  F1: KAS `initialize` response carries agentCapabilities._meta.kiro
      (checkpoints/sessionList/extensionMethods...); v2's does not.
  F2: KAS session/new returns a `sess_`-prefixed sessionId; v2 returns a UUID.

Runs BOTH engines live via the installed kiro-cli wrapper and prints the
discriminating fields verbatim. Oracle = the committed 2.11.0 live traces
(recorded by kiro's own KIRO_ACP_RECORD_PATH recorder / the reference client,
a different mechanism), compared in findings.md.
"""
import json, subprocess, sys, tempfile, threading, queue, time

def handshake(label, argv):
    cwd = tempfile.mkdtemp(prefix=f"6iek-{label}-")
    proc = subprocess.Popen(argv, cwd=cwd, stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                            text=True, bufsize=1)
    q = queue.Queue()
    threading.Thread(target=lambda: [q.put(l) for l in proc.stdout], daemon=True).start()
    resp = {}

    def send(obj):
        proc.stdin.write(json.dumps(obj) + "\n")
        proc.stdin.flush()

    def wait_for(rid, timeout):
        end = time.time() + timeout
        while time.time() < end:
            try:
                o = json.loads(q.get(timeout=0.5))
            except (queue.Empty, ValueError):
                continue
            if o.get("method") and o.get("id") is not None:
                # server->client request (e.g. auth callback): decline honestly
                send({"jsonrpc": "2.0", "id": o["id"],
                      "error": {"code": -32000, "message": "probe: no responder"}})
            elif o.get("id") == rid:
                return o
        return None

    send({"jsonrpc": "2.0", "id": 1, "method": "initialize",
          "params": {"protocolVersion": 1, "clientCapabilities": {}}})
    resp["initialize"] = wait_for(1, 60)
    send({"jsonrpc": "2.0", "id": 2, "method": "session/new",
          "params": {"cwd": cwd, "mcpServers": []}})
    resp["session/new"] = wait_for(2, 60)
    proc.kill()
    return resp

def report(label, resp):
    print(f"\n=== {label} ===")
    init = resp.get("initialize")
    if not init or "result" not in init:
        print(f"initialize FAILED: {json.dumps(init)}")
        return
    r = init["result"]
    caps = r.get("agentCapabilities") or {}
    meta = caps.get("_meta")
    print(f"protocolVersion:        {r.get('protocolVersion')}")
    print(f"agentInfo:              {json.dumps(r.get('agentInfo'))}")
    print(f"agentCapabilities keys: {sorted(caps.keys())}")
    print(f"_meta present:          {meta is not None}")
    if isinstance(meta, dict):
        kiro = meta.get("kiro")
        print(f"_meta.kiro keys:        {sorted(kiro.keys()) if isinstance(kiro, dict) else kiro}")
        if isinstance(kiro, dict):
            print(f"_meta.kiro:             {json.dumps(kiro, sort_keys=True)}")
    new = resp.get("session/new")
    if not new or "result" not in new:
        print(f"session/new FAILED: {json.dumps(new)}")
        return
    sid = new["result"].get("sessionId", "")
    print(f"sessionId:              {sid}")
    print(f"  sess_ prefixed:       {sid.startswith('sess_')}")

report("v2 (kiro-cli acp)", handshake("v2", ["kiro-cli", "acp"]))
report("KAS (kiro-cli acp --agent-engine kas)",
       handshake("kas", ["kiro-cli", "acp", "--agent-engine", "kas"]))
