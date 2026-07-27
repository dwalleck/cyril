#!/usr/bin/env python3
"""Independent oracle for slice 1's TurnId allocator (cyril-a71q C8).

Written WITHOUT reference to the Rust implementation: this is a from-scratch model
of "monotonic, checked, never reissues", so agreement between it and the Rust
allocator is evidence, not a tautology. The plan's first draft cited
`design_reanchored_falsifier.py` as S1's oracle; that file models mediation policy
over already-existing owners and contains no allocator at all. This replaces that
fictional citation.

Contract under test:
  - allocation is strictly monotonic from the start value, step 1, no gaps
  - no value is ever issued twice
  - at u64::MAX the NEXT allocation fails closed (returns None) rather than
    wrapping to 0 (wrapping_add) or reissuing u64::MAX forever (saturating_add)

Usage:
    alloc_oracle.py                 # print the boundary sequence the Rust must match
    alloc_oracle.py --check FILE    # compare against the Rust allocator's emitted lines
"""
import sys

U64_MAX = 2**64 - 1


def model(start, n):
    """Return the first `n` allocation outcomes from `start`.

    Each element is an int (issued id) or None (fail-closed exhaustion).
    Deliberately naive and independent: no shared code with the Rust.
    """
    out, cur = [], start
    for _ in range(n):
        if cur > U64_MAX:
            out.append(None)
            continue
        out.append(cur)
        cur += 1
    return out


def self_test():
    """The oracle's own fences — a broken oracle must not silently pass a slice."""
    assert model(0, 3) == [0, 1, 2], "fresh allocator must issue 0,1,2 with no gaps"
    boundary = model(U64_MAX - 1, 3)
    assert boundary == [U64_MAX - 1, U64_MAX, None], f"boundary wrong: {boundary}"
    seq = [x for x in model(0, 1000) if x is not None]
    assert len(seq) == len(set(seq)), "no value may be issued twice"
    assert seq == sorted(seq), "allocation must be strictly monotonic"
    # the two bug classes this exists to catch, stated as explicit negatives
    assert model(U64_MAX, 2) != [U64_MAX, 0], "wrapping_add would produce this"
    assert model(U64_MAX, 2) != [U64_MAX, U64_MAX], "saturating_add would produce this"
    return True


def check(path):
    """Compare the Rust allocator's emitted sequence against the model.

    Input format: one allocation per line, either a decimal id or the literal
    `EXHAUSTED`. Rust test writes it; this reads it. No shared code path.
    """
    lines = [l.strip() for l in open(path) if l.strip()]
    actual = [None if l == "EXHAUSTED" else int(l) for l in lines]
    if not actual:
        print("FAIL: no allocations recorded", file=sys.stderr)
        return 1
    start = actual[0]
    if start is None:
        print("FAIL: first allocation exhausted", file=sys.stderr)
        return 1
    expected = model(start, len(actual))
    if actual == expected:
        print(f"ALLOC-ORACLE-AGREES n={len(actual)} start={start}")
        return 0
    for i, (a, e) in enumerate(zip(actual, expected)):
        if a != e:
            print(f"FAIL at index {i}: rust={a} oracle={e}", file=sys.stderr)
            break
    return 1


if __name__ == "__main__":
    self_test()
    if len(sys.argv) > 2 and sys.argv[1] == "--check":
        sys.exit(check(sys.argv[2]))
    print("# boundary sequence the Rust allocator must reproduce")
    print(f"start=u64::MAX-1 -> {model(U64_MAX - 1, 3)}")
    print(f"start=0          -> {model(0, 3)}")
    print("ALLOC-ORACLE-SELFTEST-PASSED")
