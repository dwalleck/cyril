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

Actual X implementation is reconstructed on published EJ commit `9400e7c8ddd53776f6fd8b4d28d4c7fc69e45ab4` in `/home/dwalleck/repos/cyril-wt-fix-cyril-xavt`, branch `fix/cyril-xavt`, with common test-only auth prerequisite `6c0f41c74bd94fa49ab1b3a2ae06e3ed70d3de69`. delivery-handoff.md owns reconstruction provenance; verification.md owns final source-freeze identity, actual repository gates, independent review, real-KAS historical scope, and the actual-delivery controlled consumer supplement. The frozen combined source and published parent trees were not modified by this finalization.

Parent qualification is complete in bg128/artifact860: workspace/all-target Clippy `-D warnings`, 3,117 passing workspace tests with 13 ignored, default build and format checks. The commands below document the focused seams; this worker did not rerun them. Fresh actual-source ReviewShellApproval found no blocking behavior defect (correct/confidence0.98). F3 restored only the exact captured consent.triggeringResource before final gates. Main retains commit/tracker/PR ownership.

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

The mandatory live smoke was performed **once** on the tested combined source. It retained actual joined command/cwd wire fields and rendered the full 892-byte command/all 24 identity-filter stages plus cwd before one AllowOnce, then observed completed wire exit0 and `XAVT_BEGIN XAVT_END\n`. Stdout was not rendered and final prompt failed -32000 after token-helper timeouts; root cause is unestablished. This does not qualify the final actual-X binary as a fresh live-KAS run. The bounded actual-delivery supplement replays the exact safe pending payloads through the qualified binary, proves modal content and refusal guards, then sends one offered RejectOnce and a clean owned-peer stop. It executes nothing and replays no completed frame. verification.md owns both scopes; no second native execution or account/model call is authorized.

### Source-grounded isolated launch and frame capture

This is the historical source-grounded recipe used to establish the original real-KAS premise, retained for provenance, **not authorization to rerun it**. The finalization worker performs no build, account contact or native command execution; only the explicitly authorized owned-peer proof is current work.

- Use a private `mktemp -d` root with `umask 077`, private HOME, private KIRO_HOME and an owned workspace. Cyril uses `$HOME/.config/cyril`, not XDG_CONFIG_HOME (`crates/cyril/src/main.rs:54,88,278-309`). Its flags include `--cwd`, `--prompt`, `--agent-engine`; there is no `--config`.
- Private config: `[agent]` with `engine="kas"`, `kas_spawn="free"`, `shell="bash"`, `kas_hooks="off"`; `[memory] enabled=false`. No user grants are copied. Preserve real XDG_DATA_HOME (normally original `$HOME/.local/share`) for installed bundle/auth discovery and preserve original `$HOME/.local/bin` in PATH before replacing HOME. Normal authenticated token renewal can update the real auth store; isolation does not promise zero credential-store writes.
- Launch with HOME/private KIRO_HOME, SHELL=/bin/bash and explicit `--agent-engine kas --cwd <owned workspace> --prompt <literal harmless pipeline request>`. The prompt substitutes the real owned path and asks exactly one shell command, no file inspection/workflow creation, and waiting for approval. Use identity-filter stages with distinct segment labels and beginning/end sentinels; never Always allow.
- Original actual-frame capture used existing `experiments/kiro-proxy-rs`, NOT `run-with-proxy.sh`, with the normal Node/KAS arguments and injection absent. Its allowlisted native receipt records the observed invocation. The full recorder journal remains private and must not be opened or archived during finalization.
- Start the terminal recording before launch; retain pending native terminal text/SVG and cast. Keep full recorder output private: it contains the bearer-token callback response on client_to_agent. Publish only matching agent_to_client tool_call/tool_call_update and request_permission JSON plus nonsecret startup records proving real Node/bundle invocation. A permission frame can be a rawInput-less stub, so preceding joined-command frames are necessary. Recorder `.parsed` retains full JSON values, not original byte formatting.
- Do not rely on KIRO_ACP_RECORD_PATH (ignored by this host) or RUST_LOG=trace (Cyril's file subscriber has no EnvFilter and inbound logging only records payload shape/counts). Cyril's private log remains useful supplementary evidence, not the required raw-frame recorder.

Primary historical recipe/source provenance: existing isolated audit `experiments/conductor-spike/audit-2.32.0/live/FINDINGS.md:4-5,133-142`; KAS settings/discovery owners; CLI main.rs; existing recorder source. Finalization consumes only explicitly allowlisted nonsecret captures. No global configuration, account provisioning, daemon, or account/model operation is performed.

## Integration boundary

Following published un4p and ej7y, this actual X tree owns all core ShellCommand/converter/mediator/lifecycle and live-derived fixture changes; UI PreviewBody::Shell classification; PreviewContent/ShellBody/safe streaming scanner; command/path/editor context reservation and current-viewport command guard; long-pipe/sanitizer/cwd widget tests and xavt-labelled App tests. Existing ApprovalState constructor migrations and generic scroll dispatch remain inherited from published EJ. Only approved X hunks were applied to the shared approval.rs/app.rs files; no wholesale combined-source replacement occurred. Parent owns final format normalization, qualification and explicit issue-scoped staging. No commit/push/staging was performed by this worker. Permission choices/grant policy are deliberately unchanged.
