# cyril-8ej2 — Busy-guard the `/code` prompt-injection path (falsifiable design)

**Issue:** cyril-8ej2 (P2 bug). Second instance of the cyril-2vcc bug class, outside the K1b diff.
**Skill:** gilfoyle:falsifiable-design. Cheapest falsifier run + passed (see Falsification table).

## Probe (what the real code does — confirmed by reading, not assumed)

- `dispatch_code_command`'s `Prompt` arm (`crates/cyril/src/app.rs:1269-1289`) does `add_system_message` + `add_user_message(&text)` + `set_activity(Sending)` + returns `vec![SendPrompt{..}]` — **with no busy check.**
- The bridge's one-turn guard (`crates/cyril-core/src/protocol/bridge.rs:386-400`) rejects a `SendPrompt` while `prompt_task` is unfinished, replying `BridgeError "a turn is already in progress"` instead of starting a turn.
- Therefore: a `Prompt` response arriving while the session is `Busy` commits a `UserText` to the transcript that is **never sent**, and strands `activity` at `Sending`. This is the desync.
- Call site (`app.rs:394`) is **synchronous** and already passes `&self.session`, so the fix reads `session.status()` in place — no async needed.

Independent oracle for the premise: the bridge rejection is separate code (bridge.rs), and the cheapest falsifier (below) demonstrated the unguarded emission against the real function.

## Decision (settled rationale — not deferred)

Busy-case behavior = **advisory + drop** (tell the user, emit zero bridge commands, commit nothing), **not steer**. Why advisory over steer:
1. `dispatch_code_command` is sync and returns `Vec<BridgeCommand>`; the steer path (`dispatch_steer`) is `async` (awaits `bridge.send`). Steering would require threading async through the sync `dispatch_command_executed` caller — a larger change than the bug warrants.
2. The issue specifies "advisory or defer."
3. The Prompt arm already has the exact "can't proceed → advisory + return empty Vec" shape (the no-session branch); the busy guard mirrors it.
4. A `/code`-generated prompt is a discrete agent-command result, not a user's typed mid-turn redirect; silently converting it to a steer is more surprising than asking the user to retry.

Scope = **`Busy` only**, matching the established `classify_submit` model (`app.rs:907`, which routes only `Busy`→Steer). `Busy` is the status that carries an in-flight `prompt_task` the bridge rejects against; other statuses keep the existing prompt behavior.

## Input shapes (step 2)

`dispatch_code_command(response, session, ui)`:
- `response`: success=false → command-output (untouched); success=true → match variant.
- `CodeCommandResponse`: `Panel` / `Prompt` / `Unknown`. **Only `Prompt` is touched.**
- `Prompt` × `session.id()`: `Some` / `None` (None already → advisory + empty; busy is moot without an id).
- `Prompt` × `session.id()=Some` × `session.status()`: **`Busy`** (new guarded path) vs **not-`Busy`** (`Active`, unchanged).
- `Prompt.label`: `Some` / `None` — already handled (`unwrap_or` default), unchanged.

Out-of-scope shapes (one-line justification each):
- `status ∈ {Compacting, Initializing, Disconnected}` + Prompt: keep existing prompt path — they don't carry an in-flight `prompt_task` the bridge rejects against, and they follow the reviewed `classify_submit` model. Not part of this bug's reproduction.
- `label = None` vs `Some`: orthogonal to the guard; existing `unwrap_or("Code Intelligence")` is unchanged.

## Subtractive sweep (step 2b)

**Purely additive.** The change *adds* a busy guard to a path that had none; it removes no serialization point, lock, ordering guarantee, or uniqueness property. No invariant sweep required.

## Claims & Falsification

| # | Claim | Falsifier (input → falsifying result) | Oracle | Cost | Status | Regression fence |
|---|-------|----------------------------------------|--------|------|--------|------------------|
| 0 | (premise) The current Prompt path is **not** busy-guarded. | Busy session + Prompt → if it returns empty/no SendPrompt, premise false. | `dispatch_code_command` return Vec | 2m | **passed** (returned 1 SendPrompt + UserText + Sending) | n/a (premise; superseded by #2–4) |
| 1 | Prompt + **Active** session: emits exactly one `SendPrompt`, commits a `UserText`, sets activity `Sending`. | Active + Prompt → if result≠`[SendPrompt]` OR no `UserText` OR activity≠`Sending`, false. | return Vec + `ui.messages()` + `ui.activity()` | 2m | pending | `dispatch_code_prompt_returns_deferred_command` (extend to assert `UserText`) |
| 2 | Prompt + **Busy** session: emits **zero** bridge commands. | Busy + Prompt → if result non-empty, false. | return Vec | 2m | pending | `dispatch_code_prompt_busy_drops_no_send` |
| 3 | Prompt + Busy: commits **no** `UserText` (no commit-without-send). | Busy + Prompt → if any `UserText` in `ui.messages()`, false. | `ui.messages()` | 2m | pending | `dispatch_code_prompt_busy_commits_no_user_message` |
| 4 | Prompt + Busy: does **not** change activity to `Sending`. | set activity `Streaming`, Busy + Prompt → if activity becomes `Sending` (≠`Streaming`), false. | `ui.activity()` | 2m | pending | `dispatch_code_prompt_busy_leaves_activity` |
| 5 | Prompt + Busy: adds a `System` advisory so the drop is visible. | Busy + Prompt → if no `System` message present, false. | `ui.messages()` | 2m | pending | `dispatch_code_prompt_busy_advises` |
| 6 | Prompt + **no session**: unchanged (advisory + zero commands), independent of the new guard. | no-session + Prompt → if non-empty result OR no advisory, false. | return Vec + `ui.messages()` | 2m | pending | `dispatch_code_prompt_no_session_shows_error` (existing) |
| 7 | The guard is scoped to the Prompt arm: `Panel` still opens even when **Busy**. | Busy + Panel JSON → if no code panel opens, false. | `ui.has_code_panel()` | 2m | pending | `dispatch_code_panel_opens_when_busy` |

### Non-vacuity (named buggy impl per claim)
- **#1**: an over-eager guard that gates on *not-Active* (or any non-idle) would suppress the Active send → fails #1.
- **#2 / #3 / #4**: the **current** code (emits SendPrompt, commits UserText, sets Sending) fails all three — demonstrated by claim 0.
- **#5**: a guard that drops silently (returns empty, no `add_system_message`) → fails #5.
- **#6**: a regression that emits `SendPrompt` with no session id → fails #6.
- **#7**: a guard placed too early (returns empty for any Busy dispatch, before the variant match) → swallows the Panel → fails #7.

Each claim has a distinct test → distinct failure output (per-claim localization). Oracles are the function's own return value + `UiState` reads — independent of any other feature.

## Negative space (what this deliberately does NOT do)

1. Does **not** steer the dropped prompt (advisory only) — unlike Enter-while-busy (cyril-2vcc); a `/code` prompt isn't a user steer.
2. Does **not** guard `Compacting` / `Initializing` / `Disconnected` — only `Busy`, matching `classify_submit`.
3. Does **not** queue/auto-retry the prompt after turn-end — the user re-runs `/code`.
4. Does **not** touch the `Panel` / `Unknown` / no-session / success=false paths.
5. Does **not** make `dispatch_code_command` async (the whole reason advisory beats steer here).

## Tracker references

- cyril-8ej2 — this issue (verified: `rivets show cyril-8ej2`, status in_progress). No new deferrals introduced.
