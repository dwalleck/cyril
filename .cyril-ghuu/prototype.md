# Conversation color-source prototype

## Smallest question

Which qualified legacy color sources occur in production code across the five
conversation modules that this migration must cover?

## Probe

`.cyril-ghuu/probe.py` is a 40-line standalone probe. It runs the Rust 2024
formatter over the five real production modules so malformed or unparseable
Rust cannot silently enter the inventory, derives legacy palette color names
from the real typed declarations in `palette.rs`, excludes test modules, and
emits each qualified `Color::*` or color-valued `palette::*` source location.

```text
python .cyril-ghuu/probe.py > .cyril-ghuu/probe-output.tsv
```

The probe reports 81 source locations across 5/5 modules and 14 distinct
qualified tokens. That ticket-start inventory is frozen in
`.cyril-ghuu/legacy-color-baseline.tsv`; `Color::DarkGray` is the largest
category at 20 locations.

## Oracle

`.cyril-ghuu/oracle.sh` independently reads the unformatted source directly,
stops at each test module with `awk`, and uses `grep` with a manually pinned
allowlist of the five legacy palette color constants rather than deriving names
from `palette.rs`. Its 81-row output in `.cyril-ghuu/oracle-output.tsv` agrees
item-for-item with the Rustfmt-parsed probe output; `diff -u` reports zero
location or token disagreements.

## Disagreement resolved

The first probe version used a generic ast-grep scoped-identifier pattern. It
reported only 69 rows and missed 12 valid references that the lexical oracle
found, including the input cursor and several plain-text syntax fallbacks.
Replacing that undercounting mechanism with Rustfmt parsing produced agreement
at 81/81 rows. A regression fence for this ticket must therefore not rely on
that generic ast-grep pattern alone.

## What I learned

The real migration surface contains 81 direct qualified color references—not
the 69 found by the initial AST pattern—and 20 of them are named dark gray,
which makes the new `Subdued` role the highest-fanout compatibility mapping.

## Hard gate

- Probe written and run against the real codebase: yes.
- Independent oracle defined and run: yes.
- Probe and oracle agree on a non-trivial slice: yes, 81/81 locations.
- New learning recorded: yes, the initial AST inventory missed 12/81 locations
  and dark gray accounts for 20/81.
