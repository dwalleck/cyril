# cyril-4c4d — default KAS with prerequisite repairs

Status: approved and implemented; current verification and remote-CI gate are owned by `plan.md`.

## Route and inputs

Structural route: `route.md`. Behavior and accepted native-memory risk: `spec.md`, especially Approval. Prerequisite implementations are independently deliverable: `cyril-5nep` (Host hook tags) and `cyril-oieu` (KAS advertised commands). Both are claimed/in progress on main. Their worktrees are `../cyril-wt-fix-cyril-5nep-hook-tags` and `../cyril-wt-fix-cyril-oieu-kas-commands`; the default switch remains in `feat/cyril-4c4d-default-kas`. Externally-owned PR #167 / cyril-igrx has merged; integration and current proof are tracked in `plan.md`.

Production source investigated: `99ed7ce5`. Claim-only branch commits `63bc21be` / `a99ac821` do not change that production state. Do not import unrelated unpublished audit/tracker commits from primary main merely to obtain research references.

## Decisions and behavior

### D1 — default feature, selection and custody

Enable `kas` by default in `crates/cyril/Cargo.toml`, keeping core's feature opt-in. `AgentEngine::default()` becomes feature-aware: KAS when the compiled core supports KAS, V2 otherwise. Existing CLI-over-config precedence, `kas`/`v3` aliases and explicit V2 remain. Update consumers/fixtures that deliberately require V2, not every use of Default indiscriminately.

Amend ADR-0002: a normal production binary now contains and uses the existing KAS credential custodian; the selected-package no-default build retains compile-time exclusion. A workspace build cannot establish exclusion because workbench enables KAS through feature unification. CI must exercise the selected production package with and without defaults.

### D2 — first run and platform

Keep Free spawn and its existing actionable missing-bundle/Node/credential errors. No automatic wrapper or V2 fallback and no changed auth owner. Integrate accepted cyril-igrx platform changes before finishing the flip. Verify the rendered missing-bundle message, not just a reason-string helper.

### D3 — Host hook tags (cyril-5nep)

Carry optional `toolTags: string[]` through the existing typed host callback into the Host registry. Missing/empty tags remain meaningful; malformed present tags produce a diagnosed invalid-params error instead of silently weakening the request to ID-only matching. Do not change KAS-owned hook mode, execution authority, permission handling, or exit-2 blocking semantics.

The existing registry owns matching. Preserve raw selector semantics at load time and compile matching machinery once, using the existing regex dependency. No new generic matcher framework and no per-request regex compilation.

For PreToolUse/PostToolUse, mirror KAS 0.66.26's selector-plus-ID-regex behavior:

- A selector containing none of `\\ ^ $ ( ) [ ] { } | +` is eligible for tool-pattern matching.
- Literal tool patterns match exact supplied IDs or tags. Exact ID aliases are execute_bash/execute_pwsh and control_bash_process/control_pwsh_process in both directions; they are not tag prefixes.
- `*` and `?` patterns match complete IDs/tags with the other characters escaped. A wildcard-only pattern remains valid even if it cannot compile as a bare regex.
- Independently retain the existing unanchored ID regex fallback; regex-only patterns never run against tags. Preserve the existing Rust regex dialect rather than introducing a new JavaScript regex engine.
- A configured matcher requires toolId for tool matching, even if tags are present, matching KAS's guard. No matcher (including the upstream empty-source case) is unconditional within its trigger.
- An invalid matcher remains a diagnosed, nonmatching hook rather than being dropped or represented as an unconditional hook. Other trigger semantics stay unchanged.

Reference: local KAS 0.66.26 bundle `~/.local/share/kiro-research/kas-carves/2.28.0/node_modules/@kiro/agent/dist/server/acp-server.js`, SHA256 `3bc21b1f684cd3cc4aa0e10f198a5f97ea41398a63ed5db1858d4145fbe42828`. Helpers `wQ`, `T6t`, `OB`, `cXo` at bundled line 77 and `ARr`, `koe` at line 5492 establish classification, wildcard behavior, aliases and the missing-ID guard. Existing `kas-live-sweep-2.28.0.jsonl` records the bundle identity. No new live hook qualification has yet run.

### D4 — KAS advertised-command dispatch (cyril-oieu)

Use an explicit typed dispatch classification produced by the engine-specific command conversion. V2 agent commands retain their current execute/options RPCs. KAS advertises supported `custom-agent` commands for prompt submission; retain enough metadata to classify those commands, suppress unsupported types rather than registering an uncallable command, and preserve the exact four workflow exclusions. Do not infer protocol from names, selection flags, or input hints.

The registry retains builtin collision precedence. A prompt-routed command returns a typed submission result to App rather than sending the V2 RPC itself. App passes its original raw input through the existing turn/usage/steering lifecycle; it must not reconstruct `/name args` from trimmed parser outputs. There is no new KAS commands/execute extension, second prompt lifecycle, or native workflow control plane.

**Approved command-context boundary:** send recognized prompt-routed invocations as exactly one original text block, without automatic lesson prepending or file-expansion text. If this is the first prompt, keep first-prompt lesson injection pending for the next ordinary chat prompt. Ordinary chat, unregistered slash fallback, and their existing attachment/context behavior remain unchanged. Busy submissions retain normal steering semantics; they are not promised immediate standalone command execution.

Reason: KAS joins ACP text blocks before parsing slash commands. Prepending lessons prevents recognition; appending lessons or expanded file text changes the parsed arguments. Current registered agent-command execution does not automatically expand files or inject first-prompt lessons. Extending PromptEnvelope to encode separate resource context would add a new memory representation contract that is not needed to preserve exact command invocation. The selected exception must be documented rather than silently changing the first-prompt contract.

Reference: KAS 2.29.1 bundle `~/.local/share/kiro-research/kas-carves/2.29.1/node_modules/@kiro/agent/dist/server/acp-server.js`, SHA256 `ca0672e74c07a925cbeee0558e5e14bb77561deb2ffa0dc2d40ed4a72b441eba`. `bKr` advertises custom-agent commands; `p9e` at line 17061 parses slash text; prompt admission at line 17368 parses extracted text; `extractContentFromPrompt` joins text blocks. No commands/execute implementation exists in that bundle. Captures `experiments/conductor-spike/kas-lead-commands-2.29.1.jsonl:15,23` advertise general-task-execution and context-gatherer. Live invocation is still required, not inferred from advertisement.

### D5 — memory and deferred parity

Preserve upstream native KAS memory defaults exactly as approved in `spec.md`. No memory veto, native-memory configuration UI or claim that `/memory` governs the upstream system. The local lesson-injection exception in D4 is distinct from that native-memory decision. Preserve the spec's named follow-ups and known limitations without implying full parity.

## Placement / module shape

| Owner | Change and interface | Forbidden responsibilities / verification |
|---|---|---|
| Core KAS host callback parsing/dispatch (`domain_mediator/host.rs`, `kas/callbacks.rs`) | Carry typed optional tags and reject malformed present arrays. | No matcher logic or UI; callback-to-registry regression exercises the real parsing path. |
| Core Host hook registry (`kas/hooks.rs`) | Own compiled selector/regex classification and one membership predicate. Retain ownership; no module split. | No protocol spawning/auth or alternate hook execution path; registry outputs and real pre-tool execution establish behavior. |
| Engine-specific command converters and `CommandInfo` | Produce explicit supported dispatch classification; KAS metadata remains inside core conversion. | App must not inspect ACP/JSON or guess route from names. Compiler and independent review verify the seam. |
| Command registry / typed result | Store classified command route, preserve collision precedence, return prompt intent without rebuilding user text. Migrate all changed constructor/result callsites atomically. | No second bridge/prompt lifecycle. Tests exercise registered commands and builtin/V2 counterexamples. |
| App prompt orchestration | Wire typed intent into existing submit/steer/usage paths; preserve raw text and apply the approved command-context policy. Existing send/dispatch prompt seam owns context decisions. | Protected against new protocol parsing or command-name policy bodies. No generic abstraction or duplicated lifecycle. |
| `AgentEngine`, config, binary manifest | Feature-aware existing default owner and existing precedence. | No parallel selector. Default/no-default real binary runs cover the boundary. |
| CI and current docs | Selected-package feature lanes, ADR amendment, first-run/V2 instructions, platform facts, parity list and memory limitations. | No historical audit rewrites or unrelated dependency cleanup. |

No additional dependency or architecture parser/fence is proposed. Compiler checks and ordinary independent review are sufficient for these ownership constraints. No supplied size budget exists.

## Claims / falsification

Implementation evidence is owned by the slice checkpoints in `plan.md` and their referenced tracker records. These are the approved claims, not a separate changing results ledger.

| ID | Claim and covered cases | Falsifier / verification | Regression coverage |
|---|---|---|---|
| C1 | Default production build/config runs KAS; explicit V2 and isolated no-default binary remain usable. | Real selected-package TUI runs: default KAS turn, explicit CLI V2, config V2, CLI-over-config; no-default V2 turn. Wrong engine/spawn or lost explicit override falsifies. | Existing config/CLI/bridge behavioral tests extended at the selection boundary; fake V2 process fixtures pinned explicitly. |
| C2 | First-run failure is actionable and never silently changes engine/auth ownership. | Isolated missing-bundle TUI run renders the searched path and extraction/override remedy. | Existing discovery failure tests; integration observation of BridgeDisconnected reaching UI. |
| C3 | Tagged Host hooks execute under the same authority; ID regex compatibility and negative cases remain. | Before/after callback→registry reproducer using execute_bash with shell/@builtin tags; live tag hook writes a marker, and an exit-2 pre-tool hook blocks a disposable write. | Table covers shell/read/@builtin/@mcp, wildcard-only, wildcard tag/ID, ID aliases, ID-regex fallback, regex-only not applied to tags, absent ID, absent/empty tags, malformed tags, invalid matcher, nonmatching tools and ordinary unfiltered hooks. Use existing registry/callback test conventions, not private representation assertions. |
| C4 | A genuine advertised KAS command reaches normal ACP prompt handling with original text, never a V2 command RPC. | App-to-bridge regression red before repair, then green; live invocation of a currently advertised non-workflow command and completed response. Include no-arg and quoted/multi-space inputs. | Supported/unsupported type classification, builtin collisions, exact workflow exclusions, V2 options/execute controls and no-session/busy submission behavior. |
| C5 | Command invocation remains parseable when local lessons are available; normal chat retains lesson context and source privacy. | First registered command emits only original text and leaves pending lesson injection intact; next ordinary prompt receives prepared context once. Raw source/transcript exclude injected context as before. | Existing prompt/context lifecycle tests extended for command→ordinary prompt, session switch/cancel/busy boundaries where affected. No resource-envelope redesign. |
| C6 | All-platform default and feature policy are accurately documented and enforced. | Integrate accepted PR #167 source/evidence; selected-package default/no-default CI, Cargo tests/clippy/format/docs, real consumer receipt and independent assembled review. | Preserve existing platform discovery tests and no-KAS gates; current docs/ADR/parity review. |
| C7 | Native KAS memory is not changed or represented as controlled by Cyril. | Scoped source review of unchanged memory settings and current user docs. | N/A — approved risk is owned by spec Approval; no fabricated no-overlap guarantee or new memory test. |

Bug sensitivity uses the real pre-fix/post-fix reproducer, not arbitrary mutations. Additional custom oracles are unnecessary: source-established grammar, representative fixtures, runtime consumer observations and review cover the claims. Never use the stale orchestration-tool smoke as evidence of current KAS behavior. Required checks cannot be waived as a known failure without the contract's explicit baseline-risk process.

## Non-goals and future work

The spec owns the parity list and accepted native-memory risk. Do not broaden these repairs into general steering-discovery, settings-version policy, interactive terminals, memory UI, model fallback, auth-owner migration, or workflow-command replacement. The matcher repair preserves existing regex-engine limitations; no new JS regex dependency is introduced.

Prerequisite branches can be reviewed and published independently once qualified and authorized. The default switch integrates both repairs and PR #167 before final proof; it does not close or claim unlanded prerequisite work as complete.

## Approval

2026-10-12 requester selected **“Approve and implement”** in response to approval of D1–D5, placement/verification, and the explicit D4 command-context boundary. This authorizes implementation of independent prerequisite repairs followed by the integrated default switch, not PR merges. Prior scope and native-memory risk decisions remain owned by spec Approval. Budgeted-plan owns atomic slices; checkpointed-build owns their verification.
