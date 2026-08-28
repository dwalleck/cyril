#!/usr/bin/env python3
"""P1 — how many usage-panel refresh triggers does ONE turn produce?

Segments a committed live ACP capture into turns and counts, per turn, the
notifications that reach `refresh_usage_panel_from_log` through the usage
observer:

  * context sample -> UsageWrite::Context
      v2  : inbound `_kiro.dev/metadata` carrying `contextUsagePercentage`
            (crates/cyril-core/src/protocol/convert/kiro.rs:340-347)
      KAS : inbound `session/update` whose `update._meta.kiro.kind` is
            `context_usage` AND which carries a top-level `usagePercentage`
            in that same object — both conditions are required by the
            converter's dispatch arm (convert/kas.rs:314-326). A frame with
            `contextUsage.usagePercentage` but no `kind` does NOT convert
            and is therefore not a refresh trigger.
  * turn end -> UsageWrite::Turn   (exactly one per completed turn)

Two windows are reported, because they answer different questions and the
oracle uses the second:

  * RESPONSE window  — prompt request to the response bearing the same id.
    What the agent was actively working on.
  * INTER-PROMPT window — prompt request to the NEXT prompt request. The
    operationally relevant one: `refresh_usage_panel_from_log` fires on every
    context sample whether or not a turn is open, so what bounds the burst is
    how many arrive between two operator prompts, not how many land before the
    response.

Mechanism: structural JSON parse + id-correlated turn segmentation.
Run: python3 .cyril-nanu/probe_cadence.py
"""
import json
import sys
from pathlib import Path

TRACES = [
    ("v2", Path("experiments/conductor-spike/v2-live-session-trace-2.11.0.jsonl")),
    ("kas", Path("experiments/conductor-spike/kas-live-session-trace-2.11.0.jsonl")),
]


def is_context_sample(engine, msg):
    if engine == "v2":
        return (
            msg.get("method") == "_kiro.dev/metadata"
            and "contextUsagePercentage" in (msg.get("params") or {})
        )
    params = msg.get("params") or {}
    update = params.get("update") or {}
    kiro = ((update.get("_meta") or {}).get("kiro")) or {}
    return (
        msg.get("method") == "session/update"
        and kiro.get("kind") == "context_usage"
        and "usagePercentage" in kiro
    )


def analyse(engine, path):
    records = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            records.append(json.loads(line))
        except json.JSONDecodeError:
            continue

    response_window = []      # samples between a prompt and its own response
    inter_prompt = []         # samples between a prompt and the next prompt
    open_id = None
    resp_samples = 0
    inter_samples = 0
    started = False
    before_first = 0
    after_response = 0        # the reconciliation term: samples in neither

    for rec in records:
        msg = rec.get("msg") or {}
        direction = rec.get("dir")
        if direction == "out" and msg.get("method") == "session/prompt":
            if started:
                inter_prompt.append(inter_samples)
            started = True
            inter_samples = 0
            open_id = msg.get("id")
            resp_samples = 0
            continue
        if (
            direction == "in"
            and "result" in msg
            and open_id is not None
            and msg.get("id") == open_id
        ):
            response_window.append(resp_samples)
            open_id = None
            continue
        if direction == "in" and is_context_sample(engine, msg):
            if not started:
                before_first += 1
                continue
            inter_samples += 1
            if open_id is not None:
                resp_samples += 1
            else:
                after_response += 1

    if started:
        inter_prompt.append(inter_samples)
    if open_id is not None:               # capture ended mid-turn
        response_window.append(resp_samples)

    return response_window, inter_prompt, before_first, after_response


def main():
    for engine, path in TRACES:
        if not path.exists():
            print(f"{engine}: MISSING {path}", file=sys.stderr)
            continue
        resp, inter, before_first, after_response = analyse(engine, path)
        print(f"--- {engine} ({path.name})")
        print(f"  turns (prompt-delimited)     : {len(inter)}")
        print(f"  RESPONSE window per turn     : {resp}")
        print(f"  INTER-PROMPT window per turn : {inter}")
        print(f"  total in inter-prompt windows: {sum(inter)}")
        print(f"  samples before first turn    : {before_first}")
        print(f"  samples after a response     : {after_response}  (the two windows differ by this)")
        print(f"  max samples in one turn      : {max(inter) if inter else 0}")
        print(f"  max triggers in one turn     : {(max(inter) if inter else 0) + 1}")


if __name__ == "__main__":
    main()
