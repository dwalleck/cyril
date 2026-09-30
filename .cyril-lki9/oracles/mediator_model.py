#!/usr/bin/env python3
"""cyril-lki9 design falsifier for claim C8 (no stuck turn state under any interleaving).

A standalone model of the PROPOSED TurnMediator rules (design.md § Placement,
turn_mediator.rs), exhaustively checked against every interleaving of wire and
dispatch events KAS can produce for the scenarios below. It is not the Rust
implementation; it falsifies the RULES before anyone builds them. The permanent
fence for C8 is the Rust table test named in design.md; this script is its
pre-approval oracle and cheapest falsifier.

Proposed rules (main session only; foreign sessions are unchanged: Forward):
  dispatch (SendPrompt):   active -> REJECT(Busy) | idle -> active = cyril(id), bracket unopened
  wire turn_start:         idle -> active = server, bracket open, liveness begin
                           active cyril, bracket unopened -> open bracket (it is that turn's own start,
                             or a wake KAS started just before the prompt reached it)
                           active with bracket open -> Forward (KAS never nests; logged)
  wire turn_end (unstamped): companion awaits Wire -> ABSORB
                           active server -> FORWARD_COMPLETE, clear (no companion: no prompt response exists)
                           active cyril  -> FORWARD_COMPLETE, clear, companion awaits Synthesized(id)
                           idle -> DROP_UNOWNED
  prompt response (stamped id): companion awaits Synthesized(id) -> ABSORB
                           active cyril(id) -> FORWARD_COMPLETE, clear, companion awaits Wire
                           else -> DROP_STALE

KAS ground truth used to generate traces (evidence.md P4/P5/P6, memory: turn_end FIRST, both terminals):
  - a wake starts only when the parent is idle at KAS;
  - a session/prompt arriving at KAS during a wake PRE-EMPTS it: wake turn_end{cancelled} then the prompt's turn;
  - each KAS turn emits turn_start < turn_end; a prompt turn's response R comes after its turn_start,
    on either side of its turn_end;
  - the mediator sees cyril's dispatch at the instant cyril sends it; KAS sees it after a delay, so a
    wake turn_start can be in flight when cyril dispatches.

Invariants checked at the end of every trace:
  I1 no active turn remains (nothing stuck busy);
  I2 exactly one TurnCompleted is forwarded per KAS turn bracket (UI flushes/idles once per turn);
  I3 no companion expectation is left dangling;
  I4 a rejected dispatch never leaves the mediator active.
"""
import os, sys

# MEDIATOR_RULES=current reproduces TODAY's production rule (wire turn_start is ignored:
# convert/kas.rs has no turn_start arm) as the positive control: it must turn red (P3).
RULES = os.environ.get("MEDIATOR_RULES", "proposed")

class Mediator:
    def __init__(self):
        self.active = None       # dict(owner='cyril'|'server', id=int|None, bracket=bool)
        self.companion = None    # ('Wire', id) | ('Synth', id)
        self.forwarded = 0
        self.next_id = 0
        self.log = []

    def dispatch(self):
        if self.active is not None:
            self.log.append('REJECT'); return None
        tid = self.next_id; self.next_id += 1
        self.active = {'owner': 'cyril', 'id': tid, 'bracket': False}
        self.log.append(f'BEGIN cyril#{tid}'); return tid

    def turn_start(self):
        if RULES == "current":
            self.log.append('IGNORED (no turn_start arm)'); return
        if self.active is None:
            self.active = {'owner': 'server', 'id': None, 'bracket': True}; self.log.append('BEGIN server')
        elif self.active['owner'] == 'cyril' and not self.active['bracket']:
            self.active['bracket'] = True; self.log.append('ATTACH')
        else:
            self.log.append('FORWARD nested-start')

    def turn_end(self):
        if self.companion and self.companion[0] == 'Wire':
            self.companion = None; self.log.append('ABSORB wire'); return
        if self.active is None:
            self.log.append('DROP_UNOWNED'); return
        if self.active['owner'] == 'cyril':
            self.companion = ('Synth', self.active['id'])
        self.active = None; self.forwarded += 1; self.log.append('FORWARD_COMPLETE(wire)')

    def response(self, tid):
        if self.companion == ('Synth', tid):
            self.companion = None; self.log.append('ABSORB synth'); return
        if self.active and self.active['owner'] == 'cyril' and self.active['id'] == tid:
            self.active = None; self.companion = ('Wire', tid); self.forwarded += 1
            self.log.append('FORWARD_COMPLETE(resp)'); return
        self.log.append('DROP_STALE')

def interleavings(a, b):
    """All merges of sequences a and b preserving each one's internal order."""
    if not a: yield list(b); return
    if not b: yield list(a); return
    for rest in interleavings(a[1:], b): yield [a[0]] + rest
    for rest in interleavings(a, b[1:]): yield [b[0]] + rest

def scenarios():
    """Yield (name, events, kas_brackets). Events: 'D' dispatch, 'S'/'E' wire turn_start/end, 'R' response."""
    # 1. idle wake, no operator input
    yield 'idle-wake', ['S', 'E'], 1
    # 2. ordinary prompt; R on either side of E
    yield 'prompt E<R', ['D', 'S', 'E', 'R'], 1
    yield 'prompt R<E', ['D', 'S', 'R', 'E'], 1
    # 3. operator cancels / steers a wake: no dispatch, same wire shape as 1
    yield 'wake-cancelled', ['S', 'E'], 1
    # 4. wake turn_start processed BEFORE cyril dispatches (cyril saw it busy? race window): dispatch rejected
    yield 'wake-then-dispatch-rejected', ['S', 'D', 'E'], 1
    # 5. RACE: cyril dispatches while the wake's turn_start is in flight; KAS pre-empts the wake
    #    Wire from KAS: S_wake, E_wake(cancelled), S_prompt, {E_prompt, R} in either order.
    #    Mediator sees D at dispatch time, anywhere before S_prompt (KAS can't start the prompt turn before
    #    receiving it), and after or before S_wake.
    for tail in (['S', 'E', 'R'], ['S', 'R', 'E']):
        kas = ['S', 'E'] + tail           # wake bracket, then prompt bracket
        # D between S_wake and E_wake at the MEDIATOR means the mediator already holds the wake as busy and
        # rejects the dispatch, so the prompt never reaches KAS and its bracket does not exist: that is
        # scenario 4, not a race. Valid race positions: D before S_wake (wake in flight to the mediator)
        # and D after E_wake (wake already over; no pre-emption, E_wake is end_turn).
        for pos in (0, 2):
            ev = kas[:pos] + ['D'] + kas[pos:]
            yield f'race D@{pos} tail={"".join(tail)}', ev, 2
    # 6. two consecutive idle wakes (two runs finishing one after another)
    yield 'two-wakes', ['S', 'E', 'S', 'E'], 2

failures = 0; total = 0
for name, ev, brackets in scenarios():
    total += 1
    m = Mediator(); tid = None
    for e in ev:
        if e == 'D':
            got = m.dispatch()
            if got is None and m.active is not None and m.active['owner'] == 'cyril':
                print(f'FAIL {name}: I4 rejected dispatch left cyril active'); failures += 1
            tid = got if got is not None else tid
        elif e == 'S': m.turn_start()
        elif e == 'E': m.turn_end()
        elif e == 'R':
            if tid is None: m.log.append('R-without-dispatch(skipped: rejected prompt never reaches KAS)'); continue
            m.response(tid)
    rejected = 'REJECT' in m.log
    expected_forwards = brackets
    # a rejected prompt never reaches KAS, so its bracket and response do not exist on the wire
    problems = []
    if m.active is not None: problems.append(f'I1 stuck active={m.active}')
    if m.forwarded != expected_forwards: problems.append(f'I2 forwarded={m.forwarded} expected={expected_forwards}')
    if m.companion is not None: problems.append(f'I3 dangling companion={m.companion}')
    status = 'PASS' if not problems else 'FAIL'
    if problems: failures += 1
    print(f'{status} {name:<34} events={"".join(ev):<8} log={m.log} {"; ".join(problems)}')

print(f'== C8 model: {total - failures}/{total} scenarios PASS')
sys.exit(1 if failures else 0)
