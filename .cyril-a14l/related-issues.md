# cyril-a14l — related issues (prove-it-prototype step 0)

Tracker sweep on: layout, resize, narrow/small terminal, autocomplete,
overlay, input. Bounded search, 2026-07-15.

## Direct prior art

- **cyril-mdbp** (closed, bug) — "KAS context bar can overflow the status
  line on narrow terminals". Prior narrow-terminal fix; width-axis only.
- **cyril-cc5e** (closed, P1 blocker of this issue) — "Keep picker selection
  visible". Introduced `widgets/modal.rs::centered()` with the 4-cell margin
  clamp and pinned its arithmetic (claim C8 parity oracle). The picker got a
  visible-selection viewport; the *approval* widget still carries its own
  inline copy of the legacy geometry (`approval.rs:16-20, 60-66`).
- **cyril-ghuu / cyril-nrnq / cyril-dij8** (closed) — semantic-color
  migrations; created the render/test seams (marker theme, pinned baselines)
  this issue's tests will reuse.

## Same epic (blocked BY cyril-a14l — do not solve here)

- cyril-lme2 (crew status responsive), cyril-9ode (context/voice gauges),
  cyril-uw20 (hooks panel responsive), cyril-91iu (shortcut help overlay),
  cyril-4vvw (input multiline nav + undo — touches `input.rs`; keep the
  a14l input changes minimal to avoid churn under its feet),
  cyril-8r3u (usability contract lock — will pin whatever a14l ships).

## Filed during this probe

- **cyril-2mfa** (bug, P4) — @-file completion silently empty outside a git
  repo (`git ls-files` only, warn-log only). Discovered by the pty oracle.

No open ticket describes the 60×16 layout collapse itself; cyril-a14l is
the canonical issue.
