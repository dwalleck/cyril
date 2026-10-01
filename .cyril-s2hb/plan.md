# Plan: cyril-s2hb

## Inputs and design verification

Consumes route/spec/evidence and approved design dated 2026-09-30. F0 has observed native Linux/Windows proof; C1–C10 have complete falsifiers, independent oracles, named mutations and named checkpoint owners. File-backed capture's descriptor tradeoff is explicitly approved in spec/design. At initial approval no failed or waived fence was carried; the later explicit A1-only F19/F20 exceptions are owned by design.md and review-decisions.md. This plan declares no implementation complete.

Publication scope is A1 already merged plus the existing PR150 carrying all remaining s2hb work: A2-C is its normative opening context, followed by atomic A2 gather/facts+hiddenCLI and B diagnostics checkpoints in the same PR. F29/F30's complete identity repair and all acceptance remain in scope; no behavior is dropped.

## Publication and checkpoint sequence

The user's exact waiver is recorded here: “Ignore the 4000 line commit limit”. It removes the active aggregate commit/publication-size gate and all size-only fit claims; it does not waive acceptance, review, CI, native, lifecycle, safety, fixture, assertion, cap, or raw-content requirements. No future size-driven partition is planned. Module-specific/per-file tripwires remain independent: if any such size gate blocks progress, report the measured size and options/tradeoffs and obtain the user's choice before restructuring, changing a gate, partitioning, or seeking another waiver. No such blocker is known here.
> "Ignore the 4000 line commit limit"


Publication outcomes:
1. **A1 native prefix** remains already merged with its acceptance retained.
2. **Existing PR150 final scope** contains the A2-C normative opening context, a separately verified atomic A2 gather/facts+hiddenCLI checkpoint, and a separately verified atomic B diagnostics checkpoint. The previously qualified flag-based driver and full CLI harness remain authoritative. Final assembled qualification, full PR review, native proof and required CI run after both checkpoints.

The issue closes only after A1's merge and the final complete PR150 merge satisfy every criterion. Requester authorization extends only s2hb to20 implementation attempts/10 PR reviews without resetting counts. Historical opening counters at the R17 amendment were14/5; the later PR record is15 implementation attempts/6 PR reviews before the next full PR review, and subsequent live consumption belongs in the PR thread. Documentation opening context confers no implementation acceptance; every other issue retains5/5. No dependent issue starts before closure; no unrelated primary-checkout work is staged.

Historical A2-C opening pins: R15 had completed implementation cycle13/15 and R16 was allocated cycle14/15; cycle12 is historical. Three full PR reviews preceded A2-C; PR150's initial full review was round4/5. These are opening-history pins, not a live remaining-budget claim. A documentation PR consumes a review normally and never resets or evades the issue-wide cap.

## Module growth ledger

Production-region convention and tripwire review procedure are in design.md. Ranges are forecasts, not targets to reach by compressing code. Baselines count retained existing cfg-test helper methods outside the terminal test module.

| Module | Baseline | Projected final | Responsibility/interface delta | Protected-parent rule |
|---|---:|---:|---|---|
| review lib.rs | 0 | 90–150 | exports and structured errors | no algorithms |
| review run.rs | 0 | 210–250 | run anchors, stamp, semantic manifest I/O, raw path provenance/coherence, atomic replacement and UTF-8 text | A2 run-owner tripwire300 |
| review clock.rs | 0 | 70–120 | clock seam and UTC conversion | two actual adapters |
| review git.rs | 0 | 140–190 | explicit argv/target/error behavior | no shell |
| review gather.rs | 0 | 300–320 | gather orchestration | delegate facts/I/O; A2 Length review tripwire350 |
| review facts.rs | 0 | 533–560 | symbols/usages/docs/pages | no diagnostics/workflow; A2 Length review tripwire620 |
| review diagnostics/mod.rs | 0 | 170–220 | typed operation/report | reuse run I/O |
| review diagnostics/process.rs | 0 | 160–200 | direct child/captures/snapshots | no descendants/read threads |
| review diagnostics/command.rs | 0 | 130–185 | host command plan/environment | no implicit shell |
| core review/mod.rs | 0 | 140–160 | dialect/prefix; C8 Length review retained one owner | no resolver; tripwire190 |
| binary crtool.rs | 0 | 65–100 | clap and output/exit mapping | no run implementation |
| binary main.rs | 304 | 312–320 | early dispatch only | +20 maximum |
| core lib.rs | 19 | 20 | export only | +2 maximum |
| bridge.rs | 417 | 433–442 | resolved dialect carry/getter | +30 maximum |
| host_shell.rs | 478 | 486–492 | exhaustive dialect projection | +15 maximum |
| root/leaf/binary Cargo manifests | existing | +20 combined plus leaf metadata | leaf registration, semantic JSON and Unicode regex features | no weakened lints/extra runtime dependencies |

Module-specific tripwires and ownership checks govern growth; they are not an aggregate commit-size gate. No shared acceptance gate is replaced.

## Slice A1: construct safe native executable prefixes

**Claim IDs:** C8; introduces the C10 standalone ownership fence, whose scope expands with subsequent slices.
**Expected behavior:** canonical current executable becomes a dialect-correct, forward-slash, double-quoted prefix; listed hazards and native PowerShell delimiter characters refuse; the dialect is projected from the single existing resolved HostShell.
**Oracle:** literal prefix strings, actual native shell argv/output, Windows PowerShell5.1/pwsh7.6.6 parser observations, and the approved module ledger.
**Stress fixture:** spaced/Unicode/native drive/UNC paths, each hazard in otherwise-valid absolute paths, valid apostrophes and non-active typographic quotes, PowerShell-only U+201C/D/E, invalid UTF-8, relative/device forms, and long native path spelling. The valid control succeeds; each unsafe case returns its own typed refusal.
**Regression fence:** core review behavioral tests and `.cyril-s2hb/oracles/check_shape.py --phase prefix` for source ownership. Attempt9 replaces the unsound token-flow predicate with `cargo run -p cyril-core --features kas --profile test --example review_prefix -- <shell> <dialect>` in the existing native CI matrix; this consumer executes the generated prefix and checks exact child argv and bridge shutdown. Remove the obsolete issue-local resolved probe after promotion. Its separate path-hazard probe remains applicable.
**Named mutation:** omit dollar guard and force POSIX opening for PowerShell (C8); add forbidden `review_gather_body` to a protected parent (C10). Their independently observed red/restored-green receipts remain required. The replay additionally executes native CI against decoy/discard/early-return/shadow variants and valid macro/raw/closure controls, but its runtime verdicts are diagnostic only under design.md's explicit F19 exception: **N/A — approved risk: F19 bypassed for A1 only**. Child-role rejection retains its direct red/green proof. Source and runtime verdicts stay separate; no operational failure is claimed as semantic detection. Mutation builds use an owned target and refreshed archived crate-root timestamps.
**Complexity/production scale:** validation and spelling are O(N) path bytes, with one output allocation sized for normalized path plus fixed opening/suffix; no subprocess per character and no new persistent loop. The long-path case verifies exact output and actual execution, while source census verifies one reserved String and append-only construction. Accepted storage/work are linear, not a constant input cap.
**Wall budget/phase:** one-off prefix creation; N/A — no latency SLA. Existing bridge resolution remains once per startup.
**Module shape:** core review owns spelling; HostShell only projects its kind; BridgeHandle only carries the optional dialect. Core lib <=2, bridge <=30, HostShell <=15 production-line growth; main unchanged. Prefix retain-and-raise disposition is in design.md's C8 Length review.
**Files:** new core `src/review/mod.rs`; existing core lib.rs/protocol/bridge.rs/protocol/kas/host_shell.rs; shared issue-local route/spec/evidence/design/plan/probes, prefix checkpoint and shape oracle; existing CI KAS lane gains the standalone prefix ownership gate with full checkout history. Attempt9 promotes the resolved probe to core `examples/review_prefix.rs` and adds its native runtime step to CI's existing OS matrix. PR146's bounded qualification repairs also change cfg(unix) version.rs fixtures, add two executable fixture scripts, and add the source/runtime replay. All are owned by review-decisions.md; leaf/CLI/parity draft files remain excluded.
**Estimate:** one implementation/verification cycle; estimate is not a gate.
**Diff estimate:** N/A — no active commit-size publication limit; production growth, independent mergeability, native qualification, review and CI acceptance remain required.
**PR increment:** S2HB-A1 native prefix.
**Commands and expected results:**
- `cargo test -p cyril-core --features kas review::` → exact valid prefixes and typed hazard/namespace refusals; actual POSIX fixture emits `crtool`.
- Native `review_prefix` through bash/fish and installed Windows PowerShell5.1/pwsh7.6.6 → resolved dialect, generated command, exact child argv and shutdown. CI also executes bash on macOS. Retained separate native path-hazard/drive/UNC proof remains applicable to unchanged production bytes.
- `python3 .cyril-s2hb/oracles/check_shape.py --phase prefix` → approved path/count/protected-parent checks; C10 mutation red/restored green. Prefix responsibility remains directly reviewed; its missing automated enforcement is **N/A — approved risk: F20 bypassed for A1 only**, not proved by this command.
- C8 focused tests under each named mutation → localized red; exact restoration → green.
- `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings`, `cargo check -p cyril --features kas` → independently green prefix-only tree, with no leaf/CLI draft dependency.
- Native smoke, isolated production reconstruction/ledger comparison, fresh PR review and required current-head CI precede merge.

## PR150 A2-C opening contract context
Publication merge pins: three-way ancestor `422d16467aa48d8956d11dcc3b752fa91d53bd6f`; repaired PR150 normative source `64d9293685c1b7525c7b7dd42e48bb2f71cba7e0`; latest R17 implementation-source handoff is consumed read-only. These pins identify document provenance only and do not claim implementation success.



**Claim IDs:** N/A — requester-authorized normative opening context; final PR150 A2/B checkpoints own C1–C7, C9 and applicable C10 verification.
**Expected behavior:** the published normative artifacts preserve the requester's functional-equivalence correction, raw identity/framing contract and staged A2/B ownership without claiming implementation complete. This is historical PR150 opening context, not a restriction on its final scope.
**Oracle:** requester quotations and the approved responsibility/safety contract, inspected directly; executable evidence belongs to the final assembled PR150 checkpoints.
**Stress fixture:** opening context only; final PR150 retains the complete gather/facts, hidden-CLI, diagnostics, Unicode page, malformed-manifest and native fixtures.
**Regression fence:** opening context has no executable fence; final PR150 preserves the qualified flag-based library/CLI harness and all native/CI gates.
**Named mutation:** opening context changes no executable fence; final PR150 retains all C1–C7, C9 and C10 mutations.
**Complexity/production scale:** N/A — no runtime loop or allocation changes.
**Wall budget/phase:** opening context is documentation; final PR150 checkpoint timing/qualification remains owned by its implementation stages.
**Module shape:** no production modules change in opening context; final PR150 owns the complete approved leaf/binary seams.
**Files:** `.cyril-s2hb/{spec,design,plan,route}.md` are the normative opening context in existing PR150; the final PR150 also carries the complete implementation, harness, receipts and docs.
**Estimate:** opening context is not a completion gate; final PR150 owns implementation qualification.
**Diff estimate:** N/A — no active aggregate commit-size limit or size-only fit claim.
**PR increment:** Existing PR150, with A2-C as opening context rather than a separate documentation-only publication.
**Required review evidence:** inspect the four-file opening context and then the complete assembled PR150; run all required quality/native/CI gates and obtain a fresh independent full-PR review only after A2 and B checkpoints are assembled. No implementation-success receipt is inferred from these normative files.

**A2-C review repair:** F27 Verified/Accept — pinned head `8b748154` had no A2 checkpoint at PR150 opening, so the opening context recorded C1–C4 as wholly PENDING and carried no implementation-success receipt at that time; final PR150 A2/B checkpoints own the missing evidence. F28 Verified/Modify — stale live-count claims are removed. Historical opening pins remain R15 cycle13/15, R16 cycle14/15 and PR150 initial review4/5; later live review consumption belongs to the PR thread. No cap, acceptance requirement or executable behavior changes.

## Slice A2: complete native gather/facts and hidden command integration

**Claim IDs:** C1, C2, C3, C4, C9; C10 for complete leaf/binary ownership; retain valid A1 C8 proof.

**Expected behavior:** public gather/facts produce complete stamped artifacts and typed functional results without Python, KAS or an incomplete binary dependency. Persist optional raw filename identity as approved for F29/F30; distinct same-name symbols and exact own-definition/document lookup survive facts reuse. The complete hidden `cyril crtool gather`/`facts` dispatch runs before normal startup, uses the real clock, retains correct0/2/3 outcomes, and leaves ordinary help/startup unchanged. The previously qualified flag-based fixture driver and full CLI harness remain the verification seam.

**Oracle:** design C1/C2/C4 independent Python functional reference with shared timestamp/version, direct raw Git and independently specified semantic helper/caller/document expectations where Python shares a defect, C3 categories/context and unchanged-artifact observations, C9 actual process/filesystem observations, retained C8, and C10 compiler/dependency/ownership checks. No semantic field/type/array order/content is normalized away.

**Stress fixture:** retain assembled A2's language/Unicode, caps, binary/deleted/empty, scope, stamp, pages and precedence cases. Strengthen raw/look-alike names with identical symbol names, exact cross-file usages, colon/LF path framing, unscoped raw document sizes, typed raw/display coherence refusals, persisted-run facts rebuilding, invalid ordinary config, nonexistent agent command, startup markers, repeated gather/facts and unsupported diagnostics verb. The full CLI harness remains intact; no waiver narrows it.

**Regression fence:** leaf/public operation and full hidden-CLI cases through the existing qualified flag-based `parity_driver`/CLI harness; `.cyril-s2hb/oracles/parity.py --phase gather`; `check_shape.py --phase gather`; actual native library and binary consumers. C1–C4 and C9 fixtures remain in this single A2 increment.
The differential run is permanent functional-equivalence tooling, not a production source-string test. The fixture driver is verification-only and is not installed as a Cyril command.

**Named mutation:** retained C1 data corruption, C2 precedence/exit classification, C3 stamp refusal, C4 stored-usage cap and R17 identity/framing mutations; dispatch-after-config and forbidden protected-parent body mutations fail the A2 fences. Preserve valid negative receipts for unchanged mechanisms with renewed positive proof; no assertion, fixture, oracle, or guard is dropped.

**Complexity/production scale:** retain F+S+20 Git calls, no subprocess per hit, O(S×R+H) scanning, at most four page passes and O(B+H) storage. Scale fixture remains100 files/200 definitions/>40 usages each/2 MiB input. R17 adds O(P) persisted raw-name/coherence work for P filename bytes and one linear pass through each NUL-framed output; at most one retained raw-name copy per non-UTF8 file, borrowed facts identities, no additional Git commands or page passes. These input-size formulas are the accepted bounds for larger inputs; no fixed input limit or invented latency target is added.

**Wall budget/phase:** one-off library operations; N/A — no one-off latency obligation. No always-on loop/background task is introduced. Complexity is checked structurally and through the scale fixture, not by a flaky timing assertion.

**Module shape:** existing leaf run/Git/gather/facts/clock owners plus the approved binary adapter and <=20-line main wiring; no domain logic in main, no new leaf owner/dependency, no public operation or executor change. R17 private representation/framing stays in run/gather/facts; module-specific gather350/facts620/run300 and binary/core ownership checks remain independent of commit-size planning.

**Files:** `crates/cyril-review/{Cargo.toml,src/{lib,run,clock,git,gather,facts}.rs,examples/parity_driver.rs,tests/evidence.rs}`, `crates/cyril/src/{crtool,main}.rs`, root Cargo manifests/lockfile, functional oracle, shape phases, full CLI harness and native CI. Preserve the complete flag-based driver/CLI hunks; no size-driven JSON transport or missing-binary fallback. Only s2hb's tracker row may change; parent5gb3 and unrelated records remain unchanged. Existing core prefix/projection files are inherited from merged A1, not reintroduced. Update `README.md` and `AGENTS.md` after smoke; no unrelated skills or primary-checkout changes.

**Estimate:** one implementation/verification cycle, potentially several hours of native qualification; not a completion gate.

**Diff estimate:** N/A — no active commit-size publication limit or size-only fit claim. Production growth, module tripwires, independent mergeability, native qualification, review and CI acceptance remain required.

**PR increment:** S2HB-A2 complete gather/facts and hidden command integration.

**Commands and expected results:**
- Build the leaf and actual `cyril` binary plus the existing `parity_driver`; no diagnostics shell verb or incomplete binary dependency.
- Run the existing flag-based reference and full CLI harness on native hosts; compare semantic records, meaningful content, statuses/errors/context, patch content and raw-capture integrity.
- The sole A2 evidence owner, `.cyril-s2hb/checkpoint-A2.md`, records the malformed-manifest matrix for both gather and facts with unchanged artifacts and the scalar/greedy private page-boundary proof plus chars-to-bytes mutation; all public/reference cap, oversize-line, and content cases remain covered.
- Run `check_shape.py --phase gather`, named mutations/restoration, full startup/help/real-clock smoke and native Windows/macOS equivalents; focused failures must be localized and exact restoration green.
- Run formatting, workspace/KAS, required current-head CI, isolated reconstruction/comparison, fresh review and actual consumer smoke before merge. These are obligations, not receipts in this normative publication.

**Fixture driver and comparison reuse:** retain the previously qualified flag-based driver and full CLI harness. The size-driven serde-tagged JSON stdin/from-reader simplification is withdrawn; no new transport, dependency, public interface, CLI/environment override, missing-binary fallback, or alternate oracle is introduced. The existing independently selected raw-filename patch-comparison pass remains reused rather than creating a derived lookup table; all compiled calls, equivalent fixture inputs, fixed-clock seam, stdout/error/exit behavior and assertions remain.
C4 proof decision (A2 evidence, not a future checkpoint): the old `facts_pages_pack_unicode_characters_and_split_at_the_budget` heading/prose assertion is replaced, not re-pinned. `.cyril-s2hb/checkpoint-A2.md` records Unicode-scalar greedy packing and unchanged meaningful input lines through existing private `write_pages`/`body_characters`, including a chars-to-bytes mutation failure. It also records all public/reference cap, oversize-line and content cases and the all-platform malformed-manifest matrix for both gather and facts with unchanged artifacts. No boundary or assertion obligation is dropped.
C9 integration: the complete hidden CLI adapter, startup/help/real-clock smoke and actual `parity.py` CLI phase are part of this same A2 increment, not a later slice.


**A2 hidden-command qualification:** the actual hidden `cyril crtool gather`/`facts` adapter dispatches before config, agent, terminal or logging startup; invalid ordinary config cannot affect it, and ordinary help contains no `crtool`. The flag-based driver remains the qualified library/CLI consumer, and repeated gather/facts plus unsupported diagnostics verb cases remain required.

**A2 hidden-command mutation and shape fences:** dispatch after config must fail the actual CLI fence; forbidden review body in `main.rs` must fail C10; exact restoration returns green. The approved binary adapter and <=20-line main wiring contain no domain logic or new leaf owner/dependency. Full native shell/clock/startup/help qualification and current-head review/CI remain required.


## Slice B: execute cancellable native preflight diagnostics

**Claim IDs:** C5, C6, C7; C1/C3/C10 retained/extended and reverified for the new consumer. F0 retained mechanism proof applies because the same std file-handle strategy is selected.

**Expected behavior:** public diagnostics executes the explicit native command, preserves lossless raw captures and meaningful filtered evidence, and returns typed clean/nonzero/timeout/cancelled outcomes; cannot-start/parse/stamp failures write no reports; pre-cancel does not launch; live cancellation kills/reaps only its owned direct child. No diagnostics CLI verb exists.

**Oracle:** C5 uses functional command/argv/environment behavior plus literal child receipts; C6 uses meaningful evidence/status/content and report limits; C7 uses OS process status and independently controlled handshakes/holder; deterministic deadline/first-observation cases are not fake real-time success.

**Stress fixture:** 131,073 bytes on each stream including invalid UTF-8; 201 changed-path matches and 16 tail lines; child-emitted CRLF, if present, remains raw capture data; hostile quoted argument shapes; failed exit and signal; zero/default timeout; cancel before launch/after ready/after exit; descendant still writing after direct child exits; spawn failure and cleanup errors. Expected partial snapshots contain only bytes before sampled lengths, and holder survives until fixture-owned release.

**Regression fence:** `crates/cyril-review/tests/diagnostics.rs` with a native fixture child mode in the verification driver; `.cyril-s2hb/oracles/parity.py --phase diagnostics`; permanent native owned-process cancellation qualification. Checks assert functional statuses/content and lossless raw captures. No test kills processes by name or relies on a shared current directory/environment mutation.

**Named mutation:** C5 splits Windows raw tail/removes nonempty CARGO_ value; C6 reverses stream order or uses a199 match cap; C7 omits direct-child kill/reads past snapshot length. Deterministic terminal-decision tests also invert exit/cancel observation order. Native control proves holder/unrelated sentinel are live, so absence assertions cannot pass vacuously.

**Complexity/production scale:** command parsing O(C) characters; Windows executable candidates bounded by whitespace positions in C, with no successful-launch retry. Capture is O(B) disk/output bytes, two distinct files, zero reader threads, one direct child. Polling uses a fixed 10 ms sleep while live; terminal deadline decisions use monotonic samples, no busy-spin. Snapshot reads are capped by two sampled lengths even if holders keep appending. Filtering O(L×F) for L output lines/F changed paths (matching functional path semantics); reports retain at most200 matches/15 nonempty tail lines; no raw-output cap. Scale fixture 32 MiB dual-stream output and100 changed paths; accepted bound is two capture files, no reader thread, one spawn, at most sampled stdout+stderr+separator bytes and source report limits. It does not promise bounded disk cost for arbitrary input volume.

**Wall budget/phase:** one-off diagnostics operation; user/default timeout is the running deadline and direct-child termination/reap has the approved one-second deadline. Deterministic virtual observation tests enforce deadline/precedence decisions; real native process fixtures establish actual reaping/cleanup, with outer timeout solely a hang guard. N/A — no separate latency SLA for filesystem output, no always-on background task.

**Module shape:** create diagnostics/{mod,process,command}.rs; lib gains only exports/error variants; run owns unchanged stamp/semantic JSON/native text implementation. No protected-parent production changes are required. `python3 .cyril-s2hb/oracles/check_shape.py --phase diagnostics` verifies final ledger/dependencies and growth; re-run affected C10 mutations if fence changes. Return to design before changing ownership or thresholds.

**Files:** create `crates/cyril-review/src/diagnostics/{mod,process,command}.rs` and `tests/diagnostics.rs`; extend leaf lib.rs and verification example; extend parity/shape oracle and native CI phase; update existing docs after actual diagnostics smoke. No crtool clap diagnostics variant.

**Estimate:** one substantive implementation/verification cycle, potentially several hours of native qualification; not a completion gate.

**Diff estimate:** N/A — no active aggregate commit-size limit or size-only fit claim; diagnostics production growth and module tripwires remain review context only.

**PR increment:** Atomic B diagnostics checkpoint inside the existing PR150; it is separately verified before final assembled PR review/CI.

**Commands and expected results:**
- `cargo test -p cyril-review --test diagnostics` → typed statuses, lossless raw captures, meaningful rendered content, no report on prelaunch errors, deadline/terminal precedence, owned lifecycle assertions.
- `cargo build -p cyril-review --examples` then `python3 .cyril-s2hb/oracles/parity.py --phase diagnostics --cyril target/debug/cyril --driver target/debug/examples/parity_driver` → functional status/content/record equivalence for identical clock/command/manifest inputs, lossless raw-capture checks, and separate actual-system-clock library smoke.
- Native Windows equivalent → raw command-tail argv semantics, Windows exit-code treatment, lossless child evidence, meaningful rendered content, cancellation and holder-cleanup observations. CI macOS executes its own host fixture.
- Each mutation's focused case → named red output; restore → green. Record source hash/environment and observed terminal values.
- `python3 .cyril-s2hb/oracles/check_shape.py --phase diagnostics`, `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings` → final assembled gate passes; no source fence runs from production tests.
- Actual library consumer smoke, independent production reconstruction/ledger comparison, fresh full PR review and current-head CI must pass after both A2 and B checkpoints are assembled. Verify A1's merge plus the final complete PR150 merge and every acceptance criterion; close cyril-s2hb without changing parent cyril-5gb3.

## Self-review and handoff

Scoped R17 self-review: C1–C4 and C9 remain owned by the atomic A2 checkpoint, C5–C7 by the atomic B checkpoint, with inherited A1 claims and per-checkpoint C10 checks. The sole A2 evidence owner, `.cyril-s2hb/checkpoint-A2.md`, qualifies current A2 C1–C4, C9 and applicable C10; C1/C3/C10 remain subject to their B extension, while C5–C7 remain pending because B is unimplemented. The A2 and B checkpoints remain separately reviewable inside the existing PR150, while final full review/native/CI, merge and issue acceptance remain pending. Existing module seams and independent tripwires remain; byte/record loops retain explicit linear/input-size bounds and no new wall-time claim. No final PASS receipt is claimed by this plan.

## Planning amendments, 2026-09-30

C8 Length review in design.md retains the cohesive prefix owner and raises its tripwire to190, with updated forecast140–160 (baseline0) and30-line uncertainty. No ownership/interface or protected-parent delta changes. EvidenceParity retains required coverage without compressing fixtures. Module/per-file tripwires remain independent; if any other size gate blocks progress, report measured size and options/tradeoffs and obtain the user's choice before restructuring, changing a gate, partitioning, or seeking another waiver.

## Functional-equivalence amendment — 2026-09-30

Requester correction (verbatim):
> “It didn't need to be a 1 to 1 copy of the python script, just functionally the same”

Further requester clarification (verbatim):
> “The results and output don't have to be byte equivalent”

This supersedes active byte/text-wrapper parity requirements. A2 checks preserve semantic JSON fields/types and array order, file/symbol/usage sequence, meaningful content, stamps/refusals, exit/status/error/context, page budgets/caps, patch content, and native safety/lifecycle gates. Scope parsing/trimming uses Rust standard Unicode whitespace; generated text uses UTF-8 without a host-specific adapter or native LF-only gate. B uses functional command/status/content checks with lossless raw captures, not Python wrapper-byte parity. The C1 formatting-only manifest-key-order/newline mutant is replaced by manifest data corruption; no format-only mutation is required.

Pre-amendment byte comparisons, host-specific CRLF/CRCRLF checks, and exact stdout/stderr wording are historical evidence only. The A2-C text is historical PR150 opening context, not a restriction on final scope; no implementation-success receipt is claimed there. The sole A2 evidence owner, `.cyril-s2hb/checkpoint-A2.md`, qualifies current A2 C1–C4, C9 and applicable C10; C1/C3/C10 remain subject to their B extension, while C5–C7 remain pending because B is unimplemented. Final full-PR review, native qualification, required CI, merge and issue acceptance remain pending. Historical opening counters remain distinct from live PR consumption: R15 completed implementation cycle13/15 and R16 was allocated cycle14/15 at A2-C opening; PR150's initial full review was round4/5, later review consumption is in the PR thread, and R17 authorization is20 implementation attempts/10 PR reviews without reset. The user's exact size waiver removes only the aggregate commit/publication-size gate; it does not waive any cr…
