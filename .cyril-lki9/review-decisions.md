# Review decisions: PR #166 standards round (2026-10-12)

Source: `/code-review xhigh PR 166`, Standards axis (judgement-call smells; no
hard violations). The Spec axis found no defects; its partial/cosmetic notes
were outside this round's request and are not decided here. Requester
instruction: "fix the standards findings on the PR branch".

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | The replay hand-copies `App::run`'s permission branch, so changes to the live path would not reach it; extract an App method used by both. | code-review Standards | Verified | Code reading at a75593ad: `app.rs:854-859` (run) vs `app.rs:7786-7789` (replay) were identical copies. | Accept | 792dbb69 — `App::handle_permission_request` (review router, approval overlay, redraw); run and the replay both call it. | non-blocking — test-fidelity gap, not a product defect; app.rs production +4 within C21's cap. |
| F2 | All five replays repeat the busy/Active/Ready tail and the wake-header prefix check; fold into a helper. | code-review Standards | Verified | Code reading: identical three-assert tails at five sites; `starts_with("SYS:─── ⚙")` at three. | Accept | 2fa41fb3 — `Lki9Replay::assert_settled`, `lki9_is_wake_header`, `lki9_is_agent_text`. | non-blocking — duplication only. |
| F3 | `lki9_kinds` duplicates `lki9_system_lines`' extraction and encodes message kinds as `"SYS:"`/`"AGENT:"` string prefixes. | code-review Standards | Verified | Code reading at a75593ad `app.rs:7816`. | Modify | 2fa41fb3 — `lki9_transcript` returns the existing `ChatMessageKind` values (no new type; the real enum already carries the distinction), with count/position predicates. `lki9_system_lines` is kept for its many non-kas callers. | non-blocking — divergence: reused the existing enum instead of introducing a new one. |
| F4 | `header_seen_before_permission` is overwritten on every permission and hard-codes one test's header inside the shared harness. | code-review Standards | Verified | Code reading at a75593ad `app.rs:7729`, `7775-7779`; only `lki9_replay_gate_on_label_permission_and_step_header` reads it. | Modify | 2fa41fb3 — each `Lki9Permission` records `messages_before`; the gate-on test destructures exactly one permission and counts its own header in `messages()[..messages_before]` (exactly once, stricter than the old `Some(true)`). | non-blocking — harness clarity. |
| F5 | Busy-span labels use `std::mem::discriminant`, printing `Discriminant(N)`. | code-review Standards | Verified | Code reading at a75593ad `app.rs:7762`. | Accept | 2fa41fb3 — `lki9_variant` takes the derived-Debug variant name (`Notification` derives `Debug`). | non-blocking — diagnostics only. |
| F6 | ADR-0011 amendment heading lacks a date; "server-started turn" drifts from glossary "agent-initiated turn"; "claimed" clashes with glossary **Claim**. | code-review Standards | Verified | Other amendments date their heading (ADR-0001, ADR-0004:36/159); CONTEXT.md:152-160 uses "agent-initiated turn"; CONTEXT.md:83 defines **Claim** as a `node_start` binding. | Accept | This commit — ADR-0011 heading `(cyril-lki9, 2026-10-12)`, "agent-initiated turn", "queued for that session when the turn starts", "the completion the wake already took"; 2fa41fb3 — test doc "wake label". Pre-existing `claim_wake_labels` symbol left unchanged (outside this PR's diff). | non-blocking — terminology. |
| F7 | The `chore(rivets)` commit a75593ad also edits `plan.md`/`route.md`, so it is not tracker-only. | code-review Standards | Verified | `git show --stat a75593ad`. | Reject | N/A — rewriting published PR history to split a commit needs a force-push to a shared branch for no behavioral gain. | non-blocking — permanent non-goal; future tracker commits stay tracker-only. |

No review errors recorded: every anchor and symbol in the Standards report
matched the source at a75593ad.

## Compact repair records

### R1 — permission path (F1)

- **Delivered behavior:** `App::handle_permission_request` in
  `crates/cyril/src/app.rs`; `App::run`'s `permission_rx` arm and
  `lki9_replay` call it. Behavior-preserving; no ownership or interface change
  outside `App`'s private methods. Governing obligation: C20 replays "through
  … the App's normal session/UI routing".
- **Performed checks and results:** at 792dbb69 — the five `lki9_replay_*`
  tests and 48 `review` tests pass; clippy `-D warnings` (all-features and
  default) clean; fmt; `C21 PASS (app.rs: 4, state.rs: 0)`. Assembled gates:
  see R2.
- **Unresolved failures:** none.

### R2 — replay harness (F2–F5, F6 comment)

- **Delivered behavior:** `Lki9Permission`, `Lki9Replay::{transcript,
  assert_settled}`, `lki9_variant`, `lki9_transcript` and predicates; the five
  replays rewritten onto them with no assertion removed. Test-only.
- **Performed checks and results:**
  - M20 re-proved on the changed fences: [red](logs/2026-10-12-review-m20-red.txt)
    (5/5 fail on "every main turn reaches its end"),
    [restored green](logs/2026-10-12-review-m20-restored.txt) (byte-exact
    `cmp`, SHA-256 `9d54f016…3c79`, the same restored hash recorded in
    [plan.md](plan.md#resumed-completion-2026-10-12)).
  - Assembled gates at 2fa41fb3 (every ci.yml cargo lane, doctests, C21):
    [gates](logs/2026-10-12-review-gates.txt) — 2292 all-feature, 2285
    default, 1411 kas, 843 core-no-default tests passed.
- **Unresolved failures:** none.

### R3 — ADR terminology (F6)

Docs-only; leaves every recorded proof applicable.

## Repair review

Independent reviewer (fresh `rust-code-reviewer` agent), scope `a75593ad..2fa41fb3`
plus the ADR-0011 working-tree diff, checked at 2fa41fb3: **PASS**, no blocking
defects. It confirmed F1 is the exact former arm body with `permission_rx` read
in one place; no replay assertion weakened (F4 stricter); `messages_before` is
sound because the main transcript is push-only during a replay (the 500-message
trim and `__clear__` are unreachable for these captures); `lki9_variant` cannot
yield an empty label; and kas gating leaves the default build clean. It reran
the five replays, default and all-features clippy, and fmt. Its one
non-blocking note — "bound to that session" sits near the glossary's avoided
"session binding" — was applied as "queued for that session" (spec.md's own
wording); docs-only, so the review stays applicable.
