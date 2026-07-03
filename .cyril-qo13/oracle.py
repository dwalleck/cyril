#!/usr/bin/env python3
"""Oracle for cyril-qo13 (prove-it-prototype artifact).

Independent mechanism: raw JSON text extraction of what the REFERENCE client
(the Kiro v3 TUI, recorded via KIRO_ACP_RECORD_PATH) actually replied to each
session/request_permission in the same trace the probe replays. No cyril code,
no agent-client-protocol crate — a different implementation of "what is the
correct wire reply for picking option k".

Ground truth anchoring: the reference client picked NON-first options on
requests 2/3/10 and the agent demonstrably honored those picks downstream
(e.g. after id=3 -> option-1 "Keep the blob, add structured append", the agent
designed an appending `note add` command; the fs_writes behind ids 6-9 all
reached status=completed).

Run: python3 .cyril-qo13/oracle.py   (from repo root)
"""

import json

TRACE = "experiments/conductor-spike/kas-live-session-trace-2.11.0.jsonl"

recs = [json.loads(line) for line in open(TRACE)]
requests, replies = {}, {}
for r in recs:
    m = r.get("msg", {})
    if m.get("method") == "session/request_permission":
        requests[m["id"]] = m["params"]
    elif r["dir"] == "out" and "result" in m and m.get("id") in requests:
        replies[m["id"]] = m["result"]

for rid in sorted(requests):
    opts = requests[rid]["options"]
    result = replies.get(rid, {})
    outcome = result.get("outcome", {})
    replied = outcome.get("optionId")
    picked = next((k for k, o in enumerate(opts) if o["optionId"] == replied), None)
    kinds = ",".join(o["kind"] for o in opts)
    print(f"request id={rid} kinds=[{kinds}]")
    for k, o in enumerate(opts):
        print(f"  correct reply for pick k={k}: {o['optionId']}")
    print(
        f"  reference client replied: {replied!r} (=> picked k={picked})"
        f" outcome={outcome.get('outcome')!r}"
        f" outcome_meta={'yes' if '_meta' in outcome else 'no'}"
        f" top_meta={'yes' if '_meta' in result else 'no'}"
    )
