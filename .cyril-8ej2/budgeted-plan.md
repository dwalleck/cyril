# cyril-8ej2 — Budgeted plan

**Design:** `.cyril-8ej2/falsifiable-design.md` (approved; cheapest falsifier passed).
**Shape:** one slice. The production change is a single early-return guard in one file; the 7 design claims are distinct verification fences on that one change, not 7 separate code changes. Decomposing further would be dictation (skill red flag: "Trivial slices don't get planned. They get inlined.").

---

## Slice 1: Busy-guard the `/code` Prompt-injection arm (advisory + drop)

**Claim:** (design #1–#7) When the session is `Busy`, the `Prompt` arm of `dispatch_code_command` emits zero bridge commands, commits no `UserText`, doesn't set activity `Sending`, and adds a `System` advisory — while the `Active`, no-session, and non-`Prompt` (Panel/Unknown) paths stay exactly as they were.

**Oracle:** the function's own return `Vec<BridgeCommand>` + `UiState` reads (`messages()`, `activity()`, `has_code_panel()`), asserted directly in unit tests. Independent of any other feature; the bridge-rejection premise it defends against is separate code (`bridge.rs:386-400`), already confirmed by the design's cheapest falsifier.

**Stress fixtures** (each designed to fail a *plausible* bug, expected output written before implementation):

1. **Over-reach / wrong guard placement — `Busy` session + `Panel` response.** Plausible bug: the guard is placed before the `CodeCommandResponse` match (or gates the whole function on busy), swallowing non-Prompt responses. **Expected:** the code panel still opens (`has_code_panel() == true`), zero bridge commands. A too-broad guard fails this.
2. **Commit-order — `Busy` session + `Prompt` response.** Plausible bug: the guard is placed *after* `add_user_message`, so the desync persists. **Expected:** no `UserText` in `messages()`; the only message is the `System` advisory. A late guard fails this (a `UserText` appears).
3. **Wrong condition — `Active` session + `Prompt` response.** Plausible bug: guard checks `!= Active`/`!Idle` instead of `== Busy`, suppressing the normal send. **Expected:** exactly one `SendPrompt`, a `UserText` committed, activity `Sending`. Gating-on-not-Active fails this.

**Loop budget:** **no new production loop.** The guard is `matches!(session.status(), SessionStatus::Busy)` + early return → **O(1)**, zero syscalls. (Test code iterates `ui.messages()`, bounded by per-test message count ≈ <5 = O(small); not production.)

**Wall budget:** n/a (not an always-on phase; this is a per-`/code`-response branch).

**Doc-comment-as-contract:** the guard *is* the enforcement, and it is **load-bearing for correctness** — without it, release builds silently commit a `UserText` that is never sent (wrong output). Therefore it is a **runtime check that survives release** (`if … { advisory; return }`), **not** a `debug_assert!`. A doc comment on the branch states *why* (cyril-8ej2: bridge rejects a 2nd SendPrompt mid-turn) so a future reader doesn't "simplify" it into an assert.

**Output stream:** the advisory is **UI data** → `ui_state.add_system_message(...)` (the transcript, user-facing), matching the existing no-session branch. An optional `tracing::debug!` for the drop is a **diagnostic** → the tracing log (cyril.log), never stdout. No raw `println!`.

**Files:** `crates/cyril/src/app.rs` (only).

**Code (advisory — implementer may deviate if oracle passes + budget holds):**

```rust
// In dispatch_code_command, CodeCommandResponse::Prompt arm, immediately AFTER
// `let session_id = match session.id().cloned() { … None => return … };`
// and BEFORE any add_system_message / add_user_message / set_activity:

// cyril-8ej2: a /code prompt injected mid-turn would hit the bridge's one-turn
// guard (bridge.rs: rejects a 2nd SendPrompt while a turn is in flight), so the
// SendPrompt would be dropped AFTER we'd already committed a UserText + set
// Sending — a commit-without-send desync. Mirror the no-session branch: advise
// and drop, committing nothing. Runtime check (load-bearing): a debug_assert
// would compile out and re-open the desync in release. Scope = Busy only,
// matching classify_submit; other statuses carry no in-flight prompt_task.
if matches!(session.status(), SessionStatus::Busy) {
    tracing::debug!("/code prompt dropped: a turn is already in progress");
    ui_state.add_system_message(
        "/code: agent is busy — prompt not sent. Try again after the current turn.".into(),
    );
    return Vec::new();
}
```

**Verification (all 7 design fences):**

- [ ] **Unit tests pass.** New + extended tests in `crates/cyril/src/app.rs` `mod tests`:
  - extend `dispatch_code_prompt_returns_deferred_command` → also assert a `UserText("Analyze the code...")` was committed (claim #1).
  - `dispatch_code_prompt_busy_drops_no_send` → Busy + Prompt ⇒ `result.is_empty()` (claim #2).
  - `dispatch_code_prompt_busy_commits_no_user_message` → Busy + Prompt ⇒ no `UserText` in `messages()` (claim #3).
  - `dispatch_code_prompt_busy_leaves_activity` → set activity `Streaming`, Busy + Prompt ⇒ activity still `Streaming` (claim #4; strongest non-vacuous form — not just `!= Sending`).
  - `dispatch_code_prompt_busy_advises` → Busy + Prompt ⇒ a `System` message with the advisory text (claim #5).
  - `dispatch_code_prompt_no_session_shows_error` (existing) → unchanged (claim #6).
  - `dispatch_code_panel_opens_when_busy` → Busy + Panel ⇒ `has_code_panel()` (claim #7).
- [ ] **Stress fixtures produce expected outcome** (fixtures 1–3 above; they ARE tests above — #7 = fixture 1, #3 = fixture 2, #1 = fixture 3).
- [ ] **prove-it-prototype oracle still agrees with binary** — n/a (no external probe; the unit oracle is the function output, asserted directly). The design's premise probe (cheapest falsifier) is the historical record.
- [ ] **Loop and wall budgets hold** — O(1) guard, no loop; trivially holds.
- [ ] `cargo test` (workspace) green + `cargo clippy -- -D warnings` clean + `cargo fmt --check` (note: repo `main` has pre-existing fmt drift in bridge.rs/app.rs/main.rs per project memory — do not reformat unrelated lines; only the new guard + tests must be fmt-clean).

---

## Plan Self-Review

**1. Every loop — complexity + budget.**
- Production: none added. Guard is O(1), 0 syscalls. ✅ within budget.
- Test: `messages().iter()` over <5 messages per test = O(small), not production. ✅

**2. Every fixture — bug class it fails under.**
- Fixture 1 (Busy+Panel): guard-over-reach / placed before the variant match. ✅ adversarial, not happy-path.
- Fixture 2 (Busy+Prompt, no UserText): guard placed after the commit (desync persists). ✅
- Fixture 3 (Active+Prompt): guard checks the wrong condition (not-Active instead of ==Busy). ✅
All three are non-happy-path; each fails a distinct plausible implementation bug.

**3. Every doc-comment precondition — classified + enforced.**
- The busy guard: **load-bearing for correctness** (release-build violation ⇒ wrong output: commit-without-send) → **runtime check** (`if … return`), explicitly NOT `debug_assert!`. ✅ Doc comment states the rationale so it isn't downgraded later.
- No other "callers must X" preconditions introduced.

**4. Every write target — data or diagnostic.**
- `add_system_message` (advisory) → **data**, UI transcript (user-facing). ✅
- `tracing::debug!` (the drop) → **diagnostic**, tracing log. ✅
- No `println!`/stdout/stderr writes. ✅

**5. Every tracker reference — resolves to a covering issue.**
- cyril-8ej2 — this issue (verified `rivets show`, status in_progress; description covers exactly this path). ✅
- No "deferred"/"out of scope"/"follow-up" references in the plan. The design's out-of-scope statuses (Compacting/Initializing/Disconnected) are settled rationale (match the reviewed `classify_submit` model; no in-flight `prompt_task` to reject against), not deferred work — no tracker needed. ✅

No gaps in any of the five lists.
