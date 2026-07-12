# Prove-it prototype — cyril-q9dx

## Upstream and prior art

The signed upstream contract is `.cyril-q9dx/spec.md`; its requester reply is
`"confirmed"`. The bounded Rivets and GitHub search is recorded in
`.cyril-q9dx/related-issues.md`. Direct lineage is `cyril-ixua` → `cyril-ghuu`
→ `cyril-q9dx`, and `cyril-q9dx` blocks ANSI-16 activation in `cyril-qaq0`.

## Smallest questions

1. Can the public resolver produce the three fixed speaker assignments while
   preserving the other 113 role-mode rows?
2. Does the real conversation renderer place those three assignments on visible
   speaker markers in an 80×24 Ratatui buffer?

The full material-boundary inventory and exclusions are in
`.cyril-q9dx/material-boundaries.md`.

## Probe 1: resolver representation

`.cyril-q9dx/probe.rs` is a 54-line Rust binary. It calls the compiled public
`cyril_ui::theme::resolve` function for Cyril Dark in all four modes, applies
the signed three-field ANSI-16 candidate assignment, and emits all 116
role-mode rows before and after the assignment.

Command:

```text
cargo run --quiet --manifest-path .cyril-q9dx/Cargo.toml --bin q9dx-probe
```

Captured output: `.cyril-q9dx/probe-output.tsv`.

Observed:

- 116/116 role-mode rows emitted.
- Exactly 3 rows changed: user Gray → LightBlue, agent DarkGray → LightGreen,
  and system Gray → LightMagenta.
- 26/26 non-speaker ANSI-16 rows remained equal.
- 87/87 true-color, ANSI-256, and no-color rows remained equal.
- The 4 muted-family roles remained DarkGray, outside all 3 protected slots.

## Oracle 1: independent projection computation

`.cyril-q9dx/oracle.py` is an 81-line Python program. It does not call the Rust
resolver. It parses the 29 production source-color literals from `theme.rs`,
computes ANSI-256 and ANSI-16 projection independently from canonical palette
tables and squared RGB distance, applies the three human-signed semantic
assignments, and compares every expected TSV row with the compiled Rust probe.

Command and result:

```text
python .cyril-q9dx/oracle.py
AGREE rows=116 changed=3 roles=user,agent,system
```

Independent output: `.cyril-q9dx/oracle-output.tsv`.

## Probe 2: conversation rendering seam

`.cyril-q9dx/render-probe.rs` is a 47-line Rust binary. It creates the actual
`UiState`, adds production-shape user, agent, and system messages, renders
`widgets::chat::render` through Ratatui `TestBackend` at 80×24, and locates the
visible marker cells.

Command:

```text
cargo run --quiet --manifest-path .cyril-q9dx/Cargo.toml --bin q9dx-render-probe
```

Captured output: `.cyril-q9dx/render-probe-output.tsv`.

Observed:

| Role | Visible marker | Position | Buffer foreground |
| --- | --- | ---: | --- |
| user | `You:` | 0,0 | LightBlue |
| agent | `Kiro:` | 0,3 | LightGreen |
| system | `Q9DX-SYSTEM` | 0,7 | LightMagenta |

## Oracle 2: independent source binding

`.cyril-q9dx/render-oracle.py` is a 50-line Python program. It does not invoke
Cyril rendering. It lexically extracts each `ChatMessageKind` arm's theme-field
binding from production `chat.rs`, then compares those bindings and the signed
role colors with the runtime buffer rows, including 80×24 visibility bounds.

Command and result:

```text
python .cyril-q9dx/render-oracle.py
AGREE visible=3/3 bindings=user,agent,system colors=LightBlue,LightGreen,LightMagenta
```

Independent output: `.cyril-q9dx/render-oracle-output.tsv`.

## Material-boundary agreement

<!-- markdownlint-disable MD013 -->

| Boundary observation | Probe | Independent oracle | Result |
| --- | --- | --- | --- |
| Exactly three ANSI-16 speaker rows change | Compiled public resolver plus candidate assignment | Python source-literal parser plus independent palette calculation | Agree, 3/3 |
| The other role-mode rows remain equal | 116-row before/after TSV | Independently generated 116-row TSV | Agree, 113/113 |
| Muted roles avoid protected slots | Compiled resolver rows | Canonical ANSI-16 computation | Agree, 4/4 are DarkGray |
| Conversation binds identity markers to speaker roles | Runtime `TestBackend` cells | Lexical `ChatMessageKind` arm inspection | Agree, 3/3 |
| Ratatui preserves named colors in observable cells | Buffer foreground variants | Signed expected variants reached through independently verified bindings | Agree, 3/3 |
| Markers remain visible in the pinned viewport | Runtime coordinates | Bounds check independent of layout code | Agree, 3/3 inside 80×24 |

<!-- markdownlint-enable MD013 -->

No probe-oracle disagreement remains.

## What I learned

The system message has no `System:` label: its message text itself is the
system-colored marker, and in the production-shape three-message scene it lands
at row 7 while still preserving `Color::LightMagenta` in the Ratatui buffer.
