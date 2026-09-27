# Plan: cyril-y628

Inputs: route.md Structural; spec.md approved 2026-09-26; design.md approved 2026-09-26. All C1–C7 have independent oracles and named mutations; C1 cheapest falsifier passed before approval. No FAIL exists. No approval is inferred for publication or closing the ticket.

## PR increment and arithmetic

One increment: rejection-feedback, one end-to-end atomic slice. Default branch discovered by `git symbolic-ref refs/remotes/origin/HEAD` = origin/main; pinned source base 83b1ee65, production merge cede8f5f. A single increment is independently mergeable when the full feature and all callsite migrations/gates pass against that base. Source work is partitioned among isolated writers, not into partially implemented shipping slices.

Initial projection: implementation 550 + behavioral tests/fixtures 850 + issue-local gates/smoke 450 + workflow/docs 600 = 2,450, plus 750 churn margin = 3,200. Qualification-time plan correction: retained wire/terminal/mutation evidence and independent review/repair receipts take the forecast to 3,200 + the same 750 margin = 3,950 ≤ 4,000. Observed before final review closure: 3,741 changed lines in 51 files. This is the same atomic slice, not an ownership or acceptance change. The final census includes every new tracked candidate and documentation; over 4,000 still requires repartition before commit.

## Module growth ledger

Physical pre-test lines are signals only; declarations/comments included. Measurement convention and baseline are in design.md.

| Module | Baseline | Projected final | Responsibility / interface delta | Protected-parent rule |
|---|---:|---:|---|---|
| core/src/types/event.rs | 705 | 713–725 | Request capability and distinct rejection-feedback response variant | Data only |
| core/src/protocol/convert/mod.rs | 491 | 510–531 | Engine/variant dispatch; exact-ID behavior retained | No KAS metadata literals |
| core/src/protocol/convert/kas.rs | 582 | 605–635 | Exact eligibility and response-level metadata helper | KAS wire owner |
| core/src/protocol/domain_mediator/mod.rs | 760 | 763–772 | Capability and engine forwarding | Wiring only |
| ui/src/feedback_editor.rs | 0 | 160–300 | Concrete bounded editor and read-only value; same-owner correction from 240 accommodates CR normalization/control filtering and Unicode cursor operations | No ACP/queue/persistence |
| ui/src/traits.rs | 781 | 800–830 | Phase-owned editor, capability, derived eligibility | No editing algorithm |
| ui/src/state.rs | 2853 | 2940–2983 | Approval transition and editor delegation | No copied text algorithm |
| ui/src/widgets/input.rs | 138 | 145–160 | Shared concrete text/cursor drawing | Same chat behavior |
| ui/src/widgets/approval.rs | 325 | 390–480 | Feedback view/hint/notice, existing modal placement; same-owner correction from 445 accommodates cramped viewport and notice placement | Rendering only |
| ui/src/lib.rs | 22 | 23 | Private editor declaration | No implementation |
| cyril/src/app.rs | 2901 | 2920–2936 | Approval key/paste routing | No editor/wire policy |

Prefixes core/ui mean crates/cyril-core and crates/cyril-ui. Tests, fixtures, examples, and documentation are not production growth. Same-owner numerical overrun requires plan review; new ownership/interface changes require design reapproval.

## Slice 1: Complete KAS rejection feedback through the existing approval channel

**Claim IDs:** C1, C2, C3, C4, C5, C6, C7.

**Expected behavior:** The approved optional KAS action captures multiline feedback, preserves plain decisions, sends the exact originating reject_once ID and response-level reason once, and cannot leak text or decisions across the FIFO or into chat. All spec.md exclusions hold.

**Oracle:** C1 captured qo13 JSON; C2 captured KAS response dictionary plus raw SDK/PTY response; C3 explicit spec decision table; C4 independent character-list edit expectations and approved-bound fixture; C5 explicit two-request transcript; C6 terminal cells/raw wire and existing independent wrapping fixture; C7 module ledger/source census. No production helper is its own expected-value generator.

**Stress fixture:** Two sessions with equal tool IDs and different reject IDs while the first editor contains Unicode, quotes, leading/trailing spaces and line breaks; only the first responder receives the exact text. 4,096 scalar draft (including multibyte chars) succeeds; next typed scalar/newline and oversized paste leave draft/cursor unchanged with notice. Empty/whitespace resolves a plain rejection. Every option kind and both engine kinds are covered, with positive eligible controls. Cramped viewport and literal cursor-block text distinguish wrapping and cursor placement. Dropped response receiver must not redirect the answer.

**Regression fence:** Existing from_permission_response_* and qo13 behavioral replay; new rejection_feedback-named cases in converter/KAS and bridge harness; editor tests in feedback_editor.rs; state lifecycle tests; App modal/paste tests; approval render tests; standalone .cyril-y628/oracles/shape.py. Tests are written in this slice. No source-text behavioral tests; shape inspection remains standalone.

**Named mutation:** Each design.md C1–C7 named mutation, one at a time, on production restored byte-for-byte afterward; mutant must fail the corresponding behavior/ownership assertion, not compilation. Apply separate C2 omission/kind-guard, C4 overflow/lower/raise, C5 wrong-queue/Esc, and C6 paste/render mutations. .cyril-y628/mutate.py records per-mutation command, observed red assertion, and restored green against the candidate state. Existing files modified by concurrent actors are never restored from HEAD.

**Complexity/production scale:** Feedback insert validation O(n+m), bounded stored draft n≤4,096 scalars (≤16,384 UTF-8 bytes). A paste scan stops/refuses once normalized accepted scalar count exceeds remaining capacity, so no copied unbounded candidate is required. Cursor edit O(n) over bounded draft. Rendering reuses existing O(n) wrapping with bounded feedback text; metadata eligibility O(k) over the existing offered-options list, no new list expansion. Maximum accepted incremental work: bounded 4,097 accepted scalar visits for insertion admission plus one bounded draft splice; no per-tick parsing/serialization. Large raw control-only paste still requires O(raw bytes) filtering, same event-ingestion population as existing paste; do not clone it. Budget smoke measures 10,000 maximum-draft editing/render operations in ≤10 seconds on this workstation (generous non-timing-unit-test upper bound), with no competing Cargo work. This is not a claimed network/terminal latency SLA.

**Wall budget/phase:** Key/paste/decision are discrete one-off events: N/A — one-off phase, no continuous wall budget. Feedback render is always-on while visible: bounded 4,096 scalars, budget ≤1 ms mean per draw under the 10,000-draw controlled smoke, to remain below the existing 50 ms frame cadence. Measure in a throwaway optimized rendering smoke if test-profile measurement is close; no permanent timing-flaky test.

**Module shape:** Owners/interfaces exactly as approved design. App ≤35, generic converter ≤40, mediator ≤12, UiState ≤130 projected added production lines; other ledger ranges above. `python3 .cyril-y628/oracles/shape.py` must print C7 PASS; the forbidden App-policy mutation must print C7 with path/symbol and exit nonzero. No ownership deviations may be explained away as numeric drift.

**Files:**
- Core production: crates/cyril-core/src/types/event.rs; crates/cyril-core/src/protocol/convert/mod.rs; crates/cyril-core/src/protocol/convert/kas.rs; crates/cyril-core/src/protocol/domain_mediator/mod.rs.
- Core tests/callers: crates/cyril-core/src/protocol/convert/probe_qo13.rs; crates/cyril-core/src/protocol/bridge/tests/harness.rs; crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/mod.rs and new rejection_feedback.rs; crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/saturation.rs; exhaustive PermissionResponse tests in event.rs. Other core callers migrate only if the compiler/reference inventory identifies a constructor affected by the new request field/variant.
- UI production: crates/cyril-ui/src/feedback_editor.rs (new), lib.rs, traits.rs, state.rs, widgets/input.rs, widgets/approval.rs.
- UI/App fixtures and routing: crates/cyril/src/app.rs; crates/cyril-ui/src/floor_tests.rs; crates/cyril-ui/src/render.rs (fixtures only unless existing renderer requires phase wiring); crates/cyril-ui/src/widgets/toolbar.rs (fixtures only); crates/cyril-ui/tests/modal_theme.rs.
- Evidence: .cyril-y628/oracles/shape.py; .cyril-y628/mutate.py; .cyril-y628/smoke.py (controlled ACP/PTY evidence harness), receipts in plan.md; fixture for approved scalar bound if required by editor fence. Existing tracked user-facing docs selected after runtime proof; no unrelated docs.

**Estimate:** One atomic feature; several focused implementation/verification passes. Estimate is a decomposition signal, not permission to skip gates.

**Diff estimate:** Qualification-adjusted 3,200 changed lines plus 750 churn margin = 3,950; initial 3,200 total estimate retained above for provenance.

**PR increment:** rejection-feedback.

**Commands and expected results:** All Cargo commands use `env CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target` because inherited empty value prevents Cargo startup.
- `cargo test -p cyril-core from_permission_response_` → exact legacy selected IDs/trust/cancel/warning behavior unchanged (C1).
- `cargo test -p cyril-core --features kas rejection_feedback` → exact eligible top-level KAS reason and absence for ineligible matrix; SDK response agrees with captured contract (C2).
- `cargo test -p cyril-ui rejection_feedback` and `cargo test -p cyril-ui feedback_editor` → opt-in matrix, exact editing, bound, Esc, queue/responder attribution and render expectations (C3–C6).
- `cargo test -p cyril --features kas rejection_feedback` → real App event routes preserve draft/modal/batch/voice guards (C6).
- `python3 .cyril-y628/oracles/shape.py` → C7 PASS and localized diagnostics under mutation.
- `python3 .cyril-y628/mutate.py` → each listed named mutation produces assertion-level red, exact restoration, and green. Runner added after symbols settle; mechanism-only corrections record exact selectors here without changing oracle meaning.
- `cargo test` and `cargo test --features kas` → workspace consumers and existing behavior pass after the atomic change.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy --all-targets --features kas -- -D warnings` → no warnings and all callsites compile.
- `cargo fmt --check` → formatting matches repository convention.
- `cargo build -p cyril --features kas` then `python3 .cyril-y628/smoke.py /home/dwalleck/repos/cyril/target/debug/cyril` → actual TUI actions and raw controlled-agent response prove approved workflow; captures verify ordinary/cramped presentation, no chat leakage, and displayed limit/notice. Controlled agent is evidence infrastructure, never a production fallback.

## Self-review

Every claim assigned exactly once; all fourteen slice fields filled. Existing/new behavior fences ship with source; no waived fences. Input population, per-loop work and bounded rendering budget recorded; no cross-platform claim beyond existing terminal abstraction. Every touched production owner is in the ledger, mechanical-only fixture migrations are identified. Partition includes artifacts and churn. All exclusions use design.md taxonomy; no new deferred obligation. Slice completion is not claimed here: checkpointed-build owns its result.

## Approved newline amendment

2026-09-26: requester selected “Use Ctrl+J reliably”; spec.md and design.md record approval. Slice C4/C6 includes Ctrl+J/raw LF as the reliable newline key, retains explicitly reported Shift+Enter, and does not change startup keyboard protocols. The PTY smoke sends raw LF between two pasted fragments, asserts no response at that point, then checks the final wire explanation contains that newline. A synthetic modifier test is not offered as proof of terminal support. Other slice/growth/partition obligations are unchanged.

## Slice 1 qualification evidence

All commands below ran in the assembled feature worktree, not only writer worktrees. Source/binary fingerprints: [source-state.json](source-state.json); toolchain Rust 1.94.0. Each Cargo command used the explicit target directory recorded above. Writers skipped local validation under the delegation contract; Main supplied the assembled proof.

| Command / scenario | Observed result | Applicability |
|---|---|---|
| `cargo test --no-fail-fast` | 2,060 passed | R2 assembled state; subsequent test-helper lint correction rechecked by 22 focused UI tests |
| `cargo test --features kas --no-fail-fast` | 2,062 passed | R2 KAS state on Linux; 13 ignored and 24 filtered remain unclaimed |
| `cargo clippy --all-targets -- -D warnings` | PASS | Final production code, no warning suppressions |
| `cargo clippy --all-targets --features kas -- -D warnings` | PASS | Final production code, all targets compile |
| `cargo test -p cyril-core --no-default-features rejection_feedback_default_build_omits_metadata` | 1 passed | Explicit no-KAS proof, independent of workspace feature unification |
| `cargo test --features kas rejection_feedback` | 25 passed before R1 | Original focused feature fences; the added AltGr fence also passed in both final whole suites |
| `cargo build -p cyril --features kas` | PASS | Fresh binary used by the PTY proof |
| `cargo fmt --all --check` | PASS | Final source after temporary example removal |
| `python3 .cyril-y628/oracles/shape.py` | C7 PASS | App +12/35; mediator +3/12; generic converter +39/40; UiState +130/130 |
| `python3 .cyril-y628/mutate.py` plus affected `--only` reruns | 16 assertion-level red/restored-green pairs | Complete results in [mutation-results.json](mutation-results.json); manifest [mutations.json](mutations.json). R2 reran typed bound, render omission, AltGr and wrong-phase entry |
| `uv run --script .cyril-y628/smoke.py /home/dwalleck/repos/cyril/target/debug/cyril` | PASS, 5 raw responses, 6.219 seconds | Final actual Cyril PTY, controlled ACP endpoint, isolated synthetic auth store; no live account/model-consumption claim |

The PTY sends raw LF, not a manufactured Shift modifier. It checks that LF does not answer, then verifies the exact multiline/Unicode/quoted reason on the wire. It also exercises immediate plain rejection, 4,096-scalar acceptance plus refused overflow, whitespace-only plain rejection, and Esc-back with no intermediate response. Ordinary 100×32 and cramped 44×16 cell captures were inspected. [smoke-evidence/result.json](smoke-evidence/result.json), `response-1.json` through `response-5.json`, screen captures and `terminal.ansi` retain the proof. Upstream KAS consumption remains grounded in the pre-existing paired live captures named by route.md, not this controlled endpoint.

Final rerun used `CYRIL_FEEDBACK_SMOKE_OUTPUT=/tmp/cyril-y628-final-evidence` to avoid accepting stale receipts, then retained the byte-verified outputs under `smoke-evidence/`. It additionally sent raw AltGr-shaped CSI-u key events and verified exact `@€j` on the wire. [review-decisions.md](review-decisions.md) owns R1/R2 and F1–F4 dispositions; pre-repair fingerprints remain in `source-state-before-r1.json`. Applicable original mutation pairs are retained; affected pairs were rerun and `C4_drop_altgr`/`C6_wrong_phase_entry` added. The runner now retains full diagnostics because a long Unicode assertion had hidden its header in a suffix-only excerpt; the affected typed-bound pair was rerun successfully.

Budget harness [budget.rs](budget.rs) was temporarily installed as `crates/cyril-ui/examples/feedback_budget.rs`, run with `cargo run -p cyril-ui --example feedback_budget --profile test`, then removed from the crate. At the admitted 4,096-scalar multiline draft and 100×32 geometry: 10,000 actual approval draws took **3.471772 s**, mean **0.347177 ms** (limit 1 ms); 10,000 Backspace/Ctrl+J cycles took **0.000251 s**. Combined measured work is below the 10 s smoke ceiling. No permanent timing-sensitive test or benchmark target remains.

### Qualification corrections and retained enforcement

- Stock-terminal Shift+Enter could not prove the proposed newline contract. Requester-approved Ctrl+J amendment changes the binding, not the endpoint; raw-LF PTY proof now covers it.
- Initial PTY attempts failed before feature exercise: clap treated the mock flag as a Cyril argument, then KAS free-mode selection ignored the custom command and required auth. The harness now uses the existing `KIRO_KAS_SERVER_PATH` seam and a temporary synthetic SQLite auth store. It neither bypasses production preflight nor touches the user's login.
- The first whole-suite run caught an accidentally emptied existing approval fixture. Restored its `raw_input` index data; retained the pre-existing assertion. Both whole suites then passed.
- Qualification strengthened queued-request IDs and changed blocking test receives to `try_recv`, so the wrong-queue mutation fails locally rather than hanging.
- A cramped-render wording assertion was replaced with consumer-visible bound, cursor and warning-style assertions plus a no-warning positive control. No required presentation behavior was removed; the render-omission mutation is red.
- Initial lint failures were unused mutability, a collapsible render guard and an unhandled nested shutdown result. Fixed the code/fixture, kept every lint, reran both lint matrices and the final focused tests.
- Removed the pre-existing PermissionResponse variant-count test: it enforced the old enum's incidental count, not behavior. Exact-ID/trust/cancel tests and full workspace runs preserve its meaningful consumers; C1 mutation proves exact-ID sensitivity.
- No validator, platform guard, permission policy, modal/voice guard, transport bound, or shared source fence was disabled or orphaned. The existing chat-wrap tests ran in both full suites after extraction of the shared renderer.

No Windows execution is claimed: this Linux task adds no Windows-specific path and preserves the existing terminal abstractions; the repository's Windows CI leg remains the platform compilation oracle on publication.

### Caller, parity, reuse and symmetry receipt

Discovery used symbol-aware reference/hover attempts (empty LSP result, recorded in design.md), then exact-name source searches and compiler coverage. Final safety-net search covered `from_permission_response(`, `PermissionRequest {`, `ApprovalState {`, `text_cursor_lines(` and every new `approval_feedback_*`/`approval_begin_feedback` call across `crates/`.

Production callsites: request construction `core/protocol/domain_mediator/mod.rs:682`, response conversion `:708`; approval construction `ui/state.rs:1596`; feedback event routing `cyril/app.rs:1645–1646,1760–1769`; shared text rendering `ui/widgets/input.rs:138` and `approval.rs:307`. Existing converter callers migrated at `core/protocol/convert/mod.rs:1544,1562,1592,1642,1759,1773,1789` and `probe_qo13.rs:83`. Constructor fixtures migrated in `bridge/tests/current_runtime_contract/saturation.rs:86`, `ui/floor_tests.rs:41`, `ui/render.rs:704,797,870`, `ui/widgets/approval.rs:487,876`, `ui/widgets/toolbar.rs:783`, `ui/tests/modal_theme.rs:34`, `ui/state.rs:7486`, and `cyril/app.rs:3842,3879`. Prefixes are `crates/cyril-core/src`, `crates/cyril-ui/src`, and `crates/cyril/src` unless explicitly a tests path. New UI methods had no pre-existing callers; the listed App callsites are their production consumers.

| Added/changed symbol family or repeated construction | Reuse / deliberate divergence |
|---|---|
| `PermissionRequest`, `PermissionResponse`, `ApprovalPhase`, `ApprovalState`, `can_reject_with_reason` | Existing typed IDs, option kinds and phase-owned state; new capability and distinct variant implement C2/C3 without permitting trust+feedback combinations |
| `DomainMediator::handle_permission`, `from_permission_response`, `warn_if_foreign_permission_option`, cfg-paired `attach_rejection_metadata` | Existing immutable `Engine::kind`, request oneshot/spawn path, exact-ID selection and warning; KAS helper alone encodes metadata. No-feature dispatch is a compile-time boundary, not a runtime fallback |
| `RejectionFeedback`, `FeedbackAction`, `new`, `text`, `into_text`, `cursor`, `scalar_count`, `notice`, `is_blank`, `MAX_SCALAR_VALUES`, `max_scalars`, `limit_notice` | One private literal feeds all production limit displays/checks through one public accessor after R2. Read-only public view and crate-private ownership transfer; no draft clone on submit. Test-only approved literal 4096 deliberately repeats the independent specification oracle, detected by lower/raise mutations (C4) |
| `handle_key`, `insert_text`, `insert_char`, `normalize_and_filter`, `backspace`, `delete`, `move_left/right`, `move_line_start/end`, `move_vertical` | Existing chat scalar-editing conventions inspected; feedback adds bounded atomic insertion, filtering and multiline navigation that chat does not own. Concrete leaf is the approved C4 divergence; generic editor extraction is a permanent non-goal, not deferred cleanup. No new dependency; Rust string/char operations suffice |
| `UiState::show_approval`, `approval_select_prev/next`, `approval_confirm/cancel`, `approval_feedback_active/key/paste/cancel`, `approval_begin_feedback`, `approval_submit_feedback` | Existing front-owned VecDeque and oneshot lifecycle; common submit helper handles both entry points. No second queue, text algorithm, or response channel (C5) |
| `input::text_cursor_lines`, `input::render`, `approval::render`, `render_option_phase`, `render_reason_phase` | Extract existing `wrapped_rows`/cursor-window algorithm; retain chat oracle fixture. Reuse `modal::place`, preview/attribution and theme roles; reason layout is the existing modal's third phase (C6) |
| `App::handle_terminal_event`, `handle_approval_key` | Existing overlay priority, redraw and input-batch barrier; only route to UiState. Other modal/voice guards stay intact (C6) |
| Bridge permission fixture/captured responses and rejection-feedback fixture, converter matrix, editor `key`, state/App approval fixtures, render fixtures | Extend existing harness/probe, constructors, TestBackend and typed request/oneshot conventions. Literal expected IDs/reasons and separate approved ceiling remain independent oracles rather than production-derived expectations |
| Shape/mutation/PTY/budget evidence helpers | Issue-local tooling only, outside production and regular test discovery. Reuse existing KAS server-path/auth-fixture seam; external pyte interprets actual terminal bytes. No product fallback or new runtime dependency |

Symmetry: ordinary and reasoned rejection preserve the same selected option ID, foreign-ID warning severity and responder-failure logging. Missing/blank reason falls back to ordinary Selected; ineligible wire metadata is omitted without rewriting the selected ID. Both submit paths consume only the front responder and use the same asynchronous mediator mechanism. UI admission and wire admission intentionally differ: UI offers the action only for a capable selected RejectOnce; the wire owner independently checks configured engine, offered ID/kind and nonblank text. Differential matrix rows reuse identical reason/ID bytes across KAS/V2 and option kinds; eligible positive controls prevent vacuous omission. Key and paste share stored scalar accounting/filter policy, but paste normalizes CRLF atomically while typed keys insert one admitted scalar. Esc intentionally differs from terminal Cancel only while editing, per C5; both return to the existing choice/queue owner. Caller observability remains typed phase/response state, not hidden fallback. R1's additional modifier symmetry is owned in review-decisions.md.

R2 narrows the limit API and replaces only the new widget-fixture setup; the caller inventory above is pinned to the pre-R2 source. Its `approval_tool_call` helper is shared by old/new fixtures, and `approval_ui`, `feedback_ui`, and `render_ui` use the public UiState/TuiState boundary rather than reaching into editor state. Clippy rejected direct Option::expect calls in test bodies; the shared renderer now uses an explicit missing-approval assertion without changing the test's success condition or weakening any lint. R2 proof/dispositions are owned in review-decisions.md.

## Final checkpoint — Slice 1, C1–C7

| Gate | State | Evidence / applicability |
|---|---|---|
| 1 Affected units | PASS | Final whole default/KAS suites, followed by 22 UI rejection-feedback cases after the test-helper-only lint repair |
| 2 Falsifiers | PASS | C1 exact-ID/cancel/trust; C2 wire/engine/kind matrix and SDK roundtrip; C3 entry; C4 Unicode/bounds/modifiers; C5 FIFO/Esc; C6 real input/render; C7 ownership |
| 3 Stress fixture | PASS | Distinct responders/sessions with equal tool IDs; 4,096 multibyte scalars, overflow, multiline/control input, ordinary/cramped rendering |
| 4 Independent oracle | PASS | Literal expected IDs/reasons/limits and scalar edits agree with source; final raw ACP replies and actual terminal cells agree with approved actions |
| 5 Module shape | PASS | C7 census plus fresh isolated `FinalDesignConformance`: all eleven ledger rows MATCH, no missing/uncovered owner, no unresolved mismatch |
| 6 Production budgets | PASS | Bounded 4,096-scalar insertion/render work and measured 0.347177 ms mean draw; N/A — one-off phase for a continuous key/paste/decision wall budget |
| 7 Regression fences | PASS | Final suites and focused runs; no fence waiver |
| 8 Named mutants red | PASS | All 16 receipts; compilation failures disqualify a mutant; render omission/wrong-phase faults report missing cursor, AltGr reports wrong text |
| 9 Restoration green | PASS | Byte-exact restore and passing corresponding command for every mutant |
| 10 Parity/reuse | PASS | Caller/symbol/repeated-construction and symmetry receipt above; R1/R2 dispositions in review-decisions.md |
| 11 Preserved enforcement | PASS | Existing semantic guards retained; incidental enum-count test removed, old fixture repaired, render cases preserved through UiState; no lint/policy suppression |

Isolation record: `ReconstructOwnership` first reconstructed production without design access; frozen [reconstruction.md](reconstruction.md). Fresh `CompareApprovedDesign` found F3/F4, now corrected. Separate `ReviewAltGrRepair` approved R1. Fresh `FinalDesignConformance` verified current source against the frozen reconstruction and approved ledger, recording **PASS** in [conformance.md](conformance.md), including the complete reconstructed map and trade-off dispositions. Concrete feedback editing remains separate from chat mutation as approved; shared rendering is reused. F2's typed-Tab suggestion is a permanent non-goal, not deferred work.

Specification and design approvals remain applicable; no required behavior or risk was reduced. README documents the shipped operator controls; source/binary hashes and runtime evidence travel with the change. Initial authorization covered local implementation/commit. The requester subsequently authorized publication with “push and open a PR” (PR #131), then authorized merge with “merge when the CI is green”. Merge is conditional on green CI for the current head; cyril-y628 deliberately remains in_progress because tracker closure was not authorized.

Pre-commit size receipt: code/tests/docs/evidence census was **3,882 changed lines in 52 files**, before this closing receipt; the claim is a separate tracker-only commit. The complete atomic increment remains below the 3,950 forecast and 4,000 partition threshold. Only explicitly owned paths are committed; ambient `.rivets/.gitignore` and unrelated untracked files are excluded.

## Post-publication correction — R3 / F5

After the requester authorized publication of PR #131, the isolated `cyril-core --no-default-features` Clippy lane exposed a missing gate on a KAS-only test accessor. The earlier workspace-wide passes were genuine but did not cover that mandatory CI configuration; they must not be interpreted as complete CI coverage. [review-decisions.md](review-decisions.md#atomic-technical-repair-r3--isolated-core-feature-coverage) owns the reproduced failure, narrow repair, replacement qualification and checkpoint. Production behavior, budget and conformance evidence remains applicable; no test field, collection path, lint or CI job is disabled.
