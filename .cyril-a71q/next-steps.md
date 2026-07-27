# Next steps — cyril-a71q (handoff, 2026-07-12)

Written at commit time on the `cyril-a71q` branch. This laptop lacks the research
archive (`~/.local/share/kiro-research/` absent in WSL Ubuntu home `/home/dwalleck`);
resume on the machine that has it.

## Where things stand

| Gate | Status |
| --- | --- |
| Spec (`spec.md`) | **PASSED** — re-anchored contract, requester signed "I confirm these consequences" 2026-07-12 |
| Prototype (`prototype.md`) | passed — evidence (pinned frames, no native turn id, runtime defect repros) valid; its contract layer was voided |
| Design (`design.md`) | **PASSED** — cheapest falsifier `probes/design_reanchored_falsifier.py`: correct policy 0/34 failures, 4/4 mutations distinct; pending requester review |
| Plan (`plan.md`) | not started |
| Build | not started |

Read in this order to reconstruct context: `timing-audit.md` (why the prior choice-A
pipeline was voided) → `spec.md` (the signed contract) → `design.md` (policy + claims +
fences) → `workflow.md` (decision log incl. which prior signed decisions survive).

Contract in one line: `TurnId(u64)` stamped at dispatch; **first-source-wins release
retained** (j16p); the second same-turn signal is an absorbed *expected companion*
(id-match for synthesized, one-entry session-keyed ledger for wire `turn_end`,
absorb-first); both `{source, stop_reason}` recorded for cyril-pnwb; rate-limited turns
release via the response (cyril-3zy4 restored).

## Immediate next: tui.js / KAS-agent corroboration probe (needs the research archive)

Purpose: upgrade the ordering premise ("`turn_end` then response, both present") from a
two-turn live observation (`kas-live-session-trace-2.11.0.jsonl`) to source-confirmed
producer/client behavior. The covenant (`docs/kiro-kas-acp-covenant.md` §4) confirms
`turn_end {stopReason}` is **the** "turn completion signal" but is silent on cardinality
and ordering vs. the `session/prompt` RPC response — this probe fills that gap.

1. **Client side** — `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.11.0.js`
   (match the capture version; diff against a newer bundle if present). Grep
   `turn_end`, `session_info_update`, `stopReason`, the `session/prompt` call site.
   Questions: what does Kiro's own client key end-of-turn/busy on — `turn_end`, the RPC
   response, or first-of-either? Does it tolerate a missing signal? What does the cancel
   path do with the two stop reasons (feeds cyril-pnwb)?
2. **Emitter side** — the KAS agent bundle embedded in `kiro-cli-chat`
   (extraction precedent: `docs/kiro-agent-schema-2.8.1-kas-0.3.257.md`). Find the
   `turn_end` emission site. Is it emitted unconditionally before the prompt RPC
   resolves? Which paths skip it (rate limit, error, cancel)? Can one prompt ever emit
   two scoped `turn_end` frames?
3. **Interpretation:**
   - Corroboration → annotate design.md blindness B10/B1 with the source evidence and
     proceed to the plan.
   - Contradiction (a legitimate response-only path, response-before-turn_end emission,
     or multi-`turn_end`) → re-open spec §"Degenerate KAS input" and the
     companion-ledger bound; re-run `design_reanchored_falsifier.py` with the corrected
     input space (cheap — the traces are the contract).
   Per CLAUDE.md, for `_kiro/*` contract questions read the covenant doc first; tui.js
   and `@kiro/agent` are implementation evidence, not the wire contract.

## Then: budgeted plan (`plan.md`)

Gilfoyle budgeted-plan over design.md's 10 claims. The pending falsification fences are
the slice obligations: implementation fences for C1/C2/C6 (bridge harness: both receipt
orders, stale/companion matrix, evidence ledger), C3 (`terminal_scope_owner_matrix`),
C4 (`active_prompt_futures_bounded` — note ≤2 live futures replaces the single
`prompt_task` handle), C5 (`kas_error_is_failstop`), C7 (`only_owned_completion_mutates_main`),
C8 (exhaustion), C9 (256+1 backlog), C10 (`rate_limited_turn_releases_via_response`).
Constraints table in spec.md bounds each slice. Key seams: `bridge.rs` run_loop
(`turn_in_flight` → active record + ledger), `event.rs` `RoutedNotification`
(TurnCompleted gains `Option<TurnId>` — design decision, newtype per CLAUDE.md),
`convert/kas.rs` (unstampable wire arm), App routing (cross-session early return).

## Housekeeping

- rivets notes were added to **cyril-a71q** (timing audit summary) and **cyril-pnwb**
  (its ACTION item is answered: the 2.11.0 capture contains a live cancel with
  `turn_end.stopReason=cancelled` agreeing with the response). The tracker edit is
  committed on this branch; the **main checkout's** working tree carries the same
  `.rivets/issues.jsonl` edit uncommitted — dedupe or discard it when this branch lands.
- `.pi-subagents/` is deliberately untracked (pipeline scratch), matching main-repo
  precedent.
- Superseded artifacts kept for audit: `spec-superseded-sole-turn-end.md`,
  `spec-pre-bounded-exact-one.md`, `design-superseded-sole-turn-end.md`,
  `design-superseded-either-source.md` (its C1 impossibility is valid only for the
  over-generalized input space).
