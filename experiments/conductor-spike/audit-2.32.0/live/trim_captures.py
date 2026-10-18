#!/usr/bin/env python3
"""Bundle + trim live-lane captures into experiments/conductor-spike/<prefix>-<topic>-2.32.0.jsonl.
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
 "kas-init": [("kas-init-2.31.1-callback.jsonl","0.66.34"),("kas-init-2.31.1-none.jsonl","0.66.34"),("kas-init-2.32.0-callback.jsonl","0.66.43"),("kas-init-2.32.0-none.jsonl","0.66.43")],
 "kas-live-sweep": [("kas-live-sweep-2.31.1.jsonl","0.66.34"),("kas-live-sweep-2.32.0.jsonl","0.66.43")],
 "kas-workflow-gateoff": [("kas-workflow-gateoff-2.31.1.jsonl","0.66.34"),("kas-workflow-gateoff-2.32.0.jsonl","0.66.43")],
 "kas-workflow-gateon": [("kas-workflow-gateon-2.31.1.jsonl","0.66.34"),("kas-workflow-gateon-2.32.0.jsonl","0.66.43")],
 "kas-shell-type": [("kas-lead-shell-2.31.1.jsonl","0.66.34"),("kas-lead-shell-2.32.0.jsonl","0.66.43")],
 "kas-consent-scope": [("kas-lead-folder-2.31.1.jsonl","0.66.34"),("kas-lead-folder-2.32.0.jsonl","0.66.43"),("kas-lead-folder-control-2.32.0.jsonl","0.66.43 control re-run"),("kas-lead-shellecho-2.32.0.jsonl","0.66.43 shell echo")],
 "kas-workflow-attention": [("kas-lead-attention-2.32.0.jsonl","0.66.43")],
 "kas-output-styles": [("kas-lead-outstyle-2.31.1.jsonl","0.66.34"),("kas-lead-outstyle-2.32.0.jsonl","0.66.43")],
 "kas-diagrams": [("kas-lead-diagrams-run1-2.32.0.jsonl","0.66.43 run1"),("kas-lead-diagrams-2.32.0.jsonl","0.66.43"),("kas-lead-diagrams-2.31.1.jsonl","0.66.34")],
 "kas-thinking-intake": [("kas-lead-thinking-2.31.1.jsonl","0.66.34"),("kas-lead-thinking-2.32.0.jsonl","0.66.43")],
 "kas-recipe-include": [("kas-lead-include-2.32.0.jsonl","0.66.43")],
 "kas-workflow-interrupt": [("kas-lead-interrupt-sig15-2.31.1.jsonl","0.66.34 SIGTERM"),("kas-lead-interrupt-sig15-2.32.0.jsonl","0.66.43 SIGTERM"),("kas-lead-interrupt-sig9-2.31.1.jsonl","0.66.34 SIGKILL"),("kas-lead-interrupt-sigkill-2.32.0.jsonl","0.66.43 SIGKILL")],
 "kas-image-fish-export": [("kas-lead-own-2.31.1.jsonl","0.66.34"),("kas-lead-own-2.32.0.jsonl","0.66.43")],
 "kas-mcp-subagent": [("kas-lead-mcp-2.32.0.jsonl","0.66.43 MCP"),("kas-v3c-v2-2.32.0.jsonl","0.66.43 sub-agent")],
 "host-cloud": [("host-cloud-2.32.0.jsonl","host 2.32.0 stream-json"),("host-cloudtrace-2.32.0.jsonl","host 2.32.0 strace-extracted ACP"),("host-l4-2.32.0.jsonl","host 2.32.0 text mode")],
}
for topic, parts in TOPICS.items():
    outp = os.path.join(OUTD, f"{topic}-2.32.0.jsonl")
    if os.path.exists(outp) and "--force" not in sys.argv:
        print("EXISTS, skip", outp); continue
    n = 0
    with open(outp, "w") as out:
        for src, eng in parts:
            out.write(json.dumps({"probe": "part", "source": f"audit-2.32.0/live/{src}", "engine": eng}) + "\n")
            for l in open(os.path.join(L, src)):
                out.write(json.dumps(tr(json.loads(l))) + "\n"); n += 1
    print(topic, n, os.path.getsize(outp))
