# Design: cyril-jlxx

> Historical original record; see [archive provenance](evidence.md#archive-provenance-pr-116-review-repair). Later review fixes do not rewrite this approval or its original gates.

## Route and inputs

Empirical route: `route.md`. The complete given/when/then behavior set is its T4 row, sourced from cyril-jlxx and authoritative cyril-ukmu. `spec.md`: N/A — behavior explicit. `evidence.md` P1–P5 and linked probe artifacts are the empirical inputs; each comparison passes within its stated Linux/configured-operation bounds.

The behavior set is: supported CLI 2.21.1 KAS with actually advertised/confirmed Sonnet 4.6; authorized evidence inspection only; no repository mutation, PR-code execution, arbitrary shell, unauthorized reads, publication or authority escalation; unexpected permissions fail closed; native inherited permissions/hooks/MCP/delegation tested through configured operations and independent controls; provider/ResourceFS authority stays outside reviewer; fresh process/session/private runtime state per run; platform-qualified proof.

Load-bearing findings: CLI KAS rejects --agent/--model; native profile SetMode before prompt works; model arrives as streamed configuration; global native scope rules plus granular read_file restriction work; hooks must be explicitly off; inherited MCP can start before profile selection and survive includeMcpJson:false; clean configuration roots are required before spawn. Native subagentOrchestration defaults ON in core even with empty cli.json: granular tool restriction, not an absent flag, excludes delegation. No custom Cyril permission evaluator is selected.

## Input shapes

| Shape | Production cases | Coverage |
|---|---|---|
| S1 executable/configuration | installed supported version, absent/non-executable/wrong version, executable path with spaces/Unicode; inherited versus explicitly replaced environment; empty/replaced environment, missing HOME/KIRO_HOME/PATH, transport variables present/absent | C1, C2 |
| S2 evidence collection | empty, one, many distinct, repeated labels/content; empty text; nonempty/empty source label; content/labels with Unicode, spaces, slashes, dot-dot, newlines, JSON-looking instructions | C3, C6; empty collection/source label rejected before spawn; source labels never become filesystem paths |
| S3 representation | UTF-8 text evidence and textual descriptions of unavailable/binary sources; a label may name any original source | C3; invalid UTF-8 bytes are unreachable through String; binary capture/coverage projection belongs to verified cyril-9qn7/cyril-3qva |
| S4 bounded quantities | zero, exact limit, limit+1, large document count, large total text/output; negative lengths unreachable through usize | C6 |
| S5 native configuration stream | no session yet; session with absent/present model; catalog before/after session; model missing, wrong, desired, duplicate matching notification, later drift; selected native mode missing/present/wrong | C1, C5 |
| S6 authority attempts | allowed evidence read, outside/other-run read, write, shell, MCP/publication/delegation request; inherited broad allow, MCP launch config, executable hook; instruction-looking evidence | C2, C3, C4 |
| S7 permissions | expected/unexpected source session, zero/one/multiple options, any semantic option kind, responder already closed | C4; every request is canceled, never selected or persisted |
| S8 lifecycle | startup failure, ordinary text/tool sequence, authoritative terminal, stalled-but-open turn, cancel before/after readiness, cancel/terminal race, connection loss, dropped owning handle, shutdown with active child, repeated and concurrent independent consumers | C5, C6 |
| S9 deployment | Linux executed; native Windows build/path/environment semantics and real configured acceptance | C2/C3/C5; native execution gate remains cyril-6y1s, not satisfied by this design |

Invariant classification: additive backend operation and explicit launch configuration; no serialization, authority guard, terminal ordering or process ownership guarantee is removed. Existing Inherit callers retain their behavior; selected environments must preserve independence (C2).

## Placement

### Workbench module and interface

Create a concrete `cyril-workbench` library in the existing workspace. Its `reviewer` module owns the backend operation the desktop/queue will call. This ticket does not create the Tauri shell.

Proposed caller interface:

```rust
let run = reviewer.start(ReviewInput { instruction, documents }).await?;
let status = run.subscribe();
run.cancel(); // explicit stop, when requested
let outcome = run.finish().await;
```

`Reviewer` is configured once with the canonical supported Kiro executable, private runtime-parent location, explicit transport environment and limits. `ReviewInput` carries owned text evidence documents with source labels, not ambient filesystem authority, provider credentials, a caller-defined profile or an arbitrary bridge command. Internal generated evidence identities map back to the original labels. Repeated labels remain distinct documents; no silent deduplication. Source labels are data in a generated manifest, never OS paths.

`ReviewRun` hides native configuration, temporary files, session/model sequencing, all bridge channels, and child lifetime. A bounded watch projection reports phase/liveness; the backend retains streamed text internally and returns a typed completed/cancelled/incomplete result. A terminal is not a Findings-quality verdict. No unbounded event queue or per-chunk full-transcript clone. The backend owner retains the handle independently of window lifetime; dropping an active owning handle requests cancellation. No automatic restart or retry is added.

Owners:

- `reviewer.rs`: operation/lifecycle, typed status and result transitions, hot notification/permission drain.
- `reviewer/evidence.rs`: private per-run tree, generated document paths and source manifest, bounded complete writes before spawn, lifetime held until child completion.
- `reviewer/runtime.rs`: fixed native reviewer profile, global native permission rules, executable/version requirement, explicit allowlisted runtime environment, supported bridge configuration.
- `reviewer/types.rs`: input/status/result/error vocabulary; no provider schema or ACP wire types.

Native recipe: distinct HOME/KIRO_HOME/config/cache/temp directories for every run; a workbench-owned cwd containing only generated configuration and an inert evidence subtree; native global fs_read deny ** with exclude evidence/** and explicit allow for evidence/**; global fs_write/shell/mcp deny; granular tools [read_file]; empty MCP/power configuration; hooks Off; no caller-provided profile. Preserve existing Kiro sign-in with original native auth data location and approved transport variables. Never copy credentials or materialize user/repository configuration as active configuration. No symlinks/reparse points or user-derived path components are created from evidence input.

### Minimal reusable core change

The demonstrated integration gap is per-spawn configuration isolation: SpawnConfig cannot currently replace the child environment, and KAS handshake settings read process-global KIRO_HOME. Mutating the desktop process environment would contaminate other runs/apps and is not allowed.

Add `SpawnEnvironment::{Inherit, Replace(...)}` as a concrete owned environment value and a `SpawnConfig.environment` field. Replace applies env_clear plus the complete selected map at the existing AgentProcess spawn; redact values in Debug. The KAS settings snapshot must use the corresponding selected KIRO_HOME/HOME, not the desktop's ambient cli.json. Capture settings before constructing KasEngine, then have settings_extra return that snapshot rather than reread global environment during initialize. Keep default Inherit behavior unchanged for existing callers.

Add an optional exact expected CLI-version token to SpawnConfig, checked in the existing wrapper version probe; workbench requires 2.21.1. Do not run a second workbench --version subprocess or guess launch flags. The existing KAS wrapper flag selection remains the single implementation; apply the selected environment to that same probe and owned child. Existing callers set no exact-version constraint.

Trusted runtime installation/executable selection and parent-owned native authentication are distinct from untrusted child configuration: retain existing Free-path installation discovery and native auth-store ownership. The workbench uses the supported Wrapper path with --auth-method cli and an absolute bound executable. Host-shell ownership stays in core; workbench advertises no usable shell tools and disables hooks. This change does not create a second auth store, a process-wide environment mutation, a policy engine, a conductor stage or a second runtime.

Forbidden: workbench imports of ACP/SDK/conductor or internal protocol modules; core imports of review-domain types; review logic in TUI App, UI, memory, voice, transport or domain mediator; exposing BridgeSender/ExtMethod/Workflow to reviewer output/frontend; subprocess wrapper/runtime copies; provider connectors/publication authority in reviewer.

## Module shape

### Current inventory

Line metric: exact non-test physical prefix through the first cfg(test) marker, including documentation; a concentration signal, not semantic LOC. `module-baseline.json` records totals. Inline test members are noted where this metric is conservative.

| Module/path | Baseline | Interface, responsibility and adapter | Callers/test surface | Decision |
|---|---:|---|---|---|
| Cargo.toml | 87 | workspace/dependency ownership, no runtime bodies | cargo workspace | retain; add member/dependencies only |
| crates/cyril-core/src/types/mod.rs | 87 | public domain type declarations/exports | core and public consumers | retain; declaration/export only |
| crates/cyril-core/src/types/spawn_environment.rs | 0 | new concrete launch environment value, redacted diagnostics and effective-variable lookup | SpawnConfig, existing process builder/settings loader | create; intrinsic process-configuration seam, not a generic adapter trait |
| crates/cyril-core/src/protocol/bridge.rs | 366 | SpawnConfig and one owned bridge/thread; default inheritance; prepares existing runtime | main.rs, examples, live/core contract tests; LSP found 18 SpawnConfig references | retain; fields/defaults and launch/snapshot wiring only |
| crates/cyril-core/src/protocol/transport.rs | 332 | AgentProcess::spawn owns stdio/process group and teardown | SDK runtime and existing transport/process contract tests | deepen existing spawn implementation with selected environment; no new lifetime owner |
| crates/cyril-core/src/protocol/engine.rs | 332 | concrete engine adapters/capability composition | bridge engine_for, mediator initialize, engine tests | retain; snapshot storage and capability wiring only |
| crates/cyril-core/src/protocol/kas/settings.rs | 180 | native cli.json mapping and KAS defaults | KasEngine/settings tests | deepen existing loader with explicit effective source; preserve default mapper semantics |
| crates/cyril-core/src/protocol/kas/version.rs | 80 | wrapper version probe/flag selection | bridge, Free discovery, version tests | deepen existing probe with selected env and optional exact requirement |
| crates/cyril-core/src/protocol/kas/discovery.rs | 339 | trusted installed runtime and native auth-store discovery | Free path/auth; discovery tests | retain; mechanical callsite adaptation only if required; no reviewer policy |
| crates/cyril-core/src/protocol/kas/host_shell.rs | 478 | parent host shell resolver/executor ownership | bridge and host shell tests | retain unchanged |
| crates/cyril-core/src/protocol/domain_mediator/mod.rs | ~701 before test module, includes inline test members | serial ACP/session/domain owner | runtime-contract tests | retain unchanged; no review/environment policy body |
| crates/cyril/src/main.rs | 287 | TUI wiring into SpawnConfig | TUI build/smoke | retain; explicit default field wiring only |
| crates/cyril-workbench/src/lib.rs | 0 | public workbench module declaration | concrete backend callers | create, declaration-only |
| crates/cyril-workbench/src/reviewer.rs | 0 | start/cancel/status/finish operation and lifecycle | same backend operation in integration tests and live smoke | create |
| crates/cyril-workbench/src/reviewer/evidence.rs | 0 | generated regular-file evidence staging and temporary-tree ownership | reviewer operation | create, concrete private module |
| crates/cyril-workbench/src/reviewer/runtime.rs | 0 | fixed native recipe and environment/version configuration | reviewer operation | create, concrete private module |
| crates/cyril-workbench/src/reviewer/types.rs | 0 | invariant-bearing inputs/outcomes/errors | backend consumers | create, data-only |

Private AgentProcess constructor callers in transport tests, transport/tests/current_runtime_contract.rs and sdk_runtime/tests/process.rs migrate atomically. Existing test_bridge and KAS smoke SpawnConfig literals use defaults; main.rs spells fields explicitly and requires wiring. KasEngine literals/tests must be migrated with its settings snapshot. No shim or old parallel path remains.

### Three alternatives

| Alternative | Interface/caller | Hidden implementation and adapters | Depth/locality/trade-off |
|---|---|---|---|
| A: selected concrete workbench operation + core per-spawn environment | reviewer.start(input), run.cancel/finish | workbench hides evidence/policy/model/lifecycle; existing core owns one runtime, one process implementation | High caller leverage, one owner per concern, no worker IPC; requires a small reusable core launch/configuration change and default-caller migration |
| B: isolated workbench worker executable | spawn_review_worker(input), framed status/result IPC | worker reuses public core; parent owns framing, worker liveness and supervision | Makes whole-process env isolation easy as the probe showed, but adds protocol/supervision and a second lifetime seam; worker death versus core-owned KAS process groups needs extra ownership machinery; rejected unnecessary runtime plumbing |
| C: generic review/policy stage in core | spawn_bridge(stage_chain_with_review_policy, ...) | conductor/mediator owns evidence authorization, profile, policy decisions and result routing | Flexible for hypothetical workflows but pulls review responsibilities into core, duplicates native policy and widens internal interfaces; rejected by empirical native-first requirement and dependency direction |

Seam tests: deletion of reviewer moves configuration/model/cancel/evidence complexity back into every backend caller; caller and tests use the same start/cancel/finish interface; private evidence/runtime modules are concrete, not single-adapter traits; core launch environment is an intrinsic process-ownership seam, not a speculative provider abstraction; each responsibility has one owner and one behavioral test location.

### Module ledger and protected parents

The inventory above is the proposed approval ledger. Adapters: concrete native Kiro through existing core; no alternate production runtime. Workbench tests may bind a controlled executable at the same deployment executable seam; they must observe operations/side effects, not inspect private module plumbing.

Protected parents: bridge.rs (configuration/delegation wiring only), engine.rs (engine snapshot/capability wiring only), types/mod.rs and workbench lib.rs (declarations only), main.rs (default configuration wiring only). domain_mediator, TUI App, UI, memory, voice and SDK runtime production responsibilities are unchanged. Exit condition: no ReviewInput/ReviewRun/profile generation/evidence IO/result interpretation in any protected core/TUI parent; no ACP imports/dependencies in workbench; no duplicated process owner or generic policy trait.

Mechanical shape oracle: `.cyril-jlxx/oracles/shape.py` checks forbidden review-domain identifiers outside workbench, forbidden protocol implementation imports/ACP dependencies inside workbench, and reverse workbench dependencies. Before implementation it checks the negative placement baseline; complete mode additionally requires the six approved new owner files. This is a mechanically scoped guard, not semantic proof of every function's responsibility. Protected-parent body changes still require ledger review; plan.md must add measured growth tripwires and changed-path/body-delta checks before implementation. Named C7 mutation must turn the oracle red independently of cargo warnings.

### Review implementation corrections (2026-09-07)

The review decision ledger repairs the existing C1/C2/C5/C6 guarantees; it does not select another architecture or waive native acceptance. Privacy confirmation is a prerequisite to sending evidence: set the advertised standard `contentCollection` option to `disabled` and validate the returned catalog. An unsolicited update is not an acknowledgement of that operation.

The demonstrated missing seam is standard ACP configuration control. `types/event.rs` gains `SetConfigOption` and a response-qualified configuration notification; `domain_mediator/commands/{mod,session}.rs` owns generic dispatch and strict response decoding, using the existing bounded asynchronous command path. Generic configuration consumers in core `SessionController`, UI `UiState`, and the bridge example consume the returned catalog through their existing configuration-update arms. No reviewer policy, model name, evidence IO or provider authority enters these owners. SDK/process ownership, TUI App, memory and voice retain their production bodies.

Workbench evidence ownership gains a cancellation-safe cleanup guard, not another runtime/process owner. It waits existing core completion even when the caller executor disappears; lost completion retains and identifies the private root. This does not upgrade that completion into native Windows descendant proof: verified cyril-jlw9 and cyril-6y1s retain that separate obligation.

V12's additional public-bridge probe received `initialize`, dropped every sender, and observed no completion within three seconds; only the peer's deliberate seven-second self-exit released it. Fix this in the existing core owner: a closure-only watch channel carries real BridgeHandle/BridgeSender lifetime into `DomainMediator::run`'s initialization select. Retained sender clones keep it open; final client loss leaves through existing `shutdown_runtime`. Standalone queue/test handles have no runtime lifetime to retain. Do not add a timer, reorder commands, special-case Shutdown delivery, or copy process cleanup. Bridge factory/handle bodies may wire this guard; the mediator root permits only `DomainMediator::run` to change.

Windows construction must use compatible canonical paths and must not pretend that `XDG_DATA_HOME` redirects a native Known Folder. Validate the requested native-auth parent against `FOLDERID_LocalAppData`; credentials remain launcher-owned and are never copied. The ambient core Free/auth lookup concern remains the verified cyril-tpwn scope; this reviewer uses Wrapper with `--auth-method cli`. Keep exact CLI qualification, native policy and explicit hooks-off.

## Claims

1. C1: Inspection starts only after supported executable, native reviewer mode and catalog-confirmed Sonnet 4.6 are verified.
2. C2: Each launch has isolated native configuration and a replaced child environment without changing any other consumer's environment/settings.
3. C3: Reviewer evidence is read-only, mediated regular-file content; labels and source instructions cannot acquire resource/publication authority.
4. C4: Every permission request fails closed regardless of options, tool identity or source session.
5. C5: A run owns one fresh session/process tree, preserves authoritative lifecycle distinctions and terminates through existing core ownership.
6. C6: Evidence/output resource limits and side-effect scheduling cannot wedge permission/cancel handling or silently truncate a successful result.
7. C7: Review responsibility stays in the concrete workbench module while core retains only reusable launch/runtime ownership.

## Falsification

| # | Claim | Input shape | Falsifier and decisive control | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | Inspection requires supported executable, reviewer mode and confirmed Sonnet 4.6. | S1,S5 | Valid/missing/wrong/late configuration from a controlled executable; inspect received prompts, then run real pinned KAS. Valid configuration is the positive control distinguishing readiness gating from a broken connection. | Independent CLI catalog, peer prompt journal and live raw configuration | In reviewer.rs remove model equality from readiness guard; wrong-model peer receives a prompt and C1 fails. | model_mismatch_never_starts_inspection; late_configuration_starts_once; live smoke | local process + live campaign | PENDING — checkpointed-build per-slice gate |
| C2 | Selected child environment/configuration cannot leak ambient authority or contaminate another run. | S1,S6,S9 | Seed parent token/config canaries, launch two selected environments through core, compare child observations and unchanged parent; working MCP/hook controls exclude dead fixtures. | Child-written environment/config observation; independent MCP/hook markers | Remove env_clear in spawn_environment.rs or use ambient KIRO_HOME in settings.rs; seeded authority appears and C2 fails. | isolated_spawn_does_not_inherit_parent_authority; concurrent_settings_are_run_local; configured inheritance campaign | local process + live campaign | PENDING — checkpointed-build per-slice gate |
| C3 | Mediated evidence labels/content cannot acquire unauthorized read, mutation or execution authority. | S2,S3,S6 | Hostile labels/instructions; allowed read and attempted outside/write/shell operations, compared with working permissive configured controls. Model refusal alone does not pass. | Independent disk bytes/markers and passive native tool inventory | Remove global outside-read deny in reviewer/runtime.rs; outside canary returns and C3 fails. | evidence_labels_cannot_escape_staging; unauthorized_read_is_denied; configured read/write/shell control | local file + live campaign | PENDING — checkpointed-build per-slice gate |
| C4 | Every permission request fails closed. | S7,S6 | Peer emits permission variants including foreign session/empty options and journals response; positive delivery control excludes a disconnected peer; closed responder separately exercises visible failure. | Independent peer response journal | Replace Cancel with offered AllowOnce in reviewer.rs; peer observes allow and C4 fails. | all_permission_sources_are_cancelled; closed_permission_responder_is_visible | deterministic local process | PENDING — checkpointed-build per-slice gate |
| C5 | Each run preserves authoritative lifecycle and owns a fresh session/process tree through termination. | S5,S8,S9 | Two random identities, cross reads, same-run continuation, completion/cancel/disconnect/drop; live PIDs observed before disappearance. Silence must remain active, not terminal. | Process/session journals and independently observed descendant liveness | Treat TurnStalled as completion in reviewer.rs; active-status assertion fails C5. Separately omit Shutdown/await-completion; descendant check fails C5. | stalled_turn_stays_active; cancel_never_completes_review; fresh_runs_do_not_share_state; live lifecycle smoke | local process + live campaign | PENDING — checkpointed-build per-slice gate |
| C6 | Limits and slow observers cannot wedge permission/cancel handling or silently truncate success. | S2,S4,S8 | Stall staging/observer, cross every bound and send cancel/permission; compare unstalled/under-cap positive control. Expect no premature prompt, responsive cancel and explicit incomplete result. | Peer timing/command journal and external fixture byte counts | Await slow delivery inside reviewer.rs drain or convert output-limit failure to Completed; C6 latency/outcome assertion fails. | blocked_observer_does_not_block_cancel; incomplete_staging_never_spawns; output_limit_is_incomplete | deterministic local process/stress | PENDING — checkpointed-build per-slice gate |
| C7 | Review-domain ownership and ACP dependency direction remain confined to their approved crates. | placement | Census forbidden review symbols outside workbench and internal protocol imports/dependencies inside it; complete mode requires owners. Process-only fixture passes; misplaced review entry point must fail. | Source/dependency census compared with explicit approved owner list | Move start_review(input: ReviewInput) -> ReviewRun into protocol/transport.rs, or add workbench dependency to core; oracle emits C7 path violation. | issue-local oracles/shape.py; production mutation repeated at checkpoint | local static, measured 0.38 seconds | PASS — negative placement baseline and positive/mutation controls; checkpointed-build must repeat complete mode against implemented owners |

Limits proposed for approval: 1,024 text evidence documents, 64 MiB total staged UTF-8 evidence, 8 MiB accumulated reviewer output; configurable downward/upward by explicit backend configuration, checked arithmetic and explicit errors. Defaults are bounded resource policy, not claims that every PR fits. Startup/model readiness uses a bounded 30-second phase; silence during an active turn has no automatic success/cancel deadline. Existing core bounded shutdown is awaited, with failed teardown reported rather than claimed successful. Limit or startup failure is incomplete/unavailable, never a valid zero-finding review. Plan must record measured stress fixtures and deterministic bounds for C6; no throughput target or extra retry subsystem is introduced.

Regression-risk acceptance: None proposed. Deterministic fences protect the implementation contract; live native behavior remains an explicit execution acceptance gate, not a permanently green claim derived from mocks.

## Non-goals and future work

Permanent non-goals: arbitrary reviewer shell/PR-code execution; live publication; generic workflow/plugin/policy engine; separate worker/daemon; copied runtime; model fallback; process-global environment mutation; opaque labels used as paths; interpreting an authoritative terminal as evidence of review quality.

Verified separate ownership: cyril-9qn7 owns Tauri/Svelte/provider evidence and durable Evidence desk; cyril-u7qp owns queue/revision authorization; cyril-3qva owns Findings/coverage semantics; cyril-kxfp/cyril-cgqg own ResourceFS capture/comment pagination; cyril-6y1s owns native Windows configured enforcement. This ticket supplies a real backend reviewer operation, not a placeholder implementation of those tickets. Parent cyril-ukmu retains provider/native acceptance and usefulness release gates. No live publication is authorized.

## Falsifier run log

2026-09-06, worktree root: `python3 .cyril-jlxx/oracles/shape.py --mutation-check` exited 0 in 0.38 seconds. Output: C7 PASS, negative-placement-baseline, zero violations; process-only positive control PASS; deliberately misplaced `start_review(ReviewInput) -> ReviewRun` produced `C7: review responsibility outside workbench: crates/cyril-core/src/protocol/transport.rs`. Mutation used an isolated temporary fixture, not repository production code. This proves the present negative placement invariant and named detector, not a nonexistent production reviewer. Complete mode and production mutation remain mandatory checkpoint gates.

## Approval

Approved by the requester on 2026-09-06: "I approve this design, but make sure you're in the correct directory"

Approval covers alternative A, module/protected-parent ledger, limits and falsification contract. Approved risk acceptances: None. Verified implementation worktree `/home/REDACTED_USER/repos/cyril-wt-feat-cyril-jlxx`, branch `feat/cyril-jlxx`, using `git rev-parse --show-toplevel --abbrev-ref HEAD` with that explicit cwd. Linux proof does not satisfy native Windows acceptance.
