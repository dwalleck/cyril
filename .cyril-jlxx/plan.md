# Plan: cyril-jlxx

> Historical original record; see [archive provenance](evidence.md#archive-provenance-pr-116-review-repair). Later review fixes do not rewrite this approval or its original gates.

Approved design: design.md, requester words dated 2026-09-06. Empirical route; P1–P5 passed in evidence.md, no risk waivers. Worktree is /home/REDACTED_USER/repos/cyril-wt-feat-cyril-jlxx on feat/cyril-jlxx. All integrated artifacts/code/checks/commits run here, never the primary checkout. Isolated writers do not auto-apply to the primary; Main integrates their scoped patches into this worktree.

## Increment and arithmetic

One independently mergeable increment, `inspection-reviewer`: slices 1 and 2, ordered below. Slice 1 is independently green and its process/configuration behavior is checked before integrating slice 2. Projection: 550 + 2300 = 2850 changed lines, plus 30% churn margin (855) = 3705, below 4000. Margin covers private constructor migration and external executable fixtures. Actual cumulative diff must be checked at checkpoints; growth beyond 4000 requires independently mergeable increments, not a waiver. Workflow artifacts and retained native evidence are separately identified review material, not hidden production changes. Upstream was discovered as refs/remotes/origin/main in route.md; shape tooling discovers it independently.

## Shared implementation contract

Core exports `cyril_core::types::SpawnEnvironment` with `Inherit` default and `Replace(BTreeMap<OsString, OsString>)`; values are redacted in Debug. SpawnConfig gains `environment: SpawnEnvironment` and `required_cli_version: Option<String>`. Existing public spawn_bridge and five-part split remain unchanged. Environment is applied to the existing process and existing version probe; KAS settings are captured from that environment into KasEngine. Native wrapper model selection remains SetMode of an on-disk profile declaring model=claude-sonnet-4.6, followed by exact streamed current-value confirmation before prompt. Catalog inclusion without matching current value is not confirmation.

Workbenches use only these public core types/bridge APIs. Workbench owner selects its own concrete public input/status/result API within the approved start/subscribe/cancel/finish shape. No raw bridge commands exposed publicly. Main owns integration, validation, oracle enhancements, live acceptance and documentation.

## Module growth ledger

Counts are non-test physical prefixes; inline cfg(test) creates conservative differences described in design. Bounds are responsibility drift tripwires, not depth evidence.

| Module | Baseline | Projected final | Responsibility/interface change | Protected rule |
|---|---:|---:|---|---|
| Cargo.toml | 87 | 88–100 | Add member/dependencies only | No runtime bodies |
| crates/cyril-core/src/types/mod.rs | 87 | 89–92 | Declare/export environment | Declarations only |
| crates/cyril-core/src/types/spawn_environment.rs | 0 | 55–150 | Owned redacted process configuration | No review concepts |
| crates/cyril-core/src/protocol/bridge.rs | 366 | 375–420 | Config fields + selected snapshot/probe/spawn wiring | No new policy/lifecycle bodies |
| crates/cyril-core/src/protocol/transport.rs | 332 | 333–345 | Existing child applies selected environment | Existing process owner only |
| crates/cyril-core/src/protocol/engine.rs | 332 | 335–370 | KasEngine captured settings | Snapshot and wiring only |
| crates/cyril-core/src/protocol/kas/settings.rs | 180 | 180–230 | Select effective native settings source | Existing mapper/default semantics |
| crates/cyril-core/src/protocol/kas/version.rs | 80 | 90–150 | Existing probe receives env and exact requirement | No second probe |
| crates/cyril-core/src/protocol/kas/discovery.rs | 339 | 339–345 | Private version call migration if needed | No new discovery policy |
| crates/cyril/src/main.rs | 287 | 289–295 | Explicit default config fields | Wiring only |
| crates/cyril-core/src/protocol/domain_mediator/mod.rs | ~701 incl inline tests | unchanged production | Test literals only if needed | No production delta |
| crates/cyril-core/src/protocol/sdk_runtime/** | existing | unchanged production | Private constructor test callers | No production delta |
| crates/cyril-core/examples/**; crates/cyril-core/tests/** | existing | unchanged except constructor config | Atomic public caller migration | No new production responsibility |
| crates/cyril-workbench/Cargo.toml | 0 | 20–40 | Workspace inherited dependencies/lints | No direct ACP dependency |
| crates/cyril-workbench/src/lib.rs | 0 | 2–12 | Public module declaration | No body |
| crates/cyril-workbench/src/reviewer.rs | 0 | 250–500 | Concrete backend operation/lifecycle | Only review orchestration |
| crates/cyril-workbench/src/reviewer/evidence.rs | 0 | 100–220 | Private evidence staging/lifetime | No runtime/model logic |
| crates/cyril-workbench/src/reviewer/runtime.rs | 0 | 130–270 | Fixed native configuration/env recipe | No process owner or custom evaluator |
| crates/cyril-workbench/src/reviewer/types.rs | 0 | 120–270 | Invariant-bearing input/status/outcome/error | No effect execution |
| crates/cyril-workbench/tests/** | 0 | test-only | External executable behavior/side effects | No production helper exports |

Before source changes are integrated, Main extends shape.py with baseline production snapshots, protected-body change allowlists, growth ranges and changed-path checks. Permit only existing bridge engine_for/resolve_spawn_command/run_bridge wiring, Engine/KasEngine snapshot access, declaration-only type/main additions. Other mediator/SDK/TUI production bodies must remain unchanged. New renamed review functions in protected parents must be detected through new function declarations/diff, not vocabulary alone. Include all dependency sections (including dev/target) and mutation-check both misplaced-body and reverse-dependency arms. Complete command: `python3 .cyril-jlxx/oracles/shape.py --complete --mutation-check` → C7 PASS, localized red controls.

## Slice 1: Isolate existing core launches and KAS settings

**Claim IDs:** C2.
**Expected behavior:** Two selected launch environments affect only their own process/version probe/handshake settings; Inherit callers retain behavior; exact CLI requirement fails before session work on mismatch.
**Oracle:** External executable records received environment/configuration; parent probes compare their original values. Existing working configured hook/MCP controls from evidence.md are retained for final backend integration.
**Stress fixture:** Concurrent roots with differing cli.json flags; Unicode/spaces executable/cwd; parent authority variable omitted from replacement; absent/malformed settings; empty selected environment. Expect no inherited canary or cross-config contamination, actionable failures, no process-global mutation.
**Regression fence:** Colocated types/spawn_environment.rs tests plus public bridge/executable integration under crates/cyril-core/tests; existing transport current_runtime_contract tests exercise child environment. Names: isolated_spawn_does_not_inherit_parent_authority; concurrent_settings_are_run_local. Version mismatch and environment behavior use native runnable fixtures, cfg(unix) for POSIX execution.
**Named mutation:** Remove env_clear in types/spawn_environment.rs → parent canary leaks and C2 fails; restore → green. Change selected KIRO_HOME lookup to ambient in kas/settings.rs → two-run config assertion fails; restore → green.
**Complexity/production scale:** Environment construction/application O(E log E) through ordered maps, E<=128 variables/64 KiB typical approved map; KAS config snapshot O(J), fixture 256 KiB. Max local preparation/probe-fixture wall cost 1 second excluding compiler, and each selected config read once per launch; rationale: no per-notification filesystem reads. Production actual CLI version startup measured separately, within review readiness deadline.
**Wall budget/phase:** One-off per launch; no always-on work added. N/A — one-off phase; no per-tick wall budget.
**Module shape:** Existing core process ownership deepened, env intrinsic seam added; protected bridge +54 maximum, engine +38, types mod +5, main +8; no mediator/SDK production delta. `python3 .cyril-jlxx/oracles/shape.py --mutation-check` → C7 placement/growth PASS without requiring slice-2 owners.
**Files:** Core paths in growth ledger; current_runtime_contract.rs and sdk_runtime/tests/process.rs constructor migrations, all discovered SpawnConfig/KasEngine literals (LSP references mandatory). No workbench files in this slice.
**Estimate:** One bounded core change set; estimate is a decomposition signal only.
**Diff estimate:** 550 implementation/tests/caller lines.
**PR increment:** inspection-reviewer.
**Commands and expected results:**
- `cargo test -p cyril-core isolated_spawn_does_not_inherit_parent_authority` → replacement child omits parent canary; inherited control sees it.
- `cargo test -p cyril-core concurrent_settings_are_run_local` → separate sessions receive exactly their selected setting values; no global mutation.
- `cargo test -p cyril-core` and `cargo test` → existing contracts preserved.
- `cargo clippy -- -D warnings` and `cargo fmt --check` → unchanged safety rails, no warnings/format drift.
- Both named mutations → corresponding behavioral failure; restored runs pass. Record actual test identifiers if implementation uses more descriptive names, without weakening observable assertions.

## Slice 2: Deliver and prove the concrete inspection reviewer

**Claim IDs:** C1, C3, C4, C5, C6, C7.
**Expected behavior:** Public backend operation stages complete evidence, launches one isolated native KAS run, confirms model/mode, cancels every permission request, streams bounded text into typed lifecycle, and awaits process teardown. Native configured read-only acceptance and clean-root inherited configuration checks exercise this operation, not just the old probe.
**Oracle:** External executable journals actual received prompts/permission responses/exit; independent file bytes/markers and /proc lifecycle; native model catalog and passive tool inventory; approved shape ledger vs source/diff.
**Stress fixture:** Hostile/duplicate/Unicode labels, empty evidence/text, exact/over limits, late/missing/wrong/duplicate/drifting model config, foreign permission sources, cancelled/closed responder, pending terminal/cancel race, stalled notification, blocked observer, dropped run, output flood and two concurrent identities. Empty document collection/label errors precede spawn; empty text accepted; other failed input/phase paths report typed unavailable/incomplete rather than Completed.
**Regression fence:** crates/cyril-workbench/tests/reviewer.rs (or dedicated behavior-focused test modules in that directory) with controlled runnable executable fixture. Required behaviors/names: model_mismatch_never_starts_inspection, late_configuration_starts_once, evidence_labels_cannot_escape_staging, unauthorized_read_is_denied, all_permission_sources_are_cancelled, closed_permission_responder_is_visible, stalled_turn_stays_active, cancel_never_completes_review, fresh_runs_do_not_share_state, blocked_observer_does_not_block_cancel, incomplete_staging_never_spawns, output_limit_is_incomplete. Native policy acceptance uses real KAS and independent control; never substitutes mocks for native policy claims. Issue-local shape oracle is not a behavioral test.
**Named mutation:** C1 remove readiness model equality; C3 remove global outside-read deny; C4 replace Cancel with AllowOnce; C5 interpret stalled as completed and separately omit teardown; C6 await blocked delivery in hot drain or turn output-limit failure into Completed; C7 move review entry body to transport/add workbench dependency to core. Each produces claim-local red; restore and rerun green.
**Complexity/production scale:** Staging O(N+B), N<=1024, B<=64 MiB default; no source-label path lookup, no symlink traversal. Measure 1024 documents/64 MiB: staging <=5 seconds local disk and no event-loop blocking (spawn_blocking). Output O(T), <=8 MiB default, amortized append without full-copy per event. Watch/status O(1) per notification, no unbounded channels. Under a 64 KiB chunk stream at output cap, cancel/permission serviced <=1 second on local deterministic executable excluding native shutdown; rationale responsive desktop control independent of renderer. Checked arithmetic; overruns explicit incomplete/error. Higher explicitly configured bounds checked using same arithmetic.
**Wall budget/phase:** Staging/config creation/teardown are one-off; active drain is always-on, per-chunk synchronous CPU <=10 ms for a <=64 KiB fixture chunk and end-to-end cancellation dispatch <=1 second under blocked observer. Native model inference has no throughput or silence-success deadline; startup <=30 seconds default. Existing bounded core teardown deadline retained.
**Module shape:** Approved concrete workbench owners only; no policy body in core or TUI, no copied runtime. Numeric projections in growth ledger. `python3 .cyril-jlxx/oracles/shape.py --complete --mutation-check` → all approved owners present, protected body allowlists/growth/dependency checks PASS; both mutation arms red. Final fresh-context reviewer reconstructs responsibilities before seeing approved ledger.
**Files:** Cargo.toml/Cargo.lock; crates/cyril-workbench/Cargo.toml, src/lib.rs, src/reviewer.rs, src/reviewer/{types,evidence,runtime}.rs; tests/reviewer.rs and tests/fixtures runtime executable; issue-local native smoke/oracle artifacts. Documentation/changelog update after smoke proof per cleanup workflow, atomically with behavior if committed.
**Estimate:** One cohesive backend operation plus external boundary proof; estimate is a decomposition signal only.
**Diff estimate:** 2300 implementation/tests/fixture lines.
**PR increment:** inspection-reviewer.
**Commands and expected results:**
- `cargo test -p cyril-workbench` → behavioral cases above through start/subscribe/cancel/finish; malformed/wrong/closed states never falsely complete.
- `cargo run -p cyril-workbench --example review_smoke` (throwaway example, removed after evidence capture) → actual Sonnet-bound authorized read plus outside denial, completed typed outcome and native teardown. Exact invocation/options recorded once the approved API is implemented.
- Native operation comparisons repeat evidence.md fixture/control recipes through Reviewer::start: authorized bytes returned, unauthorized bytes absent with native failed call, mutation bytes unchanged, no shell/hook/MCP start markers; permissive synthetic controls known working. Two simultaneous Reviewer consumers receive only their random identity and leave no observed descendants. No real provider/publication operation.
- Stress executable observes dispatch timing/limits independently, meets bounds above. Named mutations fail corresponding fences then restored green. C3 native mutation must reveal outside canary; a model refusal is inconclusive and retried with the known-working operation prompt.
- `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` → workspace green with safety rails unchanged.
- Fresh-context reviewer reconstruction then ledger comparison → no unresolved placement mismatch.

## Platform and taxonomy

Native Windows behavior is UNVERIFIED on this Linux workstation; separate verified cyril-6y1s owns executed native gate. POSIX process fixtures are cfg(unix); Windows branches follow existing idioms and CI, not a local sqlite-cross-build claim. Separate ownership/non-goals remain exactly the approved design; no new deferred feature or risk waiver.

## Self-review

Each design claim has exactly one owning slice (C2 in 1; C1/C3–C7 in 2). Slice 1 verification requires no future workbench code. Slice 2 delivers the whole concrete operation; no uncallable scaffold. Every slice has all fourteen fields, fences/mutations, cost bounds and module projections. Complete shape proof and behavioral/mutation/native gates precede completion. Projection arithmetic includes churn. No slice is declared complete by this plan; checkpointed-build owns that decision.

## Review re-entry (2026-09-07)

Input: all 54 records in `review-decisions.md`, assessed against `1136dabd`. Original slices/approval remain historical records. These slices repair their committed implementation facets; no new architecture or risk waiver is selected. Integration uses the linked `pr-116` worktree, not the primary checkout.

Partition: archive PR #117 retains 3,454 historical-proof/roadmap lines (3,754 with margin). PR #118 is the independently verified generic core increment, with no workbench member/dependency. It also owns the 512 lines of generic configuration, initialize-drop and executable-fixture control artifacts; project 2,300 lines including that proof. The assembled reviewer delta was 4,219 lines before this ownership correction; moving those core artifacts into #118 leaves 3,707 reviewer/current-native-proof lines, with a 293-line margin. PR #116 descends from #118. Recompute actual deltas before publishing; no increment may exceed 4,000. No evidence is removed or relabeled as a newer execution.

Measured publication checkpoint (APlan): archive `44bcf1ec` to core `62437d73` is 2,012 insertions + 90 deletions = 2,102 lines across 35 files; core `62437d73` to reviewer `a1ac53c7` is 3,705 insertions across 32 files. These immutable revision-pair measurements are distinct from the projections above. The APython fixture/proof repair at core `7477df72` measures 2,080 insertions + 90 deletions = 2,170 core lines across 36 files. After integration and advisory records, the reviewer delta is 3,729 changed lines across 33 files, including this measurement, leaving 271 lines beneath the 4,000-line cap. No production path changed during this advisory reconciliation.

Additional bounded owners: `types/event.rs` 650→680 (command/response declarations); `commands/mod.rs` 182→190 (one dispatch arm); `commands/session.rs` 435→550 (standard config RPC and required-choice loss validation only); core `session.rs` 373→380 and UI `state.rs` 2481→2490 (existing configuration arms only). Reviewer projection becomes 250–540 for response-qualified readiness and typed failures; other workbench projections remain unchanged. Add these exact body/path allowances to the shape oracle; do not remove existing MAX_PREFIX entries or permit unrelated protected bodies.

V12 adds only lifetime wiring to bridge's existing handles/factory (404→440 prefix limit) and an initialization-vs-final-client-closure select in `DomainMediator::run` (703→735 whole-file limit; the early cfg(test) variant makes a prefix-only count unsuitable). Preserve every old allowance. Add a separately measured total-file cap for this one owner and allow no other mediator-root body change. Core source partition is independent of the workbench: caller loss cancels an already observable public bridge operation without a reviewer consumer.

### Slice 3: Correct bounded startup and isolated feature coverage (V5, V8, V19, V21, S3)

**Claim IDs:** C2, C6; repairs to original slice 1.
**Expected behavior:** Replace+Host is rejected before discovery/probe/agent start; timeout retains already-read bounded diagnostics; nested isolation checks prove their inner body ran; non-KAS core remains independently tested.
**Oracle:** External process environment/initialize journals, pre-timeout diagnostic canary, parent-observed completion files and explicit core-only build selection.
**Stress fixture:** Both version pipes exceed 1 MiB, descendant retains a pipe, hanging peer emits diagnostic then stalls; Inherit+Host positive control uses a private HOME. Existing three-second probe plus bounded reap remains.
**Regression fence:** `crates/cyril-core/tests/spawn_isolation.rs`; preserve verbose/inherited-pipe/Free rejection fences and strengthen timeout/nested execution checks.
**Named mutation:** Remove Host rejection → probe/agent marker appears; discard timeout stderr → diagnostic canary missing; stale inner --exact filter → outer completion-file assertion fails. Run host-hook negative controls with private HOME, never the user's registry.
**Complexity/production scale:** Two concurrent bounded captures, at most 1 MiB each; draining is O(total output), never retained beyond caps. Each hanging/held-pipe case must finish within ten seconds including existing teardown.
**Wall budget/phase:** One-off startup; N/A — no always-on work.
**Module shape:** Existing bridge validation/version/environment owners only; existing body allowances and maxima stay sufficient. Shape oracle PASS.
**Files:** `.github/workflows/ci.yml`; core `protocol/bridge.rs`, `protocol/kas/version.rs`, `types/spawn_environment.rs`, `tests/spawn_isolation.rs`.
**Estimate:** One atomic startup/feature-selection correction.
**Diff estimate:** 140 lines including controlled-environment test adjustments.
**PR increment:** #118 generic core prerequisite, based on #117.
**Commands and expected results:** `cargo test -p cyril-core --features kas --test spawn_isolation` → explicit isolation/diagnostic/deadline observations; `cargo test -p cyril-core --no-default-features --test spawn_isolation` and core-only clippy → no hidden KAS unification; named mutations red, restored fences green; workspace tests/clippy after this logical set.

### Slice 4: Restore review readiness, diagnostics and evidence ownership (S1, S4, S5, V10, V12, V13, V14, V22, C8, C10, C14, C15, C17a, C7)

**Claim IDs:** C1, C2, C4, C5, C6, C7; repairs to original slice 2.
**Expected behavior:** No evidence prompt before response-confirmed disabled collection and post-profile model confirmation; late disconnect/error cannot erase authoritative completion; failures retain private bounded diagnostics; failed native tools are observed without inventing policy verdicts; executor destruction cannot delete a live child's evidence.
**Oracle:** Peer's ordered wire/prompt journal, independent direct/descendant process and filesystem observations, captured tracing output, and live pinned-KAS configuration/allowed-read/denied-read results.
**Stress fixture:** Wrong-mode model followed by mode-only switch; unsolicited disabled update before rejected/coerced response; missing/malformed configuration response; privacy drift; delayed terminal/disconnect; dropped/unpolled driver; 4096-byte UTF-8 diagnostic boundary and secret canaries; original 1024-document/64 MiB staging and 8 MiB output bounds.
**Regression fence:** Workbench public-operation peer tests plus focused private diagnostic/cleanup/logging boundary tests; existing core command-contract inventory and generic configuration consumers migrate atomically.
**Named mutation:** Remove response readiness guard → early prompt journaled; retain wrong-profile model → early prompt; restore late-error demotion → complete becomes incomplete; restore raw tool logging → private canary logged; remove cleanup guard → evidence disappears before completion; expose diagnostic Debug → canary leaks. Uncertain response decoding must fail closed under the SDK's tolerant-response mutation.
**Complexity/production scale:** Configuration is O(options + choices), without copying whole catalogs per notification; diagnostics retain at most 4096 bytes each; status O(1). Original C6 staging/drain bounds remain. Cleanup adds one OS waiter only when the caller executor drops ownership; no retry/sweep service.
**Wall budget/phase:** Initial native configuration is one-off under existing startup deadline; active drain remains <=10 ms synchronous work per <=64 KiB fixture chunk and <=1 second cancellation dispatch. No inference timeout or new successful-silence rule.
**Module shape:** Standard configuration stays in existing core command owners; generic cached-model consumers only add response handling. Review policy/diagnostics/evidence lifecycle stay in workbench. Bridge handles/factory retain a closure-only client-lifetime guard; only the mediator root's existing run body may observe that guard during initialization and call existing teardown. No process/SDK implementation delta or new runtime.
**Files:** core `protocol/bridge.rs`, `protocol/domain_mediator/mod.rs`, `tests/{spawn_isolation,session_configuration}.rs`, `types/event.rs`, `protocol/domain_mediator/commands/{mod,session}.rs`, command-contract test inventory, `session.rs`; UI `state.rs`; bridge example; workbench `reviewer.rs`, `reviewer/{types,evidence,runtime}.rs`, peer fixtures/tests and smoke-example removal after capture; shape/design/plan/decision records.
**Estimate:** One complete backend-operation correction with its prerequisite standard command.
**Diff estimate:** Agent reviewer patch is 766 insertions/108 deletions before response-qualified readiness adjustments; these are existing additions in #116, so the final workbench increment uses their resulting file sizes, not cumulative edit churn. Generic core/configuration/lifetime changes belong to the independently verified core increment; respect the revised per-increment totals above.
**PR increment:** `core-review-runtime` for generic core corrections, then #116 for the complete reviewer operation; both descend from #117.
**Commands and expected results:** `cargo test -p cyril-workbench` → readiness/terminal/cleanup/privacy observations; targeted generic-consumer and command-contract cases → returned catalog applied, malformed responses rejected; actual `review_smoke` against CLI 2.21.1 → disabled collection, Sonnet 4.6, allowed synthetic evidence, refused outside read and observed teardown; named mutations red/restored green; workspace tests, clippy, fmt and complete shape oracle PASS.

V12 additional oracle/fence: the peer independently attests initialize receipt, remains open while a sender clone is retained, and writes a natural-exit marker before its finite safety exit. Dropping the last clone must yield core completion via existing teardown before that marker exists. Removing lifetime observation must turn the fence red. Baseline public probe: completion=false at three seconds, natural exit=true at 6.9947 seconds; do not treat awaiting completion as proof of timely cancellation.

Final-review grouped-choice fence: the SDK's nested group decoder also drops malformed required choices. Decode the catalog once from a borrowed raw value, then compare each decoded group's choice count with its raw array before acknowledging. Valid grouped catalogs remain supported; malformed flat current values and malformed grouped choices must both produce the typed operation failure. Optional presentation metadata retains SDK extensibility. No new schema clone, helper owner or duplicate catalog allocation.

### Slice 5: Correct native Windows path/auth construction (V15, V9)

**Claim IDs:** C1, C2, C3; platform-bound construction correction, not native acceptance.
**Expected behavior:** Ordinary canonical Windows paths no longer fail solely on a verbatim prefix; semantically necessary extended paths remain fail-closed. A requested auth parent inconsistent with the native OS Known Folder is rejected rather than silently ignored.
**Oracle:** `dunce` compatible canonicalization and `dirs::data_local_dir` use the actual native path/Known Folder APIs; Windows CI constructs the public Reviewer with a temporary executable/runtime and verifies an alternate auth directory is rejected.
**Stress fixture:** Spaces/Unicode runtime parents, ordinary canonical drive paths, alternate native-auth parent; Unix glob-escape paths remain rejected. No credentials are opened or copied.
**Regression fence:** Platform-qualified constructor tests under `reviewer/runtime.rs`; Windows CI executes Windows branches. The Linux machine does not claim native Windows KAS/process acceptance.
**Named mutation:** Restore raw std canonicalization → Windows valid-construction case fails; remove Known Folder equality guard → alternate-auth case succeeds incorrectly. The same Windows CI matrix carries these platform-specific controls.
**Complexity/production scale:** O(path length) compatible normalization plus a fixed number of canonicalization/Known Folder calls; no new event-loop or per-notification IO.
**Wall budget/phase:** One-off constructor; N/A — no always-on phase.
**Module shape:** Runtime configuration owner only; centrally declared `dunce` and Windows-only `dirs` dependencies provide safe OS wrappers, not a copied platform subsystem.
**Files:** root Cargo manifests/lock; workbench manifest, `reviewer/runtime.rs`, auth-parent field documentation in `reviewer/types.rs`; `.github/workflows/ci.yml` and `.cyril-jlxx/oracles/windows-construction.py` execute the two native-platform mutation controls and restore the source even on failure.
**Estimate:** One bounded native-construction correction.
**Diff estimate:** 180 lines including Windows tests, mutation instrument/CI invocation and lockfile updates.
**PR increment:** #116, based on #117.
**Commands and expected results:** Linux constructor/behavior checks remain green; Windows CI valid-construction/alternate-auth cases distinguish the fixed and mutated behavior; full CI matrix green. Live Windows enforcement/descendant acceptance remains cyril-6y1s/cyril-jlw9, not asserted by this slice.

Review-plan check: all accepted behavior changes have an owning slice and explicit oracle/fence; generic command plus every consumer is atomic. Historical records stay unchanged above. Unverified findings are rejected with named existing acceptance ownership, not smuggled into new behavior. Final integration and cleanup follow the exercised surfaces; historical gates do not substitute for current proof.
