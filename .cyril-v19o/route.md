# Route: cyril-v19o

Change: Consume the KAS `_kiro/powers/items_changed` push into a rendered "powers" view, without wiring any affordance to the declared-but-unimplemented `_kiro/powers/refresh`.
Date: 2026-09-10

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | **Premise A — the frame reaches cyril's converter at all**: `crates/cyril-core/src/protocol/client.rs:136-145` funnels every `_`-prefixed method to `DomainWork::ExtensionNotification` with no allowlist; `domain_mediator/inbound.rs:170` then calls `Engine::convert_ext_notification`. Unhandled methods die in the `other =>` catch-all at `convert/kiro.rs:1107-1110` (`debug!` + `Ok(None)`). Covered by current code. **Premise B — what KAS 2.21.2 actually emits, in cyril's spawn shape**: NOT covered. All committed captures are version-pinned below the installed binary (`kiro-cli 2.21.2`): `experiments/conductor-spike/kas-powers-2.20.1.jsonl` (populated, real install), `kas-powers-2.21.0-2.21.1.jsonl` + `kas-powers-2.21.1-2.21.1.jsonl` (empty graph, refresh trap), and `crates/cyril-core/tests/fixtures/kas/workflow/kas-csig-2.16.0-neutral.jsonl:5` (empty). The repo's own audit doctrine (`docs/kiro-2.20.1-wire-audit.md` §1.3 "live probing is now primary evidence; static diff is only a hypothesis generator"; AGENTS.md "wire format = binary × backend") makes a cross-version assumption exactly the unverified premise this route exists to catch, and the converter tests need a *current* captured frame as an oracle rather than JSON transcribed from a doc. | **yes** |
| 2 | Structural module shape | YES on three axes. (a) **Domain enum**: `Notification` (`crates/cyril-core/src/types/event.rs:28`) gains a variant — the single channel every consumer matches on. (b) **Public UI interface**: `TuiState` (`crates/cyril-ui/src/traits.rs:23`) gains a panel accessor beside `hooks_panel`/`code_panel`/`usage_panel` (traits.rs:107-109), which the renderer consumes via `render.rs:138-161`. (c) **New production modules**: a power payload type owner in `cyril-core/src/types/` and a widget module in `cyril-ui/src/widgets/` (registry `widgets/mod.rs:1-13`). Candidate owners, in the shape of the closest precedent (`HooksPanelState` at `traits.rs:509-516` + `widgets/hooks_panel.rs`): a `PowersPanelState` in `cyril-ui/src/traits.rs`, a `widgets/powers_panel.rs`, and a wire-payload type owned by `cyril-core`. Protected parent: `crates/cyril/src/app.rs` (6857 lines) — the App is a multi-responsibility orchestrator; the change adds only a routing arm there, in the shape of the existing `HooksChanged` arm (`app.rs:1402-1411`), and adds NO new state to `App`. | yes |
| 3 | Production-scale risk | The push is a handful of items (3 in the captured real install), delivered once per session creation; the view is a bounded, scrollable list. No latency, throughput, memory, concurrency, or data-volume dimension is in play. | no |
| 4 | Explicit behavior | **no.** The wire facts are explicit (README in the ticket: no-param `list`, unprompted session-scoped `items_changed`, `refresh` uncallable). Unresolved is the *client-side outcome*: the ticket says "populate a powers view" without naming the surface (overlay panel vs chat text), the field set the view shows, or the empty/absent states — and this repo has two live precedents that disagree (`/hooks` → overlay at `app.rs:2596-2599`; `/sessions`, `/workflow status` → chat text at `subagent.rs:71`, `workflow.rs:65`). Interrogation is required. | no |

Unknown tests: none

## Selected route

**Empirical** — the client-side outcome is unresolved (T4) and one load-bearing premise (the wire shape emitted by the *installed* binary in cyril's spawn shape) has no evidence at the installed version (T1). Precedence Empirical > Structural.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — T4 verdict `no`: the surface and the rendered field set are unresolved |
| evidence.md, probe.* | prove-it-prototype | required — Empirical route (T1 verdict): probe `kiro-cli 2.21.2` with the real powers tree seeded into a throwaway HOME |
| design.md | falsifiable-design | required (Structural-shape change: new `Notification` variant, new `TuiState` accessor, two new modules) |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Structural/Empirical route.

## Downstream sequence

interrogated-spec → prove-it-prototype → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Structural/Empirical — every downstream artifact satisfies its owning stage's completion criterion, ending with no FAIL in checkpointed-build's recorded gate.
