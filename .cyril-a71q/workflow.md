# Gilfoyle workflow: cyril-a71q

- Target: Rivets issue `cyril-a71q` — per-turn terminal ownership so stale completion cannot clear a newer turn
- Repository: `C:\Users\dwall\repos\cyril-a71q`
- Status: active
- Current phase: design passed — awaiting requester direction for the budgeted build plan
- Active run: takeover session 2026-07-12 (main-repo Claude session; prior run `f43b50eb-f6b9-4575-b0a0-32c23b0d1330` superseded)
- Active work: none in flight; next is `.cyril-a71q/plan.md` (budgeted slices with stress fixtures + oracle per slice)

## Gates

| Phase | Artifact | Gate | Requester approval | Evidence |
| --- | --- | --- | --- | --- |
| Decisions | `.rivets/issues.jsonl` record `cyril-a71q`; `.cyril-3zy4/design.md`; requester choice | passed | recorded | Tracker pins the stale-completion and cross-session failures; cyril-3zy4's cheapest falsifier makes this prerequisite observable; requester chose to implement `cyril-a71q` separately. |
| Spec | `.cyril-a71q/spec.md` | **passed (re-anchored)** | signed 2026-07-12: "I confirm these consequences" | Timing audit (`timing-audit.md`) voided the choice-A sole-`turn_end` contract: its either-source/indefinite-absence premise contradicts the live 2.11.0 capture (turn_end then response, back-to-back, both present incl. cancel) and the response-only evidence was the pipeline's own mock. Re-anchored spec retains first-source-wins release (j16p), adds `TurnId(u64)` stamping + session-match with a one-entry expected-companion ledger (tracker note option (b)), records both source/reason observations for cyril-pnwb, and restores cyril-3zy4's busy-release requirement. Prior spec preserved as `spec-superseded-sole-turn-end.md`. |
| Prototype | `.cyril-a71q/prototype.md` | passed (evidence valid; contract layer voided) | n/a | Parent independently reran and verified the primary evidence after review: 2/2 pinned KAS fixtures with no native turn id; same/cross/response_only runtime scenarios exit 0; response-only raw trace contains 0 `turn_end` (NOTE: the response_only scenario was the probe's own Node mock branch, not live KAS — see timing-audit.md §4); oracle reports exact six-defect set at 3/9 desired dispositions. Pinned frames, no-native-turn-id finding, and runtime defect reproductions remain valid and reusable. |
| Design | `.cyril-a71q/design.md` | **passed (re-anchored)** | pending review | Cheapest falsifier `design_reanchored_falsifier.py`: correct policy 0/34 failures; 4/4 mutations (session-only, no-ledger, release-first, v2-session-match) fail with pairwise-distinct signatures; `REANCHORED-CHEAPEST-PASSED`. T4/T5 demonstrate the superseded impossibility dissolves: identical visible input, same absorb action, safe outcome in both histories; double-drift liveness residual is signed. Voided predecessor preserved as `design-superseded-sole-turn-end.md`. |
| Plan | `.cyril-a71q/plan.md` | pending | pending-if-required | No build plan was produced in this correction. |
| Build | `.cyril-a71q/build-evidence.md` | pending | per-halt | |
| Closure | `.cyril-a71q/build-evidence.md` | pending | n/a | |

## Build slices

| Slice | Status | Active run | Commit/code state | Gate evidence |
| --- | --- | --- | --- | --- |
| 1 | pending | none | To be defined only after design review. | No production slice or build plan was produced here. |

## Recorded decisions and approvals

- 2026-07-12: Rivets `cyril-a71q` requires globally trustworthy per-turn ownership rather than a bare session-id comparison; the synthesized global v2 completion must remain supported.
- 2026-07-12: The tracker note requires probing whether KAS `session_info_update` carries a native turn identifier and designing jointly with `cyril-pnwb` at the shared observer seam.
- 2026-07-12: cyril-3zy4's cheapest falsifier proved that session-only policies cannot distinguish late completion for released turn A from real completion for newer turn B.
- 2026-07-12: Requester chose to implement `cyril-a71q` separately before resuming `cyril-3zy4`. Verbatim reply: "A".
- 2026-07-12: `cyril-a71q` implements trustworthy ownership/dedup only and preserves both same-turn terminal sources/reasons for later precedence handling in `cyril-pnwb`. Verbatim reply: "A".
- 2026-07-12: After exhausting the full `u64` turn-identity space in one bridge lifetime, fail closed with a visible lifecycle error/disconnect and require a fresh bridge process; never wrap or reuse an ambiguous identity. Verbatim reply: "A".
- 2026-07-12: Drop a non-owning stale completion for the same session, but preserve a completion scoped to a different session for that session's routed consumer while leaving the active main turn and bridge busy guard unchanged. Verbatim reply: "A".
- 2026-07-12: Requester consequence sign-off for the pinned ownership spec. Verbatim reply: "I confirm these consequences".
- 2026-07-12: Parent and independent read-only reviewer accepted the prove-it prototype at 6/6 PASS after real public-bridge runtime probes reproduced the exact modeled same-session and cross-session defect set.
- 2026-07-12: After the design falsifier proved the either-source/indefinite-absence KAS contract impossible, requester selected scoped KAS `turn_end` as the sole KAS release source. Prompt responses remain secondary evidence for `cyril-pnwb` and never release a KAS turn; missing `turn_end` leaves the turn busy until failure/disconnect. Verbatim reply: "A".
- 2026-07-12: Requester consequence sign-off for the revised sole-`turn_end` KAS contract, including that a rate-limit notification cannot unlock a KAS turn by itself and cyril-3zy4's immediate busy-release requirement must be revised. Verbatim reply: "I confirm these revised consequences".
- 2026-07-12: Parent independently resolved the prototype review evidence blockers by rerunning primary fixture/runtime/oracle commands, confirming exact source line counts and zero response-only `turn_end` frames, and verifying no production or staged diff.
- 2026-07-12: A superseded revised design treated repeated scoped `turn_end` as normal input and retained prompt tasks after `turn_end`, producing two blockers.
- 2026-07-12: Requester chose **A**. Normal signed KAS input is `turn_end: Option<one scoped notification>` plus `prompt_response: Option<one RPC result>`; repeated scoped `turn_end` is unsupported live-wire drift. On authoritative `turn_end`, abort the turn's prompt RPC and discard any response that would arrive afterward. Verbatim reply: "A".
- 2026-07-12: Lifecycle reconciliation: KAS prompt error/death is fail-stop in `BridgeError` → owned `TurnCompleted` → `BridgeDisconnected` order before another prompt can be accepted, preventing its optional unobserved `turn_end` from entering B.
- 2026-07-12 (takeover): Timing audit (`timing-audit.md`) found the choice-A cascade rests on an input premise ("either source, either order, either absent indefinitely") the wire research contradicts. Requester directed the re-anchor. Verbatim reply: "Start the re-anchored spec".
- 2026-07-12 (takeover): Voided by the re-anchor pending re-sign: sole-`turn_end` release authority, abort-on-`turn_end`/post-`turn_end` response discard, Busy-until-disconnect on missing `turn_end`, and the demanded revision of cyril-3zy4's busy-release requirement (decision entries above dated 2026-07-12 covering requester options "A" on those points, plus both prior consequence sign-offs). Surviving premise-independent decisions: per-turn trustworthy ownership; joint design with cyril-pnwb preserving both source/reason inputs; `u64` fail-closed exhaustion; cross-session routed visibility; at-most-one scoped `turn_end` producer contract (duplicate = unsupported drift); cyril-l7tw ordering.
- 2026-07-12 (takeover): Bonus finding recorded on cyril-pnwb: the live 2.11.0 capture contains a KAS cancel with `turn_end.stopReason=cancelled` agreeing with the prompt response — pnwb's ACTION item is answered for the observed capture.
- 2026-07-12 (takeover): Requester consequence sign-off for the re-anchored spec. Verbatim reply: "I confirm these consequences".
- 2026-07-12 (takeover): Re-anchored design gate passed. Cheapest falsifier models first-source-wins + `TurnId` stamping + absorb-first one-entry companion ledger: correct policy 0/34 failures, 4/4 buggy-policy mutations caught with distinct signatures. Signed residual: double-drift (producer omits A's `turn_end` AND B's response) leaves B Busy until the fail-stop lifecycle — requires two simultaneous unsupported omissions. Absorb-first chosen over release-first: identical under supported input, safe (vs. wrong-clear) under single-drift.

## Halt

- Phase: none
- Reason: none
- Resume condition: n/a — design passed; next phase is the budgeted build plan (`.cyril-a71q/plan.md`), pending requester direction
