# cyril-vgcm — falsifiable design: /steer clear + steering-echo re-base

## Purpose

Give cyril a queue-clear UX for steering (`/steer clear` → `_session/steer/clear`),
and make it observable — which requires re-basing the steering-echo converters
on the wire as it actually is today, because a backend rollout (2026-06-17 →
2026-07-09 window) renamed every v2 steering echo and cyril currently drops
them all, on both engines (findings F2/F3/F8; filed as cyril-ppkx, fixed here).

Everything below extends the probe record in `findings.md`; no claim
contradicts a probe result.

## Wire contract being targeted (probed, verbatim captures in findings.md)

| event | v2 (current backend, all binaries ≥2.7.0) | v2 (pre-rollout, ≤2026-06-17) | KAS 0.8.0 |
|---|---|---|---|
| envelope | `_kiro.dev/session/update` | same | `session/update` → `session_info_update` `_meta.kiro.kind` |
| queued | `AgentExecutionUserMessageQueued` `{messageId, content}` | `steering_queued` `{message}` | `steering_queued` `{messageId, content}` |
| consumed | `AgentExecutionSteeringInjected` `{messageId, content}` | `steering_consumed` `{content}` | `steering_injected` `{messageId, content}` |
| cleared | `AgentExecutionUserMessageCleared` `{messageIds}` (explicit only) | `steering_cleared` (no ids) | `steering_cleared` `{messageIds}` (explicit AND post-injection) |
| clear resp | `{cleared:true}` | unknown (accepted today) | `{cleared:true, messageIds:[…]}` |
| clear on empty | `{cleared:true}`, no broadcast | — | `{cleared:true, messageIds:[]}`, no broadcast |

## Components and changes

1. **`Notification` (types/event.rs)** — `SteeringQueued` gains
   `message_id: Option<String>`; `SteeringConsumed` gains
   `message_id: Option<String>`; `SteeringCleared` becomes
   `SteeringCleared { message_ids: Vec<String> }` (empty = "everything",
   the old dialect's shape). New `SteeringClearUnsupported { message: String }`
   (bridge-synthesized, C12).
2. **`convert/kiro.rs`** — the `kiro.dev/session/update` arm gains the three
   new-family literals; the three old-family arms stay (backend rollback
   insurance). Fixtures = captured frames.
3. **`convert/kas.rs`** (behind `kas` feature) — `session_info_to_notification`
   maps kinds `steering_queued` / `steering_injected` / `steering_cleared`.
4. **`UiState` (cyril-ui)** — `SteerEcho` chat messages gain
   `message_id: Option<String>`; reconciliation becomes id-aware (C6–C9).
   `steering_queued` counter semantics preserved except id-scoped Cleared.
5. **`SessionController`** — DELETE `steering_depth` (write-only, no readers,
   would silently drift under id-scoped semantics); keep `steering_unsupported`.
6. **`SteerCommand`** — bare `clear` arg (trimmed, exact, lowercase) returns new
   `CommandResultKind::ClearSteer`; all other non-empty args unchanged.
7. **`App`** — `dispatch_clear_steer`: gate mirrors `steer_gate` (no session /
   steering-unsupported → advisory system message); otherwise send
   `BridgeCommand::ClearSteering`. No optimistic mutation (broadcast is truth).
8. **`bridge.rs`** — `ClearSteering`'s -32601 arm stops inserting into
   `steering_unsupported` and emits `SteeringClearUnsupported` instead of
   `SteeringUnsupported` (F5 fix). Pre-send `should_skip_steer` gate stays
   (steer-unsupported ⇒ nothing to clear).

## Input shapes

Converter (per dialect): each of queued/consumed/cleared × {all fields present,
id/text field absent, ids empty, ids multi}; unknown steering-prefixed variant
(v2 → Err+drop, unchanged); non-steering KAS kinds (`turn_end`, `context_usage`,
`user_message_id_assigned`, `steering_inclusion` → unchanged).
Command args: `""`, `"clear"`, `"clear "` (trim), `"Clear"` (case → steer text),
`"clear the tests"` (multi-word → steer text), other text, `--`-style text (no
special-casing).
Dispatch state: no session; session + steering-unsupported; session ok.
Clear outcome: Ok; -32601; other error (e.g. KAS -32603 unknown session).
UI chips at Cleared{ids}: ids match Queued chips; ids match Applied chips (KAS
post-injection); ids match nothing + id-less Queued chips exist (pre-rollout
dialect / deferred echo); ids match nothing + no chips; empty ids + any chips.

## Subtractive sweep (2b)

- **Removed: "SteeringCleared zeroes all steering state unconditionally."**
  Chip-drain analysis in cyril-nvmh listed Cleared as a full drain point; after
  this change a Cleared that names only some ids drains only those. Covered by
  C6/C7 claims + fences; nvmh gets a close-out note (its calculus changes since
  echoes are live again). Toolbar reads the counter only — no other reader
  assumes flip-all.
- **Removed: "any steering-family -32601 marks the session steering-unsupported."**
  Now steer-only. The only consumer of the set is `should_skip_steer` (pre-send
  gate for both ops) — safe: a clear--32601 session can still steer (probed:
  clear acceptance is universal on the current backend; the -32601 path is
  robustness). Covered by C12.
- **Removed: `SessionController.steering_depth`.** Write-only state, zero
  readers (grepped; only a stale comment in state.rs references it). Deleting
  removes a mirror that would drift. If session-side queue view is ever needed
  (e.g. cyril-28z2 subagent steering), it returns id-aware. Covered by C13.
- **Removed (input domain): bare `"clear"` as steer text** — `/steer clear`
  no longer steers the word "clear" (C10, decision D2).

## Claims and falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| C1 | Both engines accept `_session/steer/clear` and functionally drop un-injected steers; v2 acceptance covers archived 2.7.0–2.12.0 on today's backend | live steer+clear turn w/ marker instruction + no-clear control; cleared marker appearing anywhere = false | model output text (independent of cyril) + archived-binary probes | ran | **passed** (findings F1/F8; behavior probe both engines; suppressed=true, control landed=true) | fixtures freeze captured frames; live drift tracked by audit corpus (cyril-wmc3) |
| C2 | Cleared broadcasts are id-scoped; KAS also fires post-injection for consumed ids, v2 only explicit | capture frames across steer→inject→clear turns; a v2 post-injection Cleared or an id-less current-dialect Cleared = false | verbatim wire frames (probe round 2) | ran | **passed** (turn-2 captures both engines) | same as C1 |
| C3 | New-family v2 frames convert: Queued{messageId,content}→`SteeringQueued{message, message_id}`, Injected→`SteeringConsumed`, Cleared{messageIds}→`SteeringCleared{message_ids}` | feed captured frames to `to_ext_notification`; wrong variant/fields = false | captured-frame fixtures (live wire, not hand-written) | 10m | pending | `kiro::tests::steering_new_family_*` (3 tests, per-claim outputs) |
| C4 | Old-family v2 frames still convert (message field read; ids default None/empty) | feed K1b-era captured frames (in-repo wire logs); regression of existing arms = false | `.k1b-steering/*.log` captures (independent, pre-date this design) | 5m | pending (existing tests cover; extended for ids) | existing `steering_{queued,consumed,cleared}_converts` updated |
| C5 | KAS kinds steering_queued/injected/cleared convert identically; turn_end/context_usage/user_message_id_assigned/steering_inclusion behavior unchanged | feed captured KAS frames to `session_info_to_notification`; wrong result = false; run existing kas tests for the unchanged kinds | captured KAS fixtures + existing kas fixture corpus | 10m | pending | `kas::tests::steering_kind_*` (3) + existing kas tests |
| C6 | UI `SteeringCleared{ids}`: flips exactly matching Queued chips, unmatched ids fall back oldest-id-less (one each), Applied untouched, counter -= actual flips | state test: chips [A(id1,Applied), B(id2,Queued), C(no-id,Queued)]; apply Cleared{[id1,id3]}; expect B untouched, C flipped, counter -1; any other outcome = false | hand-computed expected state (fixture from C2's captured sequences) | 15m | pending | `state::tests::steering_cleared_id_scoped_*` (per-shape asserts) |
| C7 | UI `SteeringCleared{[]}`: flips ALL Queued, counter→0 (old-dialect semantics preserved) | existing flip-all test re-pointed at empty ids; partial flip = false | existing test corpus (pre-dates design) | 5m | pending | updated `steering_cleared_flips_all` |
| C8 | UI `SteeringQueued{Some(id)}` binds id to oldest id-less Queued chip; counter unchanged | state test: two id-less chips, apply Queued{id}; expect chip1 bound, counter same; double-count = false | cyril-7z7u optimistic-count contract (committed findings) | 10m | pending | `state::tests::steering_queued_binds_id_no_count` |
| C9 | UI `SteeringConsumed`: id-match preferred, FIFO fallback, counter saturating-dec | state test: chips [A(id1),B(id2)] apply Consumed{id2} → B flips not A; Consumed{None} → FIFO; wrong chip = false | expected-state fixtures derived from captured turn-2 sequences | 10m | pending | `state::tests::steering_consumed_id_match_then_fifo` |
| C10 | `/steer clear` (trimmed exact) → `ClearSteer`; `clear <more>`/`Clear`/other text → steer; empty → usage | parse+execute tests over the arg shapes; "clear now" becoming ClearSteer = false | shape list in this doc (input-space enumeration) | 5m | pending | `commands::tests::steer_clear_subcommand_parses` |
| C11 | App ClearSteer dispatch: no-session → advisory no-send; steering-unsupported → advisory no-send; else `BridgeCommand::ClearSteering`; zero optimistic chip/echo mutation | gate unit test (mirrors `steer_gate` matrix) + assert no `add_steer_echo`/counter change on dispatch | `steer_gate` truth table (existing, committed) | 10m | pending | `app` gate test (pure fn, CI-runnable) |
| C12 | Bridge ClearSteering -32601: emits `SteeringClearUnsupported`, does NOT mark session, does NOT emit `SteeringUnsupported`; subsequent SteerSession still sends; non--32601 → BridgeError unchanged | unit test on the clear error path; today's impl (inserts into the set) MUST fail it pre-fix | today's bridge behavior as the named buggy implementation (non-vacuity by construction) | 15m | pending | `bridge::tests::clear_32601_does_not_poison_steer` |
| C13 | `SessionController` drops `steering_depth`; `steering_unsupported` semantics unchanged; full suite passes | build + full test suite after removal; any legitimate reader breaking = false | compiler + existing test corpus | 5m | pending | compile + updated `steering_state_transitions_and_reset` |

**Cheapest falsifier run (step 6): PASSED** — scratch test fed all six captured
2.12.0 frames (3 v2-new, 3 KAS) through the CURRENT converters: v2-new →
`Err(unhandled variant)` ×3, KAS → `None` ×3 (run recorded in the session;
scratch reverted). The design premise — converters must be extended — survived
its kill attempt. C1/C2 falsifiers already ran live as the probe legs.

Non-vacuity spot-checks: C12's fence fails against today's code by
construction. C6's fence fails against a flip-all implementation AND against a
len-based (id-blind) implementation (the Applied-chip shape distinguishes
them). C8's fence fails against an implementation that re-counts wire echoes
(the double-count bug). C4's fence fails against "delete the old arms"
simplification. C10's fence fails against a `starts_with("clear")` parse.

## Negative space (deliberately not doing)

1. **No Esc-to-unqueue keybinding.** The AC offers `/steer clear` OR Esc;
   Esc is already overloaded (drill-in exit, cancel-busy) and an invisible
   queue-clear on Esc would surprise. Settled rationale, not deferred work.
2. **No per-steer selective clear.** The wire method clears the whole queue;
   no per-id clear exists on either engine.
3. **No multi-client/foreign-steer counting or echo display** — tracked at
   cyril-8lfs (verified open).
4. **No nvmh bounded safety net** for phantom chips — tracked at cyril-nvmh
   (verified open); this PR revives the echo pipeline, which changes that
   issue's calculus (close-out note planned, not a fix here).
5. **No subagent steering** (`/steer @name`) — tracked at cyril-28z2 (verified
   open).
6. **KAS `steering_inclusion` and `user_message_id_assigned` kinds stay
   ignored** — fileMatch steering catalog territory, tracked under the KAS-6
   reframe (reference memory + ROADMAP KAS track); not steering-queue state.
7. **No version/capability pre-gating of clear** — runtime -32601 fallback is
   the gate (F1 lesson: static/aprior gating of this surface is unreliable).

## Open decisions for the hard pause

- **D1 both echo families in kiro.rs** — recommend YES (backend rolled out the
  rename in a 3-week window; rollback/staging plausible; cost ≈ 3 match arms).
- **D2 `/steer clear` carves the bare word "clear"** — recommend accept
  (exact-match-only keeps `clear the tests please` steerable; steering the
  bare word "clear" at an agent is vanishingly rare).
- **D3 id-scoped Cleared semantics** (C6/C7) — recommend as specified; the
  KAS post-injection cleared makes id-blind flip-all actively wrong.
- **D4 silent-success dispatch** — recommend: no message on successful clear
  dispatch; chips flip when the broadcast lands; advisory messages only for
  no-session / unsupported / -32601. (Matches steer's echo-driven philosophy.)
- **D5 delete `steering_depth`** — recommend delete (write-only, no readers,
  drift-prone under id-scoping).
