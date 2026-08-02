#!/usr/bin/env python3
"""cyril-b4y4 probe: standalone model of run_loop's turn-mediation policy.

Models the proposed TurnMediator::observe() decision table AS I READ IT from
bridge.rs:2124-2281 (plus the SendPrompt busy-guard/allocate at 1116/1134).
No cyril code is imported — this is the model under test. The oracle is the
REAL run_loop driven with the byte-same scenario (oracle_scenario.rs), and
the two outputs are compared line by line.

Scenario grounding: live-confirmed KAS ordering (turn-end-ordering captures,
2026-08-01 / 2.16.0) — every turn emits BOTH terminals, wire turn_end first,
stamped prompt response 0-1ms behind. Turn 1 exercises that order; turn 2 the
reverse (response first), which the dedup comment says must also hold.
"""

S, F = "sess_fake-0", "sess_foreign"  # main + foreign session ids
KAS = True  # engine bound for the whole scenario (owes_wire_companion)

active = None            # (owner, session)
companion = None         # (owner, session, awaiting, first_source)
next_id, out = 0, []


def send_prompt():
    global active, next_id
    if active is not None:
        out.append("PROMPT-REJECTED busy")
        return
    active = (next_id, S)
    out.append(f"PROMPT-ACCEPTED turn#{next_id}")
    next_id += 1


def observe(turn, session, label):
    """The modelled mediation policy. Returns nothing; appends disposition."""
    global active, companion
    if turn is not None:  # stamped arm (bridge-synthesized)
        if companion and companion[2] == "Synthesized" and companion[0] == turn:
            companion = None
            out.append(f"{label}: ABSORB synthesized turn#{turn}")
        elif active and active[0] == turn:
            companion = (active[0], active[1], "Wire", "Synthesized") if KAS else None
            active = None
            out.append(f"{label}: RELEASE-BY-OWNER turn#{turn} FORWARD")
        else:
            out.append(f"{label}: DROP-STALE turn#{turn}")
    else:  # unstamped arm (KAS wire turn_end) — matched by session
        if companion and companion[2] == "Wire" and companion[1] == session:
            companion = None
            out.append(f"{label}: ABSORB wire {session}")
        elif active and active[1] == session:
            companion = (active[0], active[1], "Synthesized", "Wire")
            active = None
            out.append(f"{label}: RELEASE-BY-SCOPE turn#{companion[0]} FORWARD")
        elif active:
            out.append(f"{label}: FORWARD-FOREIGN {session} main-untouched")
        else:
            out.append(f"{label}: DROP-UNOWNED {session}")


# --- scenario: two KAS turns, all six dispositions ---
send_prompt()                 # turn#0 accepted
observe(None, S, "f1")        # wire turn_end, live order -> release by scope
observe(0, S, "f2")           # its synthesized twin -> absorbed
send_prompt()                 # turn#1 accepted (guard cleared by f1)
observe(0, S, "f3")           # stale duplicate of turn#0 -> dropped
observe(None, F, "f4")        # foreign session terminal -> forwarded, no touch
observe(1, S, "f5")           # response FIRST this time -> release by owner
observe(None, S, "f6")        # wire turn_end second -> absorbed (Wire)
observe(None, S, "f7")        # third terminal, nothing owed -> dropped silent
send_prompt()                 # turn#2 accepted (ledger empty, guard clear)

# --- extension: precedence cells the design's matrix stands on ---
observe(2, S, "f8")           # stamped release -> Wire expectation dangles
send_prompt()                 # turn#3 accepted on SAME session: M3 state --
                              # dangling Wire companion on S + active turn on S
observe(None, S, "f9")        # ABSORB-FIRST: eaten by turn#2's expectation,
                              # must NOT release (or clear) active turn#3
observe(None, S, "f10")       # companion gone -> NOW releases turn#3 by scope
observe(None, None, "f11")    # global unstamped, idle, Synthesized owed:
                              # no Wire match, no active -> dropped
send_prompt()                 # turn#4 accepted
observe(3, S, "f12")          # owner-keyed absorb of turn#3's synthesized twin
                              # wins over stale-drop while turn#4 is active

print("B4Y4-PROBE-BEGIN")
print("\n".join(out))
print("B4Y4-PROBE-END")
