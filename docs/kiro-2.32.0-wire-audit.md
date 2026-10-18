# kiro-cli 2.32.0 wire audit — no new KAS methods, three payload shifts, and a P1 that was cyril's own

Audited 2026-10-18. Baseline: 2.31.1 ([kiro-2.31.1-wire-audit.md](kiro-2.31.1-wire-audit.md)). 2.32.0 is the
only release after 2.31.1 (the user first asked for 2.32.2, which no channel publishes).
Scope: discovery on the **KAS (v3) track**: new or changed commands, messages carrying new fields, and feature
flags for new or limited-release features. KAS is cyril's current path; v2-only findings are demoted.
Five lanes: KAS static (core + workflow merged, because the KAS delta is one file), TUI-bundle static, Rust-host
static, the standing **v3-coverage** parity inventory, and one live lane (KAS 0.66.34 vs 0.66.43 paired on the
same day, the host's cloud path, and the real cyril binary on KAS through hex1b).

**Verdict: SAFE for cyril — no wire break.** KAS 0.66.34 → 0.66.43 adds no `_kiro/*` method (123 = 123), no
`session_info_update` kind (25 = 25), no `sessionUpdate` variant and no feature flag (30 = 30, same defaults); the
initialize result is identical live. The release moves behaviour through **fields on existing frames**. The audit's
most important result is not a 2.32.0 change at all: a pre-existing cyril bug that breaks every shell approval for
bash/zsh users on KAS.

The three findings that matter most:

1. **cyril's `shell_type` reply defeats every KAS shell approval for bash/zsh/sh users (P1, LIVE) — cyril-k8wc.**
   cyril answers `_kiro/terminal/shell_type` with the family name `"posix"`; KAS's dialect mapper accepts only
   concrete names and maps `"posix"` to `"unknown"`. KAS then ignores V3 allow rules, re-asks every command, and
   marks each request `persistableConsent:false`, so cyril's overlay offers only Allow / Deny. Same on 0.66.34. It is a
   regression from cyril's own cyril-6bol fix (2026-08-02). Fix: report the shell's basename.
2. **"Always allow" persists only through the permission-response consent echo, and cyril writes nothing on KAS
   (P1, LIVE) — cyril-auxt re-scoped.** KAS writes `permissions.yaml` (user or workspace scope) only when the response
   carries `_meta.kiro.consent{capability, scope, resource, workspaceRoot}`. Kiro's TUI has sent this echo **since
   2.31.1** (corrects the 2.31.1 coverage row). cyril sends none and, on KAS, also writes no V2 or V3 rule, so every
   grant dies with the session. The 2.32.0 custom-folder write approval is the same echo with a `"<dir>/**"` resource.
3. **0.66.43 payload shifts that reach cyril:** a repeat-cap workflow pause now **auto-wakes the parent (billed,
   unlabelled in cyril) — cyril-6lql**; every workflow event carries `attention[]` (which step waits on the user) —
   note on cyril-4u4a; and output styles grow to five, chosen per prompt with `session/prompt _meta.kiro.outputStyle`,
   which cyril never sends — cyril-j66p.

Evidence: [`experiments/conductor-spike/audit-2.32.0/`](../experiments/conductor-spike/audit-2.32.0/) (`RUN-STATE.md`,
`<lane>-findings.md`, live captures); trimmed capture bundles `experiments/conductor-spike/kas-*-2.32.0.jsonl`,
`host-cloud-2.32.0.jsonl`. Capture names below refer to the live lane directory; the frame-level citations (135, checked by
`check-citations.py`) are in `audit-2.32.0/live-findings.md`.

## 1. Releases and artifacts

| ver | build (UTC) | BUILD_HASH | tar sha256 | @kiro/agent | KAS | TUI | host |
|---|---|---|---|---|---|---|---|
| 2.32.0 | 2026-10-15T21:16Z | d6fb1e94 (== 2.31.1; a train stamp, trap 20) | 9d1527e4… (== latest manifest) | 0.66.43 | distinct: one file | distinct | distinct (all three) |

- installed `kiro-cli`, `kiro-cli-chat`, `kiro-cli-term` == archived 2.32.0 (`cmp`); KAS carve == self-extracted
  `~/.local/share/kiro-cli/kas/2.32.0-f3c56062…`; TUI carve == installed `tui.js` (sha256 `eba39f7b7914f608`).
- **KAS:** all 12 `dist/server/*.d.ts` byte-identical; `acp-server.js` 12,702,158 → 12,963,334 B (+261,176);
  `externals.json` −`@huggingface/transformers` +`onnxruntime-node`; `node_modules` loses `sharp`, `@img`,
  `@huggingface`, `protobufjs`, `@protobufjs`, `long`, `flatbuffers`, `guid-typescript`, `platform`, `detect-libc`
  (carve 176 M → 153 M). They were transitive dependencies of transformers.js; the Knowledge tool's embeddings now
  run an inlined tokenizer on `onnxruntime-node` (MiniLM-L6, 384 dims, model assets pinned by commit and sha256).
  Image resizing was already photon-based.
- **Frames** (csize / dsize): KAS tar 27,808,487 / 183,255,040 → 20,826,442 / 159,590,400; Node and Bun ELFs
  size-identical; TUI 2,124,271 / 13,692,707 → 2,133,192 / 13,723,687.
- **Host size:** `kiro-cli-chat` 216,000,680 → 207,496,744 B. The KAS frame accounts for −6,982,045, the TUI frame
  +8,921, and the rest is −1.53 MB of real Rust code (`.text` −887,200) spread across the V1 chat crate. No crate or
  first-party source file was removed (rustls 0.23.45 → .46).
- **Changelog source:** `stable/2.32.0/feed.json` (34 lines), copied to `audit-2.32.0/feed-2.32.0.json`.

### 1a. Changelog reconciliation

| # | line (abridged) | location / verdict |
|---|---|---|
| 1 | `--cloud`/`--repo` non-interactive, print cloud session id | host `launch.rs` (+468 lines): gates on KAS `executionTargets`/`sourceProviders`; LIVE §5 |
| 2 | OSC 7501 program status; `KIRO_NO_PROGRAM_STATUS` | TUI only (T10); cyril-j9r4 |
| 3 | [V3] write approvals: custom folder at session/workspace/user scope | TUI composes the existing consent echo (`resource:"<dir>/**"`); KAS unchanged; LIVE §2a |
| 4 | [V3] `/workflow run` confirmation form with plan preview | TUI reuses `listRecipes[].plan`; no new method; cyril-4u4a note |
| 5 | [V3] Proactive / Explanatory / Learning output styles | KAS `outputStyle` option +3 values; per-prompt `_meta`; LIVE; cyril-j66p |
| 6 | [V3] workflow coding/planning/design steps use Powers | `wf-coder`/`wf-planner`/`wf-design` + `kiro_powers` |
| 7 | [V3] workflow design steps use configured MCP | `wf-design` `includeMcpJson:true` |
| 8 | Startup What's new introduces memory | TUI, gated on rollouts `memory` + `memory_controls`; cyril-0na7 note |
| 9 | [V3] Always Allow scope page shows scope, `s` cycles | TUI-local (label `global` "always" → "user") |
| 10 | V2 subagents follow the session's model fallback | host v2 `inherited_model_fallback`; demoted |
| 11 | [V3] step session opens on its task | KAS passes `originalUserRequest` as first-turn context instead of prepending it |
| 12 | [V3] prompt-created workflow shows its plan for approval | KAS creator-as-step + `request_permission` with `workflowPlan`; LIVE; unreachable in cyril (cyril-8wh0) |
| 13 | [V3] no `sharp`/libvips bundled | §1 |
| 14 | [V3] running workflows accept plain-language changes | `update_workflow{replace_remaining, workflowPrompt}` → `edit-workflow-r<N>` step (model tool; unreachable in cyril) |
| 15 | [V3] saved-session lists refresh without blocking | KAS session-index background revalidation |
| 16 | [V3] review loops pause after two rejected reviews | bundled `feature-pipeline` loops 3/abort → 2/pause; repeat-cap parent wake (§4) |
| 17 | [V3] `/chat save` non-`.zip` name says zip archive | TUI copy |
| 18 | [V3] `/chat load` explains an altered archive | host `import-session` message |
| 19 | [V3] non-interactive `--resume-id` of a cloud session | host `launch.rs` |
| 20 | MCP OAuth links disappear on connect | TUI `_kiro/mcp/status` notice lifecycle; dark `_kiro/mcp/oauthCompleted` (§7) |
| 21 | plain-text non-interactive explains cancel/refusal | host strings; LIVE: group SIGINT exits 130 with no message (CHANGED) |
| 22 | non-interactive exits nonzero when stdout closes | host logic; LIVE CONFIRMED (`PIPESTATUS=1 0`) |
| 23–25 | V2 `/model` fallback None, vendor-default row, crew stage error | host v2; demoted (cyril-ci3j note) |
| 26 | [V3] paused step in a parallel branch finishes | KAS `update_workflow` `nodeId` may name a paused step |
| 27 | [V3] turn not ended early when reasoning spends the output budget | KAS max-tokens recovery (one re-invoke); LIVE NOT-REACHED |
| 28 | [V3] buffered workflow messages survive a restart | KAS `notification.{bufferOnly,callerSessionId}` + persisted handoff files; LIVE |
| 29 | [V3] command approval checks the running shell's syntax, incl. fish | KAS feeds the terminal dialect into approvals; new fish parser; exposed cyril-k8wc |
| 30 | [V3] completed steps can send follow-up to their parent | KAS finished-step `send_message` info allowed |
| 31 | [V3] org MCP registry `oauth`/`oauthScopes` | KAS registry schema + merge |
| 32 | [V3] oversized images retry at the provider limit | KAS `imageRefit`; LIVE: a 9000×9000 PNG succeeds on both engines, no wire delta |
| 33 | [V3] resumed sessions show saved model/effort early | KAS cold-catalog one-option pin on `session/load`; LIVE |
| 34 | [V3] pause messages no longer promise auto-resume | KAS pause texts reworded; LIVE (graceful shutdown) |

## 2. cyril on KAS by default: the parity list (cyril-4c4d)

### 2a. Flip blockers at HEAD 7c7b0d1d

| # | issue | what a cyril user on KAS sees | status |
|---|---|---|---|
| 1 | cyril-zbu7 | the session dies ~1 h after the last kiro-cli token refresh | open, unchanged |
| 2 | **cyril-k8wc** (NEW) | every bash/zsh shell command asks, every time, with only Allow / Deny | LIVE-confirmed; child of cyril-c6hv |
| 3 | cyril-auxt (re-scoped) | "Always allow" never outlives the session; cyril writes nothing on KAS | blocked on k8wc; fix = consent echo (gn07 folded in) |
| 4 | cyril-ebqu + cyril-1v25 | sub-agent tool rows un-nested; its answer and MCP "Tool Load" rows dropped | open (P2), re-seen live |

Fixed since 2.31.1: **cyril-oieu** (#169, ff878634; skills, prompts and custom agents dispatch as prompt text; issue
still `in_progress`, awaiting closure) and **cyril-vh69** (#171, closed; `/model`, `/effort`, `/mode` verified live on
0.66.43).

**Permission persistence, live (TUI-L1, V1, S4/S4b):** KAS S8e reads `consent.scope || "session"`; `persistConsent`
writes `$HOME/.kiro/settings/permissions.yaml` (user) or `$HOME/.kiro/workspace-roots/<hash>/permissions.yaml`
(workspace) and a fresh `session/new` then runs without asking (`kas-lead-shellecho-2.32.0.jsonl`). A bare
allow_always covers only the exact resource for the session (control arm re-asks for `src/b`). The TUI's echo builder
is `{kiro:{consent:{...capability, scope, ...resource, ...workspaceRoot}, ...rejectionReason}}` in both 2.31.1 and
2.32.0. cyril sends only `rejectionReason` (`crates/cyril-core/src/protocol/convert/kas.rs:67-91`), and its V2
trust-write path never runs on KAS because KAS options carry no trust-tier patterns (`crates/cyril/src/app.rs:1913-1918`).

### 2b. P2/P3 parity items this cycle

| item | cyril | issue |
|---|---|---|
| output styles (5) via `_kiro/config/template` + per-prompt `_meta.kiro.outputStyle`; `set_config_option outputStyle` is a silent no-op | NOT-SENT (`domain_mediator/commands/session.rs:475` builds the prompt with no `_meta`) | cyril-j66p (P2) |
| `settings.thinking` removed from AgentSettings → `intake.unknown-key` | still sent (`kas/settings.rs:79,96`); `/thinking` via configId still works | cyril-53qx note |
| `customAgent/{not_found,config_error}` | DROPPED (`convert/kiro.rs:1284`) | cyril-7xxk (P3, filed this cycle; proposed in 2.31.1) |
| model `rateMultiplier` credit column + picker sort (TUI) | DROPPED (`convert/kas.rs:108`) | cyril-lxuo note |

Unchanged from 2.31.1: `/context` `/compact` (cyril-1aq0), compaction visibility (cyril-58uv), MCP status and OAuth
(cyril-nk4o, cyril-tqr8), model fallback (cyril-aoby), governance (cyril-0o7e). Full nine matrices:
`audit-2.32.0/v3-coverage-findings.md`.

## 3. KAS 0.66.34 → 0.66.43

| item | gate / default | live | cyril | issue |
|---|---|---|---|---|
| `_kiro/terminal/shell_type` dialect mapper (`cb` → `jh`, same logic): `"posix"` → `"unknown"` → `{effect:"ask"}`, `persistableConsent:false`; dialect now also fed into execute_bash approvals (`InterpreterChangedError` on mismatch) | always | `kas-lead-shell-2.32.0.jsonl`, `kas-lead-shell-2.31.1.jsonl` | BROKEN for bash/zsh/sh (`kas/host_shell.rs:166-171`) | cyril-k8wc (P1) |
| `outputStyle` options `[default, concise]` → `[default, proactive, concise, explanatory, learning]` (template only; `promptScoped`) | per prompt | `kas-live-sweep-2.32.0.jsonl` | NOT-SENT | cyril-j66p |
| client capability `_meta.kiro.diagrams` → `create_diagram` tool, diagram IR in `<session>/diagrams/dg_<uuid>.json`, tool_call `_meta.kiro.{toolId, diagram{id,sessionId,schemaVersion}, diagramError}` | opt-in, off; Kiro's TUI does not send it | `kas-lead-diagrams-2.32.0.jsonl` | NOT-SENT | cyril-68ny (P3) |
| client meta `agentHostsForegroundShell` (KAS runs foreground shell itself even with `terminal:true`) | opt-in | STATIC | NOT-SENT | cyril-7p7n note |
| AgentSettings `thinking` REMOVED; thinking/effortLevel are session-layer config rows (`session-reasoning` source in the bundle) | — | kiro.log `unknown-key` (KAS-L3) | still sent | cyril-53qx note |
| hidden settings `evalStrategy`/`evalTreatments` + env `KIRO_EVAL_STRATEGY`/`KIRO_EVAL_TREATMENTS` → `x-kiro-eval-*` headers pin a backend A/B treatment | unset | kiro.log: `evalStrategy` now a known key | NOT-SENT (must stay so) | — |
| `workflows.creatorEffort` (`low`…`max`) for the creator step | optional | STATIC | NOT-SENT by design (ADR-0011) | cyril-0asq note |
| `session/request_permission` gains `toolCall.content` Text + `_meta.kiro.workflowPlan`; `pending_interaction` carries it | workflow gate on | `kas-lead-wfplan-2.32.0.jsonl` | unreachable (ADR-0011) | cyril-8wh0 (P3, blocked on 0asq) |
| cold-catalog one-option model pin in the `session/load` result | default | `kas-workflow-gateon-2.32.0.jsonl` | HANDLED (`domain_mediator/commands/session.rs:181-190`) | — |
| max-tokens "reasoning ate the budget" re-invoke (`syntheticUserMessageReason:"max-tokens-recovery"`), no new stop reason | always | NOT-REACHED | would replay as user text | — |
| new fish parser; `imageRefit` retry; MCP registry `oauth`/`oauthScopes`; hooks `didChange` per-session source; session-index background revalidation; c2s `analysisScope` | — | image: no wire delta | n/a | — |

Flags: KAS feature-flag table 30 = 30 (same keys, defaults, env names); `AB_*` 16 = 16; `KIRO_*` +`KIRO_EVAL_STRATEGY`,
+`KIRO_EVAL_TREATMENTS`. Memory default unchanged `{mode:"disabled", reflection:false}`.

## 4. Workflows (v3)

Reachability first (ADR-0011): cyril never sets `workflows.enabled`, so KAS never registers the model-facing workflow
tools, and `_kiro/workflow/new` takes only `workflowPath` or an inline `workflow`. Rows below say which side they fall on.

| item | reachable in cyril? | live | cyril | issue |
|---|---|---|---|---|
| **repeat-cap pause auto-wakes the parent**: `_kiro/session/notify` (severity info) + `send-message-wake` turn, billed (0.223 credits, ran `read_file`); 0.66.34 wakes only on terminal statuses | yes (any recipe with `onMaxIterations:"pause"`; bundled `feature-pipeline` is now 2/pause) | `kas-workflow-gateoff-2.32.0.jsonl`, `#482` | unlabelled generic wake (`turn_mediator.rs:1297-1323`; labels only terminal runs, `workflow.rs:836-849`) | cyril-6lql (P2) |
| `attention[]` `{kind:"permission"\|"question", since, toolCallId?, consentRound?, nodeId, nodePath, sessionId}` on every lifecycle event, state, node and list row | yes (gate-off verified) | `kas-lead-attention-2.32.0.jsonl` | DROPPED (`workflow.rs:294-316`) | cyril-4u4a note |
| `inspect` gains `pendingSteps`; `steps_queued` re-emitted on restore | yes | `kas-lead-wfplan-2.32.0.jsonl` | DROPPED / HANDLED (`workflow.rs:746-760`) | cyril-4u4a note |
| step-verdict messages persisted with `notification.{bufferOnly, callerSessionId}`, replayed as user chunks | yes | `kas-workflow-gateoff-2.32.0.jsonl` | DROPPED; renders as user text on replay | cyril-fb1m note |
| creator-as-step: `workflowPrompt` launches a `create-workflow` step that asks plan approval; plain-language edits run `edit-workflow-r<N>`; with the gate on, the parent model answered a rejected plan for the user | **no** | `kas-lead-wfplan-2.32.0.jsonl` | — | cyril-8wh0 (fixture `kas-workflow-plan-2.32.0.jsonl`) |
| sequence `includedFrom{name,path}` on plan and node tree; include paths must be absolute / `bundled://` / `generated://` | yes | `kas-lead-include-2.32.0.jsonl` | DROPPED (`convert/kas/workflow.rs:341-345`) | cyril-inwg note |
| pause texts: "…; resuming the run retries the step." (graceful shutdown; SIGKILL keeps "Interrupted by agent restart") | yes | `kas-lead-interrupt-sig15-2.32.0.jsonl` | HANDLED verbatim | — |
| bundled agent `wf-code-reviewer` (registered, unreferenced); `wf-workflow-creator` re-described; `update_workflow` loses `remainingSteps`; nested workflows refused | model-facing | `kas-init-2.32.0-callback.jsonl` | n/a | cyril-inwg note (skill refresh) |

Recipe zod schema unchanged (minifier rename only); `docs/kiro-kas-workflow-recipe-schema-2.26.0.json` stays valid.
Bundled recipe set unchanged (7). All six gate-off runs match event-for-event across engines (`wf_parity.py`).

## 5. v2 (Rust) engine and launcher — demoted

- **Kiro's host is now a second KAS client for cloud sessions.** Non-interactive `kiro-cli chat --cloud` gates on
  KAS `executionTargets`, `--repo` on `sourceProviders` + `_kiro/sourceProviders/list`. LIVE: the host passes env
  `KIRO_REMOTE_SESSIONS_ENDPOINT` to KAS, which then advertises `executionTargets:["local","cloud-sandbox"]`,
  `sourceProviders:true` and 30 extension methods (a direct spawn: `["local"]`, false, 28); `session/new` carries
  `_meta.kiro{executionTarget:{kind:"cloud-sandbox"}, sessionSource:"remote", isEmptyWorkspace:true}`
  (`host-cloud-2.32.0.jsonl`). This account is cloud-entitled. cyril reads neither key (`protocol/fingerprint.rs:30-36`)
  — cyril-tikf note.
- **Rollout registry:** 23 rows; only `v3_prompt.internal_treatment_percent` 50 → 75 (external 10%). No row for
  cloud, OSC 7501 or memory. `acp --agent-engine` default still `v2`; Classic deprecation copy unchanged — cyril-a7nk note.
- **Env:** +`KIRO_STANDALONE_CUTOVER` (chat env table, autocomplete distribution, unannounced).
- **Settings keys** 120 → 123: +`chat.disabledModelFallbacks`, +`chat.modelPicker.sortBy`, +`chat.modelPicker.lastUsed`.
- **v2:** new `chat-cli-v2/src/api_client/fallback_choice.rs`; `FallbackArgs` +`disabled`; subagent spawn
  `inherited_model_fallback`; crew `DiscardUnstartedSpawn`; inbound `_kiro.dev/telemetry/modeChanged` — cyril-ci3j note.
- **Doc manifests** (103 + 140 docs) byte-equivalent: no unannounced feature node this cycle.
- **Unchanged:** `acp --help`, KAS token path (`kas_token.rs`, `get-kas-token`), hidden helpers.

## 6. What Kiro's own TUI now sends and consumes

- **Handshake frozen:** `initialize` clientMeta and `.settings`, `session/new|load|prompt` `_meta` (`displayText`,
  `outputStyle`), the 7 parsed agent capabilities and the KAS spawn argv/env are identical after renames. 74 = 74 slash
  commands. 10 = 10 rollout gates (`memory` 1 → 2 call sites). It does not send `diagrams` or
  `agentHostsForegroundShell`, and reads none of 0.66.43's new fields.
- **Consumes one new method, dark:** `_kiro/mcp/oauthCompleted {serverName, success}`, armed only by
  `_kiro/openExternalUrl` with new params `serverName` + `oauthCompletion:"notification"`; KAS 0.66.43 has 0 hits for
  either — cyril-nk4o note.
- **Permission responses:** the consent echo (§2a); the folder-grant offer is KAS-only, main session only,
  `capability==="fs_write"`, not `askType:"explicit"`.
- **TUI-only:** OSC 7501 program status (cyril-j9r4); model picker sort and credit column (cyril-lxuo);
  `PromptFailedError` turn-failure copy; What's-new memory section; `/chat save` zip copy; V2 fallback picker.

## 7. Unannounced or dark features

| feature | gate | default | evidence |
|---|---|---|---|
| `create_diagram` + diagram IR viewer contract | `clientCapabilities._meta.kiro.diagrams` | off; no Kiro client sends it | LIVE fixture `audit-2.32.0/live/kas-lead-diagram-ir-dg_4a819cea-….json` |
| backend experiment pinning | `settings.evalStrategy`/`evalTreatments`, `KIRO_EVAL_*` | unset | bundle; kiro.log known-key |
| agent-hosted foreground shell | `_meta.kiro.agentHostsForegroundShell` | off | bundle |
| MCP OAuth completion notification | TUI-armed; KAS sends nothing | dark | TUI `_kiro/mcp/oauthCompleted` |
| `wf-code-reviewer` bundled agent | registered, unreferenced | — | `kas-init-2.32.0-callback.jsonl` |
| `KIRO_STANDALONE_CUTOVER` | env | unset | host env table |

## 8. Live results

All legs serial on 2026-10-18 17:17–18:45 UTC, one backend, one in-driver token renewal, no login/logout, no forging.
135 frame citations pass `check-citations.py`.

| leg | result |
|---|---|
| 0 handshake | initialize identical OLD/NEW; only init delta is `_kiro/configuration/state` (new bundled agent, re-described creator) |
| 1 sweep | parity; run 1 voided (driver bug) and re-run. **Correction:** "session/load now carries the saved model" was catalog warmth; the real change is the cold-catalog pin |
| 2a/2b workflow gate off/on | 6/6 and gate-on runs match; NEW: repeat-cap wake, `attention[]`, `bufferOnly` |
| 3 leads | every lead has a verdict (`live-findings.md`, Lead verdicts): KAS-L1 CONFIRMED (worse: no always-allow), TUI-L1 CONFIRMED (not new), TUI-L2 CONFIRMED (no visible change on a non-coding prompt), HOST-L1 REFUTED for this account (entitled), HOST-L3 CHANGED (`--repo` warns then runs), HOST-L4b CHANGED (no cancel message) |
| 4 field sweep | only NEW paths: `attention[]` and `notification.{bufferOnly,callerSessionId}`; `unread-fields.py` 79 UNREAD triaged into `docs/wire-ignored-fields.txt` (42 lines) → exit 0 |
| 5 cyril-on-KAS (cyril b9b6e95d, hex1b) | S1 startup OK (~5.7 s to model); S3 pickers work; S4 (SHELL=bash) only Allow / Deny; S4b (fish) Always-allow is session-only, nothing written; S5/S6 orphans dropped; S7 KIRO_HOME agents load. **S2 VOID**: the binary predates #169 (merged 19 min after the build) |

New cyril bugs from the user's seat: the approval overlay logs a WARN on every render frame (~14k lines per approval;
cyril-q0kg), and write paths are clipped at the box width (cyril-rcub). CLAUDE.md's `cyril.log` location is stale
(cyril-s18c).

Side effect: HOST-L1/L3 created three real cloud sessions on the entitled account; the lane deleted them with
`--delete-session` (remote deletion unverified). The raw strace held a live token and was deleted after redaction.

## 9. Issues

New this cycle:

| id | P | title |
|---|---|---|
| cyril-k8wc | P1 | KAS: shell_type reply "posix" makes KAS treat every command as an unknown shell |
| cyril-j66p | P2 | KAS output styles: cyril never sends `session/prompt _meta.kiro.outputStyle` |
| cyril-6lql | P2 | repeat-cap workflow pause auto-wakes the parent (billed) and cyril shows it unlabelled — P2 by user decision (lki9/pd4b policy: the wake is accepted, the fix is the label) |
| cyril-q0kg | P2 | approval overlay logs a WARN on every render frame |
| cyril-8wh0 | P3 | KAS workflow plan approvals render as "Preview unavailable" (unreachable until cyril-0asq) |
| cyril-rcub | P3 | approval overlay clips long write paths |
| cyril-68ny | P3 | KAS `create_diagram`: decide whether to advertise `diagrams` and how to render the IR |
| cyril-7xxk | P3 | KAS `_kiro/customAgent/{not_found,config_error}` dropped |
| cyril-j9r4 | P4 | OSC 7501 program status |
| cyril-s18c | P4 | docs: CLAUDE.md `cyril.log` location |

Notes added: cyril-auxt (re-scope to the consent echo; blocked on k8wc), cyril-gn07 (answered; `needs-info` dropped,
folded into auxt), cyril-4c4d (blockers at HEAD), cyril-4u4a, cyril-inwg, cyril-53qx, cyril-tikf, cyril-a7nk,
cyril-nk4o, cyril-lxuo, cyril-1v25, cyril-ebqu, cyril-fb1m, cyril-58uv, cyril-7p7n, cyril-iq2c, cyril-0na7, cyril-0asq,
cyril-lyrx, cyril-ci3j. cyril-k8wc and cyril-7xxk are children of cyril-c6hv.

## 10. Method notes for the next audit

New traps this cycle (also in `.claude/skills/kiro-release-audit/references/traps.md`):

- **Name the cyril entry point before rating a gap.** The creator-as-step plan approval was first filed P2; ADR-0011
  keeps the workflow gate off and `_kiro/workflow/new` takes no `workflowPrompt`, so it is unreachable (P3).
- **Spread-built objects hide from literal greps.** The TUI's consent echo is built with spreads; `consent:{capability`
  matches 0. Grep the container key (`kiro:{consent`) and read the object. This hid the echo from the 2.31.1 audit.
- **Build cyril at the HEAD the verdict is about, and re-check HEAD at fold time.** Another session merged the fix
  under test 19 minutes after the leg-5 build.
- **A cloud-entitled account turns `--cloud` probes into real remote sessions.** Ask before a probe whose success path
  creates remote state; delete it in the same leg.
- **Pair `session/load` legs at the same catalog warmth** (trap 1 variant): a warm vs cold model catalog changes the
  load result's config options on both engines.
- **Probe drivers answer `shell_type` with `"bash"`**, so no driver capture can show a cyril-specific host-callback
  bug; the cyril-on-KAS leg (or a cyril-shaped arm) is the only place cyril's own replies are exercised.
