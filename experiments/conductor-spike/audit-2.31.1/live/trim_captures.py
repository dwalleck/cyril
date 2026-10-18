#!/usr/bin/env python3
"""Bundle + trim live-lane captures into experiments/conductor-spike/<prefix>-<topic>-2.31.1.jsonl.
Each output = header record per part {"probe":"part","source":..,"engine":..} followed by that capture's records;
strings > 1500 chars truncated (marked), lane tmp prefix replaced by <LIVE_TMP>, real home by <HOME>. Already token-redacted by the drivers."""
import json, os, re, sys
L = os.path.dirname(os.path.abspath(__file__)); OUTD = os.path.abspath(os.path.join(L, "..", ".."))
TMP = os.path.join(L, "tmp"); HOME = os.path.expanduser("~")
def tr(o):
    if isinstance(o, dict): return {k: tr(v) for k, v in o.items()}
    if isinstance(o, list): return [tr(v) for v in o]
    if isinstance(o, str):
        s = o.replace(TMP, "<LIVE_TMP>").replace(HOME, "<HOME>")
        return s if len(s) <= 1500 else s[:1500] + f"…<trimmed {len(s)-1500} chars>"
    return o
TOPICS = {
 "kas-init": [("kas-init-2.29.1-callback.jsonl","0.66.33"),("kas-init-2.29.1-none.jsonl","0.66.33"),("kas-init-2.31.1-callback.jsonl","0.66.34"),("kas-init-2.31.1-none.jsonl","0.66.34")],
 "kas-live-sweep": [("kas-live-sweep-2.29.1.jsonl","0.66.33"),("kas-live-sweep-2.31.1.jsonl","0.66.34"),("kas-live-sweep-2.31.1-run1-authwait.jsonl","0.66.34 (void: auth-wait)")],
 "kas-workflow-gateoff": [("kas-workflow-gateoff-2.29.1.jsonl","0.66.33"),("kas-workflow-gateoff-2.31.1.jsonl","0.66.34")],
 "kas-workflow-gateon": [("kas-workflow-gateon-2.29.1.jsonl","0.66.33"),("kas-workflow-gateon-2.31.1.jsonl","0.66.34")],
 "kas-kirohome": [("kas-lead-kirohome-2.29.1.jsonl","0.66.33"),("kas-lead-kirohome-2.31.1.jsonl","0.66.34")],
 "kas-mcp-ondemand": [("kas-lead-mcp-2.29.1.jsonl","0.66.33"),("kas-lead-mcp-2.31.1.jsonl","0.66.34")],
 "kas-sandbox": [("kas-lead-sandbox-2.29.1.jsonl","0.66.33"),("kas-lead-sandbox-2.31.1.jsonl","0.66.34")],
 "kas-workflow-launch-defaults": [("kas-lead-wfdefaults-2.29.1.jsonl","0.66.33"),("kas-lead-wfdefaults-2.31.1.jsonl","0.66.34")],
 "kas-handshake-cyril-vs-tui": [("kas-v3d-handshake-2.29.1.jsonl","0.66.33"),("kas-v3d-handshake-2.31.1.jsonl","0.66.34")],
 "kas-model-fallback": [("kas-v3d-fallback-2.29.1.jsonl","0.66.33"),("kas-v3d-fallback-2.31.1.jsonl","0.66.34")],
 "kas-session-load": [("kas-v3d-sessionload-2.29.1.jsonl","0.66.33"),("kas-v3d-sessionload-2.31.1.jsonl","0.66.34"),("kas-v3d-sessionload-cross-0.66.33to34.jsonl","0.66.34 loading 0.66.33 HOME")],
 "kas-command-dispatch": [("kas-v3c-v1-2.29.1.jsonl","0.66.33"),("kas-v3c-v1-2.31.1.jsonl","0.66.34")],
 "kas-subagent-interleave": [("kas-v3c-v2-2.31.1.jsonl","0.66.34")],
 "kas-mode-switch": [("kas-v3c-v3-2.29.1.jsonl","0.66.33"),("kas-v3c-v3-2.31.1.jsonl","0.66.34")],
 "kas-effort-level": [("kas-v3c-effort-2.31.1.jsonl","0.66.34")],
 "kas-agent-upgrade-permissions": [("kas-v3c-l9-2.31.1.jsonl","0.66.34")],
 "v2-live-sweep": [("v2-live-sweep-2.29.1.jsonl","v2 2.29.1"),("v2-live-sweep-2.31.1.jsonl","v2 2.31.1")],
 "v2-easein": [("v2-easein-2.31.1.jsonl","v2 2.31.1 launcher")],
}
for topic, parts in TOPICS.items():
    outp = os.path.join(OUTD, f"{topic}-2.31.1.jsonl")
    if os.path.exists(outp) and "--force" not in sys.argv:
        print("EXISTS, skip", outp); continue
    n = 0
    with open(outp, "w") as out:
        for src, eng in parts:
            out.write(json.dumps({"probe": "part", "source": f"audit-2.31.1/live/{src}", "engine": eng}) + "\n")
            for l in open(os.path.join(L, src)):
                out.write(json.dumps(tr(json.loads(l))) + "\n"); n += 1
    print(topic, n, os.path.getsize(outp))
