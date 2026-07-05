# cyril-0v42 — pre-PR /code-review decisions (2026-07-05)

Two-axis review vs `main` (Standards / Spec sub-agents). 0 hard violations.
Every finding verified against code + cited standard before accept/reject.

## Accepted (fixed pre-PR)

- **F1 canonicalize catch-all** (Standards #1, CLAUDE.md missing-vs-corrupt):
  fresh-file fallback now fires ONLY on `ErrorKind::NotFound`; EACCES/ELOOP
  and friends propagate. Comment now matches behavior.
- **F2 JoinError log level** (Standards #2): `debug!` → `warn!` — a panicked
  write task is abnormal, unlike the ordinary io failures `io_err` logs.
- **F3 ErrorKind fidelity** (Standards #6): directory refusal uses
  `ErrorKind::IsADirectory` (stable since 1.83) instead of `InvalidInput`.
- **F4 exact error-wording fences** (Standards #5, CLAUDE.md "test error
  messages explicitly"): the three constant refusal messages now use
  `assert_eq!` on the full string; path-bearing messages (temp-create,
  resolver -32603 wrap) legitimately keep `contains`.
- **F5 C1 fence mode assert** (Spec a1): unwritable-parent fence now also
  asserts the target's MODE survived, closing the design-table gap
  ("content/mode intact").
- **F6 design-table fence names** (Spec a2): table synced to the shipped
  test names so the design greps to the suite.
- **F7 oracle work-dir bloat** (Standards artifact note): oracle.sh trims
  >10M files from its kept work dir (S11/S12 kill-test bodies were 768MB
  per run).

## Rejected (with verification)

- **R1 named const for -32603** (Standards #3): the literal matches the
  established style of `io_err`/`to_native_checked`/terminal_io in the same
  module; importing auth.rs's const into one new call site would make the
  file inconsistent with itself. A module-wide const unification is a style
  sweep outside this change's scope and not tracker-worthy.
- **R2 persist error flattens source chain** (Standards #4, AGENTS.md:125):
  the rule's own qualifier applies — "only flatten at the outermost boundary
  where the chain is logged." `io::Error` is the helper's public type, the
  immediate next hop (`io_err`) flattens to the ACP wire string and logs,
  and no caller branches on `source()`. The format! keeps the inner error's
  Display, so no wire-visible information is lost.
- **R3 artifact volume** (Spec b): `.cyril-0v42/` artifacts are the
  pipeline's committed audit trail (house convention, cf. `.cyril-7bdu/`).
