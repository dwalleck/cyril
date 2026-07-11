# Cyril Dark projection prototype

## Smallest question

For the 18 explicit RGB roles pinned in `.cyril-ixua/spec.md`, which ANSI-256
and ANSI-16 entries satisfy the specified minimum squared RGB-distance rule?

## Probe

`.cyril-ixua/probe.rs` is a 52-line standalone Rust probe. It carries the
production-shape Cyril Dark role values, independently generates fixed xterm
indices 16–255 and the canonical ANSI-16 table, and emits one projection row
per explicit role. It runs without using the proposed theme module:

```text
rustc --edition 2024 .cyril-ixua/probe.rs \
  -o .cyril-ixua/probe-bin.exe
.cyril-ixua/probe-bin.exe
```

The captured 18-row result is in `.cyril-ixua/probe-output.tsv`.

## Oracle

`.cyril-ixua/oracle.py` independently parses the compatibility mapping from the
signed spec rather than trusting the probe's embedded values. It constructs the
xterm cube and grayscale ramp with Python `itertools.product`, brute-forces both
ANSI palettes, and compares every role and index from the probe. Running
`python .cyril-ixua/oracle.py ./.cyril-ixua/probe-bin.exe` reports
`AGREE 18/18 role projections`; there are zero role-value, ANSI-256, or ANSI-16
disagreements.

## Existing partial-work check

The unverified partial ANSI-256 projector rounds directly into the 6×6×6 cube
and never considers the grayscale ramp. Applying that mechanism to the probe
data disagrees with the agreed oracle on 8/18 roles: chrome, code, selection,
muted text, border, user message, agent message, and diff context. Its
threshold-based ANSI-16 heuristic also disagrees on 9/18 roles, including
chrome, selection, cyan accents, and message colors. This is not a newly
discovered substrate bug: `cyril-ixua` already requires auditing and correcting
the unverified partial work before implementation is accepted.

## What I learned

Both partial projectors violate the pinned nearest-distance rule: cube rounding
fails 8/18 ANSI-256 roles and threshold classification fails 9/18 ANSI-16 roles.

## Hard gate

- Probe written and run against the signed Cyril Dark role data: yes.
- Independent oracle defined and run: yes.
- Probe and oracle agree on a non-trivial slice: yes, 18/18 projections.
- New learning recorded: yes, the partial projectors fail 8/18 ANSI-256 and
  9/18 ANSI-16 cases.
