# cyril-xavt design

## Route and inputs

Structural; behavior source is route.md T4 and full Rivets issue. Existing-system premise resolved by installed KAS 0.66.43 source and committed 2.32.0 live frames; no fresh discovery probe required.

Installed package root:
`/home/dwalleck/.local/share/kiro-cli/kas/2.32.0-f3c560628fc814c59257082d0d914826583f83aa202043fad5ae73f24a037721/node_modules/@kiro/agent/`.
Its package.json identifies version0.66.43 and `dist/server/acp-server.js` entry. Source line12276 defines string command and optional/null cwd; line12279 emits `{...arguments, command}` before approval, line12281 retains it in updates and resolves supplied cwd; line11843 maps run_command/control_process/get_process_output to execute kind; line17174 ordinarily omits tool-id metadata on shell tool calls. Thus neither execute kind alone nor title text alone is sufficient classification. Permission `_meta.kiro.toolId` is run_command, not the model-facing execute_bash/execute_pwsh name.

Live capture `experiments/conductor-spike/audit-2.32.0/live/kas-clientcreator-2.32.0.jsonl:324,328,335-337` has execute initial call with rawInput.command/cwd, a stub permission request with run_command metadata, raw-input updates omitting kind, then terminal/create using command/cwd. No-cwd capture `experiments/conductor-spike/audit-2.32.0/live/kas-lead-interrupt-sig15-2.32.0.jsonl:52,56,63-65` omits cwd on initial/updated rawInput while consent has workspaceRoot. Consent workspaceRoot is not the requested cwd and must not be substituted.

## Placement and module shape

| Owner | Decision and interface | Forbidden |
|---|---|---|
| core types/tool_call.rs | Deepen existing ToolCall with optional typed ShellCommand holding verbatim command + optional cwd, borrowed getters and a builder. Existing constructor unchanged. A raw-input replacement invalidates/replaces any stale projection; an omitted raw input preserves it. | No terminal sanitization, wrapping or UI dependencies. |
| core protocol/convert/kas.rs | Parse a shell preview from the final joined permission ToolCall only for actual shell context (run_command metadata, or execute kind plus string command); non-shell command-like JSON is not a shell preview. cwd absent/null stays None; malformed cwd is not invented; malformed/absent command stays unavailable. | No title parsing, cwd inference from consent/terminal events, generic ACP rule changes, grant-policy changes. |
| core protocol/domain_mediator/mod.rs | Wire that existing converter after ledger join and only for KAS. No new responsibility body in mediator. | No JSON shape interpretation or rendering. |
| UI traits.rs and widgets/approval.rs | Consume typed shell data; command takes body priority over generic content. Display-only sanitization replaces terminal controls/bidi formatting; preserve command newlines and meaningful spaces, unlike plan whitespace collapse. Show supplied cwd separately, sanitized too. Shared geometry establishes actual visible command context before confirmation. | No ACP import, no KAS JSON probing, no command execution. |
| UI preview navigation | Reuse approved cyril-ej7y bounded preview navigation/window so a command exceeding the visible budget is fully reachable rather than silently title/body truncated. This dependency is implemented first in the issue stack. | No new generic overlay framework, no changing Up/Down option selection or reason editing keys. |

No responsibility split, dependency, or new orchestrator cluster. Compiler boundaries and independent review suffice; no bespoke source gate. Original raw data remains available for unchanged grant matching; typed preview must not be an authorization input.

## Observable behavior and preserved invariants

1. Command-only shell raw input is a usable preview; UI does not rely on the title. Include cwd exactly when KAS supplied a string, never infer it from workspaceRoot.
2. Command is visible alongside an actionable option/reason editor. Below sufficient rows/width, Enter stays inert and Esc remains available. Existing path-tail rule remains independently enforced if a path exists.
3. Full normal pipe command appears without a marker at a roomy viewport. Longer commands remain bounded on screen, have explicit navigation/omission affordance, and can be traversed to their tail before Allow. Do not claim simultaneous visibility of an arbitrarily long command.
4. Preserve option windowing, path precedence/elision, input nonoverlap, request-local drafts/queueing, later trust/scope/folder authority, and no mutable render cache.
5. Non-shell command-like data cannot be mislabeled; missing/malformed command still produces existing unavailable behavior. Optional malformed cwd cannot hide a valid command.

## Claims / falsification

| ID | Claim and cases | Verification / falsifier | Regression coverage | Status |
|---|---|---|---|---|
| C1 | Typed conversion reflects full joined shell command and supplied/absent cwd; stub and kind-omitting update preserve data; unrelated command JSON is not shell. | Existing core converter/ledger/mediator test seams using live-derived initial/update/stub fields. Wrong command/cwd, classification by title, or a stale raw replacement falsifies. | Add focused core behavior tests; preserve existing ledger tests. | PASS — actual-delivery full suite860, source review, and exact rawInput-less captured-permission consumer proof; verification.md. |
| C2 | UI renders command and cwd safely without loss of meaningful script whitespace; full long pipe command body is available independently of title. | Widget TestBackend test + insta snapshot for long pipeline; controls/bidi/multiline/absent cwd cases. A retained dangerous formatting codepoint, flattened meaningful script structure, missing command segment/cwd or unavailable marker with valid command falsifies. | Existing approval widget public render seam; typed fixtures. | PASS — repaired regression history454→542/575/688, actual-delivery860 and full892-byte actual modal/wire reconciliation; verification.md. |
| C3 | Command/path context and choice/editor share actual drawn cells before Enter; zero width/border/label-only context cannot answer. | Existing App draw_frame + handle_key size sweeps extended to shell preview and scroll positions. Compare drawn buffer with oneshot answer; Esc works throughout. | Reuse un4p geometry fences and ej7y navigation tests. | PASS — full suite860 plus actual-delivery cwd-only/tiny Enter refusal, reason/chat/selection controls and one RejectOnce; verification.md. |
| C4 | Real normal KAS flow exposes complete long piped command before Allow, with supplied cwd when present. | Launch actual built Cyril using KAS0.66.43 in an owned temporary workspace; request harmless sentinel pipeline; retain raw permission frame + native rendered approval capture BEFORE Allow, then Allow and observe sentinels. UI-only fixture is insufficient. | Mandatory live acceptance receipt; no substitute oracle. | PASS within recorded scope — one historical tested-combined real-KAS run proves before-Allow command/cwd and successful execution wire. Exact captured-frame proof separately links actual delivery; no fresh actual-delivery KAS execution, rendered stdout or clean final-model-turn claim. See verification.md. |
| C5 | Core/UI ownership and permission semantics unchanged except typed display data. | Compiler/dependency checks and independent review of conversion wiring, field merge lifecycle and UI consumers. | Existing full suites and reviewer diff inspection. | PASS — actual-delivery all-target Clippy/full suite/build/format860 and canonical fresh source review correct/confidence0.98, including F3 applicability; verification.md. |

Bug sensitivity: parent runs focused new tests against pre-change source then repaired source, or applies smallest named real-defect restoration when sequencing makes that necessary. No second implementation/oracle.

## Non-goals

No permission grants/persistence changes; no shell execution, process lifecycle, hidden cwd inference, other provider schema expansion, or native PowerShell qualification claim. These are permanent exclusions of this issue, not deferred scope.

## Approval

**DG approved, 2026-10-19 — Main**, after reading the complete design. Governing requester delegation relayed verbatim: **"Delegate routine sign-offs" — "I select and record evidence-backed designs. I still ask before scope changes, relaxed acceptance, architecture changes outside the existing boundaries, or explicit risk acceptance."** This is delegated design approval, not a requester verbatim design sign-off. Approved typed ShellCommand on ToolCall, post-join KAS converter projection preserving raw content/permission authority, whitespace-safe UI and bounded-preview reuse exactly as designed. No permission, reference-fidelity, scope or risk relaxation authorized.

## Qualification layout repair — DG approved

Parent575 passes the four FE0F cases and workspace3117 tests, but all-target Clippy reports ChatMessageKind::ToolCall at296 bytes against SteerEcho73; build did not run. The new inline optional ShellCommand is two owned string slots. Do not box every chat/tool call to compensate for an optional permission-only projection.

**Approved implementation:** private ToolCall.shell_command is now Option<Box<ShellCommand>>; with_shell_command moves the existing value into Box::new, and the unchanged borrowed getter uses as_deref. No allocation for absent shell context; one box only for an attached projection; no getter string copies. Constructor/builder/getter signatures, raw permission JSON and replacement/omission lifecycle remain unchanged.

**Ownership rationale:** production attaches the projection only after the ledger join in DomainMediator::handle_permission, then moves that shell-bearing snapshot into approval state. Rendering borrows it; the inspected production path does not clone or merge the attached snapshot. Existing owned optional-payload precedents are PermissionRequest.workflow_plan: Option<Box<WorkflowPlan>> and WorkflowRunCompleted's boxed final state with a borrowed getter. Arc would avoid deep copies for future explicitly shared shell-bearing snapshots, but introduces reference-count/header work without an observed production sharing requirement. Box preserves current explicit clone semantics without adding string copies at attachment/getters.

**Reference inventory:** primary-worker LSP references for ShellCommand, with_shell_command and shell_command returned inconsistent empty results and were reported. Source fallback finds seven builder callsites (one production attachment), nine ToolCall getter callsites (one production UI classifier), eleven merge_update and three presence-aware merge entrypoints, and eighteen domain PermissionRequest literals. All public callers remain unchanged. The unrelated host-execution ShellCommand lifetime type is not this projection.

**Static containment audit:** ChatMessageKind owns one TrackedToolCall wrapper with no extra fields; Notification's two tool variants each own one ToolCall, but its independent MetadataUpdated payload remains large. DomainWork has comparable routed alternatives and its permission alternative is ACP, not the domain request. Existing CommandOutcome/SessionStart pointer boundaries remain unchanged; PreviewBody only borrows ShellCommand. No other targeted containing enum was found requiring a repair.

**Source-derived predictions, not executed layout measurements:** an optional pointer replaces the48-byte inline projection with8 bytes on the pinned x64 toolchain. ToolCall296 becomes256 versus73 (difference183). PermissionRequest616 becomes576. MetadataUpdated has an unchanged conservative304-byte payload floor; RoutedNotification's other fields contribute at least72 bytes (session, turn and initiation), so its floor remains376. Thus unboxed ReplayFrame's difference is at most200. Rust1.94 Clippy tests strictly greater than its default200 threshold; no threshold/lint change is proposed.

Both interim ReplayFrame boxes have been removed together: the two inline variants and constructor paths are restored, along with the original four handler moves in App's two replay helpers. Every assertion and public production interface is unchanged. No net test-support API/boxing delta remains relative to the pre-repair source; the earlier helper-only patch is superseded and must not be applied.

**Verification:** existing projection lifecycle, joined permission, App/renderer and replay tests plus mandatory all-target Clippy/build/format gates supply the checks; no representation-specific size test or bespoke oracle. Parent final combined688 and actual-delivery860 establish the final layout/gate result. The original combined real-KAS receipt and actual-delivery exact captured-frame RejectOnce proof remain separately scoped in verification.md. The private-field/builder/getter repair is three source lines; helper retirement restores its original pre-box source. This design's implementation worker ran no gates.

DG,2026-10-20 — Main: “DG APPROVED: privateOption<Box<ShellCommand>> at existingToolCallowner; buildermovesstringsonceintobox onlywhenpresent, getteras_deref preservesborrowedAPI. Sourcefactno productionsharing/cloneafterattach makesArcunnecessary; don'tboxallToolCalls. RemoveBOTHinterimReplayFrameboxes+theirconstructor/consumerchanges asobsolete, preservingalltestassertions. Static≤200Replaygap andChat183gap needactualfinalclippy confirmation; no threshold/lintchanges.” This is delegated implementation approval, not a new requester scope/risk decision.

## Modal ownership repair — additional Main DG

This is an append-only approval addendum. All original requirements, claim/falsifier text, approvals and qualification-layout DG above remain unchanged; their gates860 / review0.98 and related delivery references describe the historical October20 qualification, not the current held af78 checkpoint. Main explicitly approved this additional bounded modal repair under delegated evidence-backed design authority; it restores the existing rendering obligation without replacing the original X contract or relaxing acceptance. The immutable authority is `local://approved-overlay-wide-boundary-repair.md`, SHA-256 `475c6c4ea2ff7f896b3f50397f4a1336cfbfa508d6b28f72d726248670bd5540`, mapped by the [portable index](evidence-current-modal532-b6a846be-index.json).

### Approved owner and invariants

- Existing `crates/cyril-ui/src/widgets/modal.rs` owns one crate-private `clear(&mut Frame<'_>, Rect)` operation. Clip the rectangle to the Buffer, then evict whole painted grapheme spans intersecting it on affected rows, including leads left of the rectangle and trailing cells right of it. Preserve every disjoint outside cell/style and all other rows.
- Use actual released `CellWidth`, arbitrary span widths and safe row-bounded arithmetic, rather than a fixed one-cell halo. The allocation-free walk starts at the Buffer's left edge because continuation ownership has no backpointers. Production performs no string/Buffer allocation or copying, grapheme normalization, backend substitution or diff-flag workaround.
- Migrate all twelve existing clearing callers: five approval phases, six other modals and floated suggestions; remove obsolete `Clear` imports. Preserve geometry, paint/key stacking, current-viewport confirmation, command/path bytes, permission authority, reason/editor handling and ordinary input. Retain released core0.1.2/widgets0.3.2. No new module, dependency, trait, cache, global repair pass or exported API is authorized.

### Evidence attribution

The retained original captured-command case passed, while the original controlled case failed at reason wait with zero observations and narrow controls unreached. Frozen ANSI replay establishes the empty body/FF9E at `(8,12)`; the internal App Buffer skip mechanism remains source-inferred, not directly instrumented or a timing diagnosis. The original regression has exactly two decisive backend-emission REDs at original `render.rs:1112`; the third failure at1142 is a distinct restoration-oracle correction, not another emission RED or proof of completed controls. The corrected disjoint width-one combining cases preserve outside cells/styles and require no close re-emission; crossing-border, released diff/backend and restoration fences remain.

The current 532-input `b6a846be…` / private App `8e2b7c3a…` qualification and both accepted exact-once consumer cases are recorded centrally in [current verification](verification.md#current-held-af78-checkpoint), with chronology in [review-decisions.md](review-decisions.md#current-modal-repair-review-and-proof-scope). Source review is NONE / correct / 0.97, source-only; Main retains semantic applicability after authoritative rustfmt-only normalization, not a new review round. Original exact10 and unchanged Core197 evidence are retained, not replayed. The separate MCP thirteenth caller/final integration, native glyph/reflow, CI and whole-issue/global PASS are not covered by this twelve-caller approval.
