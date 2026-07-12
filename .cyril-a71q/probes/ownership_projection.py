#!/usr/bin/env python3
"""Compare current bridge projection with an independent hidden-owner history."""
from dataclasses import dataclass


@dataclass(frozen=True)
class Event:
    op: str
    session: str | None = None
    owner: int | None = None
    source: str | None = None


TRACES = {
    "same_session_stale": [Event("start", "S", 1), Event("end", "S", 1, "kas"),
        Event("start", "S", 2), Event("end", "S", 1, "response"), Event("end", "S", 2, "kas")],
    "cross_session": [Event("start", "S", 1), Event("end", "X", 9, "kas"), Event("end", "S", 1, "kas")],
    "global_v2": [Event("start", "S", 1), Event("end", None, 1, "response")],
    "kas_dual": [Event("start", "S", 1), Event("end", "S", 1, "kas"), Event("end", None, 1, "response")],
}


def hidden_history(events):
    active, completed, out = None, set(), []
    for event in events:
        if event.op == "start":
            active = (event.session, event.owner)
            continue
        foreign = active and event.session not in (None, active[0])
        owned = active and event.owner == active[1] and not foreign
        if foreign:
            out.append("forward-foreign")
        elif owned and event.owner not in completed:
            out.append("complete-active")
            completed.add(event.owner)
            active = None
        else:
            out.append("drop-stale")
    return out


def current_projection(events):
    active, out = None, []
    for event in events:
        if event.op == "start":
            active = event.session
        elif active is None:
            out.append("drop")
        else:
            out.append("complete-active")
            active = None
    return out


for name, events in TRACES.items():
    oracle = hidden_history(events)
    current = current_projection(events)
    print(f"{name}: oracle={oracle} current={current} agree={oracle == current}")
