#!/usr/bin/env python3
"""Classify each batch run's wedge signature + cancel outcome (cyril-bh7g / cyril-14ou).

Signature A (emission-death candidate): KAS logged `Execution succeeded` for the wedged
turn and persisted turn_end promptly -> wire.jsonl decides emitted-vs-lost.
Signature B (backend stall, the run-1-archived shape): execution never succeeded.
"""

import glob
import json
import pathlib
import re
import sys

HERE = pathlib.Path(__file__).resolve().parent

for run_dir in sorted(HERE.glob("captures/run-*")):
    if run_dir.name.endswith("archived"):
        continue
    verdict_p = run_dir / "verdict.json"
    if not verdict_p.exists():
        print(f"{run_dir.name}: no verdict yet")
        continue
    v = json.loads(verdict_p.read_text())
    line = f"{run_dir.name}: outcome={v['outcome']} rounds={v['rounds_answered']} cancel={v.get('cancel_result')}"

    sid = None
    for l in open(run_dir / "stdout.jsonl"):
        m = re.search(r'sess_[0-9a-f-]{36}', json.loads(l)["text"])
        if m:
            sid = m.group(0)
            break

    if v["outcome"] == "wedged" and sid:
        # KAS internal view: did the last execution succeed? was turn_end persisted?
        began = succeeded = 0
        for logp in glob.glob(str(pathlib.Path.home() / ".kiro/logs/*/kiro.log")):
            body = open(logp, errors="replace").read()
            if sid not in body:
                continue
            began = body.count("Execution began")
            succeeded = body.count("Execution succeeded")
        n_te = 0
        for tp in glob.glob(str(pathlib.Path.home() / f".kiro/sessions/*/{sid}/messages.jsonl")):
            n_te = sum(1 for x in open(tp) if json.loads(x)["payload"].get("type") == "turn_end")
        # wire view: terminal frames for the wedged (last) turn?
        wire = [json.loads(x) for x in open(run_dir / "wire.jsonl")]
        prompts = [f for f in wire if isinstance(f["msg"], dict) and f["msg"].get("method") == "session/prompt" and f["dir"] == "client->agent"]
        last_id = prompts[-1]["msg"]["id"] if prompts else None
        wire_te = wire_resp = cancel_sent = post_cancel = 0
        cancel_ts = None
        for f in wire:
            m = f["msg"]
            if not isinstance(m, dict):
                continue
            if m.get("method") == "session/cancel":
                cancel_sent += 1
                cancel_ts = f["ts"]
            if m.get("method") == "session/update":
                u = m["params"].get("update", {})
                if u.get("sessionUpdate") == "session_info_update" and u.get("_meta", {}).get("kiro", {}).get("kind") == "turn_end":
                    wire_te += 1
                    if cancel_ts and f["ts"] > cancel_ts:
                        post_cancel += 1
            if "result" in m and isinstance(m.get("result"), dict) and "stopReason" in m["result"]:
                wire_resp += 1
                if m.get("id") == last_id:
                    line += f" [wedged-turn response ON WIRE: {m['result']['stopReason']}]"
        sig = "B-stall" if succeeded < began else "A-internal-complete"
        line += f" | sig={sig} began={began} succeeded={succeeded} persisted_turn_end={n_te} wire_turn_end={wire_te} wire_resp={wire_resp} cancel_frames_sent={cancel_sent} post_cancel_turn_end={post_cancel}"
    print(line)
