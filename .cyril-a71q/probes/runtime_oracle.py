#!/usr/bin/env python3
"""Artifact-only oracle: named mock owners determine expected dispositions."""
from pathlib import Path
ROOT = Path(__file__).parent / 'output' / 'runtime'

def text(name):
    return (ROOT / name).read_text()

def before(body, first, second):
    return first in body and second in body and body.index(first) < body.index(second)

same_out, same_trace = text('same-stdout.txt'), text('same-mock-trace.txt')
cross_out, cross_trace = text('cross-stdout.txt'), text('cross-mock-trace.txt')
response_out = text('response_only-stdout.txt')
response_trace = text('response_only-mock-trace.txt')
items = [
 ('same/A scoped terminal', 'complete-active', 'terminal-1 scope=sess_main kind=turn-completed' in same_out),
 ('same/A late response', 'drop-stale', 'terminal-2 scope=global kind=turn-completed' not in same_out),
 ('same/C before B terminal', 'reject-busy', not before(same_trace, 'recv prompt name=C', 'owner=B-owned')),
 ('cross/X foreign terminal', 'forward-foreign', 'terminal-1 scope=sess_foreign kind=turn-completed' in cross_out),
 ('cross/C before B terminal', 'reject-busy', not before(cross_trace, 'recv prompt name=C', 'owner=B-owned')),
 ('cross/B owned terminal', 'complete-active', 'terminal-2 scope=sess_main kind=turn-completed' in cross_out),
 # RELABELLED 2026-07-26 (cyril-a71q re-anchor). These three encoded the VOIDED
 # sole-turn_end contract: "the response forwards 0 completions... T remains Busy
 # until an existing owned failure... the response alone admits 0 later prompts"
 # (spec-superseded-sole-turn-end.md:42,54). The APPROVED re-anchored spec inverts
 # it: "it releases T and forwards exactly 1 completion (liveness -- Busy never
 # persists past an available terminal source)" (spec.md:73,246). First-source-wins
 # is retained, so a response-only turn releases via its response and the next
 # prompt is legitimately accepted. The captures are unchanged and still valid --
 # only the contract layered on top of them was wrong, exactly as timing-audit.md
 # predicted for prototype artifacts. Under the approved contract these three are
 # ALREADY CORRECT in shipped cyril; they were never defects.
 ('response/R1 prompt response', 'releases-first-source',
  'terminal-1 scope=global kind=turn-completed' in response_out),
 ('response/R2 prompt accepted', 'accept-after-release', 'recv prompt name=R2' in response_trace),
 ('response/R2 prompt response', 'releases-first-source',
  'terminal-2 scope=global kind=turn-completed' in response_out),
]
for name, expected, agrees in items:
    actual = 'expected' if agrees else 'CURRENT-DEFECT'
    print(f'{name}: hidden_expected={expected} comparison={actual}')
print(f'item_agreement={sum(x[2] for x in items)}/{len(items)}')
defects = [name for name, _, agrees in items if not agrees]
print('defect_items=' + ', '.join(defects))
# The REAL defect set under the approved re-anchored contract: three same/cross
# ownership bugs. The former `revised`/`model` flags counted the three response/*
# items as defects too; those encoded the voided sole-turn_end spec and were
# relabelled above, so those flags are removed rather than left reading False and
# looking like a regression.
real_defects = {'same/A late response', 'same/C before B terminal', 'cross/C before B terminal'}
found = set(defects)
print(f'real_defect_set_reproduced={real_defects == found}')
print(f'unexpected_defects={sorted(found - real_defects) or "none"}')
print('BUILD-TARGET: real_defect_set_reproduced flips True->False and '
      'item_agreement 6/9->9/9 when cyril-a71q lands; any other movement is drift.')
