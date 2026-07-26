# cyril-a71q — budgeted plan

Design: `design.md` (DESIGN GATE PASSED; requester approval 2026-07-26).
Contract: `spec.md` (sign-off "I confirm these consequences", 2026-07-12).
Ordering premise: source-corroborated in `corroboration-2026-07-26.md`.

11 slices. Claim coverage matches the design's C1–C10 exactly (C1 and C6 land across
two slices each; every other claim maps to one).

## Scale constants (used by every budget below)

| quantity | production value | source |
|---|---|---|
| live bridge-owned prompt futures | ≤ 2 | design "Architecture"; C4 |
| companion-ledger entries | ≤ 1 | design "Mediation policy"; C6 |
| active turn records | ≤ 1 | `turn_in_flight` is `Option`, bridge.rs:760 |
| notification channel capacity | 256 | `NOTIFICATION_CAPACITY`, bridge.rs:18 |
| terminal signals per accepted turn | ≤ 2 (KAS), 1 (v2) | timing-audit §1; covenant §4 |
| turns per session, upper realistic | ~10^3 | interactive TUI session |

**Every slice below introduces O(1) work per notification.** There is no new loop over a
collection anywhere in this plan — the mediation state is one `Option` record plus one
`Option` ledger entry. Per-slice loop budgets restate this explicitly rather than omitting
it, and any slice that *did* introduce iteration would be a budget violation to be split.

---

## FLAGGED DEVIATION FROM THE DESIGN (placement only — requester decision)

`design.md` §Architecture and `next-steps.md` both say *"`TurnCompleted` gains
`Option<TurnId>`"*. Measured blast radius of that literal placement:

```
Notification::TurnCompleted { … } construction sites: 80, across 9 files
  cyril-ui/src/state.rs 28 · cyril-core/src/session.rs 10 · bridge.rs 9
  types/event.rs 4 · subagent_ui.rs 3 · convert/kas.rs 2 · +3 more
RoutedNotification construction sites:                   6, across 3 files
```

Adding a required field to the variant forces `turn: None` into ~70 unrelated test
constructions and violates this skill's ≤2-files-per-slice rule outright.

**Proposed placement instead: the envelope.** `RoutedNotification { session_id,
notification }` gains `turn: Option<TurnId>`. Rationale:

- Ownership is *routing* metadata, and the envelope is already exactly that — it carries
  `session_id` for the same reason. `Notification` stays pure domain content, matching
  the crate boundary CLAUDE.md states.
- The synthesized completion already becomes a `RoutedNotification` at the send site
  (`turn_tx.send(note.into())`, bridge.rs ~:901) — the natural stamping point.
- The wire `turn_end` arm in `convert/kas.rs` produces a `Notification` that is wrapped
  with session scope and **no** turn id — which is precisely the design's "unstampable
  wire arm", now expressed by the envelope rather than by a `None` inside the payload.

**This changes placement, not policy.** The mediation rules (id-match → absorb/release/
stale; session-match absorb-first for the identity-free arm) are unchanged, so
`design_reanchored_falsifier.py` — which models policy over abstract owners, not Rust
types — remains valid and passing without edit. If the requester prefers the literal
design wording, S2 becomes an 80-site mechanical slice and must be split by file; say so
and I will re-cut it.

---

## Slice 1: `TurnId` newtype with a fail-closed allocator

**Claim:** C8 — identity uniqueness; a counter wrap must never recreate a live owner.
**Oracle:** `design_reanchored_falsifier.py`'s abstract owner allocator (monotonic, never
reissues) — compare the Rust allocator's emitted sequence against it for the boundary
window.
**Stress fixture:** allocator primed to `u64::MAX - 1`. Expected, written before
implementation: allocation N succeeds → `TurnId(u64::MAX)`; allocation N+1 returns
`None`/`Err` (fail closed); the allocator does **not** wrap to 0 and does **not** reissue
`u64::MAX`. Counter-fixture: a fresh allocator issues `0, 1, 2` with no gaps.
Bug class targeted: `wrapping_add`/`saturating_add` — saturating is the subtle one, it
silently reissues `u64::MAX` forever rather than wrapping visibly.
**Loop budget:** no loop. Allocation is O(1) (one checked add). Production scale ~10^3
allocations per process; exhaustion is unreachable in practice and the fence exists to
make the unreachable path *fail closed* rather than *fail silently*.
**Wall budget:** n/a (not an always-on phase).
**Files:** `crates/cyril-core/src/types/turn.rs` (new), `crates/cyril-core/src/types/mod.rs`.

**Doc-comment contract:** "the allocator must never reissue a live owner" is
**load-bearing for correctness** — a reissued id makes a stale completion match a live
turn, which is the exact bug this issue exists to fix. Enforcement is therefore a
**runtime** `checked_add` returning `None`, not `debug_assert!`. The design says
"**checked** — not saturating" for this reason.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture produces expected outcome (N ok, N+1 fails closed, no wrap, no reuse)
- [ ] Oracle agrees on the boundary sequence
- [ ] O(1) allocation confirmed; no loop introduced

---

## Slice 2: envelope carries `Option<TurnId>`

**Claim:** substrate for C1/C2 — completions become distinguishable by owner.
**Oracle:** type-level; the falsifier's owner-stamped vs identity-free distinction.
**Stress fixture:** construct one envelope per arm — synthesized (`Some(owner)`), wire
`turn_end` (`None`), v2 response (`Some(owner)`) — and assert the three are
distinguishable. Bug class: collapsing "absent id" into a sentinel such as `TurnId(0)`,
which would make the wire arm collide with the first-ever allocated owner. CLAUDE.md's
"`Option` for absent, never a sentinel" rule is the thing under test.
**Loop budget:** no loop; one `Option<TurnId>` field per envelope, O(1).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/types/event.rs`, `crates/cyril-core/src/protocol/client.rs`.

**Verification:**
- [ ] Unit tests pass; workspace compiles (6 construction sites updated)
- [ ] Stress fixture: `TurnId(0)` and `None` are distinct, non-equal, non-convertible
- [ ] Oracle unaffected (placement-only change)
- [ ] No loop introduced

---

## Slice 3: allocate the owner at dispatch; hold an active record

**Claim:** C1 (part 1) — each accepted turn has exactly one immutable owner + session.
**Oracle:** falsifier's active-record model.
**Stress fixture:** accept turn A; mid-turn `NewSession` retargets `active_session_id`;
assert A's record still names A's **original** session. Bug class: reading the mutable
`active_session_id` at completion time instead of the captured owner session — the
existing code does exactly this for cancel (bridge.rs:913), so the fixture is a
regression fence against re-introducing it elsewhere.
**Loop budget:** no loop; `Option<ActiveTurn>` replaces `Option<SessionId>`, O(1)
per SendPrompt.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture: mid-turn retarget does not mutate the active record
- [ ] Oracle agrees
- [ ] O(1) per dispatch

---

## Slice 4: mediation — id-stamped completions

**Claim:** C1 (part 2) — absorb / release / stale, by owner match.
**Oracle:** `design_reanchored_falsifier.py` T3/T4 arms.
**Stress fixture:** turn A releases; turn B accepted; **A's late response arrives stamped
with A's owner**. Expected, written first: 0 completions forwarded, B still busy. This is
the literal bug in the issue title — under the old session-only guard it clears B.
Counter-fixture: B's own completion **does** release B (the fence must not freeze).
**Loop budget:** no loop; one owner comparison per completion, O(1).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture: stale-stamped forwards 0, B unaffected; counter-fixture releases B
- [ ] Oracle agrees (T3/T4)
- [ ] O(1) per completion

---

## Slice 5: mediation — identity-free `turn_end`, absorb-first + companion ledger

**Claim:** C1 (part 3) + C6 — the wire arm resolves by session with a one-entry ledger,
and both `{source, reason}` observations are recorded.
**Oracle:** falsifier T1/T2 (`both_evidence`, `companion_absorbed`) — evidence sets are
encoded independently of the policy there, so the oracle can disagree with the policy.
**Stress fixture:** **both receipt orders** for one KAS turn —
(a) `turn_end` → response, and (b) response → `turn_end` — must produce an *identical*
observable outcome: exactly 1 forwarded completion, 2 recorded observations, ledger empty
at rest. Bug classes targeted: (i) first-wins that **drops** the companion instead of
absorbing it, losing the second `{source, reason}` (kills cyril-pnwb's evidence — this is
falsifier mutation M2); (ii) release-first ordering, which under single drift clears the
newer turn (mutation M3); (iii) a ledger that grows past one entry.
Order (b) is the defensive case per timing-audit §2 — unobserved live, so the fixture is
the only thing exercising it.
**Loop budget:** no loop; ledger is `Option<Expectation>`, ≤1 entry by construction,
O(1) insert/match/clear. Registering a new expectation replaces a dangling one — bounded,
not accumulating.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture: both orders identical; 1 forwarded, 2 recorded, ledger empty
- [ ] Oracle agrees (T1/T2 evidence assertions)
- [ ] Ledger provably ≤1 entry at every step

---

## Slice 6: cancel and shutdown target the immutable owner

**Claim:** C4 — ≤2 bridge-owned futures; cancel targets the owner session; shutdown
aborts all with 0 required completions.
**Oracle:** falsifier T6 scope arms + process-liveness assertion.
**Stress fixture:** turn A in flight; mid-turn session retarget; `CancelRequest` → assert
the cancel names **A's** session, not the retargeted one. Shutdown fixture: 2 live futures
→ abort → assert 0 completions required and no future outlives `run_loop`
(`ps -o stat=` per `feedback_process_liveness_tests_portable`, not `/proc`).
Bug class: the existing `turn_in_flight.as_ref().or(active_session_id)` fallback silently
becomes wrong once the record is owner-keyed.
**Loop budget:** shutdown aborts ≤2 futures — O(1) at a fixed bound, not O(n).
**Wall budget:** shutdown abort must complete within the existing graceful-shutdown
window; assert bounded, no unbounded await.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture: cancel hits owner session; shutdown leaves 0 live futures
- [ ] Oracle agrees
- [ ] ≤2 futures bound asserted, not assumed

---

## Slice 7: scope isolation — foreign completions route without touching main

**Claim:** C3 — foreign scoped completions forward once to their routed consumer with
zero main mutation; unowned same-session/no-active signals drop.
**Oracle:** falsifier T6; plus a public-state delta snapshot of `SessionController`.
**Stress fixture:** `terminal_scope_owner_matrix` — the cross product of
{global, main-session, foreign-session} × {owned, stale, no-active}. Expected outcomes
written before implementation. Bug class: the cross-session split-brain named in the
issue — a foreign session's completion clearing the main guard.
**Loop budget:** no loop; one scope comparison per completion, O(1). The matrix is a
**test** fixture: 9 cases, O(1) at test scale.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`, `crates/cyril/src/app.rs`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture: all 9 matrix cells match pre-written expectations
- [ ] Zero main-state delta on every non-owned cell
- [ ] O(1) per completion

---

## Slice 8: lifecycle fail-stop preserved under ownership

**Claim:** C5 — `BridgeError` → owner-keyed `TurnCompleted` → `BridgeDisconnected`;
no turn B accepted in the failed lifetime; only the dying owner's marker satisfies its
deferred disconnect.
**Oracle:** falsifier T5 `lifecycle_order`.
**Stress fixture:** kill the connection mid-turn A, then deliver a **stale** completion
(not A's owner) — assert it does **not** satisfy A's deferred disconnect, and that the
ordering `BridgeError → TurnCompleted → BridgeDisconnected` still holds. Bug class: the
deferred-disconnect gate at bridge.rs:1752 currently fires on *any* observed completion;
owner-keying it is the fix, and a stale marker satisfying it is the regression.
**Loop budget:** no loop; one owner comparison in the deferred-disconnect arm, O(1).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture: stale marker does not satisfy deferred disconnect; order preserved
- [ ] Oracle agrees (T5)
- [ ] Existing `failstop_disconnect_survives_full_channel` (bridge.rs:1929) still passes

---

## Slice 9: consumer effects — only the owned release mutates main

**Claim:** C7 — absorbed/stale/foreign/rate-limit cause zero main completion transitions.
**Oracle:** public-state delta table computed independently of the bridge (snapshot
`SessionController` + `UiState` before/after), per design C7.
**Stress fixture:** feed one of each event class and diff the public state. Expected: a
non-zero delta for the owned release **only**; byte-identical state for the other four.
Bug class: forwarding the absorbed companion and relying on downstream dedupe — the
design explicitly rejects that; the fixture fails if the companion reaches consumers.
**Loop budget:** no loop; the delta comparison is a test-only snapshot diff over a fixed
small struct, O(1) at test scale.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`, `crates/cyril/tests/event_routing.rs`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture: exactly one class produces a main delta
- [ ] Oracle (independent snapshot) agrees
- [ ] No loop introduced

---

## Slice 10: backpressure — owned terminal survives a full channel

**Claim:** C9 — classification is not lost at capacity; semantics hold in receipt order.
**Oracle:** independently generated id range + receiver-order ledger (design C9).
**Stress fixture:** pause the receiver, fill the channel to **256**
(`NOTIFICATION_CAPACITY`, bridge.rs:18), block the terminal **257th**, resume, then
reconcile order and ownership. Expected: the owned terminal is delivered, not dropped, and
receipt order is preserved. Bug class: `try_send` dropping at capacity, or clearing the
active record *before* delivery is confirmed — either loses the release.
**Loop budget:** the fixture enqueues 257 items — O(capacity), 257 operations, far under
the 10^6 ceiling. Production behavior is unchanged: still one awaited send per
notification, O(1).
**Wall budget:** the fill+drain must complete well inside the test timeout; assert
bounded, and treat a hang as failure rather than slowness.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture: 257th terminal delivered, order preserved, ownership intact
- [ ] Oracle agrees
- [ ] 257 ops ≪ 10^6 budget; production path still O(1) per notification

---

## Slice 11: rate-limited turns release via the response

**Claim:** C10 — a rate-limited turn releases the busy guard (restores cyril-3zy4); no
observation-alone release.
**Oracle:** App completion count + fake-server prompt transcript (design C10).
**Stress fixture:** inject a rate limit around **both** receipt orders. Expected: the turn
releases via its response and is **not** left Busy. Bug classes: (i) mapping
`RateLimited` to a release on mere observation, which would release turns that never ran;
(ii) the voided choice-A behavior where the response never releases — the regression this
slice exists to prevent, and the direct conflict the timing audit found with cyril-3zy4.
**Loop budget:** no loop; O(1) per completion.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture: rate-limited turn releases in both orders; no Busy residue
- [ ] Oracle agrees (completion counts)
- [ ] O(1) per completion

---

## Plan Self-Review

**1. Every loop — complexity stated, within budget?**
No slice introduces a loop over a collection in production code. Mediation state is
`Option<ActiveTurn>` + `Option<Expectation>`, both O(1). Two *test* fixtures iterate:
S7's 9-cell matrix and S10's 257-item channel fill — 266 operations total, ≪ 10^6.
Shutdown (S6) aborts ≤2 futures, a fixed bound. **No gaps.**

**2. Every fixture — which bug class, adversarial not happy-path?**
S1 saturating-reuse · S2 sentinel-collapse (`TurnId(0)` vs `None`) · S3 mutable-session
read · S4 the issue's literal stale-clears-B bug, with a freeze counter-fixture ·
S5 companion-dropped / release-first / unbounded ledger, across both receipt orders ·
S6 cancel-follows-retarget · S7 cross-session split-brain (full matrix) · S8 stale marker
satisfying deferred disconnect · S9 companion leaking to consumers · S10 `try_send` drop
at capacity · S11 observation-alone release and the voided never-release.
Every slice carries a counter-fixture or a matrix, not a single happy path. **No gaps.**

**3. Every doc-comment precondition — classified, enforced?**
One precondition in this plan: S1's "the allocator must never reissue a live owner."
Classified **load-bearing for correctness** (a reissued id makes a stale completion match
a live turn — the bug this issue exists to fix), so enforced by a **runtime** `checked_add`
returning `None`, surviving release builds. No `debug_assert!`-only contracts. **No gaps.**

**4. Every write target — data or diagnostic?**
This plan writes to no stdout/stderr stream. Its only outputs are (a) channel sends —
already-classified protocol data on the existing notification channel, and (b) `tracing`
calls for drops/absorptions, which are **diagnostics** and go to the existing tracing
subscriber (cyril logs to `cyril.log`, never stdout — a TUI owns stdout, so a stray
`println!` would corrupt the display). No new `println!` anywhere. **No gaps.**

**5. Every tracker reference — resolves to a real issue covering the work?**
| reference | issue | status | covers it? |
|---|---|---|---|
| target issue | `cyril-a71q` | open, in_progress | yes — turn-seq dedup + cross-session |
| busy-guard release for rate limits | `cyril-3zy4` | open | yes — "a rate-limited turn must release the busy guard"; S11 restores it |
| stop-reason authority deferred, not decided | `cyril-pnwb` | open | yes — owns precedence; S5 records tuples only, selects none |
| stream-ordering (agent text after TurnCompleted) | `cyril-9akh` | open | yes — explicitly out of scope per `related-issues.md` |
| reconnect/respawn after disconnect | `cyril-gua0` | open | yes — out of scope; S8 preserves fail-stop only |
| origin of the dedup seam | `cyril-j16p` | closed | yes — first-source-wins retained, not revised |
No deferral in this plan lacks a citation; no new issue needed at plan time. **No gaps.**

All five lists are empty of gaps.
