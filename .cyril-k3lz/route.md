# Route: cyril-k3lz

Change: Thinking on/off toggle on both engines — KAS `thinking` configOption (0.66.8) and the v2 `reasoning` command (2.23.0+).
Date: 2026-09-25

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | Every wire premise is covered by committed live captures on the current release (2.24.0 / KAS 0.66.8): (a) KAS `thinking` select option shape, presence only for toggleable models, `set_config_option thinking=off` effect + effort cap, `bogus`→off, no option on GPT — `docs/kiro-2.24.0-wire-audit.md` § 7.1, captures `experiments/conductor-spike/kas-new-surface-{noprompt,turns}-0668-2.24.0.jsonl`; (b) v2 `commands/execute {command:"reasoning", args:{thinkingEnabled:false}}` → `{success:true, message:"", data:{thinking, effortLevels, defaultThinkingEnabled, defaultEffort}}`, next `_kiro.dev/metadata` carries `reasoning.thinkingEnabled:false`; unknown arg keys (`enabled`, `thinking`, `value`) are silently ignored — `v2-reasoning-args{,2}-2.24.0-2.24.0.jsonl` lines 24–33; (c) `reasoning` is not in `commands/available` and `commands/options reasoning` is -32700 — same captures line 10; (d) v2 `reasoning` block parsing already shipped and fenced (cyril-q1xs, PR #129). Not probeable here (no kiro-cli in this environment) and not a design premise: whether the v2 command's persisted "reasoning default" (`defaultThinkingEnabled` flips with the toggle; static `reasoning.rs` "persists a reasoning default") is user-global — recorded as a known caveat, same class as cyril-v2ol. | no |
| 2 | Structural module shape | Adds an engine-neutral thinking-control domain type in `cyril-core/types`; new state on `SessionController` (command gating reads it) and `UiState` (toolbar reads it) fed from two existing notifications (`MetadataUpdated.reasoning` — v2; `ConfigOptionsUpdated`/`ConfigOptionSet` `thinking` entry — KAS); a new builtin slash command in `CommandRegistry` choosing between two existing bridge primitives (`ExecuteCommand` vs `SetConfigOption`); a new `TuiState` accessor + toolbar render. Current owners: `convert/` (wire→domain, unchanged), `SessionController` (session facts), `UiState` (display), `commands/builtin.rs` (local commands). Candidate owners are the same modules — no responsibility moves — but a public trait (`TuiState`) and public domain types grow, and `App` (protected multi-responsibility orchestrator) must not absorb the logic. | yes |
| 3 | Production-scale risk | None: one command per user action, one small state field per notification; no latency/throughput/memory/concurrency/data-volume dimension. | no |
| 4 | Explicit behavior | Unresolved: command name and syntax (`/thinking` vs `/reasoning`; toggle vs explicit on/off; no-arg behavior), toolbar presentation of on/off/unknown, behavior when the current model is not toggleable or state is unknown, how the v2 unknown `thinkingEnabled` (absent on a toggleable model) is shown, whether cyril mirrors KAS's effort cap in display, feedback message after a toggle. | no |

Unknown tests: none

## Selected route

Structural — T2 fires (public domain type + `TuiState` + new builtin command), with unresolved behavior (T4 no) requiring interrogation.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | required — unresolved behavior to interrogate (T4 verdict) |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict): all wire behavior covered by committed 2.24.0 captures |
| design.md | falsifiable-design | required |
| plan.md | budgeted-plan | required |

Oracle checkpoint in `checkpointed-build`: required — Structural route

## Downstream sequence

interrogated-spec → falsifiable-design → budgeted-plan → checkpointed-build

## Terminal criterion

Structural — every downstream artifact satisfies its owning stage's completion criterion, ending with no FAIL in checkpointed-build's recorded gate.
