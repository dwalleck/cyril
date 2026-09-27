# Route: cyril-ell0

Change: Delete the dead `stream_buffer` module from `cyril-ui` (`StreamBuffer` has zero production consumers) — requester option (a), no replacement, no wiring.
Date: 2026-09-27

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The only premise is "`StreamBuffer` has zero consumers outside its own file". Covered by current repository evidence at `f9bc81d8`: `grep -rn 'StreamBuffer\|stream_buffer' crates/` returns (a) 13 self-hits inside `crates/cyril-ui/src/stream_buffer.rs`; (b) the declaration `crates/cyril-ui/src/lib.rs:14 pub mod stream_buffer;`; (c) the legacy config-key string `stream_buffer_timeout_ms` at `crates/cyril-core/src/types/config.rs:153`, `crates/cyril-core/tests/nd4h_legacy_config_compat.rs:3,40`, `crates/cyril/tests/nd4h_source_fences.rs:272` — a different entity (config compatibility), fenced by the nd4h suite; (d) `UiState::flush_stream_buffer` at `crates/cyril-ui/src/state.rs:2971` and its caller `crates/cyril/src/app.rs:875` — a documented no-op stub (`-> bool { false }`) that neither imports nor constructs `StreamBuffer`. `lib.rs:23` re-exports only `error::{Error, ErrorKind, Result}`. `.cyril-nd4h/findings.md` finding #2 independently recorded the same census. No external or system behavior is involved. | no |
| 2 | Structural module shape | Interface touched: `cyril_ui::stream_buffer::StreamBuffer::{new, push, should_flush, flush}` — a `pub mod` on the `cyril-ui` library crate root; deletion removes it from the crate's public surface (module-shape trigger "removes … interface"). Ownership: the module's responsibility (debounced streaming-text flush at newline/code-fence boundaries or after a timeout) has no production owner today (never constructed) and, per the requester, gets none afterwards ("Do NOT wire it into the streaming path; do NOT add a replacement") — no responsibility moves. Current owner `crates/cyril-ui/src/stream_buffer.rs` → `delete`; candidate owners: none. Protected parents: `crates/cyril-ui/src/lib.rs` (declaration removal only); `crates/cyril-ui/src/state.rs` and `crates/cyril/src/app.rs` (must gain nothing — no re-homing). Dependency direction unchanged (`cyril-ui` → `cyril-core` only; the module depended on `std` alone). Length-review lookup: `ls scripts/` = `session-worktree.sh`, `tests`; `grep -rIl -i 'max.lines\|line.count\|length.gate\|wc -l' scripts/ .github/workflows/` = no hits → no repository length gate exists; the change shrinks `cyril-ui` (−149 lines; `lib.rs` 23 → 22), so no threshold is approached (projected growth: 0, margin: 0). Shape change → YES. | yes |
| 3 | Production-scale risk | None: no runtime path executes the module today (never constructed), so deleting it changes no latency, throughput, memory, concurrency, or data-volume characteristic. | no |
| 4 | Explicit behavior | Requester decisions fix every choice (option (a); legacy-config fences untouched; before/after grep receipts in the PR; docs describing the module as live are corrected, historical mentions are reported only). Behavior contract: **B1** — Given HEAD `f9bc81d8`, when `crates/cyril-ui/src/stream_buffer.rs` is deleted and `pub mod stream_buffer;` is removed from `crates/cyril-ui/src/lib.rs`, then `cargo check --workspace --all-targets --all-features` exits 0 (no consumer breaks). **B2** — Given the change, when `grep -rn 'StreamBuffer\|stream_buffer' crates/` runs, then the hits are exactly the legacy-config strings (`config.rs`, `nd4h_legacy_config_compat.rs`, `nd4h_source_fences.rs`) plus the pre-existing `flush_stream_buffer` stub and its caller (`state.rs`, `app.rs`), all byte-identical to HEAD. **B3** — Given the change, when the three legacy-config files are diffed against the default-branch merge-base, then the diff is empty and the `nd4h_*` tests still pass. **B4** — Given the change, when `CLAUDE.md`, `AGENTS.md`, `README.md` are searched case-insensitively for `stream buffer`, then no hit remains (the cyril-ui `Owns:` line no longer lists it); the dated `docs/plans/2026-03-21-*` mentions remain byte-identical. **B5** — Given the change, when the six CI gate commands (fmt check; clippy all-features; nextest all-features; doctest all-features; clippy default; nextest default) run, then each exits 0. | yes |

Unknown tests: none.

## Selected route

Structural — T2 fires: a `pub mod` leaves `cyril-ui`'s public crate surface. T1 and T3 are no and T4 is yes, so no interrogation or probe stage is needed; the requester pre-approved this exact scope (recorded verbatim in `design.md`).

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict yes; contract B1–B5 above) |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict no) |
| design.md | falsifiable-design | required |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Structural route.

## Downstream sequence

falsifiable-design → budgeted-plan → checkpointed-build (interrogated-spec and prove-it-prototype are N/A per the table above).

## Terminal criterion

Structural — every downstream artifact satisfies its owning stage's completion criterion, ending with no `FAIL` in `checkpointed-build`'s recorded gate (`plan.md`, Slice 1 gate record) and a `PASS` isolated design-conformance review (`design.md`, Final design-conformance review).
