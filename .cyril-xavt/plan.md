# cyril-xavt implementation plan

Consumes approved design.md (DG Main, 2026-10-19). One independently integrated issue increment following un4p → ej7y. Core conversion and UI consumption land atomically in this issue; neither an unused type-only PR nor a UI-only schema parser is deliverable. Parent owns gates, smoke, docs after smoke, independent review, commit and this issue's PR. Worker never runs checks mid-flight. No feature line budget supplied.

## Slice1: typed joined shell preview with safe bounded UI

**Outcome:** design C1–C5: actual full command and supplied cwd, not title, available before Allow, with bounded navigable safe display and actual visible command confirmation context.

**Changes:** core types/tool_call.rs adds ShellCommand(command:String,cwd:Option<String>), new + borrowed getters, ToolCall optional field/builder/getter; raw replacement invalidates stale typed data while absent raw input preserves it. KAS converter classifies after ledger join, using run_command metadata or execute+string-command context and rejecting explicit other tool ids. Mediator only wires converter for KAS; authorization/raw input unchanged. UI consumes typed field, shows cleaned command first and supplied cwd separately, preserving meaningful spaces/newlines and replacing unsafe controls/bidi. Reuse ej7y body window; command cannot be confirmed if only label/cwd/omission hint is drawn. Extend real render/key tests and long-pipe inline or reviewed insta snapshot.

**Verification:**
- Parent: converter/domain/ledger/type tests with retained live-derived field shapes: initial+kind-omitting update+permission stub; command/cwd present/absent/null/malformed; explicit unrelated tool id and nonexecute command objects; raw replacement/absence lifecycle → exact typed data, no false shell classification, no stale projection.
- Parent: actual widget render long-pipe snapshot and control/bidi/multiline/cwd cases; actual App width/height/scroll positions through Enter/Esc → every command segment reachable, meaningful script layout retained, no command-less confirmation.
- Parent: real KAS0.66.43 in owned temporary workspace with harmless sentinel pipeline; retain raw payload plus native rendered approval BEFORE Allow and scroll tail if needed, then observe execution sentinels. Supplied cwd must be visible if present. No fixture substitutes for this mandatory evidence.
- Parent: existing approval/path/floor suites and all repository gates, then docs after smoke and independent issue review → own PR with no blocking defect or missing evidence.

**Dependencies:** approved ej7y bounded preview interface and un4p actual-width/tail guards. Existing installed KAS source and committed captures resolve payload premise; no new external assumption blocks implementation.

## Checkpoint handoff

The current publication checkpoint in `/home/dwalleck/repos/cyril-wt-fix-cyril-xavt` holds HEAD `bc6f48a21016320cc6491356f880f37463cf8c6e` plus MERGE_HEAD `af78b1615bd5b97b4c702542fd3ab193151bd4c1`; HEAD alone is not the assembled source. The original reconstruction on published EJ `9400e7c8ddd53776f6fd8b4d28d4c7fc69e45ab4` with common test-only auth prerequisite `6c0f41c74bd94fa49ab1b3a2ae06e3ed70d3de69` remains historical in delivery-handoff.md. [Current verification](verification.md#current-held-af78-checkpoint) owns current source pins and qualification; the [portable index](evidence-current-modal532-b6a846be-index.json) maps immutable contracts and receipts without changing their bytes. Frozen combined source and original approved inputs remain untouched.

Historical October20 qualification completed in bg128/artifact860: workspace/all-target Clippy `-D warnings`, 3,117 passing workspace tests with 13 ignored, default build and format checks; the then-fresh actual-source ReviewShellApproval was correct/confidence0.98, after F3's exact captured consent.triggeringResource restoration. Those are historical results, not current gates. The added modal DG now has a separately pinned 532-input `b6a846be…` / private App `8e2b7c3a…` checkpoint: retain the initial fmt CHECK FAIL and seven subsequent successful command records. Source review is NONE / correct / 0.97, source-only, with Main retaining post-rustfmt applicability. Original exact10 results and unchanged Core197 evidence are retained rather than replayed; the old530 execution pin plus two posthoc fixture supplements is not retroactively called old532. Full command records and scope remain central in verification.md, not duplicated here.

Focused regression command references (not instructions to rerun a reported failure):
```
env -u CARGO_TARGET_DIR cargo test -p cyril-core types::tool_call::tests
env -u CARGO_TARGET_DIR cargo test -p cyril-core --features kas permission_shell_command
env -u CARGO_TARGET_DIR cargo test -p cyril-core --features kas shell_preview
env -u CARGO_TARGET_DIR cargo test -p cyril-core --features kas v2_shell_shaped_permission
env -u CARGO_TARGET_DIR cargo test -p cyril-ui preview_body_prefers_typed_shell_command
env -u CARGO_TARGET_DIR cargo test -p cyril-ui shell_
env -u CARGO_TARGET_DIR cargo test -p cyril xavt_app_
env -u CARGO_TARGET_DIR cargo test -p cyril-ui approval
```

The mandatory live smoke was performed **once** on the historical tested combined source. It retained actual joined command/cwd wire fields and rendered the full 892-byte command/all 24 identity-filter stages plus cwd before one AllowOnce, then observed completed wire exit0 and `XAVT_BEGIN XAVT_END\n`. Stdout was not rendered and final prompt failed -32000 after token-helper timeouts; root cause is unestablished. Neither the old private338 captured-frame supplement nor the current private8e owned consumer is a fresh live-KAS run. Current captured-command and controlled cases each passed exactly once with one RejectOnce and no execution; the original controlled FAIL remains distinct and preserved. [Current verification](verification.md#current-held-af78-checkpoint) owns both current receipts, historical scopes and native/global exclusions. No second native execution or account/model call is authorized.

### Source-grounded isolated launch and frame capture

This is the historical source-grounded recipe used to establish the original real-KAS premise, retained for provenance, **not authorization to rerun it**. This publication refresh consumes retained qualification and owned-consumer receipts only; it performs no build, checks, formatter, runtime, account contact or native command execution.

- Use a private `mktemp -d` root with `umask 077`, private HOME, private KIRO_HOME and an owned workspace. Cyril uses `$HOME/.config/cyril`, not XDG_CONFIG_HOME (`crates/cyril/src/main.rs:54,88,278-309`). Its flags include `--cwd`, `--prompt`, `--agent-engine`; there is no `--config`.
- Private config: `[agent]` with `engine="kas"`, `kas_spawn="free"`, `shell="bash"`, `kas_hooks="off"`; `[memory] enabled=false`. No user grants are copied. Preserve real XDG_DATA_HOME (normally original `$HOME/.local/share`) for installed bundle/auth discovery and preserve original `$HOME/.local/bin` in PATH before replacing HOME. Normal authenticated token renewal can update the real auth store; isolation does not promise zero credential-store writes.
- Launch with HOME/private KIRO_HOME, SHELL=/bin/bash and explicit `--agent-engine kas --cwd <owned workspace> --prompt <literal harmless pipeline request>`. The prompt substitutes the real owned path and asks exactly one shell command, no file inspection/workflow creation, and waiting for approval. Use identity-filter stages with distinct segment labels and beginning/end sentinels; never Always allow.
- Original actual-frame capture used existing `experiments/kiro-proxy-rs`, NOT `run-with-proxy.sh`, with the normal Node/KAS arguments and injection absent. Its allowlisted native receipt records the observed invocation. The full recorder journal remains private and must not be opened or archived during finalization.
- Start the terminal recording before launch; retain pending native terminal text/SVG and cast. Keep full recorder output private: it contains the bearer-token callback response on client_to_agent. Publish only matching agent_to_client tool_call/tool_call_update and request_permission JSON plus nonsecret startup records proving real Node/bundle invocation. A permission frame can be a rawInput-less stub, so preceding joined-command frames are necessary. Recorder `.parsed` retains full JSON values, not original byte formatting.
- Do not rely on KIRO_ACP_RECORD_PATH (ignored by this host) or RUST_LOG=trace (Cyril's file subscriber has no EnvFilter and inbound logging only records payload shape/counts). Cyril's private log remains useful supplementary evidence, not the required raw-frame recorder.

Primary historical recipe/source provenance: existing isolated audit `experiments/conductor-spike/audit-2.32.0/live/FINDINGS.md:4-5,133-142`; KAS settings/discovery owners; CLI main.rs; existing recorder source. Finalization consumes only explicitly allowlisted nonsecret captures. No global configuration, account provisioning, daemon, or account/model operation is performed.

## Integration boundary

The original X increment following un4p and ej7y owns core ShellCommand/converter/mediator/lifecycle and live-derived fixture changes; UI PreviewBody::Shell classification; PreviewContent/ShellBody/safe streaming scanner; command/path/editor context reservation and current-viewport command guard; long-pipe/sanitizer/cwd widget tests and xavt-labelled App tests. Existing ApprovalState constructor migrations and generic scroll dispatch remain inherited from EJ. Only approved X hunks were reconstructed in shared approval.rs/app.rs; no wholesale combined-source replacement occurred. The separately appended modal DG adds whole-span clearing at its existing owner and migrates twelve existing callers without changing permission choices/grant policy. Its already-qualified eleven repair owners (README plus ten UI files) are distinct from the historical eleven feature paths. Main owns the current publication allowlist, held-merge completion and publication; this metadata refresh changes no source and performs no staging. No final-integration MCP thirteenth-caller, native-glyph or whole-stack acceptance is implied.
