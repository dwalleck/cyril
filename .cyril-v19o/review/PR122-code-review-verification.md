# PR122 review verification — `/powers` panel

- **Judged state:** `dwalleck/cyril#122` head `b82576ce` (== `origin/feat/cyril-v19o`), base `main` `7bbc2632`. Head confirmed still current 2026-09-10.
- **Read-only worktree:** `/home/dwalleck/repos/cyril-wt-pr122-verify` (detached at the head).
- **Review under test:** `PR122-code-review.md` (469 lines).
- **Method:** eight read-only scouts (one per file cluster), each given only its findings and told the review's `CONFIRMED` stamp is a hypothesis; then parent-side re-proofs of every claim that decides an Accept/Modify/Reject, including one executed probe. **No code changed. Verification is not authorization to fix.**
- **Evidence states:** `Verified` / `Refuted` / `Unverified` / `Not-applicable`. **Decisions:** `Accept` / `Modify` / `Reject`.

---

## Decision log

| # | Finding | Reviewer | Evidence | Decision | Fix verdict | Note |
|---|---|---|---|---|---|---|
| 1 | Fence forbids `_kiro/powers/list`, which works | CONFIRMED | **Verified** | **Modify** | incorrect | The message is false for `list`; the prohibition is a ratified spec decision. See below. |
| 2 | `/help` never lists `/powers` | CONFIRMED | **Verified** | Accept | correct (incomplete fence) | Dead store at `mod.rs:396`; move above `HelpCommand::new` at `:382`. |
| 3 | Key priority inverted vs render z-order | CONFIRMED | **Verified** | Accept | incomplete | 15 inverted pairs, 5 new (exact); worst case is approval, not picker. Needs a direction-aware policy. |
| 4 | Non-vacuity guard satisfied by a comment | CONFIRMED | **Verified** | Accept | incomplete | Real and understated: test-code hits also satisfy it. Fix must anchor to the converter module. |
| 5 | `CHROME_ROWS = 4` drops a power | CONFIRMED | **Verified** | Accept | incomplete | Numbers exact; fix both uses of the constant; add the clamp-reachable fence. |
| 6 | Overflow silently truncated | CONFIRMED | **Verified** | Accept (reword) | no-fix-proposed | "Unlike every sibling" is wrong — hooks has no affordance either. Cheapest fix is title text. |
| 7 | `· steering` truncated away | CONFIRMED | **Verified** | Accept | incomplete | 101 cells, not ~103; reorder breaks spec shape + a named-mutation anchor. Budget the width instead. |
| 8 | Scroll clamps to last index | CONFIRMED | **Verified** | Accept | incomplete | Derivation exact; the state has no window. Same clamp in `refresh`; PageDown 5 overshoots a squeezed popup. |
| 9 | Explicit JSON `null` discards the catalog | PLAUSIBLE | **Verified** (executed) | Accept | correct | Reproduced with a verbatim struct mirror. Fix both defaulted fields. PLAUSIBLE is the right label. |
| 10 | `powers` never cleared | PLAUSIBLE | **Refuted as stated** | **Reject** | incorrect | Process-global is pinned (spec.md:118). Clear-on-`SessionCreated` is the one unsafe variant. |
| 11 | Re-spelled push vanishes silently | CONFIRMED | **Verified** | Accept | correct | Mirror the workflow sibling's `starts_with` warn. |
| 12 | Blank name → blank row, sorted first | CONFIRMED | **Verified** | Accept | correct | Reuse `discovery.rs`'s `nonempty`; apply to mcp entries and the false `title()` doc. |
| 13 | Paste guard omits the powers panel | CONFIRMED | **Verified** | Accept (reword) | no-fix-proposed | Pre-existing and wider: every overlay except usage is omitted. One predicate, three guards. |
| 14 | Two new fences cannot fail | CONFIRMED | **Verified** (14a/b/c) | Accept | no-fix-proposed | 14b's "delete the arm" is E0004, not a green test; the neutered-arm case is caught elsewhere. |
| 15 | CLAUDE.md overlay chain stale | CONFIRMED | **Verified** | Accept (extend) | incomplete | Also stale for `usage` since `main`, and duplicated verbatim in AGENTS.md. |
| 16 | Sort-key docs overstate "case-insensitive" | CONFIRMED | **Verified** | Accept | correct | Prose only — the code matches spec.md:34. Two test comments repeat the claim. |
| 17 | `/powers` inert in a default build | CONFIRMED | **Verified** | **Modify** | incomplete | The message fix is the necessary half; gating reverses spec.md:110 and leaves the false advice standing. |
| 18 | Transport fence order-coupled | CONFIRMED | **Verified** | Accept | no-fix-proposed | Deterministic fix exists; the fence's own comment concedes nothing pins the order. |
| 19 | `refresh_powers_panel` returns `true` for a no-op | CONFIRMED | **Verified** | Accept (nit) | no-fix-proposed | Real doc/code gap; same defect in `refresh_hooks_panel`; dead store at `app.rs:2041`. |
| 20 | Fifth hand-copied overlay | — | **Verified root cause**, magnitude imprecise | Accept (direction) | incomplete | ~17 edit sites is an *under*count (~20); "compiler never checks" is too strong. |
| R1 | Dropped `sessionId` is harmless | retracted | **Retraction confirmed**, sharper reason | Accept | incomplete | The comment is wrong for a reason the review missed: extension frames route globally. |
| R2 | Turn-liveness stamp costs the cancel affordance | retracted | **Retraction confirmed** | Accept | no-fix-proposed | ≤30 s chip delay survives; a stale field doc (`state.rs:95-96`) will make reviewers re-derive it. |

---

## The findings that change the plan

### Finding 1 — the message is wrong; the prohibition is ratified (Modify, not delete)

The fence's assertion text at `crates/cyril/tests/powers_source_fence.rs:179-186` and header `:10-16` claim both pull methods are "a control that cannot work". That is false for `list`:

- `experiments/conductor-spike/kas-powers-2.21.2.jsonl:17` → request `_kiro/powers/list` at ts 1789045487.9596608, after `session/new` (`:5`, reply `:15`); `:20` → `{"errors": [], "powers": […3 items…]}`.
- Same on 2.20.1: `experiments/conductor-spike/kas-powers-2.20.1-verdict.json` `list_empty` and `list_after` both return 3 powers with `errors: []`.
- `docs/kiro-2.20.1-wire-audit.md:138-140`: "not advertised … yet dispatches fine". Unadvertised ≠ non-functional. (`refresh` → `-32603` is the one measured-broken method.)

But "delete or invert the fence" (`Suggested fix order` #1) is not a test edit. The prohibition implements ratified behavior: `spec.md:64` (B8), `:77` (SC5 mandates a source fence over `refresh` **and** `list`), `:86` ("even though it works and is unadvertised: the requester re-confirmed push-only after this row was challenged"), `:111` ("Push only (as specced)" over a pull or an `r` key). Inverting it is a spec change wearing a test-edit costume, and it is not a prerequisite for anything: finding 9 has a push-side fix, and finding 10 is refuted.

Also: the review's anchor (`:146`) is misdescribed — line 146 is `sources.len() >= 100`; the prohibition lives at `:116-135` (scanner), `:154-171` (census loop), `:179-186` (message), `:202-207` (the `list` unit case).

**Minimal correct change:** keep the prohibition, correct the rationale in three strings (`:14-16`, `:181-184`, `:206`) to "`refresh` is declared-but-unimplemented; `list` is unadvertised, so it cannot be feature-detected and its contract is unverifiable". Recovery via `list` is a *product decision* (below) that would additionally require the transport fence's offside filter (`current_runtime_contract/powers.rs:105-118`), `builtin.rs:457-478`, `convert/kas/powers.rs:28-33`, and spec re-approval.

### Finding 3 — verified, and worse than the review's example

The key chain (`app.rs:1648-1677`) and the paint chain (`render.rs:139-164`) are the same order, so **every** pair is inverted — the topmost overlay is the last to get keys. With six overlays that is all 15 pairs; the PR adds exactly 5 (`(approval,powers)`, `(picker,powers)`, `(hooks,powers)`, `(powers,code)`, `(powers,usage)`). No open path guards (`show_picker` at `state.rs:2148-2180` assigns unconditionally; `CommandOptionsReceived` → `app.rs:1425-1438`).

The review's `/model` example is right; the worst case it missed is **approval**: an unprompted permission request is painted first (`render.rs:139`) and then erased by the powers panel's `Clear`, while `handle_key:1648` hands it every key — Enter answers an unseen permission prompt.

A one-line guard is wrong in the obvious places: refusing a user-initiated open silently drops the requested picker; reordering `handle_key` alone leaves hidden overlays inert. The correct shape is direction-aware (user-initiated opens displace; push-driven opens refuse with a visible line) — i.e. part of finding 20.

### Finding 9 — reproduced

A verbatim mirror of `WirePower`/`WirePowersChanged` (attributes unchanged) was compiled and run:

```
ERROR null on defaulted item field: field=powers[1].hasSteeringFiles err=invalid type: null, expected a boolean
ERROR null on defaulted mcp list:      field=powers[0].mcpServerNames err=invalid type: null, expected a sequence
OK    null on Option item field
```

`to_notification` turns that into `warn!` + `Ok(None)` (`powers.rs:104-123`), and `SessionController.powers` is written only by the `PowersChanged` arm. The existing malformed-frame fence covers `"powers": null` (the required array), not a null on a defaulted item field. Fix: `Option<Vec<String>>` / `Option<bool>` + `unwrap_or_default()` in `From<WirePower>` — both fields — plus one positive fence row. Trigger realism stays unproven (PLAUSIBLE is the right label), and severity is "first push only": a later push keeps the prior catalog (`powers.rs:100-103`).

### Finding 10 — refuted as stated (Reject)

The retention is a pinned decision, not an oversight: `spec.md:118` — "Process-global; latest push wins … No per-session keying; a `/new` does not clear the panel data", on P5 (the catalog is the user-level `~/.kiro/powers` install). "Session A's catalog shown to B" is not a mis-mapping if the set is process-global. The in-repo capture already answers the review's probe partially: `crates/cyril-core/tests/fixtures/kas/workflow/kas-csig-2.16.0-neutral.jsonl` is one connection with three sessions, and all three received a push — `/new` self-heals.

The proposed direction is also unsafe: `clear on SessionCreated` discards a valid catalog whenever the push lands after `SessionCreated` (the transport harness scripts exactly that order and asserts push-before-created; the +18 ms live gap is wall-clock, not a wire guarantee), and nothing pins that `session/load` re-pushes. Only reword the doc that overclaims (`session.rs:47-51`), or open the pull-path decision (below). Two review facts are also wrong: `modes`/`models`/`cached_model` **are** assigned by the `SessionCreated` arm; and the review's `PowersManager` citation appears nowhere in the tree.

### Finding 17 — the message is the fix (Modify)

Inertness is certain: `kas` is default-off (`cyril/Cargo.toml:18`), `engine_for` returns `Err("KAS engine requires a build with --features kas")` (`bridge.rs:283-284`), and `PowersChanged` has one producer, the KAS-only converter. So a default build always answers "No powers reported yet — start a KAS session first." — advice the binary cannot honour.

Gating the registration is the wrong branch: it contradicts `spec.md:110` ("Registered unconditionally for both engines"), needs a new command-source seam and fence churn, and *still* leaves the wrong sentence in the states that survive (KAS session pre-push; dropped frame). Reword `builtin.rs:496` to state the general precondition (build with `--features kas`, run with `--agent-engine kas`), keep the "No powers reported yet" prefix (both existing fences assert that substring), and update the one artifact pinning the old wording (`.cyril-v19o/plan.md:97`).

---

## Where the review document is factually wrong

1. **#1 anchor `:146`** is the walk non-vacuity assert, not the prohibition (`:116-135`/`:154-186`/`:202-207`).
2. **#3 "96 cols vs 80"** is the *desired* width; on an 80-column terminal `place` clamps both to 76, so the panels are the same width. The overpaint argument needs ≥5 powers, not generic width.
3. **#4** understates its own finding: even with the guard fed stripped text, `crates/*/src` includes `#[cfg(test)]` code naming the push (`engine.rs:817,836,844`; `bridge/tests/harness.rs:519`), so deleting the converter still leaves `push_seen_in` non-empty.
4. **#5 "whenever `place` clamps"** — the drop needs a catalog ≥5 powers *and* a placed height of 17–18 (H = 24 or 25 on an empty 5-row input). Today's 3-power install loses nothing; at 100×24 it shows 3 powers in a 13-row box with 2 blank rows.
5. **#6 "unlike every sibling overlay"** — `hooks_panel` has neither overflow row nor key footer; four of six overlays have some affordance, and the two that do not are hooks and powers.
6. **#7 "~103-cell line"** — 101 cells (recomputed token by token). Conclusion unaffected.
7. **#8 "(3 blank of 5)"** for the hooks comparison matches no constructible geometry; the direction (powers is worse because rows are 3 lines) holds, the number does not. `:2476` is the fn signature; the clamp is `:2478-2479`.
8. **#10** — wrong comparison set (see above) and unsourced `PowersManager`.
9. **#11 "engine.rs:334-340 defends this"** — that comment defends the converter *ordering* as perf-neutral, which is true; it does not endorse silence.
10. **#14b "deleting that arm keeps both halves passing"** — the match is exhaustive (`app.rs:1993-2075`, no `_` arm), so deletion is E0004; a neutered arm *is* caught by `powers_submit_distinguishes_unloaded_from_empty` (`app.rs:5572-5580`).
11. **#15 "four lines below"** — the rule is five lines below `:295`; and the chain has been stale for `usage` since `main`.
12. **#17** cites a bare `bridge.rs:283-284`; the file is `crates/cyril-core/src/protocol/bridge.rs` (there is no `crates/cyril/src/bridge.rs`).
13. **#2 "14 commands listed"** is the v2 count; a KAS registry prints 15 (host mode) or 16.

## Defects the review missed or understated

- **Approval overlay erased by powers** (worst case of #3; Enter answers an unseen permission prompt).
- **`refresh_powers_panel`'s clamp** (`state.rs:2451`) strands the viewport the same way as #8, and PageDown's 5 overshoots a squeezed popup.
- **`refresh_hooks_panel`** (`state.rs:2369-2381`) carries the identical unconditional-`true`/doc mismatch as #19.
- **`AGENTS.md:286-298`** duplicates the stale overlay chain (#15) and is a doc surface the review did not mention.
- **`state.rs:95-96`** still says the stall chip is "cleared by ANY other notification" — false since the six-variant allowlist; it is the sentence that makes retraction 2 sound plausible on a read.
- **The `/#14` fence-file split**: the ordering fence (`state.rs:5633`), the App fence (`app.rs:5516`) and the CRLF fence (`powers_source_fence.rs:259`) are three separate defects in three files with three repairs.
- **Test-fixture labeling slip** at `app.rs:5594-5595` ("Five powers" over a 12-power fixture; `:5615` calls offset 6 the "five-power window plus one").

## Items that are product decisions, not fixes

1. **Pull-based recovery via `_kiro/powers/list`.** Proven working, unadvertised (so not feature-detectable). Requires re-approving `spec.md:64/77/86/111`, updating the transport fence, the builtin prose and the design record. If taken, findings 9/10 become moot for recovery — but 9 still deserves the null fix.
2. **Catalog lifecycle on `/new` / disconnect.** `spec.md:118` pins process-global. Any change is a spec decision; do not clear on `SessionCreated` as an intermediate step.
3. **`/powers` registration on non-KAS builds.** `spec.md:110` pins unconditional. Rewording is behavior-preserving; gating is not.
4. **Overlay displacement policy.** The codebase's own doctrine (push-driven opens never pop a modal over the user) conflicts with "last-opened wins"; the policy has to be chosen, then encoded once.

## Verified fix order (supersedes the review's)

1. **Push-side hardening, no spec change:** #9 (`Option` on both fields + fence row), #11 (near-miss warn), #12 (`nonempty` + `title()` doc), #4 (guard fed stripped text, anchored to the converter module), #2 (move the push, add the registry-wide `/help` fence).
2. **Widget/state arithmetic + affordances:** #5 (constant in both places, clamp-reachable fence), #7 (steering budgeted out of the line), #8 (window-aware clamp or render-side clamp; same for `refresh`), #6 (title affordance — zero rows), #19 (changed-check, gate the clone, delete the dead store; mirror in hooks).
3. **Fence repairs:** #14a (assert ids, fix the case fixture), #14c (feed CRLF to the function under test, or delete the tautology), #18 (order-agnostic collection), #13 + #15 + #16 (doc/predicate corrections, both doc files).
4. **Root cause:** #20 + #3 — one overlay slot/predicate; then the five fixes above stop regressing. Tier the work: the shared predicate and mutual-exclusion semantics first, the `Overlay` enum second.
5. **Only with re-approval:** #1 invert (pull), #10 lifecycle, #17 gating.

## Verification limits

- Scouts ran read-only (no cargo/builds). The only execution performed was the parent-side serde mirror probe for #9; every other magnitude (positions, cell counts, blank rows) is arithmetic derived from source constants, and the review's magnitudes were checked against that arithmetic, not against a running app.
- Not executed: a TestBackend draw at 100×24 with `input_top = 18` (would settle #5/#8 visually), the transport fence (#18) under a delayed push, and any live KAS run.
- The two retractions both hold. R1's residual is documentation only, but the correct reason is that extension frames route globally (`inbound.rs:170-183`), so the frame's `sessionId` is the only session carrier and is dropped deliberately because the payload is not session-scoped — not "the session is already known from the routing envelope".
