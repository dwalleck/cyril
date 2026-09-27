# Design: cyril-ell0 — delete the dead `stream_buffer` module

## Route and inputs

- Route: **Structural** (`route.md`, T2 yes: `pub mod stream_buffer` leaves `cyril-ui`'s public surface; T1 no, T3 no, T4 yes).
- Behavior set: `route.md` T4 evidence, contract **B1–B5** (spec.md is `N/A — behavior fully explicit`).
- Empirical premises: none (`route.md` T1: the zero-consumer census is current repository evidence at `f9bc81d8`; `evidence.md` is `N/A`).
- Requester decisions (from the batch instruction, 2026-09-27): option (a) delete the file and the `pub mod` line; no wiring, no replacement; the nd4h legacy-config tests and the `config.rs` comment mentioning `stream_buffer_timeout_ms` stay untouched; before/after grep receipts go in the PR; docs describing the module as a live component are corrected, historical descriptions are reported not rewritten.

## Input shapes

The "inputs" of a deletion are the references to the deleted entity. Census at `f9bc81d8` (`grep -rn 'StreamBuffer\|stream_buffer' crates/`; `grep -rni 'stream buffer' --exclude-dir=target --exclude-dir=.git .`):

| Shape | Instances at `f9bc81d8` | Status |
|---|---|---|
| Type identifier `StreamBuffer` outside the module file | 0 | Covered — C2 |
| Module path `stream_buffer::` / `cyril_ui::stream_buffer` | 0 | Covered — C2 |
| `mod stream_buffer` declaration | 1 — `crates/cyril-ui/src/lib.rs:14` | Covered — C1 |
| Re-export `pub use stream_buffer::…` | 0 (`lib.rs:23` re-exports `error` only) | Covered — C1 (absence of declaration) and C2 (absence of path) |
| Doc prose naming the module as a **live** component (`stream buffer`) | `CLAUDE.md:155`, `AGENTS.md:154` (cyril-ui `Owns:` line) | Covered — C4 |
| Generated component catalog naming the module (found by the isolated review; missed by the initial census, which searched `docs/`, `README.md`, and the two top-level guides) | `.agents/summary/components.md:186–188` (`### stream_buffer.rs — StreamBuffer`, live catalog, hand-annotated 2026-07-27); `.agents/summary/review_notes.md:57` (documentation-gap row citing the 149-LOC file) | Covered — C4 (extended) and C5 (paths added to the approved set) |
| Doc prose naming it **historically** | `docs/plans/2026-03-21-cyril-v2-architecture-design.md:68,354,404,420`; `docs/plans/2026-03-21-cyril-v2-implementation.md:703,706,720,723,1339` | `N/A — permanent non-goal`: dated historical plan documents; requester: "report, do not rewrite". C5 fences that `docs/` stays untouched. |
| Same-stem string for a different entity: legacy config key `stream_buffer_timeout_ms` | `crates/cyril-core/src/types/config.rs:153`; `crates/cyril-core/tests/nd4h_legacy_config_compat.rs:3,40`; `crates/cyril/tests/nd4h_source_fences.rs:272` | Covered — C3 (byte-identical to merge-base) |
| Same-stem symbol for a different entity: `UiState::flush_stream_buffer` no-op stub + caller + comment | `crates/cyril-ui/src/state.rs:2968–2973`; `crates/cyril/src/app.rs:874–877` | `N/A — intended future work (tracker ID: cyril-bj5o; this session is read-only on rivets)`: the requester scope names only the module and its declaration; the stub is a distinct pre-existing dead path. C5 fences that both files stay byte-identical. Surfaced in the PR "Discovered, not fixed" and the final report; relabeled from "permanent non-goal" per `review-decisions.md` F1. |
| Colocated unit tests of the module (9 `#[test]` fns) | `stream_buffer.rs:68–149` | Deleted with the file — C1; no other test references them (C2 census) |
| Captured wire traces / logs containing the string (they embed a copy of `app.rs`) | `experiments/conductor-spike/*.jsonl`, `experiments/conductor-spike/logs/*.log` | `N/A — permanent non-goal`: immutable evidence captures, not documentation |

Decision cells: `N/A — reason`: the change introduces no predicate, allowlist, bound, capability, dispatch, or boundary representation. It is a pure deletion plus two one-line prose corrections.

## Removed invariants (subtractive sweep)

The change is **subtractive**: it removes the constraint "the `cyril-ui` crate exports `StreamBuffer`". What that constraint silently guaranteed: nothing — a type that is never constructed enforces no runtime invariant, and no reader depends on its presence (C2 census). Still-safe notes:

- The streaming path's flush-on-boundary behavior is owned by `UiState::apply_notification` / `commit_streaming` (CLAUDE.md "Streaming Content Model") and was never delegated to `StreamBuffer`; it is unchanged and fenced by C5 (`state.rs`, `app.rs` byte-identical).
- Legacy config-key acceptance (`stream_buffer_timeout_ms` accepted and ignored) is owned by `config.rs` serde handling and fenced by the nd4h suite; unchanged (C3).
- Weakened fence: the 9 deleted unit tests fenced only the deleted type; their detection set covered no surviving code. Recorded under checkpointed-build item 11 (Preserved enforcement) as a deletion authorized by the Approval below.

## Placement

No new capability. `N/A — reason`: the requester's option (a) retains no capability (Owner: none; New seam: none; Forbidden: re-homing the debounce into `UiState`/`App` — fenced by C5).

## Module shape

T2 records a module-shape change (interface removal), so the module-shape design procedure applies.

### Inventory

| Module/path | Production lines | Interface | Responsibility clusters | Dependencies | Callers / tests | Classification |
|---|---:|---|---|---|---|---|
| `crates/cyril-ui/src/stream_buffer.rs` | 67 (149 total; `#[cfg(test)]` at line 68) | `StreamBuffer::new(Duration)`, `push(&str) -> Option<String>`, `should_flush() -> bool`, `flush() -> Option<String>` | debounced streaming-text flush at `\n` / code-fence boundaries or after a timeout (never exercised) | `std::time` only | callers: none; tests: 9 colocated | `delete` |
| `crates/cyril-ui/src/lib.rs` | 22 (23 total) | crate root: module declarations + `pub use error::{Error, ErrorKind, Result}` | module registry of `cyril-ui` | — | compile | `retain` (one declaration removed) |

### Seam tests

1. Deletion test — deleting `stream_buffer.rs` makes no hidden complexity reappear anywhere (zero callers): the module was not pass-through, it was dead.
2. Interface test — no caller or external test crosses the interface; nothing to preserve.
3. Adapter test — N/A: no seam is created.
4. Locality test — after the deletion the debounce responsibility has no owner and no verification location, which is the approved end state (permanent non-goal).

### Alternatives

`N/A — requester decision`: ownership is not a decision here. The requester chose option (a) (delete, no replacement); option (b) (wire into the streaming path) is a recorded permanent non-goal for this change. No candidate owner exists because no capability is retained.

### Approved module ledger

| Module/path | Interface | Owns | Hides/reuses | Must not own | Adapters | Tests through | Change |
|---|---|---|---|---|---|---|---|
| `crates/cyril-ui/src/stream_buffer.rs` | (removed) `StreamBuffer::{new,push,should_flush,flush}` | (removed) debounced streaming flush | `std::time` | — | N/A | 9 colocated unit tests, deleted with it | `delete` |
| `crates/cyril-ui/src/lib.rs` | module declarations + `error` re-export | `cyril-ui` module registry | — | any responsibility body | N/A | compile (`cargo check`) | `retain` |

### Protected parents

| Protected parent | Baseline responsibilities | Allowed change | Forbidden change | Exit condition |
|---|---|---|---|---|
| `crates/cyril-ui/src/lib.rs` | module declarations; `pub use error::*` | remove `pub mod stream_buffer;` | any addition | 22 lines; no `stream_buffer` token (C1) |
| `crates/cyril-ui/src/state.rs` | `UiState` streaming model, overlays, input | none | any change (no re-homing of the debounce) | byte-identical to merge-base (C5) |
| `crates/cyril/src/app.rs` | event loop, wiring | none | any change | byte-identical to merge-base (C5) |

### Shape fence

`.cyril-ell0/oracles/shape_fence.sh` — a standalone bash oracle, run from anywhere inside the repo. It discovers the default branch via `git symbolic-ref refs/remotes/origin/HEAD`, computes the merge-base with `HEAD`, and prints exactly one `C<n> PASS|FAIL: <detail>` line per claim C1–C5 (details name the offending path/line), exiting 1 if any line is `FAIL`. Path space is partitioned so each mutation localizes to one claim: C1 owns `crates/cyril-ui/src/stream_buffer.rs` and the `mod stream_buffer` declaration; C3 owns the three legacy-config files; C5 owns every other path (tracked diff vs merge-base, plus untracked files under `crates/`, minus C1's file).

### Length review

`N/A — no trigger`: no repository length gate exists (`route.md` T2 lookup), and the change shrinks the crate (−149 lines; `lib.rs` 23 → 22). Projected growth 0, margin 0.

## Claims

- **C1** — `crates/cyril-ui/src/stream_buffer.rs` is absent and `crates/cyril-ui/src/lib.rs` contains no `mod stream_buffer` declaration.
- **C2** — No file under `crates/` contains the identifier `StreamBuffer` (whole word) or the module path `stream_buffer::`.
- **C3** — `crates/cyril-core/src/types/config.rs`, `crates/cyril-core/tests/nd4h_legacy_config_compat.rs`, and `crates/cyril/tests/nd4h_source_fences.rs` are byte-identical to the default-branch merge-base.
- **C4** — `CLAUDE.md`, `AGENTS.md`, and `README.md` contain no case-insensitive `stream buffer` mention, and the generated component catalog `.agents/summary/components.md` and its `review_notes.md` contain no `StreamBuffer` / `stream_buffer` mention. *(Technical correction after the isolated review, 2026-09-27: the `.agents/summary/` pair is a live per-module catalog — "Generated: 2026-04-11", hand-annotated 2026-07-27 — that the original file set missed; the claim's meaning, "no live-component description survives", is unchanged.)*
- **C5** — Relative to the default-branch merge-base, the change touches only `crates/cyril-ui/src/stream_buffer.rs` (deleted), `crates/cyril-ui/src/lib.rs`, `CLAUDE.md`, `AGENTS.md`, `.agents/summary/components.md`, `.agents/summary/review_notes.md`, and `.cyril-ell0/**` (C3's three files are C3's to judge); in particular `crates/cyril-ui/src/state.rs`, `crates/cyril/src/app.rs`, and `docs/**` are untouched and no untracked file appears under `crates/`. *(Same correction: the two catalog files joined the approved path set.)*

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | file absent; no declaration | `mod stream_buffer` declaration; module file | `test ! -e crates/cyril-ui/src/stream_buffer.rs && ! grep -q 'mod stream_buffer' crates/cyril-ui/src/lib.rs` → true; the file existing or the declaration present falsifies. Other cause for the same observation: none — the checks are direct existence tests. | `git ls-files crates/cyril-ui/src/stream_buffer.rs` prints nothing **and** `env -u CARGO_TARGET_DIR cargo check --workspace --all-targets --all-features` exits 0 (a declaration without the file fails E0583; a tracked file shows in `ls-files`) — git index + rustc module resolution, not filesystem/grep | M1: re-create `crates/cyril-ui/src/stream_buffer.rs` containing `pub struct Placeholder;` (untracked, no `StreamBuffer` token, no declaration) → fence prints `C1 FAIL: crates/cyril-ui/src/stream_buffer.rs exists`, exit 1; other lines stay PASS | `.cyril-ell0/oracles/shape_fence.sh` C1 | seconds | PASS — Slice 1 gate (plan.md): fence green, M1 red, restored green |
| C2 | zero consumers | `StreamBuffer` identifier; `stream_buffer::` path | `grep -rnw 'StreamBuffer' crates/; grep -rn 'stream_buffer::' crates/` → no output; any hit falsifies. Positive control (absence claim): the same grep at `f9bc81d8` finds 13 `StreamBuffer` hits inside the module file, so the search can observe the token. Other cause: a hit in a comment only — still a violation of the claim as stated (textual reference). | `env -u CARGO_TARGET_DIR cargo check --workspace --all-targets --all-features` exits 0 — rustc name resolution fails (E0432/E0433) on any code reference to a deleted module; a different mechanism from text search | M2: append `pub use crate::stream_buffer::StreamBuffer;` to `crates/cyril-ui/src/lib.rs` → fence prints `C2 FAIL: crates/cyril-ui/src/lib.rs:<n>:pub use crate::stream_buffer::StreamBuffer;`, exit 1; C1 stays PASS (no `mod stream_buffer`), C5 stays PASS (`lib.rs` allowlisted); the oracle also goes red (E0433) | `.cyril-ell0/oracles/shape_fence.sh` C2 | seconds (fence); minutes (oracle) | PASS — pre-deletion form at `f9bc81d8` (run log) and rerun on the changed tree at the Slice 1 gate (plan.md): six B2 lines, zero identifiers/paths; M2 red, restored green |
| C3 | legacy-config fences untouched | the three `stream_buffer_timeout_ms` files | `git diff --quiet $(git merge-base HEAD origin/main) -- <3 files>` → exit 0; any diff falsifies. Other cause: none — byte comparison. | `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core -p cyril --all-features -E 'binary(/nd4h/)'` → the `nd4h_legacy_config_compat` and `nd4h_source_fences` binaries run and pass (9/9 at the Slice 1 gate, `ell0/logs/nd4h.log`) — behavioral (the tests exercise legacy-key acceptance), not byte identity. *(Corrected per `review-decisions.md` F2: the draft wrote `test(nd4h)`, which filters test names and selected zero tests; `binary(/nd4h/)` is the command actually run.)* After merge the C3 fence is vacuous (merge-base = main); this inherited suite is the durable fence. | M3: append one empty line to `crates/cyril-core/tests/nd4h_legacy_config_compat.rs` → fence prints `C3 FAIL: crates/cyril-core/tests/nd4h_legacy_config_compat.rs differs from merge-base`, exit 1; C5 stays PASS (C3 files are excluded from C5's scan) | `.cyril-ell0/oracles/shape_fence.sh` C3 (PR-scoped: vacuous once merged; the durable behavioral fence is the inherited nd4h suite, untouched — item 11) | seconds | PASS — Slice 1 gate (plan.md): fence green, nd4h binaries green (oracle), M3 red (re-proved after a message-spacing fix to the fence), restored green |
| C4 | no live-component doc mention | `stream buffer` prose in `CLAUDE.md`, `AGENTS.md`, `README.md`; `StreamBuffer`/`stream_buffer` entries in `.agents/summary/components.md`, `.agents/summary/review_notes.md` | `grep -in 'stream buffer' CLAUDE.md AGENTS.md README.md; grep -n 'StreamBuffer\|stream_buffer' .agents/summary/components.md .agents/summary/review_notes.md` → no output; any hit falsifies. Positive controls: at `f9bc81d8` the greps hit `CLAUDE.md:155`, `AGENTS.md:154`, `components.md:186`, `review_notes.md:57`. Other cause: none. | Structural cross-check: every component named on the cyril-ui `Owns:` line maps to an existing module under `crates/cyril-ui/src/` (`state.rs`, `traits.rs`, `widgets/`, `render.rs`+`text.rs`, `highlight.rs`, `file_completer.rs`); a name with no module is stale — mapping vs string search | M4: re-insert `, stream buffer` at the end of the cyril-ui `Owns:` line in `CLAUDE.md` → fence prints `C4 FAIL: CLAUDE.md:<n>:…stream buffer`, exit 1; M4b: append the heading `### \`stream_buffer.rs\` — StreamBuffer` to `.agents/summary/components.md` → `C4 FAIL: .agents/summary/components.md:<n>:…`, exit 1; C5 stays PASS in both (paths allowlisted) | `.cyril-ell0/oracles/shape_fence.sh` C4 | seconds | PASS — Slice 1 gate (plan.md): fence green, Owns-line → module mapping clean (oracle), M4 red, restored green |
| C5 | path allowlist / no re-homing | every path outside C1's and C3's ownership | `{ git diff --name-only $(merge-base); git ls-files --others --exclude-standard crates/; }` minus C1's file, filtered by the allowlist → empty; any other path falsifies. Other cause: none — set membership. | `git diff --numstat $(merge-base) -- crates/` sums to exactly 0 added / 150 deleted lines (149 file + 1 `lib.rs`) — a magnitude measurement that a wired replacement (added lines) would also break, independent of path-set membership | M5: append `// cyril-ell0 mutation` to `crates/cyril-ui/src/state.rs` → fence prints `C5 FAIL: unexpected path crates/cyril-ui/src/state.rs`, exit 1; C1–C4 stay PASS | `.cyril-ell0/oracles/shape_fence.sh` C5 | seconds | PASS — Slice 1 gate (plan.md): fence green, numstat 0 added / 150 deleted (oracle), M5 red, restored green |

## Non-goals and future work

Permanent non-goals (no tracker issue):

- Option (b) — wiring `StreamBuffer` into the streaming path or adding a replacement debounce. Requester decision: "Do NOT wire it into the streaming path; do NOT add a replacement."
- Rewriting the dated historical plan documents `docs/plans/2026-03-21-cyril-v2-architecture-design.md` and `docs/plans/2026-03-21-cyril-v2-implementation.md`. Requester: "report, do not rewrite, anything describing it historically."

Intended future work:

- Removing the `UiState::flush_stream_buffer` no-op stub (`crates/cyril-ui/src/state.rs:2968–2973`), its per-tick caller, and the stale `// Flush stream buffer on tick` comment (`crates/cyril/src/app.rs:874–877`). Not in the approved scope of this change (a distinct entity; left byte-identical, fenced by C5) and deliberately not removed in this PR. **tracker ID: cyril-bj5o** — this session is read-only on rivets (batch instruction), so the placeholder stands in for the verified ID the contract normally requires; `review-decisions.md` F1 records the relabel from "permanent non-goal".

## Falsifier run log

Cheapest falsifier (C2, pre-deletion form — "outside the module file and its declaration, zero references"), run 2026-09-27 at `f9bc81d8` in the linked worktree:

```
grep -rn 'StreamBuffer\|stream_buffer' crates/
```

Result — 19 lines: 13 inside `crates/cyril-ui/src/stream_buffer.rs` (positive control: the token is observable); `crates/cyril-ui/src/lib.rs:14 pub mod stream_buffer;` (the declaration, C1); `config.rs:153`, `nd4h_legacy_config_compat.rs:3,40`, `nd4h_source_fences.rs:272` (`stream_buffer_timeout_ms`, C3 entity); `state.rs:2971` and `app.rs:875` (`flush_stream_buffer` stub, a different symbol — verified by reading `state.rs:2968–2973`: `pub fn flush_stream_buffer(&mut self) -> bool { false }` with no import of the module). Zero `StreamBuffer` identifiers and zero `stream_buffer::` paths outside the module → **PASS**.

C4 positive control, same revision: `grep -rni 'stream buffer' --exclude-dir=target --exclude-dir=.git .` → `AGENTS.md:154`, `CLAUDE.md:155`, `crates/cyril/src/app.rs:874` (comment on the stub call), `docs/plans/2026-03-21-cyril-v2-architecture-design.md:354`, plus JSONL/log captures embedding `app.rs`.

## Approval

Requester (batch orchestrator relaying the user's approved scope), 2026-09-27, verbatim:

> "Option (a) from the issue: DELETE crates/cyril-ui/src/stream_buffer.rs and the `pub mod stream_buffer;` line in crates/cyril-ui/src/lib.rs (precedent: cyril-85py zero-reader removal). Do NOT wire it into the streaming path; do NOT add a replacement."

> "Leave the nd4h legacy-config compatibility tests that mention the string `stream_buffer_timeout_ms` (crates/cyril-core/tests/nd4h_legacy_config_compat.rs, crates/cyril/tests/nd4h_source_fences.rs, and the comment in crates/cyril-core/src/types/config.rs) untouched — they fence legacy config-key handling, not this module."

> "Also check docs/ and README for references to the module and remove any that describe it as a live component (report, do not rewrite, anything describing it historically)."

> "If the route comes out Structural because a `pub mod` leaves cyril-ui's public surface: the requester's design approval is this exact scope (delete, no replacement, no consumers exist). Record that approval verbatim in design.md and proceed through the stages the skill requires without waiting."

Scope note recorded by the design owner: the `CLAUDE.md`/`AGENTS.md` cyril-ui `Owns:` line edits fall under the approved "remove any that describe it as a live component" instruction and under checkpointed-build's stale-prose sweep obligation (falsified prose); they are two-word removals on one line each.

Risk acceptances approved: None.

## Final design-conformance review

Isolation method: a fresh general-purpose subagent (no implementation transcript, no plan rationale) instructed to reconstruct the change from production code in this worktree only, read-only, with `.cyril-*/` and `.rivets/` forbidden and no cargo. Date: 2026-09-27. Tree reviewed: `f9bc81d8` + the uncommitted slice (before the repair below).

Reviewer's reconstruction (summarized from its report):

- Changed-path set: `crates/cyril-ui/src/stream_buffer.rs` deleted (149 lines: the struct, four methods, private `find_boundary`, 9 unit tests exercising only this type); `crates/cyril-ui/src/lib.rs:14` declaration removed; `CLAUDE.md:155` / `AGENTS.md:154` one prose line each. No production-body change, no test change outside the deleted file.
- `cyril-ui` surface: 15 `pub mod`s + one re-export (`pub use error::{Error, ErrorKind, Result}`); nothing ever re-exported `StreamBuffer`.
- Identifier census: whole-word `StreamBuffer` 0, `stream_buffer::` 0, `mod stream_buffer` 0; six `stream_buffer` substring hits in two unrelated families (`UiState::flush_stream_buffer` stub + its `app.rs` call; the removed config key `stream_buffer_timeout_ms` in the three nd4h-fenced files).
- Ownership of streaming flush: `UiState` (`state.rs` L485–503 `AgentMessage` arm, `commit_streaming` L1203–1212, `flush_streaming_agent_text` L1229–1235), event-boundary driven, no newline/code-fence detection, no timer; `flush_stream_buffer` L2968–2973 is a truthful no-op stub. No production path constructs a debounce buffer before or after the change.
- Dependency direction: `cyril-ui` → `cyril-core` only; `cyril-workbench` → `cyril-core` only (does not depend on `cyril-ui`).
- Protected files: `config.rs`, `nd4h_legacy_config_compat.rs`, `nd4h_source_fences.rs` unmodified.
- Verdict: (i) zero-consumer deletion YES; (ii) re-homing NONE; (iii) protected files CLEAN; (iv) falsified prose left behind: YES, one item — `.agents/summary/components.md:186–188` still catalogued `stream_buffer.rs` as a `cyril-ui` module (and `review_notes.md:57` cited it at 149 LOC). `docs/plans/` mentions judged dated history.

Comparison with this design: the reconstruction matches the module ledger (`stream_buffer.rs` → delete; `lib.rs` → retain with one declaration removed), the protected-parent table, the dependency direction, and claims C1–C3 and C5 as written. **Mismatch:** C4's file set (`CLAUDE.md`, `AGENTS.md`, `README.md`) and C5's approved path set omitted the generated component catalog under `.agents/summary/`, so the design's "no live-component description survives" intent was not fully fenced. **Disposition:** bounded repair within the approved contract (stale-prose sweep obligation; approved behavior, ownership, and risk unchanged): the two catalog entries were removed, C4 and C5 were extended to cover them (technical correction recorded inline above), and the changed fence was re-proved (M4, new M4b, M5 red; restored green) — record in `plan.md`, "Bounded repair after the isolated review". Reviewer side notes: mixed index state (resolved at commit by staging all four edits); the `flush_stream_buffer` stub and `app.rs:874` comment survive (intended future work — `review-decisions.md` F1; tracker ID to be filed by the orchestrator); `docs/plans/2026-03-21-cyril-v2-architecture-design.md:404` still advertises the removed config key `stream_buffer_timeout_ms` (pre-existing, outside the nd4h `doc_surfaces()` list; reported, not fixed).

Result: **PASS** after the repair — every changed production module maps to exactly one ledger row; the one mismatch is resolved and re-fenced.
