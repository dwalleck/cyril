# Route: cyril-gl0m

Change: `TrackedToolCall::primary_path()` rawInput fallback also probes the v2 `fs_read` shape `rawInput.operations[0].path` (latent — reachable only when a tool call arrives with `operations[]` but no `locations[]`).
Date: 2026-09-27

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | Premise: v2 `fs_read` sends `rawInput: {"__tool_use_purpose": …, "operations": [{"mode": "Line", "path": "/abs/path"}]}` with no top-level `path`/`file_path`. Covered by current repository evidence: committed capture `experiments/conductor-spike/v2-turn-sweep-2.21.2-2.21.2.jsonl` (frame `toolCallId: toolu_bdrk_014Mqz6cqGFyUGVQUrVQyG1t`, `kind: read`, `rawInput.operations[0].path = /home/dwalleck/.claude/tmp/turnsweep-cwd-qj9of1dj/PROBE.txt`, alongside `locations[0].path` of the same value) and `docs/kiro-2.21.2-wire-audit.md` § 6b "Fields cyril does not model" (all 4 observed `operations[]` calls also carried `locations[]`, so branch 1 hit and the fallback never fired). The `fs_read` `operations[]` schema is also the documented tool-input schema. No new probe required: the fix reads a shape already captured; the latent branch is exercised by unit test, not by wire. | no |
| 2 | Structural module shape | Interface unchanged: `TrackedToolCall::primary_path(&self) -> Option<&str>` keeps its signature and its documented resolution order; the change is additive inside the third (rawInput) branch. Owner unchanged: `crates/cyril-ui/src/traits.rs` `TrackedToolCall` is the display-interpretation wrapper (CLAUDE.md "Component Separation": `primary_path()` is a presentation concern living here). No dependency-direction change (`serde_json::Value` probing already present). Callers (grep `primary_path` in `crates/`): `crates/cyril-ui/src/widgets/chat.rs:323`, `:330`; `crates/cyril-ui/src/widgets/approval.rs:74`; tests in `crates/cyril-ui/src/state.rs:4613/4624/4835/5426` — none affected by an additive fallback. Length gate lookup: `grep -rln -i "max.lines\|line.count\|file.length\|length.gate" scripts/ .github/ Cargo.toml` → no matches; no `clippy.toml` / `.cargo` config → no repository file-length gate exists (evidence of absence). `traits.rs` = 1446 lines before the change; projected growth ≈ +8 production lines + ~70 test lines (margin ±20); no threshold to trigger. | no |
| 3 | Production-scale risk | Pure function over an in-memory `serde_json::Value`, called per render frame per displayed tool call. Added cost: at most three extra map/array lookups when the flat keys miss. No I/O, allocation, concurrency, or data-volume dimension. | no |
| 4 | Explicit behavior | Given a tool call with non-empty `locations`, when `primary_path()`, then `locations[0].path` (unchanged). Given no locations and a `ToolCallContent::Diff`, then the diff `path` (unchanged). Given neither and `rawInput` carrying a string under flat `file_path` or `path`, then that string (unchanged). Given neither, no flat key, and `rawInput.operations[0].path` is a string, then that string (new). Given none of the above — including `operations` empty or `operations[0]` lacking a string `path` — then `None`, with no new logging (requester decision: "Return None (unchanged) when none match; no logging change required"). Resolution order fixed by the requester: `locations().first()` → Diff content path → flat rawInput keys → `rawInput.operations[0].path`. | yes |

Unknown tests: none

## Selected route

Local — explicit behavior, wire premise already captured in-repo, additive change inside one existing display method with an unchanged interface and owner.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict); requester decisions recorded in T4 |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict); the wire shape is in the committed 2.21.2 capture |
| design.md | falsifiable-design | N/A — Local route: no design gate |
| plan.md | budgeted-plan | N/A — Local route: no plan gate |

Oracle checkpoint in `checkpointed-build`: N/A — Local route: checkpointed-build does not run

## Downstream sequence

none — implement with normal repository fix/TDD (red fence first, minimal fix, focused verification, full CI-mirroring gate before push)

## Terminal criterion

Local — focused behavioral verification: `cargo nextest run -p cyril-ui primary_path` (the four `primary_path_*` fences in `crates/cyril-ui/src/traits.rs`), red before the fix and green after; plus the CI-mirroring gate set recorded in the PR body.

Result: 2026-09-27 | `cargo nextest run -p cyril-ui primary_path --no-fail-fast` (fences in, product code untouched) | FAIL as intended — `primary_path_resolves_fs_read_operations_path_without_locations` panicked with `left: None`, `right: Some("/abs/path/PROBE.txt")`; the other three fences passed (3 passed, 1 failed)
Result: 2026-09-27 | `cargo nextest run -p cyril-ui primary_path --no-fail-fast` (after the fix) | PASS — 4 tests run: 4 passed

Final T2 recheck: `crates/cyril-ui/src/traits.rs` 1446 → 1521 lines (+78/−3, the only file changed; production delta is the 5-line doc comment plus the `operations[0].path` probe, the rest is the fence module). No new responsibility, interface, seam, or length trigger — no repository length gate exists (see T2 evidence), so no `Length review` applies. Route stands as Local.

### Review round 1 (PR #133; decisions in `review-decisions.md`)

The round changed the production path (per-probe `as_str` fall-through and an empty-path filter inside the same rawInput branch), which invalidates the focused PASS above for that path under the contract's Evidence validity; it is re-recorded here. The route is unchanged: the repair is a technical correction inside the same function, governed by the approved resolution order (T4) and CLAUDE.md "Zero sentinel values" — no new interface, owner, seam, or unresolved decision, so T1–T4 verdicts stand.

Result: 2026-09-27 | `cargo nextest run -p cyril-ui primary_path --no-fail-fast` (three review fences added, product at `1e741638`) | FAIL as intended — `primary_path_falls_through_non_string_flat_key` (`left: None`, `right: Some("/ops/after-null.rs")`) and `primary_path_is_none_for_empty_path_strings` (`left: Some("")`, `right: None`); 5 passed, 2 failed
Result: 2026-09-27 | `cargo nextest run -p cyril-ui primary_path --no-fail-fast` (after the repair) | PASS — 7 tests run: 7 passed
Result: 2026-09-27 | `cyril-gl0m-mutate-order.sh` probe-order swap | baseline `1e741638`: mutant survived 4/4 (F1 verified); post-repair: killed — only `primary_path_prefers_flat_key_over_operations` red, 6/7, file restored byte-identical

Final T2 recheck (round 1): `traits.rs` 1521 → 1568 lines (+51/−4 versus `1e741638`; production delta is a 3-line doc-comment extension plus the same 5-line probe chain rewritten, the rest is three fences). Still no length trigger; ownership unchanged. Route stands as Local.

Round 1a (re-review nits N1–N3, doc + fences only, no production behavior change):
Result: 2026-09-27 | `cargo nextest run -p cyril-ui primary_path --no-fail-fast` | PASS — 9 tests run: 9 passed
Result: 2026-09-27 | `cyril-gl0m-mutate-nits.sh` (`operations.last()` mutant; per-probe-filter mutant) | each killed by exactly its target fence (8/9), file restored byte-identical both times
Final T2 recheck (round 1a): two fences and a one-sentence doc qualification added; probe chain unchanged. Route stands as Local.
