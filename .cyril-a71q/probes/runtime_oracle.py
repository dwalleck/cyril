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
 ('response/R1 prompt response', 'secondary-nonterminal',
  'terminal-1 scope=global kind=turn-completed' not in response_out),
 ('response/R2 prompt accepted', 'reject-busy', 'recv prompt name=R2' not in response_trace),
 ('response/R2 prompt response', 'secondary-nonterminal',
  'terminal-2 scope=global kind=turn-completed' not in response_out),
]
for name, expected, agrees in items:
    actual = 'expected' if agrees else 'CURRENT-DEFECT'
    print(f'{name}: hidden_expected={expected} comparison={actual}')
print(f'item_agreement={sum(x[2] for x in items)}/{len(items)}')
defects = [name for name, _, agrees in items if not agrees]
print('defect_items=' + ', '.join(defects))
existing = {'same/A late response', 'same/C before B terminal', 'cross/C before B terminal'}
revised = {'response/R1 prompt response', 'response/R2 prompt accepted',
 'response/R2 prompt response'}
print(f'existing_defect_set_preserved={existing == set(defects) & existing}')
print(f'revised_spec_defect_reproduced={revised.issubset(defects)}')
print(f'model_defect_reproduced={(existing | revised).issubset(defects)}')
