# Route: cyril-jhmi

Change: `/context` items — read the camelCase `autoIncluded` flag the live wire serves (kiro-cli ≥ 2.16.0) so the `(auto)` tag renders, keep the pre-2.16.0 `auto_included` spelling as the fallback, and fence both wire shapes.
Date: 2026-09-27

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | Two premises, both covered by evidence in the repository plus the reproducible archive scan below. **P1 — the current wire serves camelCase `autoIncluded`:** `experiments/conductor-spike/v2-turn-2.17.0.jsonl` (committed b036cbe1, 2026-08-11) is a raw JSON-RPC line capture (no `{ts,dir,msg}` envelope, so no `ts` exists); its `/context` responses at 0-based frames 5 (`id: 3`), 11 (`id: 5`) and 16 (`id: 8`) carry `"autoIncluded": true` on every `~/.kiro/skills/*/SKILL.md` item (117 occurrences file-wide, 0 snake_case). **P2 — "no capture has ever shown snake_case" (the issue's stated premise for dropping the `auto_included` read):** FALSIFIED. Committed captures with `"auto_included": true` on live `/context` items: `experiments/conductor-spike/test_bridge-2.1.0.out`, `experiments/conductor-spike/logs/conductor-2.5.1.log` (raw compact wire frame, key order `name,tokens,matched,percent,auto_included` — identical to 2.17.0's order with the camelCase key, so it is the wire, not a cyril re-serialization), `experiments/conductor-spike/test_bridge-conductor-2.7.1.out`, `experiments/conductor-spike/test_bridge-2.12.0.out` (2026-07-09). Archive scan (`.cyril-jhmi/pin-rename.py`, output in `.cyril-jhmi/pin-rename-output.txt`): `kiro-cli-chat` 2.12.0, 2.12.1, 2.12.3, 2.13.0, 2.14.0, 2.14.1, 2.14.2, 2.15.0 contain the literal `auto_included` (1×) and never `autoIncluded`; 2.16.0, 2.16.1, 2.16.2, 2.17.0 contain `autoIncluded` (1×) and never `auto_included`. **The key was renamed in kiro-cli 2.16.0.** The 2.16.0 capture (`v2-turn-2.16.0.jsonl`) has neither spelling (no auto-included items in that session) and is uninformative — the issue's "absent in 2.16.0" reading is an absence of items, not of the field. No premise remains unverified. | no |
| 2 | Structural module shape | `append_context_items` (`crates/cyril/src/app.rs:2478`, private free fn, single caller `format_command_response` at `app.rs:2572`) keeps its signature, owner and responsibility (JSON `/context` item → display row); the change is a key-name read inside one existing responsibility. No interface, schema, seam or dependency direction changes. Length gate lookup: `grep app\.rs scripts/ .github/ .githooks/` → none; no line-count gate script in the repo; the protected-parent ledgers in `.cyril-gl5s/` (2,723-line baseline) and `.cyril-v19o/` (6,857-line baseline) were change-scoped fences owned by those changes' own `oracles/`, not standing repository gates — no inherited threshold applies. Baseline 7,619 lines (incl. tests); actual delta +8 production lines (doc comment +5, one `.or_else` read +1, comment +2) and +74 test lines (new fence + legacy-fixture doc comment); uncertainty ±0 (measured: `git diff --numstat main` = +87/−5). No length-review trigger. | no |
| 3 | Production-scale risk | Renders one `/context` response (≤ ~50 items) on demand; no latency, throughput, memory, concurrency or data-volume dimension. | no |
| 4 | Explicit behavior | Complete contract (behavior source; `spec.md` is N/A). **G/W/T-1:** given a `/context` `commands/execute` response whose `contextFiles.items[]` entry carries `"autoIncluded": true` (kiro-cli ≥ 2.16.0), when `format_command_response("context", …)` renders it, then the item row ends with ` (auto)` (row format `    {name} — {tokens} tokens ({pct:.1}%){tags}\n`, unchanged). **G/W/T-2:** given an item carrying the pre-2.16.0 spelling `"auto_included": true` (kiro-cli ≤ 2.15.0) and no `autoIncluded` key, when rendered, then the row ends with ` (auto)` (fallback read; zero regression for older binaries). **G/W/T-3:** given an item with neither key, or a flag that is not boolean `true`, then no ` (auto)` tag (unchanged). **G/W/T-4:** given an item with `"matched": false`, then ` (unmatched)` is appended after any ` (auto)` (unchanged). Ordering, category summaries and the `tools`-without-`items` case are unchanged. **D1 resolved 2026-09-27 — option (B), read `autoIncluded` first and fall back to `auto_included`.** Rationale recorded from the requester: the issue text itself conditions keeping the snake_case read on a payload being shown to use it, and the captures plus the binary scan (rename pinned to kiro-cli 2.16.0) satisfy that condition, so the approved contract determines the answer; no new risk acceptance is being made. | yes |

Unknown tests: none

## Selected route

Local — no unverified premise, no module-shape or scale risk, behavior fully explicit after D1. **Route correction record (Direct stage entry):** the first routing pass (commit 8fc059f4) recorded T4 = `no` (Structural) because the approved "drop the snake_case read" instruction rested on the falsified premise P2, leaving D1 open; the requester resolved D1 as (B) with the rationale above, T4 was re-run → `yes`, precedence (T1 no, T2 no, T3 no, T4 yes) yields Local, and this file was rewritten. No other verdict changed.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict; D1 resolved by the requester and recorded in the T4 evidence row) |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict); P1/P2 evidence is in the repository captures and `.cyril-jhmi/pin-rename-output.txt` |
| design.md | falsifiable-design | N/A — Local route: no design gate |
| plan.md | budgeted-plan | N/A — Local route: no plan gate |

Oracle checkpoint in `checkpointed-build`: N/A — Local route: checkpointed-build does not run

## Downstream sequence

none — implement with normal repository fix/TDD

## Implementation record

- Red first (commit 8fc059f4): `format_response_context_items_render_auto_tag_from_camel_case_wire` — capture-derived fixture (frame 5 of `v2-turn-2.17.0.jsonl`, `id: 3`, verbatim subset) asserting the exact "Context files" block incl. ` (auto)`; FAILED on HEAD f9bc81d8 exactly on the missing tags (receipt `.cyril-jhmi/red-fence-on-head-f9bc81d8.log`).
- Fix: `append_context_items` reads `autoIncluded` and falls back to `auto_included`; fn doc comment states the 2.16.0 rename and cites `.cyril-jhmi/pin-rename-output.txt`.
- Legacy fence: the existing snake_case fixture is retained and renamed `format_response_context_breakdown_lists_files_pre_2_16_snake_case` with a doc comment naming its era and evidence.
- Blast radius: no signature, name or caller change (`append_context_items` is private with one caller); `tethys callers` not needed.

## Terminal criterion

Local — focused behavioral verification: `cargo test -p cyril format_response_context` (all three context fixtures green, the new fence red-before/green-after), then the full CI-mirror gate (`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo nextest run --workspace --all-features`; `cargo test --doc --workspace --all-features`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo nextest run --workspace`), each run through the scratchpad `cargo-clean.sh` wrapper (unsets the empty `CARGO_TARGET_DIR`).

Results are appended below after the hand-off.

Result: 2026-09-27 | `cargo test -p cyril format_response_context` (fix applied) | PASS — 3 passed (`format_response_context_breakdown`, `format_response_context_breakdown_lists_files_pre_2_16_snake_case`, `format_response_context_items_render_auto_tag_from_camel_case_wire`); receipt `.cyril-jhmi/green-fence-after-fix.log`. Red half: same fence FAILED on HEAD f9bc81d8 before the fix (`.cyril-jhmi/red-fence-on-head-f9bc81d8.log`).
Result: 2026-09-27 | `cargo fmt --all -- --check` | PASS
Result: 2026-09-27 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS
Result: 2026-09-27 | `cargo nextest run --workspace --all-features` | PASS — 2058 tests run, 2058 passed, 13 skipped
Result: 2026-09-27 | `cargo test --doc --workspace --all-features` | PASS
Result: 2026-09-27 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS
Result: 2026-09-27 | `cargo nextest run --workspace` | PASS — 2056 tests run, 2056 passed, 13 skipped

Final T2 recheck (2026-09-27): `crates/cyril/src/app.rs` — one existing responsibility, no interface/seam/owner change, no repository length gate; measured delta versus main `git diff --numstat main -- crates/cyril/src/app.rs` = +87/−5 (7,619 → 7,701 lines; production +8, tests +74). No new length-review trigger; no evasion (no comments removed, no packing, no counting changes). T2 verdict unchanged: no.
