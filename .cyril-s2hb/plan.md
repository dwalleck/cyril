# Plan: cyril-s2hb

## Inputs and design verification

Consumes route/spec/evidence and the approved design. Round7's requester choices in spec.md supersede file-backed capture and permit necessary safe capture/launch dependencies, native executable identity and null stdin. The original A2/B checkpoints retain their exact checked-source evidence; new repair obligations are not qualified by those receipts. All16 required CI jobs passed for published head4c52d13b, but thirteen blocking review rows prevent merge. Main owns bounded run-input repair18 and the pending affected Git/capture/launch designs; A1-only F19/F20 exceptions remain unchanged.

Publication scope is A1 already merged plus the existing PR150 carrying all remaining s2hb work: A2-C is its normative opening context, followed by atomic A2 gather/facts+hiddenCLI and B diagnostics checkpoints in the same PR. F29/F30's complete identity repair and all acceptance remain in scope; no behavior is dropped.

## Publication and checkpoint sequence

The requester’s aggregate publication-size waiver is quoted once below. It removes the active aggregate commit/publication-size gate and size-only fit claims; it does not waive acceptance, review, CI, native, lifecycle, safety, fixture, assertion, cap or raw-content requirements. No future size-driven partition is planned. Module-specific/per-file tripwires remain independent: if another size gate blocks progress, report the measured size and options/tradeoffs and obtain the requester’s choice before restructuring, changing a gate, partitioning or seeking another waiver. No such blocker is known here.
> "Ignore the 4000 line commit limit"


Publication outcomes:
1. **A1 native prefix** remains already merged with its acceptance retained.
2. **Existing PR150 final scope** contains complete A2/B and their round7 repairs. The qualified flag-based driver and full CLI harness remain authoritative. Prior conformance and CI results apply to their exact source/inputs, not the newly approved replacement capture/launch contract. Required repair proof, independent repair/final review, current repaired-head CI and publication remain pending before merge/issue acceptance.

The issue closes only after A1's merge and the final complete PR150 merge satisfy every criterion. Requester authorization extends only s2hb to20 implementation attempts/10 PR reviews without resetting counts. Eighteen implementation attempts are complete, including R7RunInputs; full PR consumption remains7/10. Subsequent live consumption is also recorded in the PR thread. Documentation confers no implementation acceptance; every other issue retains5/5 and parent cyril-5gb3 remains untouched. No dependent issue starts before closure; unrelated primary-checkout work remains unstaged.

Historical A2-C opening pins: R15 had completed implementation cycle13/15 and R16 was allocated cycle14/15; cycle12 is historical. Three full PR reviews preceded A2-C; PR150's initial full review was round4/5. These are opening-history pins, not a live remaining-budget claim. A documentation PR consumes a review normally and never resets or evades the issue-wide cap.

## Module growth ledger

Production-region convention and tripwire review procedure are in design.md. Ranges are forecasts, not targets to reach by compressing code. Baselines count retained existing cfg-test helper methods outside the terminal test module.

| Module | Baseline | Forecast | Actual production lines at this atomic checkpoint | Responsibility/interface delta | Protected-parent rule |
|---|---:|---:|---:|---|---|
| review lib.rs | 0 | 145–170 | 139 | exports and structured errors | no algorithms; ceiling180 unchanged |
| review run.rs | 0 | R7RunInputs252–262 plus10 uncertainty | 261 | anchors, stamp, strict JSON, early facts-map validation, raw provenance, atomic replacement | A2 run-owner tripwire300 unchanged |
| review clock.rs | 0 | 70–120 | 61 | clock seam and UTC conversion | two actual adapters; ceiling150 unchanged |
| review git.rs | 0 | 140–190 | 134 | explicit argv/target/error behavior | no shell |
| review gather.rs | 0 | 300–320 | 236 | gather orchestration | delegate facts/I/O; A2 tripwire350 |
| review facts.rs | 0 | 533–560 | 504 | symbols/usages/docs/pages | no diagnostics/workflow; A2 tripwire620 |
| review diagnostics/mod.rs | 0 | 200–225 | 203 | typed operation/report | reuse run I/O; ceiling280 unchanged |
| review diagnostics/process.rs | 0 | 230–270 | 276 | direct child/captures/snapshots | no descendants/read threads; selected ceiling300 |
| review diagnostics/command.rs | 0 | 200–235 | 217 | host command plan/environment | no arbitrary-text shell wrapper; explicit native Windows batch dispatch per spec; selected ceiling270 |
| core review/mod.rs | 0 | 140–160 | 145 | dialect/prefix; C8 retains one owner | no resolver; tripwire190 |
| binary crtool.rs | 0 | 65–100 | 66 | clap and output/exit mapping | no run implementation |
| binary main.rs | 304 | 312–320 | A2/current source; unchanged by B | early dispatch only | +20 maximum |
| core lib.rs | 19 | 20 | A1/current source; unchanged by B | export only | +2 maximum |
| bridge.rs | 417 | 433–442 | A1/current source; unchanged by B | resolved dialect carry/getter | +30 maximum |
| host_shell.rs | 478 | 486–492 | A1/current source; unchanged by B | exhaustive dialect projection | +15 maximum |
| root/leaf/binary Cargo manifests | existing | necessary safe capture/launch additions PENDING concrete design | N/A — metadata, not a production-region module count | retain leaf boundaries; exact necessary APIs/dependencies must be qualified before selection | no weakened lints, unsafe code, or unrelated runtime dependency |

Module-specific tripwires and ownership checks govern growth; they are not an aggregate commit-size gate. No shared acceptance gate is replaced.

Design.md’s selected B owner/budget decision owns the requester’s choice and current ceilings; this ledger adopts it without a second decision. Actual counts above are the authoritative restored diagnostics-phase C10 measurements for the R7RunInputs checkpoint, using design.md’s physical raw-file production-region convention, including the production/test separator rather than declaration-expanded `read` rendering. `review-decisions.md` and `evidence-R7RunInputs.json` own this repair's checked-source provenance and gates; `checkpoint-B.md` and `evidence-B.json` retain prior B results. Process exceeds its initial forecast but remains below its approved ceiling; forecasts are not squeezing/splitting/compaction targets. The aggregate waiver authorizes no module-ceiling change, and all other gates remain.


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

**Stress fixture:** retain assembled A2's language/Unicode, caps, binary/deleted/empty, scope, stamp, pages and precedence cases. Linux malformed-byte/replacement-look-alike collisions retain identical symbol names, exact cross-file usages, independent Git patches, raw document sizes and persisted facts rebuilding. Actual Unix admitted-Unicode colon/LF paths prove framing, same-symbol/own-definition/caller hits, distinct documents and persisted rebuild on Linux/macOS. Existing all-platform Unicode and raw metadata type/range/display-coherence malformed-manifest cases remain unchanged. Retain invalid ordinary config, nonexistent agent command, startup markers, repeated gather/facts and unsupported diagnostics verb. The full CLI harness remains intact; no waiver narrows it.

**A2 admitted-input correction:** the fixture-domain repair described in design.md, A2 admitted-input fixture correction, is implemented. Complete malformed-byte identity proof remains Linux-only; the actual Unix admitted-Unicode colon/LF same-symbol/document/rebuild consumer covers the admitted Linux/macOS domain, and existing all-platform Unicode/malformed-manifest cases remain unchanged. The owning checkpoints record Linux/Windows qualification and the historical pre-product APFS fixture failure; current-head macOS plus required CI remains pending. No production error is silenced and no native macOS behavior is dropped.


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

**Claim IDs:** C5, C6, C7; C1/C3/C10 retained/extended for the diagnostics consumer. The former F0 spool proof is historical; round7's replacement stream/launch premises require qualified safe APIs before implementation.

**Expected behavior:** public diagnostics preserves native executable identity/raw arguments, explicit native batch behavior, null stdin, lossless stdout/stderr streams, meaningful reports and typed clean/nonzero/timeout/cancelled outcomes. Cannot-start/parse/manifest failures write no reports; pre-cancel does not launch; live cancellation kills/reaps only its direct child. POSIX shlex/no-shell behavior remains; arbitrary command text is never shell-wrapped. No reader may strand on descendant-held EOF, no silent output cap is added, and no diagnostics CLI verb exists.

**Oracle:** C5 uses functional command/argv/environment behavior plus literal child receipts; C6 uses meaningful evidence/status/content and report limits; C7 uses OS process status and independently controlled handshakes/holder; deterministic deadline/first-observation cases are not fake real-time success.

**Stress fixture:** 131,073 bytes on each stream including invalid UTF-8; 201 changed-path matches and 16 tail lines; child-emitted CRLF, if present, remains raw capture data; hostile quoted argument shapes; failed exit and signal; zero/default timeout; cancel before launch/after ready/after exit; descendant still writing after direct child exits; spawn failure and cleanup errors. Expected partial snapshots contain only bytes before sampled lengths, and holder survives until fixture-owned release.

**Regression fence:** standalone `crates/cyril-review/tests/diagnostics.rs` uses per-host runnable fixtures and works without a prebuilt example; the explicitly built verification driver's native child/holder/sentinel modes serve `.cyril-s2hb/oracles/parity.py --phase diagnostics`. Public native qualification independently proves cancellation, direct-child reaping, live inherited holder/unrelated sentinel, no EOF wait, late holder writes after return, capture contents and cleanup. The observer uses the standard temporary-file collection decision in design.md and waits for the exact driver process, not inherited pipe EOF. A private production bounded-read test samples a real file, appends known late bytes, invokes the actual reader and asserts the exact sampled prefix. Private terminal-decision tests prove deadline/precedence. No new production seam/backend/clock hook/private filename convention is added. Checks assert functional statuses/content and lossless raw captures without process-name killing or shared cwd/environment mutation; checkpoint-B.md and evidence-B.json own qualification.

**Replacement capture ownership:** former mode0600/temporary-file fences qualify only the retired spool strategy. The stream design must qualify private handle ownership/inheritance, bounded terminal collection and closure on native hosts. Persistent artifact permissions and universal Windows ACL hardening remain non-goals; exact new mechanism fences are PENDING — design owner.


**Named mutation:** C5 splits Windows executable raw tail/removes nonempty CARGO_ value; C6 reverses stream order or uses a199 match cap; C7 omits direct-child kill/reads past snapshot length. The kill fault is attributed to the native lifecycle fence; removing the actual reader's take cap must compile and fail the deterministic private real-file boundary fence, not falsely qualify through a native fixture that releases its holder only after the operation returns. Inverting exit/cancel observation order is attributed to the private terminal-decision fence. Native controls prove holder/unrelated sentinel are live, so absence assertions cannot pass vacuously. This approved layered proof preserves every C7 obligation under delegated Gilfoyle verification decisions; it adds no production seam/backend/clock hook/private filename convention or acceptance waiver.

**Complexity/production scale:** preserve command/input-linear work, one directly owned child, no retry of selected-process failure and no stranded reader threads. Raw storage remains proportional to captured bytes without a whole-output cap. Filtering remains O(L×F), with200 matches/15 nonempty tail lines. The replacement must define a fair live-read quantum and finite terminal byte boundary before implementation; neither can become an unbounded drain that postpones cancellation. Retain the32 MiB dual-stream/100-path scale fixture. Exact safe pipe/native-launch complexity and measured growth are PENDING — affected design owner; no threshold change is authorized.

**Wall budget/phase:** one-off diagnostics operation; user/default timeout is the running deadline and direct-child termination/reap has the approved one-second decision deadline, not an arbitrary filesystem syscall latency bound. Narrow private deterministic terminal-decision tests enforce deadline/precedence decisions; real public native process fixtures independently establish actual reaping/cleanup, with outer timeout solely a hang guard. N/A — no separate latency SLA for filesystem output, no always-on background task.

**Module shape:** create diagnostics/{mod,process,command}.rs; lib gains only exports/error variants; run owns unchanged stamp/semantic JSON/native text implementation. No protected-parent production changes are required. `python3 .cyril-s2hb/oracles/check_shape.py --phase diagnostics` verifies final ledger/dependencies and growth; re-run affected C10 mutations if fence changes. Return to design before changing ownership or thresholds.

**Files:** create `crates/cyril-review/src/diagnostics/{mod,process,command}.rs` and `tests/diagnostics.rs`; extend leaf lib.rs and verification example; extend parity/shape oracle and native CI phase; update existing docs after actual diagnostics smoke. No crtool clap diagnostics variant. Required-CI repair17 additionally corrects the existing peer-close filter in `crates/cyril/tests/memory_runtime.rs`; no memory production change. `checkpoint-B.md` owns that bounded repair and its evidence disposition under the unchanged required-CI obligation.

**Estimate:** one substantive implementation/verification cycle, potentially several hours of native qualification; not a completion gate.

**Diff estimate:** N/A — no active aggregate commit-size limit or size-only fit claim; diagnostics production growth and module tripwires remain review context only.

**PR increment:** Atomic B diagnostics checkpoint inside the existing PR150; it is separately verified before final assembled PR review/CI.

**Commands and expected results:**
- `cargo test -p cyril-review --test diagnostics` → typed statuses, lossless raw captures, meaningful rendered content, no report on prelaunch errors, deadline/terminal precedence, owned lifecycle assertions.
- `cargo build -p cyril-review --examples` then `python3 .cyril-s2hb/oracles/parity.py --phase diagnostics --cyril target/debug/cyril --driver target/debug/examples/parity_driver` → functional status/content/record equivalence for identical clock/command/manifest inputs, lossless raw-capture checks, and separate actual-system-clock library smoke.
- Native Windows equivalent → raw executable command-tail argv semantics, explicit native `.bat`/`.cmd` batch parsing, Windows exit-code treatment, lossless child evidence, meaningful rendered content, cancellation and holder-cleanup observations. Completed native qualification belongs to `checkpoint-B.md` and `evidence-B.json`; its prior batch preflight is not a substitute for those results. Current-head macOS CI must execute its own host fixture.
- Each mutation's focused case → named red output; restore → green. Record source hash/environment and observed terminal values.
- `python3 .cyril-s2hb/oracles/check_shape.py --phase diagnostics`, `cargo fmt --check`, `cargo test`, `cargo clippy -- -D warnings` → final assembled gate passes; no source fence runs from production tests.
- Actual library consumer smoke, independent production reconstruction/ledger comparison, fresh full PR review and current-head CI must pass after both A2 and B checkpoints are assembled. Verify A1's merge plus the final complete PR150 merge and every acceptance criterion; close cyril-s2hb without changing parent cyril-5gb3.

## Self-review and handoff

Scoped self-review: A2 owns C1–C4/C9 and applicable C10; B owns C5–C7 and C1/C3/C10 extensions; unchanged A1 retains its evidence and narrowly scoped exceptions. Round7 selects thirteen repairs and rejects six scope/intentional behaviors. R7RunInputs closes F04/F17 with all eleven bounded gates and independent repair review passing; eleven selected repairs remain. Git/identity and capture/launch design selections remain pending. Old qualification, conformance and green baseline CI do not close those defects. Final repaired-source runtime/native/mutation/quality/conformance/review/CI, merge and issue acceptance remain required.

## Planning amendments, 2026-09-30

C8 Length review in design.md retains the cohesive prefix owner and raises its tripwire to190, with updated forecast140–160 (baseline0) and30-line uncertainty. No ownership/interface or protected-parent delta changes. EvidenceParity retains required coverage without compressing fixtures. Module/per-file tripwires remain independent; if any other size gate blocks progress, report measured size and options/tradeoffs and obtain the user's choice before restructuring, changing a gate, partitioning, or seeking another waiver.

## Functional-equivalence amendment — 2026-09-30

The requester’s verbatim reference-fidelity correction is owned by `spec.md`, Functional-equivalence amendment; this plan adopts that decision.

This supersedes active byte/text-wrapper parity requirements. A2 checks preserve semantic JSON fields/types and array order, file/symbol/usage sequence, meaningful content, stamps/refusals, exit/status/error/context, page budgets/caps, patch content, and native safety/lifecycle gates. Scope parsing/trimming uses Rust standard Unicode whitespace; generated text uses UTF-8 without a host-specific adapter or native LF-only gate. B uses functional command/status/content checks with lossless raw captures, not Python wrapper-byte parity. The C1 formatting-only manifest-key-order/newline mutant is replaced by manifest data corruption; no format-only mutation is required.

Pre-amendment byte comparisons, host-specific CRLF/CRCRLF checks and exact stdout/stderr wording remain historical evidence only. A2-C is historical PR150 opening context, not a restriction on final scope or implementation acceptance. Current Linux/Windows A2/B runtime, quality and mutation qualification is owned by the checkpoint/evidence records; the final assembled obligations named in Self-review and handoff remain pending. Publication and checkpoint sequence owns issue-wide authorization, current consumption and historical opening counters without reset. The aggregate exception removes only its named publication-size gate; no criterion, native/CI proof, safety obligation, fixture, cap or independently approved module ceiling is waived.

## Round7 repair sequence

1. **R7RunInputs, completed attempt18:** existing run owner validates facts metadata before side effects and parses JSON bytes strictly; existing facts/diagnostics consumers share that narrow helper. The compact record in review-decisions.md and evidence-R7RunInputs.json own actual red/green, compiled consumer smoke, Linux/native Windows matrices, quality and independent repair-review proof. No-recompute gather's symbols-only contract remains untouched.
2. **Git/facts/caller workspace:** after the repeated-product-repair design comparison, correct generated path identity, caller scope/root lookup, target option safety, parsed Git output policy, required status, modified declaration identity and the CLI's parsed cwd handoff. Main owns the integrated checkpoint and counter update; this plan does not predeclare success or hide extra attempts.
3. **Capture/native launch/stdin:** after safe API and native premise qualification, implement the requester's three decisions in a compatible concrete process boundary. Existing capture/command budgets remain; any measured new size trigger goes to the requester before size-driven changes.

Each atomic change retains independently checked publication and repair-diff review. Required evidence follows actual affected consumers, not file names; final assembly is requalified once all selected repairs are integrated. No counter reset, automatic cap extension, acceptance waiver or parent issue change is authorized.
