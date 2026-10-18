# LIVE lane — kiro-cli 2.31.1 → 2.32.0 wire audit (KAS 0.66.34 → 0.66.43)

Started 2026-10-18 17:17 UTC. Lane dir: `experiments/conductor-spike/audit-2.32.0/live/`.
All legs strictly serial; direct KAS carve spawns (`<node> --experimental-wasm-modules <acp-server.js>
--transport=stdio --auth=acp-callback`), fresh temp `HOME`, real `XDG_DATA_HOME`.

Engines
- OLD = `~/.local/share/kiro-research/kas-carves/2.31.1/node_modules/@kiro/agent/dist/server/acp-server.js` (0.66.34, 12,702,158 B)
- NEW = `~/.local/share/kiro-research/kas-carves/2.32.0/node_modules/@kiro/agent/dist/server/acp-server.js` (0.66.43, 12,963,334 B)
- node = `~/.local/share/kiro-cli/node` (v22, bundled)

## Summary

1. All legs ran serially 2026-10-18 17:17-18:45 UTC on one backend; direct KAS carves 0.66.34 (2.31.1) and 0.66.43 (2.32.0), the archived 2.32.0 host for HOST leads, and cyril b9b6e95d (`--features kas`) for leg 5. One in-driver token renewal (17:52), no login/logout, no forging. hex1b drove cyril.
2. **KAS-L1 / cyril-k8wc CONFIRMED LIVE (P1, pre-existing on 0.66.34 too):** `shellType:"posix"` (what cyril sends whenever `$SHELL` is bash/zsh/sh) makes KAS ignore V3 allow rules, re-ask every command and withhold "Always allow" (`persistableConsent:false`). With `bash`/`zsh` the same rule auto-runs. cyril sends `fish` for this user (SHELL=/bin/fish), `posix` with SHELL=bash.
3. **Always-allow persistence (cyril-auxt) works only through the response `_meta.kiro.consent` echo** (scope session/workspace/user, folder globs), the same on 0.66.34 and 0.66.43. A bare allow_always covers only the exact resource for the session. On KAS cyril writes nothing (no V2 and no V3) and sends no echo, so every grant is session-only (seen from the user's seat in S4b).
4. **NEW P1 candidate:** in 0.66.43 a repeat-cap `paused` run auto-wakes the parent with `_kiro/session/notify` + a `send-message-wake` turn (billed, 0.22 credits, with tool calls). After a user rejects a workflow plan, the parent model then answers the creator for the user ("plan is exactly right … proceed").
5. 0.66.43 wire deltas (LIVE): workflow `attention[]` on every event, state and list (permission/question); `_kiro/workflow/steps_queued`; `notification.{bufferOnly,callerSessionId}`; output styles proactive/explanatory/learning via per-prompt `_meta.kiro.outputStyle`; the `diagrams` capability adds `create_diagram` with a JSON IR; `settings.thinking` is now an unknown-key; the cold-catalog one-option model pin on `session/load`; the new pause text on graceful shutdown; bundled agent `wf-code-reviewer`; `evalStrategy` becomes a known key. initialize and extensionMethods are unchanged.
6. cyril-on-KAS: cyril-oieu still fails ("command failed"). cyril-vh69 pickers WORK. The sub-agent "Subagent Response" and MCP "Tool Load" orphans are still dropped (answer visible via the parent tool), and the child's tool row is un-nested. KIRO_HOME agents load. New cyril bugs: the approval-preview WARN spams the log (~14k lines per approval) and the write-approval path is truncated.
7. HOST: this account IS cloud-entitled. The host enables cloud via env `KIRO_REMOTE_SESSIONS_ENDPOINT`, which a direct spawn lacks, and sends `session/new _meta.kiro{executionTarget:{kind:"cloud-sandbox"}, sessionSource:"remote", isEmptyWorkspace:true}`. `--repo` with an unknown repo warns and runs anyway. The lane created three cloud sessions and deleted them.
8. NOT run: v2 pair (leg 6, trap 24), TUI-L4 OAuth (no OAuth-requiring MCP server; static says dark), HOST-L5. NOT-REACHED: max-tokens recovery, byte-oversized image retry.

## Legs

| leg | command / probe | capture(s) | duration | status |
|---|---|---|---|---|
| 0 help | `node --experimental-wasm-modules <acp-server.js> --help` both | `tmp/help-{old,new}.{out,err}` | 2×<1 s | completed |
| 0 init | `probe-acp-init-2.32.0.py` AUTH=callback/none × OLD/NEW | `kas-init-{2.31.1,2.32.0}-{callback,none}.jsonl` | 4×3-4 s | completed |
| 1 sweep | `probe-kas-live-2.32.0.py` SCENARIO=sweep OLD→NEW | `kas-live-sweep-{2.31.1,2.32.0}.jsonl` | 54.8 / 52.3 s | run1 VOIDED (p4 on bogus model), re-run completed |
| 2a workflow gate off | same driver SCENARIO=workflow GATE=off | `kas-workflow-gateoff-{2.31.1,2.32.0}.jsonl` | 144 / 230 s | completed |
| 2b workflow gate on | GATE=on | `kas-workflow-gateon-{2.31.1,2.32.0}.jsonl` | 257 / 520 s | completed (NEW +263 s = driver wait + one natural 178 s stall) |
| 3 KAS-L1 shell_type | `probe-kas-leads-2.32.0.py shell` | `kas-lead-shell-{2.32.0,2.31.1}.jsonl` | 70 / 58 s | completed |
| 3 TUI-L1 folder grant | `… folder` | `kas-lead-folder-2.32.0.jsonl`, `kas-lead-folder-control-2.32.0.jsonl`, `kas-lead-folder-2.31.1.jsonl` | 364 + 29 / 118 s | NEW control arm VOIDED (session/new waited on auth), re-run |
| 3 V1 shell echo | `… shellecho` | `kas-lead-shellecho-2.32.0.jsonl` | 56 s | completed |
| 3 KAS-L5 attention | `… attention` | `kas-lead-attention-2.32.0.jsonl` | 57 s | completed |
| 3 TUI-L2 output styles | `… outstyle` | `kas-lead-outstyle-{2.32.0,2.31.1}.jsonl` | 40 / 38 s | completed |
| 3 HOST-L1..L3 | `probe-kas-hostcloud-2.32.0.py` + strace run | `host-cloud-2.32.0.jsonl`, `host-cloudtrace-2.32.0.jsonl` | ~1 min | completed |
| 5 cyril-on-KAS | hex1b + `probe-kas-cyrilseed-2.32.0.py` S1-S7 (+S4b) | `cyril-kas/*` | 18:06-18:20 | completed |
| 3 KAS-L2/TUI-L5 diagrams | `… diagrams` | `kas-lead-diagrams-run1-2.32.0.jsonl`, `kas-lead-diagrams-2.32.0.jsonl`, `kas-lead-diagrams-2.31.1.jsonl` | 32 + 21 / 17 s | run1 diagram turn re-run with pre-seeded file |
| 3 KAS-L3 thinking | `… thinking` | `kas-lead-thinking-{2.32.0,2.31.1}.jsonl` | 13 / 13 s (zero-turn) | completed |
| 3 KAS-L6 wfplan | `… wfplan` | `kas-lead-wfplan-2.32.0.jsonl` | 69 s | completed |
| 3 TUI-L3 include | `… include` | `kas-lead-include-2.32.0.jsonl` (+ 2 failed seeds) | 24 s | completed on 3rd seed |
| 3 TUI-L6 interrupt | `… interrupt` INTR_SIG=15/9 | `kas-lead-interrupt-{sig15,sig9}-2.31.1.jsonl`, `kas-lead-interrupt-sig15-2.32.0.jsonl`, `kas-lead-interrupt-sigkill-2.32.0.jsonl` | 4×28 s | completed |
| 3 HOST-L4 | `probe-kas-hostcloud-2.32.0.py stdout_closed,stdout_file,sigint` | `host-l4-2.32.0.jsonl` | ~1 min | SIGINT arm re-run against the process group |
| 3 own checks | `… image,fish,export` | `kas-lead-own-{2.32.0,2.31.1}.jsonl` | 71 / 70 s | completed |
| 4 sweep/unread | `sweep-new-fields.py --diff`, `unread-fields.py` | `sweep-new-fields-diff-{sweep,wfoff,wfon}.txt`, `unread-fields-2.32.0.txt` (+ NEW MCP `kas-lead-mcp-2.32.0.jsonl`, sub-agent `kas-v3c-v2-2.32.0.jsonl`) | – | completed |
| 6 v2 pair | – | – | – | NOT RUN (trap 24) |

## Deltas

| item | gate/default | STATIC lead or NEW | LIVE evidence | cyril | proposed issue? |
|---|---|---|---|---|---|
| repeat-cap pause auto-wakes the parent (`_kiro/session/notify` + `send-message-wake` turn) | default | KAS-L8 | kas-workflow-gateoff-2.32.0.jsonl#450"iteration limit (1)", kas-workflow-gateoff-2.32.0.jsonl#482"send-message-wake" | HANDLED as unlabelled generic wake (`crates/cyril-core/src/protocol/turn_mediator.rs:1297-1323`; labels only terminal runs `crates/cyril-core/src/workflow.rs:836-849`) | P1 candidate (extends cyril-lki9) |
| workflow `attention[]` (permission/question) on events, state, list | default | KAS-L5 | kas-lead-attention-2.32.0.jsonl#66"permission", kas-workflow-gateoff-2.32.0.jsonl#992"question" | DROPPED (`crates/cyril-core/src/workflow.rs:294-316`) | P2 new |
| `_kiro/workflow/steps_queued` | gate on (plan flow) | KAS-L6 | kas-lead-wfplan-2.32.0.jsonl#225"pendingSteps" | named in ext tables (`crates/cyril-core/src/protocol/engine.rs:746-747`) | note on cyril-8wh0 |
| `workflowPlan` on plan-approval request + pending_interaction | gate on | KAS-L6 | kas-lead-wfplan-2.32.0.jsonl#93"workflowPlan" | unreachable (ADR-0011) | cyril-8wh0 (fixture `kas-workflow-plan-2.32.0.jsonl`) |
| `_meta.kiro.notification.{bufferOnly,callerSessionId}` | default | NEW | kas-workflow-gateoff-2.32.0.jsonl#1171"bufferOnly" | DROPPED (0 hits) | allowlist via cyril-fb1m |
| output styles proactive/explanatory/learning; per-prompt `_meta.kiro.outputStyle` | default (template only) | TUI-L2 | kas-live-sweep-2.32.0.jsonl#8"explanatory", kas-lead-outstyle-2.32.0.jsonl#171"explanatory" | NOT-SENT (0 hits) | cyril-j66p |
| `diagrams` capability → `create_diagram` + IR | client cap opt-in | KAS-L2/TUI-L5 | kas-lead-diagrams-2.32.0.jsonl#72"schemaVersion" | NOT-SENT | P3 new (renderer) |
| `settings.thinking` → intake unknown-key | – | KAS-L3 | kiro.log only (see KAS-L3) | cyril still sends (`crates/cyril-core/src/protocol/kas/settings.rs:79,96`) | add to cyril-53qx |
| `evalStrategy` known connection key | – | KAS-L3 | kiro.log only | NOT-SENT | – |
| session/load cold-catalog one-option model pin | default | KAS-L4 | kas-workflow-gateon-2.32.0.jsonl#2550"loadTiming" | HANDLED (`crates/cyril-core/src/protocol/domain_mediator/commands/session.rs:181-190`) | – |
| graceful-shutdown pause text "…resuming the run retries the step." | default | TUI-L6 | kas-lead-interrupt-sig15-2.32.0.jsonl#102"resuming the run retries the step" | HANDLED (`crates/cyril-core/src/protocol/convert/kas/workflow.rs:303`) | – |
| bundled agent `wf-code-reviewer`; `wf-workflow-creator` re-described | default | NEW | kas-init-2.32.0-callback.jsonl#4"wf-code-reviewer" | n/a (catalog) | – |
| include validation: absolute paths only, matching `injectOriginalUserRequest` | default | NEW (newness vs 0.66.34 not tested) | kas-lead-include-2.32.0.jsonl#24"includedFrom" | `includedFrom` DROPPED | note on cyril-inwg |
| host cloud: `KIRO_REMOTE_SESSIONS_ENDPOINT` → executionTargets cloud-sandbox; session/new `executionTarget` | host-set env | HOST-L1 | host-cloudtrace-2.32.0.jsonl#2"cloud-sandbox", host-cloudtrace-2.32.0.jsonl#4"executionTarget" | NOT-SENT | cyril-tikf note |
| consent echo persists folder/scope grants (workspace/user permissions.yaml) | default | TUI-L1 (not new: 0.66.34 same) | kas-lead-folder-2.32.0.jsonl#194"workspace" | NOT-SENT (`crates/cyril-core/src/protocol/convert/kas.rs:67-91`) | cyril-gn07 / cyril-auxt re-scope |

## Lead verdicts

| lead | verdict | evidence |
|---|---|---|
| KAS-L1 shell_type "posix" (cyril-k8wc) | CONFIRMED (pre-existing on 0.66.34); worse than predicted: KAS also withholds always-allow | kas-lead-shell-2.32.0.jsonl#45"persistableConsent", kas-lead-shell-2.32.0.jsonl#119"terminal/create", kas-lead-shell-2.31.1.jsonl#44"persistableConsentReason" |
| KAS-L2 diagrams | CONFIRMED (new in 0.66.43) | kas-lead-diagrams-2.32.0.jsonl#71"create_diagram" |
| KAS-L3 settings.thinking | CONFIRMED (unknown-key); CHANGED detail: layer `session`, not `session-reasoning` | kas-lead-thinking-2.32.0.jsonl#28"setting/thinking" |
| KAS-L4 cold-catalog pin | CONFIRMED (model; no effort select because model auto) | kas-workflow-gateon-2.32.0.jsonl#2550"loadTiming" |
| KAS-L5 attention[] (gate off) | CONFIRMED | kas-lead-attention-2.32.0.jsonl#68"consentRound" |
| KAS-L6 workflowPrompt plan approval (cyril-8wh0) | CONFIRMED; plus parent self-approval after reject | kas-lead-wfplan-2.32.0.jsonl#92"pending_interaction", kas-lead-wfplan-2.32.0.jsonl#164"exactly right" |
| KAS-L7 max-tokens recovery | NOT-REACHED | 0 hits |
| KAS-L8 repeat-cap wake | CONFIRMED | kas-workflow-gateoff-2.32.0.jsonl#451"turn_start" |
| TUI-L1 folder grant (cyril-gn07) | CONFIRMED (echo required; workspace/user persist to permissions.yaml); not new in 0.66.43 | kas-lead-folder-control-2.32.0.jsonl#72"src/b/y.txt", kas-lead-folder-2.32.0.jsonl#331"user" |
| TUI-L2 output styles (cyril-j66p) | CONFIRMED (no visible change on a non-coding prompt; instruction persisted) | kas-lead-outstyle-2.32.0.jsonl#223"learning" |
| TUI-L3 include plan/includedFrom | CONFIRMED | kas-lead-include-2.32.0.jsonl#33"includedFrom" |
| TUI-L4 OAuth completion | NOT RUN (lowest priority; needs an OAuth MCP server; static: dark) | – |
| TUI-L5 diagrams | = KAS-L2, CONFIRMED | kas-lead-diagrams-2.32.0.jsonl#72"schemaVersion" |
| TUI-L6 interrupted-step text | CONFIRMED for graceful shutdown; SIGKILL keeps the old text | kas-lead-interrupt-sig15-2.32.0.jsonl#102"resuming the run retries the step", kas-lead-interrupt-sigkill-2.32.0.jsonl#130"Interrupted by agent restart" |
| HOST-L1 cloud gate | REFUTED for this account (entitled): cloud session ran; shape captured | host-cloudtrace-2.32.0.jsonl#4"cloud-sandbox" |
| HOST-L2 pre-spawn flag validation | CONFIRMED | host-cloud-2.32.0.jsonl#52"cannot be combined", host-cloud-2.32.0.jsonl#53"local sessions" |
| HOST-L3 --repo gate | CHANGED (warns, then runs and clones) | host-cloud-2.32.0.jsonl#59"git_clone" |
| HOST-L4 closed stdout / SIGINT | (a) CONFIRMED; (b) CHANGED (rc 130, no cancel message) | host-l4-2.32.0.jsonl#0"PIPESTATUS=1 0", host-l4-2.32.0.jsonl#207"sigint" |
| V1 shell consent echo (v3-coverage) | CONFIRMED (user/workspace permissions.yaml; fresh session runs without asking) | kas-lead-shellecho-2.32.0.jsonl#125"terminal/create" |
| V2 cyril's real shell_type | RESOLVED: `fish` with SHELL=fish, `posix` with SHELL=bash (kiro.log in cyril-on-KAS S1/S4) | `cyril-kas/S4-trust-approval1.txt` |

## Feature flags flipped

| flag | handshake path | positive control | observed effect |
|---|---|---|---|
| `configurationState:true` | `initialize.clientCapabilities._meta.kiro` | itself (`_kiro/configuration/state` pushed in every probe arm) | connection + session observer pushes; layers now end `workspace, agent, session` |
| `diagrams:true` | `initialize.clientCapabilities._meta.kiro.diagrams` | configurationState push ×2 | NEW: `create_diagram` tool + IR; OLD: nothing |
| `settings.thinking` | `…_meta.kiro.settings` | configurationState | NEW `intake.unknown-key`, OLD `unread-key` |
| `settings.evalStrategy` | `…_meta.kiro.settings` | configurationState | NEW `unread-key`, OLD `unknown-key` |
| `settings.workflows.enabled` | initialize + `session/new._meta.kiro.settings` | `workflowsEnabled:true` in session/new | run_workflow/update_workflow plan flow (KAS-L6) |
| shellType `posix`/`bash`/`zsh`/`fish` | `_kiro/terminal/shell_type` reply | configurationState | posix → unknown shell, every command asks, no always-allow |
| per-prompt `outputStyle` | `session/prompt._meta.kiro.outputStyle` | configurationState | `outputStyleInstruction` persisted on NEW (explanatory, learning) |
| `KIRO_REMOTE_SESSIONS_ENDPOINT` (host env, STATIC from bundle) | child env | host `--list-models` | executionTargets + cloud-sandbox, sourceProviders |

## sweep-new-fields + unread-fields
See "Leg 4" in the progress log: only NEW paths = `attention[]` family and `notification.{bufferOnly,callerSessionId}`; unread-fields 79 UNREAD (23 NEW-only, 56 standing), every line triaged into an allowlist line or issue in the Leg 4 table.

## cyril-on-KAS
See "Leg 5 — cyril-on-KAS" in the progress log (row per scenario S1-S7 + S4b).

## Proposed issues/notes (grep'd `.rivets/issues.jsonl`; no tracker edits from this lane)
- **NOTE cyril-k8wc (P1)**: CONFIRMED LIVE on 0.66.43 and 0.66.34. Arms rule_posix asked and offered only accept/reject; rule_bash and rule_zsh ran `terminal/create` with no request and no `policy.shell.type.unknown`; norule_posix re-asked after allow; norule_bash did not. cyril sends `posix` for bash/zsh users (S4 user's seat: only Allow/Deny).
- **NOTE cyril-auxt (P1)**: the persistence mechanism is the response `_meta.kiro.consent{capability, scope:"session"|"workspace"|"user", resource(glob), workspaceRoot}`. It writes `$HOME/.kiro/settings/permissions.yaml` (user) or `$HOME/.kiro/workspace-roots/<hash>/permissions.yaml` (workspace), the same on 0.66.34. On KAS cyril currently writes nothing at all (S4b), and no agent JSON write is needed. It depends on cyril-k8wc (posix offers no allow_always).
- **NOTE cyril-gn07**: the control (no echo) re-asks for every new resource; session scope covers the folder only in-session. The echo is required for any widened grant.
- **P1 NEW — repeat-cap pause wakes the parent (billed, unsolicited) and the parent model can approve plans for the user**: 0.66.43 `_kiro/session/notify` + `send-message-wake` turn on every repeat-cap `paused`. In the plan flow, a user reject is answered by the parent model. Extends cyril-lki9 (wake labelling and policy); related cyril-pd4b.
- **P2 NEW — model workflow `attention[]`** (permission/question waiting markers on events, state and list; cyril DROPS them at `workflow.rs:294-316`).
- **NOTE cyril-j66p**: template option values and descriptions captured. Per-prompt `_meta.kiro.outputStyle` is the only working channel; set_config_option is a silent no-op.
- **NOTE cyril-8wh0**: fixture `experiments/conductor-spike/kas-workflow-plan-2.32.0.jsonl`; workflowPlan key set, step shape (`effortSource`, `task{text,truncated}`), pending_interaction carries the same plan, `steps_queued`.
- **NOTE cyril-53qx**: add `thinking` to the dead connection-layer keys cyril sends (0.66.43 unknown-key).
- **NOTE cyril-tikf**: the host enables cloud via env `KIRO_REMOTE_SESSIONS_ENDPOINT`. Exact session/new `_meta.kiro{executionTarget:{kind:"cloud-sandbox"}, sessionSource:"remote", isEmptyWorkspace:true}`; this account is entitled.
- **NOTE cyril-1v25 / cyril-ebqu / cyril-2yn8**: still dropped on 0.66.43 from the user's seat. The sub-agent answer is visible only via the invoke tool's completion; the child Read File row is top-level and appears after the sub-agent row.
- **P3 NEW — cyril approval overlay log spam**: `crates/cyril-ui/src/widgets/approval.rs:119` warns on every render while an approval is open (~14k lines, 3.9 MB per short session).
- **P3 NEW — write-approval path truncated** to the overlay width ("…/conductor-spike/a"); the user cannot see which file is being written.
- **P3 NEW — `diagrams` capability**: cyril could advertise it and render the IR (`kas-lead-diagram-ir-dg_*.json` is the fixture).
- **NOTE cyril-inwg**: include paths must be absolute/`bundled://`/`generated://`; included recipes must match `injectOriginalUserRequest`; `includedFrom` is on listRecipes plan and run_start nodeTree.
- **NOTE (docs)**: CLAUDE.md "Logs go to cyril.log in the working directory" is stale; the log is `$HOME/.config/cyril/cyril.log`.

## Method notes
- **Trap 21/16 (re-runs):** sweep run1 voided (driver p4 used the bogus model); folder control arm voided (session/new waited on the init token, 180 s timeout, M2-style); diagram run1 re-run (file seeded after session start); include needed two seed fixes; SIGINT re-sent to the process group.
- **Trap 1:** a leg-1 OLD/NEW "load carries model" difference turned out to be catalog warmth, not version. It was corrected after comparing every session/load in the lane (correction section). Pair attribution needs the same warm/cold state.
- **Traps 5/28:** every probe used the full `initialize.clientCapabilities._meta.kiro` path with `configurationState:true` as the positive control; KAS debug lines were read from `$HOME/.kiro/logs/<ts>/kiro.log`.
- **Trap 6/7:** one renewer (the driver, 17:52); the host legs ran inside the token window with temp HOME + real XDG_DATA_HOME; no login/logout.
- **Trap 8:** host legs used the archived 2.32.0 bin first on PATH; child exe recorded = archived kiro-cli-chat + self-extracted KAS (== carve).
- **Trap 11:** sweep-new-fields only-OLD paths in leg 1 are p4 coverage, not removals.
- **Trap 13:** no forging; max-tokens recovery and byte-oversized images NOT-REACHED.
- **Trap 25:** no v2-on-KAS probes. cyril's own `_kiro.dev/commands/execute` failure was observed only from the user's seat (S2).
- **Trap 27:** all citations 0-based and checked with `check-citations.py`.
- The host ignores `KIRO_ACP_RECORD_PATH`, so the host ACP frames were taken with `strace -f -e trace=write`. The raw trace held a live token and was deleted after redacted extraction.
- **Cloud side effect:** HOST-L1/L3 created 3 real cloud sessions, all deleted via `--delete-session`; remote deletion is unverified.
- hex1b `capture_terminal_text` output was large, so screens were saved as SVG (`capture_terminal_screenshot`, now in `tmp/cyril-kas-svg/`) and converted to `.txt` with `tools/svg2txt.py`. Casts over 1 MB were gzipped.
- New scripts: `experiments/conductor-spike/probe-acp-init-2.32.0.py`, `probe-kas-live-2.32.0.py`, `probe-kas-leads-2.32.0.py`, `probe-kas-hostcloud-2.32.0.py`, `probe-kas-cyrilseed-2.32.0.py`; lane helpers `wf_parity.py`, `trim_captures.py` (2.32.0 topics), `tools/svg2txt.py`. Bundles `experiments/conductor-spike/{kas-init,kas-live-sweep,kas-workflow-gateoff,kas-workflow-gateon,kas-shell-type,kas-consent-scope,kas-workflow-attention,kas-output-styles,kas-diagrams,kas-thinking-intake,kas-recipe-include,kas-workflow-interrupt,kas-image-fish-export,kas-mcp-subagent,host-cloud,kas-workflow-plan}-2.32.0.jsonl` (credential scan clean). Do not commit `live/tmp/`.

## Progress log (appended as legs finish)

### Leg 0 — `--help` + initialize-only matrix (DONE 17:17-17:19 UTC)
- `--help`: neither engine has a help surface; both start, print nothing on stdout, exit 0 on stdin EOF. stderr identical
  modulo version (`kas.server.starting … 0.66.34|0.66.43`), pid and timings (`tmp/help-{old,new}.{out,err}`).
- initialize (`probe-acp-init-2.32.0.py`, full path `clientCapabilities._meta.kiro.{configurationState,settings,telemetryEnabled}`;
  positive control `configurationState:true` → `_kiro/configuration/state` pushed in all 4 arms, e.g. kas-init-2.32.0-callback.jsonl#4"_kiro/configuration/state"):
  captures `kas-init-{2.31.1,2.32.0}-{callback,none}.jsonl`. callback vs none: initialize result IDENTICAL on both engines;
  callback asks `_kiro/auth/getAccessToken` 1× at init, none 0×.
- OLD→NEW initialize result: IDENTICAL (normalised for tmp paths). `extensionMethods` 28 = 28 (no new `_kiro/*` client-callable
  method in 0.66.43), `authMethods`, all `_meta.kiro` caps unchanged. LIVE.
- OLD→NEW `_kiro/configuration/state` push (connection observer) — the only init-time delta, LIVE:
  - NEW bundled agent `krn:kiro-agent-defaults::agent/wf-code-reviewer` ("Targeted review of changed behavior and required verification.
    Reports blocking findings and writes the workflow verdict.") — kas-init-2.32.0-callback.jsonl#4"wf-code-reviewer"; absent in kas-init-2.31.1-callback.jsonl#4.
  - `wf-workflow-creator` description rewritten: OLD "Generates a workflow for the run_workflow tool … returns a generated:// reference" →
    NEW "Runs as a workflow step that designs the run's steps from a brief or an edit request, saves them with save_workflow_definition,
    and adds them to the run with update_workflow." (kas-init-2.32.0-callback.jsonl#4"update_workflow") — matches the changelog
    "Creating a workflow from a prompt shows its plan for your approval" / "Running workflows accept plain-language changes".
  - Layers (9), steering (3), bundled workflows (7: autoresearch, feature-pipeline, goal, investigate, publish-pr, ralph,
    semantic-review-multi-model) unchanged.

### Leg 1 — paired SCENARIO=sweep (DONE 17:21-17:23 UTC; run1 VOIDED, re-run)
- Driver `probe-kas-live-2.32.0.py` (copy of 2.31.1 + p4 write-outside, config-option census, second-connection `session/load`).
- run1 (17:19-17:21) VOIDED: driver bug — p4 ran while p3's bogus model was still selected (both engines -32000 InvalidModelError);
  kept as `tmp/kas-live-sweep-*-run1-p4bogus.jsonl`. Fixed (restore model via `set_config_option model` before p4), re-run both.
- OLD `kas-live-sweep-2.31.1.jsonl` (327 frames, 54.8 s), NEW `kas-live-sweep-2.32.0.jsonl` (286 frames, 52.3 s). Both: p1 read end_turn ~7 s,
  p2 shell end_turn + 1 permission, p3 bogus model -32000 InvalidModelError/CLIENT_ERROR, p4 end_turn; no auth renewal; init ok.
  Durations match the 2.31.1 sweep (~7 s/turn) — not suspiciously short (trap 21).
- **NEW `session/load` result carries the saved `model` configOption** (changelog "Resumed sessions show their saved model and effort before
  the model list loads"), LIVE: NEW result configOptions = [mode, **model**, memoryReflection, autopilot, contentCollection], model option
  complete (currentValue `auto`, 21 options with `_meta.kiro.{hasEffort,effortLevels,defaultEffortLevel,effortSchemaPath,rateMultiplier,rateUnit,thinkingToggleable}`)
  — kas-live-sweep-2.32.0.jsonl#276"claude-opus-5.5"; OLD result has only [mode, memoryReflection, autopilot, contentCollection] and the model
  arrives later in a `config_option_update` — kas-live-sweep-2.31.1.jsonl#318"contentCollection", kas-live-sweep-2.31.1.jsonl#325"config_option_update".
  Same in run1 (NEW load result `model: bogus-model-zz9` = the saved model, even an invalid one). cyril HANDLED generically:
  `crates/cyril-core/src/protocol/domain_mediator/commands/session.rs:181-190` maps `LoadSessionResponse.config_options` →
  `Notification::ConfigOptionsUpdated` after `SessionCreated`. Load result `_meta.kiro.loadTiming` phases identical in set (16 phases incl.
  `user-agent-gate`); no new `_meta` key.
- **Output styles** (changelog "Proactive, Explanatory, and Learning output styles"), LIVE: `_kiro/config/template` `outputStyle` option
  (category `output_style`, `_meta.kiro.promptScoped:true`) OLD options [default, concise] → NEW [default, **proactive**, concise, **explanatory**,
  **learning**] with descriptions — kas-live-sweep-2.32.0.jsonl#8"explanatory" vs kas-live-sweep-2.31.1.jsonl#8"concise". `outputStyle` is NOT a
  session configOption on either engine (session/new and load lists unchanged); `session/set_config_option configId:"outputStyle"` is
  accepted silently and not echoed (kas-live-sweep-2.32.0.jsonl#222"contentCollection") — same as OLD. cyril: NOT-SENT/never reads
  `outputStyle` (0 hits in `crates/`; it never calls `_kiro/config/template`). The `promptScoped` channel is checked in leg 3.
- Write outside the workspace (p4): OLD model wrote it after one `session/request_permission` (kinds allow_once/allow_always/reject_once/
  reject_always, `_meta.kiro{toolId:"fs_write", consent{capability:"fs_write", resource:<abs path>, askType:"implicit"}, consentRound:1}`) —
  kas-live-sweep-2.31.1.jsonl#231"askType"; NEW model **declined without a tool call** and asked the user to confirm the out-of-workspace
  path (kas-live-sweep-2.32.0.jsonl#217"turn_end", zero permission frames). One sample each = model variance OR a 0.66.43 system-prompt change
  (lead for the kas lane); the 2.32.0 write-approval frame is captured in leg 3 (W).
- Everything else unchanged: model option `_meta.kiro` key set identical, mode list identical (7), memory default `{disabled,false}` on both,
  `set_config_option` silent acceptance of unoffered values (`effortLevel:"explanatory"`, `thinking:"explanatory"`) on both.
- sweep-new-fields (`sweep-new-fields-diff-sweep.txt`): 756 vs 788 paths; only-NEW = `reload.config_options.model` (the summary echo of the
  load-result delta above); only-OLD 33 = the p4 write coverage OLD exercised and NEW didn't (preview/checkpoint/diff content) — coverage, not removal.

### Leg 2a — SCENARIO=workflow GATE=off pair (DONE 17:23-17:30 UTC)
- OLD `kas-workflow-gateoff-2.31.1.jsonl` (144.4 s), NEW `kas-workflow-gateoff-2.32.0.jsonl` (230.3 s; +86 s = the driver's 90 s
  wait for a cancel-wake that had already been absorbed, see below). Both completed with the driver summary; `workflowsEnabled` false on both.
- Run parity (`wf_parity.py`): all 6 runs MATCH event-for-event (agenturi 5, bgwatch 10, linear 13, loop 23, park 10, watch 11);
  every terminal run auto-woke the parent on both; 6 = 6 permission asks.
- **NEW: a repeat-cap `paused` run auto-wakes the parent session (unsolicited billed turn)**, LIVE. On NEW, right after the loop run's
  `run_complete status:"paused"` (kas-workflow-gateoff-2.32.0.jsonl#444"paused") KAS sends `_kiro/session/notify` to the parent
  `{severity:"info", sender:"step", workflowId, nodeId:"loop", message:"A workflow you launched (\"loop\") paused: repeat \"loop\" reached its
  iteration limit (1) without meeting its stop condition. Read the last step's result, explain what remains unresolved, and ask the user whether
  to grant more iterations or end this repeat and continue. …"}` (kas-workflow-gateoff-2.32.0.jsonl#450"iteration limit (1)") and immediately starts
  a parent turn (kas-workflow-gateoff-2.32.0.jsonl#451"turn_start") whose chunks carry `agentInitiatedReason:"send-message-wake"`
  (kas-workflow-gateoff-2.32.0.jsonl#482"send-message-wake"); that turn ran a `read_file` tool and billed 0.223 credits before its turn_end
  (kas-workflow-gateoff-2.32.0.jsonl#652"turn_end"). The second pause (after extendRepeat) sent a second notify (kas-workflow-gateoff-2.32.0.jsonl#582"iteration limit (2)").
  OLD: the same paused `run_complete` (kas-workflow-gateoff-2.31.1.jsonl#447"paused") is followed by NO notify and NO parent turn; OLD
  parents wake only on terminal statuses. `send-message-wake` occurs 0× in the OLD capture. Matches the changelog "Workflow review loops pause
  after two rejected reviews and ask you how to continue". cyril: HANDLED as a generic agent-initiated wake (logged, no run label —
  `crates/cyril-core/src/protocol/turn_mediator.rs:1297-1323` names `send-message-wake`; `crates/cyril-core/src/workflow.rs:836-849` queues wake
  labels only for terminal statuses), so the user sees an unlabelled agent turn that asks a question about a paused run. **P1 candidate:
  new unsolicited billed parent turn on every repeat-cap pause** (extends cyril-lki9).
- The driver's `loop.wake_after_cancel: {woke:false}` on NEW is an artifact of the above: the parent was already inside the pause-wake turn
  when the user cancel landed, so the cancel's terminal wake was folded (the NEW parent turn after cancel, kas-workflow-gateoff-2.32.0.jsonl#716"turn_end").
- sweep-new-fields (`sweep-new-fields-diff-wfoff.txt`): 1141 vs 1118 paths; 26 only-NEW, all LIVE new fields:
  - **`attention[]`** on every `_kiro/workflow/*` event (`run_start`, `node_start`, `node_complete`, `paused`, `node_paused`, `run_complete`,
    `loop_iteration`, `watch_poll`; `[]` when nothing waits), on `finalState`/`state` and on `root.children[]`. Populated on the parked
    completion step: `state.attention:[{kind:"question", since, nodeId:"ask", nodePath, sessionId}]`, node-level `[{kind, since}]`
    (kas-workflow-gateoff-2.32.0.jsonl#992"question"). cyril DROPPED: `WorkflowRun` has no attention field (`crates/cyril-core/src/workflow.rs:294-316`).
  - **`_meta.kiro.notification.{bufferOnly:true, callerSessionId}`** on step-verdict notifications replayed as `user_message_chunk`
    (kas-workflow-gateoff-2.32.0.jsonl#1171"bufferOnly") — 0 hits for either key in `crates/`. DROPPED.
  - only-OLD: `rawInput.cwd` (model-chosen optional arg) and the summary's `wake_after_cancel.*` (artifact above).

### Leg 2b — SCENARIO=workflow GATE=on pair (DONE 17:30-17:43 UTC)
- OLD `kas-workflow-gateon-2.31.1.jsonl` (257.2 s), NEW `kas-workflow-gateon-2.32.0.jsonl` (520.0 s). Both complete (driver summary present),
  `workflowsEnabled` true on both (gate positive control), no auth renewal.
- Run parity: all 11 runs MATCH event-for-event (agenturi 5, bgwatch 10, branches 13, fail 5 incl. retry-node, launcher (custom agent
  `run_workflow`) completed, linear 13, loop 23, loopnone 12, park 10, usertier 5, watch 11); every terminal run auto-woke the parent; 13 vs 14
  permission asks (model tool volume). `recipes_changed` 2 = 2.
- NEW's extra 263 s = (a) the same 84 s driver wait for an already-absorbed cancel wake (repeat-cap pause wake, as in leg 2a:
  kas-workflow-gateon-2.32.0.jsonl#647"retriedNodeIds"), and (b) **one natural 178 s silent backend stall** on the park step: `turn_start`
  then `checkpoint_restore_availability` (kas-workflow-gateon-2.32.0.jsonl#994"checkpoint_restore_availability") and nothing for 177.6 s until
  the first chunk (kas-workflow-gateon-2.32.0.jsonl#995"agent_message_chunk"); no watchdog/notice frame. Passive, not version-attributable.
- Repeat-cap pause wake reproduced with the gate on (same `_kiro/session/notify` + `send-message-wake` parent turn).
- sweep-new-fields (`sweep-new-fields-diff-wfon.txt`): 1331 vs 1305 paths; only-NEW = the same `attention[]` family and
  `notification.{bufferOnly,callerSessionId}` as leg 2a, plus `_meta.kiro.resources[]` on tool_call_update (present in the OLD sweep capture too —
  coverage, not new); only-OLD = model-chosen `rawInput.{depth,explanation}` and the wake artifact.

### Leg 3 / KAS-L1 — `_kiro/terminal/shell_type` "posix" (cyril-k8wc) (DONE 17:44-17:46 UTC) — P1 CONFIRMED LIVE
Probe `probe-kas-leads-2.32.0.py shell`, captures `kas-lead-shell-2.32.0.jsonl` (5 arms) and `kas-lead-shell-2.31.1.jsonl` (4 arms),
KIRO_LOG_LEVEL=debug (kiro.log under `$HOME/.kiro/logs/`). Handshake like cyril (`terminal:true`, fs read/write, `_meta.kiro.configurationState:true`
→ `_kiro/configuration/state` pushed 3× in every arm = positive control). Workspace agent `.kiro/agents/auditx.json` (tools shell+read),
selected with `set_config_option mode=auditx` (accepted in every arm). The probe answers `_kiro/terminal/shell_type` with the arm's name.

| arm | V3 rule | shellType | asks t1 | asks t2 (after allow_always) | options offered | kiro.log |
|---|---|---|---|---|---|---|
| rule_posix (NEW) | allow `echo AUDITX*` | `posix` (= cyril) | **1** | – | accept / reject only | `policy.shell.type.unknown {"shellName":"posix"}` |
| rule_bash (NEW, control) | same | `bash` | 0 | – | – | `policy.shell.type.resolved {"shellName":"bash","shellParser":"posix"}` |
| rule_zsh (NEW) | same | `zsh` | 0 | – | – | `…resolved {"shellName":"zsh","shellParser":"posix"}` |
| norule_posix (NEW) | none | `posix` | 1 | **1 (re-asked)** | accept / reject only | `policy.shell.type.unknown` |
| norule_bash (NEW) | none | `bash` | 1 | 0 | accept / always-accept / reject / always-reject | `…resolved` |
| rule_posix / rule_bash / norule_posix / norule_bash (OLD 0.66.34) | as above | as above | 1 / 0 / 1 / 1 | – / – / **1** / 0 | identical to NEW | identical |

- The rule-honoured control runs `terminal/create echo AUDITX` with no request: kas-lead-shell-2.32.0.jsonl#119"terminal/create" (bash),
  kas-lead-shell-2.32.0.jsonl#186"terminal/create" (zsh). With `posix` the SAME rule is ignored and KAS asks:
  kas-lead-shell-2.32.0.jsonl#45"persistableConsent". The request carries `_meta.kiro.consent{capability:"shell", resource:"echo AUDITX",
  askType:"implicit", **persistableConsent:false, persistableConsentReason:"Kiro could not parse this command, so a saved rule would never match it.
  You can still allow or deny it for this run. You can allow shell resource '*' in your permissions file to bypass all command parsing."**, workspaceRoot}`
  and only `accept`/`reject` options — KAS withholds `always-accept`/`always-reject` entirely. With `bash` the request instead carries
  `consent.triggeringResource` and all four options (kas-lead-shell-2.32.0.jsonl#366"triggeringResource").
- norule_posix re-asks the identical command in the same session (kas-lead-shell-2.32.0.jsonl#252"echo AUDITY" then
  kas-lead-shell-2.32.0.jsonl#290"echo AUDITY"); norule_bash answered `always-accept` once and the re-run went straight to
  `terminal/create` (kas-lead-shell-2.32.0.jsonl#404"terminal/create").
- Pre-existing: 0.66.34 behaves identically (kas-lead-shell-2.31.1.jsonl#44"persistableConsentReason", kas-lead-shell-2.31.1.jsonl#121"terminal/create",
  kas-lead-shell-2.31.1.jsonl#226"echo AUDITY").
- Side fact: an `always-accept` on bash is session-scoped here — the agent file's `permissions.rules` stayed `[]` after the grant (both engines).
- cyril: answers `{"shellType":"posix"}` for every POSIX shell (`crates/cyril-core/src/protocol/kas/host_shell.rs:165-171` `wire_name()`,
  sent by `crates/cyril-core/src/protocol/kas/terminal_io.rs:283-285`). ⇒ On KAS every cyril shell command asks, V3 allow rules never match,
  and "Always allow" is never even offered. **cyril-k8wc CONFIRMED LIVE**; its acceptance check (V3 allow `echo AUDITX*` → `terminal/create`, no
  request, no `policy.shell.type.unknown`) is exactly arm rule_bash/rule_zsh above. Fix direction: send the concrete shell basename
  (`bash`/`zsh`/`sh`/`dash`/`ksh`), which KAS maps to `shellParser:"posix"` itself.

### Leg 3 / TUI-L1 — folder write grant via the `_meta.kiro.consent` echo (cyril-gn07) (DONE 17:47-17:57 UTC)
Probe `probe-kas-leads-2.32.0.py folder`; NEW captures `kas-lead-folder-2.32.0.jsonl` (arms session/workspace/user) +
`kas-lead-folder-control-2.32.0.jsonl` (control, re-run), OLD `kas-lead-folder-2.31.1.jsonl` (all 4 arms). Positive control
`_kiro/configuration/state` pushed 3× per arm. Per arm a fresh HOME + git workspace; three turns: write `<ws>/src/a/x.txt` (answer
`always-accept`), write `src/b/y.txt` same session, then `session/new` same workspace and write `src/c/z.txt`.
- RE-RUN: the first NEW control arm's `session/new` timed out (180 s) waiting for the init-time token (180 s left at start, renewal at +272 s;
  method note M2) — voided, control re-run alone with a fresh token.
- Request shape (both engines): in-workspace writes ARE asked in the default `vibe` mode; options accept/always-accept/reject/always-reject;
  `_meta.kiro.consent{capability:"fs_write", resource:"src/a/x.txt" (workspace-RELATIVE), askType:"implicit", workspaceRoot}`, no
  `triggeringResource` for fs_write (kas-lead-folder-2.32.0.jsonl#48"src/a/x.txt").

| arm (response to the first ask) | same session `src/b` | new session `src/c` | file written | OLD 0.66.34 |
|---|---|---|---|---|
| control: `always-accept`, no `_meta` (= cyril today) | **re-asked** (kas-lead-folder-control-2.32.0.jsonl#72"src/b/y.txt") | **re-asked** (kas-lead-folder-control-2.32.0.jsonl#127"src/c/z.txt") | none | identical |
| `consent{scope:"session", resource:"<ws>/src/**"}` | not asked | re-asked (kas-lead-folder-2.32.0.jsonl#128"src/c/z.txt") | none | identical |
| `scope:"workspace"` (kas-lead-folder-2.32.0.jsonl#194"workspace") | not asked | not asked | `$HOME/.kiro/workspace-roots/<hash>/permissions.yaml` = `rules: [{capability: fs_write, effect: allow, match: [<ws>/src/**]}]` | identical |
| `scope:"user"` (kas-lead-folder-2.32.0.jsonl#331"user") | not asked | not asked | `$HOME/.kiro/settings/permissions.yaml` (same rule) | identical |

- Verdict: a bare `always-accept` grants only the exact resource for the session; the folder pattern and the scope come ONLY from the
  response `_meta.kiro.consent` echo. This is NOT new in 0.66.43 — 0.66.34 honours the same echo and writes the same files
  (kas-lead-folder-2.31.1.jsonl#345"src/a/x.txt", kas-lead-folder-2.31.1.jsonl#485"src/a/x.txt"); the 2.32.0 change is the TUI's folder/scope UI.
  Neither engine writes the agent JSON. cyril sends no consent echo (`crates/cyril-core/src/protocol/convert/kas.rs:67-91` attaches only
  `rejectionReason`) ⇒ cyril's "Always allow" on KAS is exact-resource and session-only.

### Leg 3 / V1 (v3-coverage) — shell consent echo (DONE 17:58 UTC)
Probe `probe-kas-leads-2.32.0.py shellecho`, capture `kas-lead-shellecho-2.32.0.jsonl` (shellType `bash`, no V3 rule).
- `always-accept` + `_meta.kiro.consent{capability:"shell", scope:"user", resource:"echo AUDITV1", workspaceRoot}` → same session and a FRESH
  session both run `terminal/create` without asking (kas-lead-shellecho-2.32.0.jsonl#77"terminal/create", kas-lead-shellecho-2.32.0.jsonl#125"terminal/create");
  `$HOME/.kiro/settings/permissions.yaml` gains `{capability: shell, effect: allow, match: [echo AUDITV1]}`.
- `scope:"workspace"` → `$HOME/.kiro/workspace-roots/<hash>/permissions.yaml` (same rule); fresh session not asked (kas-lead-shellecho-2.32.0.jsonl#273"terminal/create").
- The shell request carries `consent.triggeringResource:"echo AUDITV1"` (kas-lead-shellecho-2.32.0.jsonl#38"triggeringResource").
- The "cyril-exact" arm (posix, no rule, allow_always without echo) is KAS-L1 norule_posix: KAS does not even offer `always-accept` and re-asks.
  Control (bash, allow_always without echo) = KAS-L1 norule_bash: no re-ask in the same session.
- ⇒ cyril-auxt can persist "Always allow" with the consent echo (scope user/workspace) instead of agent-JSON writes, once cyril-k8wc
  (concrete shell name) is fixed.

### Leg 3 / KAS-L5 — workflow `attention[]` on a cyril-style gate-OFF launch (DONE 17:58-17:59 UTC)
Probe `probe-kas-leads-2.32.0.py attention`, capture `kas-lead-attention-2.32.0.jsonl`: no `workflows.enabled` (`workflowsEnabled:false`),
`_kiro/workflow/new {workflowPath:"bundled://investigate", inputs:{brief, report_path}}` + invoke (the cyril path); each step-session
permission ask held open 10 s while the probe called `_kiro/workflow/list` and `inspect`. Run completed; parent auto-woke.
- While the ask is open, both `_kiro/workflow/list` (kas-lead-attention-2.32.0.jsonl#66"permission") and `inspect`
  (kas-lead-attention-2.32.0.jsonl#68"consentRound") carry `attention:[{kind:"permission", since, toolCallId, consentRound:1, nodeId:"investigate",
  nodePath, sessionId}]`. Every `_kiro/workflow/*` notification carries top-level `attention:[]` (empty here: no notification fired during the
  open ask). With leg 2's `kind:"question"` (parked completion step) that is two attention kinds seen live.
- KAS-L5 **CONFIRMED** on the gate-off path. cyril DROPPED (`crates/cyril-core/src/workflow.rs:294-316` has no field). `memoryConfigSource`,
  `backgroundExecution`, `rootConversationId`, `nodePlan` are not new (same counts in the OLD gate-off capture).

### Leg 3 / TUI-L2 — output styles (cyril-j66p) (DONE 18:00-18:02 UTC)
Probe `probe-kas-leads-2.32.0.py outstyle`, KIRO_LOG_LEVEL=debug, captures `kas-lead-outstyle-2.32.0.jsonl` and `kas-lead-outstyle-2.31.1.jsonl`.
Same question five times in one session: no `_meta` / `outputStyle:"default"` / after `set_config_option outputStyle=explanatory` /
`outputStyle:"explanatory"` / `outputStyle:"learning"` (prompt frames kas-lead-outstyle-2.32.0.jsonl#171"explanatory", kas-lead-outstyle-2.32.0.jsonl#223"learning").
- (1) The option exists only in `_kiro/config/template`: NEW options `default` "Use Kiro's standard response style." / `proactive` "Start work
  immediately and make reasonable assumptions about routine decisions." / `concise` "Keep responses brief and conversational unless the task needs
  more structure." / `explanatory` "Add brief Insight blocks that explain non-obvious code and implementation choices." / `learning` "Explain choices
  and leave small, meaningful pieces of code for you to write." (category `output_style`, `_meta.kiro.promptScoped:true`); OLD [default, concise]
  (see leg 1). NOT in session/new configOptions or any `config_option_update` on either engine (ids seen: mode, model, memoryReflection,
  autopilot, contentCollection).
- (2) Replies: no visible difference for this non-coding question — none: "List comprehensions are faster mainly because the iteration …";
  default: "List comprehensions run their looping and list-building in optimized C code …"; explanatory: "List comprehensions execute their
  looping and element-building through a specialized bytecode path …"; learning: "List comprehensions run their looping and list-building through a
  specialized bytecode path …". No `Insight` block (the instructions target code changes).
- (3) Applied evidence: kiro.log has NO outputStyle line at debug (the `outputStylePromptApplied` count is a telemetry metric, not a log line);
  the proof is the persisted user message: NEW `messages.jsonl` stores `outputStyleInstruction` for the explanatory turn ("Do the task normally,
  and add brief `Insight` blocks before or after relevant code changes. …") and the learning turn ("… leave a few lines marked `TODO(human)` and
  send a `Learn by Doing` request …"); none for none/default/after-set_config; OLD stores none (key unknown to 0.66.34). The bundle path is
  `extractPromptOutputStyleId(t._meta?.kiro)` → `t.outputStyle` (STATIC confirmation of the key name).
- (4) `session/set_config_option configId:"outputStyle"` is accepted silently, not echoed, and has no effect on the next prompt (no instruction
  persisted) — the style is per prompt only.
- cyril: NOT-SENT (no `outputStyle` anywhere in `crates/`). cyril-j66p CONFIRMED: a cyril route needs the template read plus a per-prompt
  `_meta.kiro.outputStyle`.

### Leg 3 / HOST-L1..L3 — host non-interactive V3 cloud/repo (DONE 18:03-18:07 UTC)
Probe `probe-kas-hostcloud-2.32.0.py` (archived 2.32.0 `bin/` first on PATH; child exes recorded = archived `kiro-cli-chat` + the
self-extracted `~/.local/share/kiro-cli/kas/2.32.0-f3c56062…/acp-server.js` == the carve), capture `host-cloud-2.32.0.jsonl` (stream-json
frames per arm). The ACP handshake was captured once with `strace -f -e trace=write` around a text-mode `--cloud` run and extracted to
`host-cloudtrace-2.32.0.jsonl` (tokens, machineId, userId redacted; the raw strace was deleted).
- Positive control `chat --list-models --agent-engine v3`: rc 0, model table, no KAS child (host-cloud-2.32.0.jsonl#0"listmodels").
- **HOST-L1: this account IS cloud-entitled** (the lead's non-entitled prediction is REFUTED for this account). `--cloud` ran a real cloud session:
  rc 0, KAS stderr `acp.remote_sessions.enabled {"endpoint":"https://app.kiro.dev"}`, `sandboxStatus{backend:"bubblewrap", networkMode:"default_allowed"}`,
  workspace `/projects/sandbox`, reply billed 0.039 credits (host-cloud-2.32.0.jsonl#24"remote_sessions"). Text mode prints `[cloud] session <id>` on stderr.
  - Host initialize (host-cloudtrace-2.32.0.jsonl#0"kiro-cli-non-interactive"): clientInfo `kiro-cli-non-interactive`, `fs`/`terminal` false,
    `_meta.kiro{hooks{enabled,v2}, knowledge, settings{knowledge, codeIntelligence, workflows:{enabled:false}, goal:{enabled:false}, memory}, telemetryEnabled, telemetry{…}}`.
  - Host-spawned KAS initialize result (host-cloudtrace-2.32.0.jsonl#2"cloud-sandbox"): `executionTargets:["local","cloud-sandbox"]`,
    `sessionSources:["local","remote"]`, `sessionListScopes:["workspace","user"]`, `sourceProviders:true`, extensionMethods 30 (= leg 0's 28 +
    `_kiro/sourceProviders/list`, `_kiro/sourceProviders/listResources`). The direct spawn in leg 0 (same account, same day) returns
    `executionTargets:["local"]`, `sessionSources:["local"]`, `sourceProviders:false`, 28 methods. Cause (STATIC, bundle): the host sets env
    `KIRO_REMOTE_SESSIONS_ENDPOINT` (KAS reads `--remote-sessions-endpoint` or that env; child argv carries no extra flag). ⇒ a cyril direct spawn
    never sees cloud capability unless it sets that env.
  - **`session/new` shape cyril-tikf needs** (host-cloudtrace-2.32.0.jsonl#4"executionTarget"):
    `{cwd, mcpServers:[], _meta:{kiro:{settings{memory, workflows:{enabled:false}, goal:{enabled:false}}, executionTarget:{kind:"cloud-sandbox"},
    sessionSource:"remote", isEmptyWorkspace:true}}}`; result `{sessionId, _meta{id, title, agentMode, workspacePaths:[], createdAt, lastModifiedAt,
    source:"remote", executionTarget:{kind:"cloud-sandbox"}}}` (host-cloudtrace-2.32.0.jsonl#6"remote").
  - Cleanup: the three cloud sessions this lane created (03f28a3c…, 01584308…, 5d15eb4a…) were deleted with `kiro-cli chat --agent-engine v3
    --delete-session <id>` ("✔ Deleted chat session …"); whether that removes the REMOTE record, not only a local index, is unverified.
- **HOST-L2 CONFIRMED**: `--cloud --resume-id 0000…` → rc 1 "error: --cloud/--repo start a new cloud session and cannot be combined with
  --resume/--resume-id; to continue a cloud session pass only --resume-id <id>" (host-cloud-2.32.0.jsonl#52"cannot be combined"); `--cloud --sandbox`
  → rc 1 "error: --sandbox applies to local sessions and cannot be combined with --cloud" (host-cloud-2.32.0.jsonl#53"local sessions"); no KAS child
  in either (pre-spawn validation).
- **HOST-L3 CHANGED**: `--cloud --repo owner/name` does NOT stop at a source-provider error: stderr `[warn] repository "owner/name" was not found
  among your connected source providers`, then the cloud session starts anyway and runs a `git_clone` tool for `owner/name`
  (host-cloud-2.32.0.jsonl#59"git_clone") that fails in the sandbox (host-cloud-2.32.0.jsonl#78"failed"); rc 0, a billed turn ran on the bare prompt.

### Leg 5 — cyril-on-KAS (DONE 18:06-18:20 UTC)
Build: `env -u CARGO_TARGET_DIR cargo build --features kas` at primary-checkout HEAD **b9b6e95d** (fix(cyril-5nep) #168, after #171 pickers) →
`target/debug/cyril`. Env names verified in code (`KIRO_KAS_SERVER_PATH`, `KIRO_AGENT_PATH`: `crates/cyril-core/src/protocol/kas/discovery.rs:428-429`;
`--agent-engine`: `crates/cyril/src/main.rs:40`). Driver: hex1b MCP (available in this session), one bash terminal 160x48, one asciinema
cast per scenario in `cyril-kas/`; screens saved as SVG via `capture_terminal_screenshot` and converted to `<scenario>-<step>.txt` with
`tools/svg2txt.py`. Each scenario = one cyril process with a fresh temp HOME + `git init`/`git add` workspace (`probe-kas-cyrilseed-2.32.0.py`),
`KIRO_LOG_LEVEL=debug` so the KAS child writes `$HOME/.kiro/logs/<ts>/kiro.log`. Token renewed by the probe driver at 17:52 (valid to 18:51);
the whole leg ran 18:06-18:20 inside it.
- Method notes: `cyril.log` lives at `$HOME/.config/cyril/cyril.log` (`crates/cyril/src/main.rs:278-310`), NOT the workspace (CLAUDE.md
  "Logs go to cyril.log in the working directory" is stale). cyril presents as `kiro-cli` by default ("presenting as kiro-cli (default, ADR-0008)").
- **V2 (v3-coverage): cyril's real `_kiro/terminal/shell_type` reply follows `$SHELL`**: with this user's `SHELL=/bin/fish` cyril answers `fish`
  (kiro.log `policy.shell.type.resolved {"shellName":"fish","shellParser":"fish"}`, S1); with `SHELL=/bin/bash` it answers `posix`
  (kiro.log `policy.shell.type.unknown {"shellName":"posix"}` ×2, S4). So cyril-k8wc bites every bash/zsh/sh user, not fish users.

| scenario | issue | expected (static row) | observed on screen | evidence | verdict |
|---|---|---|---|---|---|
| S1 startup | – | toolbar shows engine/model/mode; model arrives via config_option_update | toolbar `sess_… · vibe · auto` (mode `vibe`, model `auto`; no engine label) ≈5.7 s after launch (upper bound incl. tool latency); "Connecting to agent… / Session created: sess_…"; context bar `Context: 1% · Context Files 0% …` | `cyril-kas/S1-startup-toolbar.txt`, `S1-startup.cast`, `S1-cyril.log` | CONFIRMED (works) |
| S2 commands | cyril-oieu | skill/prompt commands fail via `_kiro.dev/commands/execute` | `/` completion lists cyril built-ins + KAS custom agents (`/context-gatherer`, `/general-task-execution`); `/pro` filters to `/probe-prompt (file prompt)` and `/probe-skill Probe skill. …`. `/probe-skill hello` → "/probe-skill: command failed."; `/probe-prompt world` → "/probe-prompt: command failed."; no turn ran. cyril.log: `_kiro.dev/commands/execute` → -32603 "[PersistenceClassification] Ext method … has no persistence classification" (×2) | `cyril-kas/S2-commands-slash.txt`, `S2-commands-slash-pro.txt`, `S2-commands-skill-run.txt`, `S2-commands-prompt-run.txt`, `S2-cyril.log` | CONFIRMED (flip blocker stands) |
| S3 settings | cyril-vh69 (closed) | `/model` with per-model effort, `/effort`, `/mode` via configOptions | `/thinking` (positive control) → "Thinking can't be toggled on the current model. Usage: /thinking [on\|off]" (model `auto` has `thinkingToggleable:false`); `/model` → picker of 21 models, `auto ✓` with description; picking `claude-sonnet-5.5` → "Model set to claude-sonnet-5.5." and toolbar `claude-sonnet-5.5 ◇ high` (default effort); `/effort` → picker low/medium/high ✓/xhigh/max, pick xhigh → "Effort set to xhigh.", toolbar `◇ xhigh`; `/mode` → Default ✓/Spec/Quick Spec/Bug Fix/Plan/Autonomous/semantic_reviewer. Glitch: the first `/effort` (typed, Esc, Enter) did nothing; the retry opened the picker | `cyril-kas/S3-settings-thinking.txt`, `S3-settings-model.txt`, `S3-settings-model-picked.txt`, `S3-settings-effort.txt`, `S3-settings-mode.txt` | CONFIRMED fixed (vh69 works on 0.66.43) |
| S4 trust (SHELL=bash) | cyril-auxt + cyril-k8wc | Always allow persists inertly to V2 | custom agent `trustprobe` picked via `/mode`; `echo TRUSTME` approval overlay offers ONLY "Allow / Deny" ("Preview unavailable"); after `/new` + `/mode trustprobe` the same command asks again; agent file unchanged; kiro.log `policy.shell.type.unknown {"shellName":"posix"}` | `cyril-kas/S4-trust-approval1.txt`, `S4-trust-approval2-newsession.txt`, `S4-trustprobe.{before,after}.json`, `S4-cyril.log` | CHANGED: the user cannot even choose Always allow (k8wc) |
| S4b trust (SHELL=fish, cyril as-is for this user) | cyril-auxt | same | overlay offers "Allow / Always allow / Deny / Always deny"; chose Always allow; same-session repeat: the model answered from context (no tool call — not a test); after `/new` + `/mode trustprobe` the command **asks again**; `trustprobe.json` byte-identical before/after and cyril.log has no persist line — on KAS cyril writes NOTHING (the KAS `allow_always` option carries no V2 trust-tier patterns, so `App::persist_trust_grant` (`crates/cyril/src/app.rs:1913-1918`) never runs); no `$HOME/.kiro/settings/permissions.yaml` either (no consent echo) | `cyril-kas/S4b-trust-approval1.txt`, `S4b-trust-approval2-newsession.txt`, `S4b-trustprobe.{before,after}.json`, `S4b-cyril.log` | CHANGED: grant is session-only; cyril neither writes V2 nor V3 |
| S4b write outside workspace | 2.32.0 folder scopes / cyril-gn07 | write approval with custom-folder scopes | prompt sent while busy became a **steer** ("↳ steer: … — applied"); "Write File" overlay shows the path TRUNCATED to the box width ("/home/dwalleck/repos/cyril/experiments/conductor-spike/a") + content preview "OUTSIDE"; options Allow / Always allow / Deny / Always deny — no folder or scope choice (KAS offers none on the wire either; the scope comes from the client's consent echo, see TUI-L1). Always allow → file written, "DONE" | `cyril-kas/S4b-write-outside-approval.txt`, `S4b-write-outside-done.txt` | CONFIRMED gap (no scope UI, no echo) + NEW display bug (path truncation) |
| S5 sub-agent | cyril-ebqu, cyril-1v25 | child frames ride parent; Subagent Response orphan dropped | "I'll delegate this to a sub-agent." → `✓ Sub-agent: general-task-execution` with `\| ALPHA` under it, then a top-level `✓ Read File` row AFTER the sub-agent row (the child's tool call, un-nested), then "DONE: ALPHA". cyril.log shows the orphan `ToolCallUpdate` titled "Subagent Response" (id never started) — no row for it; the answer reaches the screen only via the invoke tool's completion content | `cyril-kas/S5-subagent-done.txt`, `S5-cyril.log` | CONFIRMED (orphan dropped; child row un-nested and mis-ordered) |
| S6 MCP | cyril-1v25, cyril-2yn8 | Tool Load orphan dropped; MCP call titled from LLM | "I'll load the probe_echo tool and call it." → approval "@probesrv/probe_echo / Preview unavailable / Allow·Always allow·Deny·Always deny" → `✓ @probesrv/probe_echo` → "ECHO:HELLO-MCP (server probesrv)". cyril.log: orphan `ToolCallUpdate "Tool Load" Completed` — no row on screen | `cyril-kas/S6-mcp-approval.txt`, `S6-mcp-done.txt`, `S6-cyril.log` | CONFIRMED (Tool Load invisible; title readable) |
| S7 KIRO_HOME | cyril-d7ve | KAS honours absolute KIRO_HOME; cyril's agent path does not | `/mode` lists `kh-agent` (from `$KIRO_HOME/agents`), not `home-agent` (`$HOME/.kiro/agents`); KAS logs/sessions/session-index land under `$KIRO_HOME` | `cyril-kas/S7-kirohome-modes.txt`, `S7-cyril.log` | CONFIRMED (KAS side); cyril trust writes stay on `~/.kiro` (static, unchanged) |

- NEW cyril bug seen in S4/S4b/S6: while an approval overlay is open, cyril logs WARN `approval preview raw_input has no string text/content field`
  (`crates/cyril-ui/src/widgets/approval.rs:119`) on every render frame — 13,886 and 14,226 lines (3.9 MB `cyril.log`) in two short sessions.
  Lane copies are filtered (count noted at the end of each file); full logs in `tmp/`.

### Leg 3 / KAS-L2 = TUI-L5 — `diagrams` client capability (DONE 18:20-18:23 UTC)
Probe `probe-kas-leads-2.32.0.py diagrams` (KIRO_LOG_LEVEL=debug), arm A `initialize.clientCapabilities._meta.kiro.diagrams:true`, arm B absent;
positive control `_kiro/configuration/state` pushed 2× per arm. run1 (`kas-lead-diagrams-run1-2.32.0.jsonl`, both arms) wrote `calc.py` after
session/new and the model could not find it → the diagram turn was re-run with the file pre-seeded (`kas-lead-diagrams-2.32.0.jsonl`, arm A);
OLD arm A `kas-lead-diagrams-2.31.1.jsonl`.
- NEW kiro.log `[KiroAgent] Resolved capabilities: {…, "diagrams":true}` for A (false for B); OLD's capability line has no `diagrams` key.
- NEW A tool list (model-reported) ends with `create_diagram`; B and OLD-A lists are identical without it (run1 summary record lists both).
- NEW A: tool_call "Create Diagram" `_meta.kiro{toolOrigin:"acp", toolId:"create_diagram"}` (kas-lead-diagrams-2.32.0.jsonl#71"create_diagram");
  completion `_meta.kiro.diagram{id:"dg_<uuid>", sessionId, schemaVersion:1}`, `rawInput{template:"call-hierarchy", question, subjects, options,
  nodes, graph{edges[{from,to,kind,at}]}}`, `rawOutput{diagramId, sessionId, schemaVersion, message}` (kas-lead-diagrams-2.32.0.jsonl#72"schemaVersion").
  IR file `$HOME/.kiro/sessions/<hash>/<sid>/diagrams/dg_<uuid>.json` copied to `kas-lead-diagram-ir-dg_4a819cea-682b-4e52-bf40-d1371dc68d72.json`:
  `{irType:"diagram", schemaVersion:1, identity{template:"call-hierarchy@1", subjects, inputs{direction}}, content{shape:"graph", nodes[{id,name,
  locations[{filePath}],origin,kind}], edges[{from,to,sites[{filePath,startLine}],kind}]}, presentation{question, layout:"tree", orientation}}`.
- OLD 0.66.34 with the same flag: no tool, model says "I don't have a diagram tool". ⇒ **NEW in 0.66.43, gated on a client capability**
  (Kiro's TUI does not send it). cyril NOT-SENT (no `diagrams` in `crates/`). KAS-L2 / TUI-L5 CONFIRMED.

### Leg 3 / KAS-L3 — `settings.thinking` intake (zero turns) (DONE 18:24 UTC)
Probe `probe-kas-leads-2.32.0.py thinking`, captures `kas-lead-thinking-2.32.0.jsonl`, `kas-lead-thinking-2.31.1.jsonl` (KIRO_LOG_LEVEL=debug;
positive control `configurationState:true` → state pushes 4 / 2).
- `initialize…settings:{thinking:{enabled:true}}`: NEW kiro.log `configuration.intake.unknown-key {"key":"setting/thinking","layer":"client-connection"}`;
  OLD `configuration.intake.unread-key` for the same key. ⇒ 0.66.43 dropped `thinking` from the connection-layer schema; cyril still marshals it
  (`crates/cyril-core/src/protocol/kas/settings.rs:79` from `chat.enableThinking`, and DEFAULTS_ON at `:96`) — a third dead key next to
  `subagentOrchestration`/`todoList` (cyril-53qx).
- After `set_config_option model=claude-opus-5` then `thinking=on`, NEW pushes `_kiro/configuration/state` (session observer) with
  `krn:::setting/effortLevel` ("high") and `krn:::setting/thinking` ("on"), both asserted/contributed by layer `session`
  (kas-lead-thinking-2.32.0.jsonl#28"setting/thinking"); layers list now ends `…, client-connection, workspace, agent, session`. The lead's
  "source `session-reasoning`" is not the literal on the wire — the layer is `session`. CHANGED (shape detail).
- Control `settings:{evalStrategy:"audit-probe"}`: NEW `intake.unread-key` (known key), OLD `intake.unknown-key` ⇒ `evalStrategy` is a new
  connection-layer key in 0.66.43. No `evalStrategy.experimentConfig.created` line at debug in either engine (zero-turn session). KAS-L3 CONFIRMED
  for the thinking→unknown-key flip; experimentConfig line NOT-REACHED.

### Leg 3 / KAS-L6 (+L5, L8) — workflowPrompt plan approval, gate ON (cyril-8wh0, fixture) (DONE 18:24-18:26 UTC)
Probe `probe-kas-leads-2.32.0.py wfplan`, capture `kas-lead-wfplan-2.32.0.jsonl`; `settings.workflows.enabled` at initialize AND session/new
(`workflowsEnabled:true`); parent prompt "Use the run_workflow tool with a workflowPrompt … create hello.txt containing HI …". First plan ask
answered `reject` (after a 10 s hold + list/inspect), second `accept`, step tool asks `accept`. Run completed; parent auto-woke. Redacted fixture:
`experiments/conductor-spike/kas-workflow-plan-2.32.0.jsonl` (a header record plus twelve frames of the lane capture: kas-lead-wfplan-2.32.0.jsonl#92-296"workflowPlan").
- (1) `session/request_permission` on the creator step session (kas-lead-wfplan-2.32.0.jsonl#93"workflowPlan"): title "Add 1 workflow step to
  wf_<id>", options accept/always-accept/reject/always-reject, `_meta.kiro{toolId:"update_workflow", workflowPlan:{workflowId, nodeId:"create-workflow",
  kind:"launch", workflowRef:"generated://gen_<id>", keptStepIds:["create-workflow"], steps:[{nodeId:"write-hello", type:"step", agentName:"wf-coder",
  effortLevel:"low", effortSource:"step", task:{text:<first ~160 chars>, truncated:true}}]}}`. No `omitted` key in this one-step plan.
- (2) `toolCall.content[0]` = `{type:"content", content:{type:"text", text:"Steps to add after 'create-workflow':\n\nwrite-hello: step, agent wf-coder,
  model auto, effort low\n  Task: Create a file named hello.txt … con..."}}`.
- (3) The preceding `pending_interaction` session_info carries the same `_meta.kiro.workflowPlan` (kas-lead-wfplan-2.32.0.jsonl#92"pending_interaction").
- Reject → the run pauses (`run_complete:paused`), the creator `send_message`s the parent (`_kiro/session/notify` severity `warning`, "The proposed plan …
  was rejected. What would you like changed before I hand it off?", kas-lead-wfplan-2.32.0.jsonl#109"was rejected"); the parent auto-wakes and **its
  model answers on the user's behalf** ("The proposed plan is exactly right … Please proceed", kas-lead-wfplan-2.32.0.jsonl#164"exactly right"),
  after which the creator re-proposes the same plan (second ask, kas-lead-wfplan-2.32.0.jsonl#213"workflowPlan"). A user reject is therefore
  overridden by an unsolicited billed parent turn unless the client intercepts the wake.
- NEW notification `_kiro/workflow/steps_queued {workflowId, pendingSteps[{nodeId,type,agentName,effortLevel}], parentSessionId, attention}` then
  `{pendingSteps:[], resolution:{outcome:"applied"}, …}` (kas-lead-wfplan-2.32.0.jsonl#225"pendingSteps", kas-lead-wfplan-2.32.0.jsonl#233"applied");
  0 hits in either gate-on pair capture. cyril names it in its ext-method tables (`crates/cyril-core/src/protocol/engine.rs:746-747, 914`).
- L5 rider: while the plan ask was held open, `inspect` showed node-level `attention:[{kind:"permission", since, toolCallId, consentRound:1}]` and, on the
  second hold, `awaitingUserInput:true` on the node (3 hits in this capture, 0 in the gate-on pair).
- cyril cannot reach this flow today (no `workflows.enabled`, ADR-0011) — fixture only; the cyril-on-KAS part was dropped per orchestrator.

### Leg 3 / TUI-L3 — recipe `include` → `plan[]` / `includedFrom` (DONE 18:27-18:29 UTC)
Probe `probe-kas-leads-2.32.0.py include` (gate off). Two failed seeds first, kept as validation evidence:
- relative include path `parts/child.workflow.json` → listRecipes row `validationError: "Invalid workflow: include 'inc' (parts/child.workflow.json): include
  path must be absolute, 'bundled://<name>', or 'generated://<id>'; relative paths and bare recipe names are not supported …"` and `_kiro/workflow/new`
  -32603 with the same text (in `kas-lead-interrupt-sigkill-2.32.0.jsonl`, the combined include+interrupt run);
- absolute path but child recipe default `injectOriginalUserRequest` → "the recipe's injectOriginalUserRequest is true (the default) but this workflow's is
  false; set the same value on this workflow or include a different recipe." (`tmp/kas-lead-include-run2-inject-2.32.0.jsonl`).
- Valid seed (`kas-lead-include-2.32.0.jsonl`): `_kiro/workflow/listRecipes {}` row carries `description` and `plan:[{nodeId:"inc", type:"sequence",
  steps:[{nodeId:"inc/c1", type:"step", agentName:"wf-coder"}], includedFrom:{name:"child", path:<abs>}}, {nodeId:"p2", …}]`
  (kas-lead-include-2.32.0.jsonl#24"includedFrom"); `run_start.nodeTree` has the same shape (kas-lead-include-2.32.0.jsonl#33"includedFrom"); run
  completed; parent woke. TUI-L3 CONFIRMED. cyril: `includedFrom` 0 hits in `crates/` (node descriptors do not model it) → DROPPED; the
  `inc/c1` nested node id is carried as an opaque id.

### Leg 3 / TUI-L6 — interrupted-step pause text (DONE 18:26-18:32 UTC)
Probe `probe-kas-leads-2.32.0.py interrupt`: a one-step recipe running `sleep 45`; once its tool call started, the KAS process group was
killed, KAS respawned on the same HOME, `session/load` parent, then `inspect`/`list`/`load`. Four arms:

| engine | kill | `pauseReason` after restart | evidence |
|---|---|---|---|
| 0.66.43 | SIGTERM + stdin close | "Step interrupted (agent shutdown or connection reset); resuming the run retries the step." | kas-lead-interrupt-sig15-2.32.0.jsonl#102"resuming the run retries the step" |
| 0.66.34 | SIGTERM + stdin close | "Step interrupted (agent shutdown or connection reset); will resume." | kas-lead-interrupt-sig15-2.31.1.jsonl#104"will resume" |
| 0.66.43 | SIGKILL | "Interrupted by agent restart; the previously running step was paused for resume." | kas-lead-interrupt-sigkill-2.32.0.jsonl#130"Interrupted by agent restart" |
| 0.66.34 | SIGKILL | same as above | kas-lead-interrupt-sig9-2.31.1.jsonl#103"Interrupted by agent restart" |

- TUI-L6 CONFIRMED for a graceful shutdown (the changelog's "pause messages no longer say a run will resume on its own"); a hard kill takes a
  different, unchanged path. No `_kiro/workflow/*` notification is replayed on the reloading connection. cyril HANDLED: `pauseReason` is parsed
  (`crates/cyril-core/src/protocol/convert/kas/workflow.rs:303`, `:842`) into `WorkflowRun::run_pause_reason` (`crates/cyril-core/src/workflow.rs:313`)
  and shown verbatim.

### Leg 3 / HOST-L4 — closed stdout and SIGINT in text mode (DONE 18:31-18:33 UTC)
Probe `probe-kas-hostcloud-2.32.0.py stdout_closed,stdout_file,sigint`, capture `host-l4-2.32.0.jsonl` (one turn each).
- (a) `kiro-cli chat --no-interactive --agent-engine v3 "reply with exactly ok" | head -c 0` → `PIPESTATUS=1 0` (kiro-cli exits 1)
  (host-l4-2.32.0.jsonl#0"PIPESTATUS=1 0"); control `> out.txt` → `PIPESTATUS=0` (host-l4-2.32.0.jsonl#1"PIPESTATUS=0"). CONFIRMED.
- (b) SIGINT 8 s into a long turn: to the launcher pid alone → ignored, the turn ran to completion, rc 0 after 43.6 s (host-l4-2.32.0.jsonl#3"sigint");
  to the whole process group (what a terminal Ctrl+C does) → rc 130 after 8.7 s, and stderr carries NO "Error: the turn was cancelled before it
  finished" line (host-l4-2.32.0.jsonl#207"sigint"; 0 hits for "cancelled" in the capture). CHANGED: the cancel message did not appear for a
  group-wide SIGINT (the predicted message may need a SIGINT that reaches only the chat process).

### Leg 3 / own changelog checks — oversized image, fish approvals, session export (DONE 18:33-18:36 UTC)
Probe `probe-kas-leads-2.32.0.py image,fish,export`, captures `kas-lead-own-2.32.0.jsonl`, `kas-lead-own-2.31.1.jsonl` (same workload both engines).
- **Oversized image** ("retry at the provider's size limit"; sharp/libvips unbundled): a 9000×9000 PNG (253,405 B, over the 8000 px edge
  limit) as an ACP `image` block (kas-lead-own-2.32.0.jsonl#42"image/png") → `end_turn` "Red" in 10.6 s (kas-lead-own-2.32.0.jsonl#54"end_turn");
  OLD (with sharp) also `end_turn` "Red" in 11.2 s (kas-lead-own-2.31.1.jsonl#55"end_turn"); 1000×1000 control "Blue" on both; no
  `display_error`, no retry frame on the wire. No observable delta for this size — the retry path is internal (or not triggered by a
  compressible PNG); NOT-REACHED for a byte-oversized image.
- **Fish shell-syntax approval**: `shellType:"fish"` vs `"bash"`, commands `ls` and `cat PROBE.txt; and echo OK` — every command asked on both
  engines with the full four options and `consent.triggeringResource` (kas-lead-own-2.32.0.jsonl#247"and echo OK", fish arm); no parse-failure
  (`persistableConsent:false`) in either shell, so fish-syntax commands are parsed for the fish dialect. (The probe host runs commands
  with bash, so `and` failed at execution — a probe artefact.) No delta seen.
- **Session export** (`/chat save` archive): `_kiro/session/export` ignores `path`/`outputPath` and always writes
  `$TMPDIR/kiro-exports-XXXX/kiro-session-<sid>.zip` with `{success:true, filePath}` (kas-lead-own-2.32.0.jsonl#338"kiro-session-") — same on
  OLD (kas-lead-own-2.31.1.jsonl#350"kiro-session-"). The changelog's "non-.zip name says it's a zip archive" / "altered archive" messages are
  TUI-side; no KAS import method exists to test `/chat load`. cyril never calls `_kiro/session/export`.

### Leg 4 — sweep-new-fields + unread-fields (DONE 18:37-18:45 UTC)
- sweep-new-fields per pair: `sweep-new-fields-diff-sweep.txt` (leg 1), `sweep-new-fields-diff-wfoff.txt` (leg 2a), `sweep-new-fields-diff-wfon.txt`
  (leg 2b) — summarised under each leg. Only genuine NEW paths: workflow `attention[]` (all events/state/list) and
  `_meta.kiro.notification.{bufferOnly,callerSessionId}`; everything else is coverage (trap 11).
- unread-fields over the NEW captures cyril would receive (`kas-live-sweep-2.32.0.jsonl`, `kas-workflow-gateoff-2.32.0.jsonl`, `kas-lead-mcp-2.32.0.jsonl`
  (MCP, `probe-kas-leads-2.31.1.py mcp` arm none on 0.66.43), `kas-v3c-v2-2.32.0.jsonl` (sub-agent, `probe-kas-v3cover-2.31.1.py v2` on 0.66.43),
  `kas-lead-attention-2.32.0.jsonl` (cyril-style gate-off workflow)) → `unread-fields-2.32.0.txt`: 1301 paths, 147 topmost never named, **79 not in
  `docs/wire-ignored-fields.txt`** (exit 1). Same run over the OLD captures: 56 → 23 are NEW-only (listed first below); 56 pre-date 0.66.43 and are
  standing gaps the allowlist seed (2026-10-16) does not cover.
- Triage (no cyril code fix is proposed from this lane; every line is an allowlist line or an issue — the lane does not edit `docs/`):

| UNREAD path(s) | new in 0.66.43? | triage | proposed `docs/wire-ignored-fields.txt` line / issue |
|---|---|---|---|
| `_kiro/workflow/{run_start,node_start,node_complete,node_paused,paused,run_complete,loop_iteration,watch_poll}.attention`, `run_complete.finalState.attention`, `…finalState.root.children[].attention`, `result:_kiro/workflow/inspect.state.attention`, `…inspect.state.root.children[].attention`, `result:_kiro/workflow/list.runs[].attention` | YES | proposed issue (P2): show "waiting on you" (kind permission/question) per run/node from `attention[]` | `*.attention*   # workflow waiting-on-user markers (0.66.43); tracked: <new issue>` |
| `…run_start.inputs.{brief,report_path}`, `…finalState.inputs.*`, `result:…inspect.state.inputs.*`, `result:…new.initialState.inputs.*` | YES (bundled `investigate` inputs; dynamic keys) | by design: recipe-defined input names; `WorkflowRun.inputs` is an opaque `serde_json::Value` (`crates/cyril-core/src/workflow.rs:297`) | `*.inputs.*   # recipe-defined input names (dynamic keys); kept opaque in WorkflowRun.inputs` |
| `session/update.update._meta.kiro.notification.{bufferOnly,callerSessionId}` | YES | tracked: step-verdict relay metadata | `session/update.update._meta.kiro.notification.*   # step-verdict relay metadata on replay; tracked: cyril-fb1m` (also covers `notification.{eventType,nodeName,notifyId}` below) |
| `_kiro/progressive_context/items_changed`, `_kiro/steering/documents_changed`, `_kiro/tools/didChange` (whole) | no | tracked | `_kiro/progressive_context/items_changed   # tracked: cyril-58uv`; `_kiro/steering/documents_changed   # tracked: cyril-58uv, cyril-2nz7`; `_kiro/tools/didChange   # tracked: cyril-58uv` |
| `…paused.pause.iterationsRun`, `…finalState.pause.iterationsRun`, `…inspect.state.pause.iterationsRun`, `…root.children[].extendedMaxIterations`, `…watchCursor.outputFile`, `…stopInitiator` (run_complete/inspect/load), `…parentModelId` (finalState/inspect/new), `result:_kiro/workflow/retry.retriedNodeIds` | no | by design for now (run-panel detail cyril does not render); propose folding into the attention issue's panel work | `*.pause.iterationsRun`, `*.extendedMaxIterations`, `*.watchCursor.*`, `*.stopInitiator`, `*.parentModelId`, `result:_kiro/workflow/retry.retriedNodeIds`   `# workflow run detail not rendered` |
| `result:session/list.sessions[]._meta.kiro.{agentMode,executionTarget,serverSession,settings.memory.reflection}`, `result:session/load._meta.{agentMode,contextUsage.*Threshold,executionTarget,kiro.loadTiming,lastModifiedAt}`, `result:session/new._meta.{agentMode,ftaEnabled,lastModifiedAt,schemaVersion,semanticReviewEnabled,specPlanEnabled,specSkipClarificationEnabled,specWorkflow,workflowsEnabled}` | no | `executionTarget` tracked by cyril-tikf; spec/fta flags by cyril-ea67; the rest is session metadata cyril does not show | `result:session/*._meta*.executionTarget   # tracked: cyril-tikf`; `result:session/new._meta.{fta,spec,semanticReview}*   # tracked: cyril-ea67`; `result:session/*._meta*   # session record metadata, not displayed` |
| `session/request_permission._meta.kiro.mcpTool` | no | tracked | `… # trusted MCP tool identity; tracked: cyril-2yn8` |
| `session/request_permission._meta.kiro.agentManagesTrust` | no | proposed note on cyril-auxt (the agent declares it manages trust itself — relevant to the consent-echo re-scope) | `… # tracked: cyril-auxt` |
| `session/request_permission._meta.kiro.workflowWatch` | no | by design (watch approvals render as ordinary approvals) | `… # watch-command approval context; not rendered` |
| `session/update.update._meta.kiro.{pendingInteraction,interactionType,interactionResolved,selectedOption}` | no | by design: cyril drives approvals from `session/request_permission`; the info kinds are mirrors | `… # pending_interaction/interaction_resolved mirror the permission request; tracked: cyril-58uv` |
| `session/update.update._meta.kiro.{displayError,errorType,retryErrorType}` | no | tracked (display_error kind dropped: `crates/cyril-core/src/protocol/convert/kas.rs:506-640` matches no `display_error`) | `… # tracked: cyril-58uv` |
| `session/update.update._meta.kiro.{availability,agentMode,userMessageId,userMessageTag,toolOrigin,steeringClearedIds,preview.modifiedContent,preview.originalContent}` | no | `availability` (checkpoint_restore_availability) tracked by cyril-fb1m; `toolOrigin` by cyril-f4tt-adjacent tool identity (cyril-2yn8); the rest are ids/previews cyril does not render | one line each with the reason/issue named here |
| `session/update.update.availableCommands[]._meta.kiro.commandId` | no | tracked: advertised-command dispatch | `… # tracked: cyril-oieu` |

### Correction to leg 1 + KAS-L4 / KAS-L7 (passive, from every session/load in the lane captures)
- **CORRECTION (leg 1 "load result carries the saved model")**: the OLD/NEW sweep comparison was confounded by catalog warmth. Across all
  captures: OLD includes the full `model` select in the `session/load` result when the model catalog is already warm
  (kas-workflow-gateoff-2.31.1.jsonl#1168"loadTiming", 21 options) and omits it when cold (kas-live-sweep-2.31.1.jsonl#318"loadTiming",
  kas-workflow-gateon-2.31.1.jsonl#2396"loadTiming"). NEW with a warm catalog sends the full select (kas-live-sweep-2.32.0.jsonl#276"loadTiming");
  NEW with a **cold** catalog sends a **one-option pin** `{id:"model", currentValue:"auto", options:[{value:"auto", name:"Auto"}]}` with no `_meta.kiro`
  (kas-workflow-gateon-2.32.0.jsonl#2550"loadTiming") and the full 21-option select follows in a `config_option_update` 5 frames later
  (kas-workflow-gateon-2.32.0.jsonl#2555"config_option_update"). So the 0.66.43 change is the cold-catalog pin (KAS-L4 CONFIRMED); no
  one-option `effortLevel` select appeared because the saved model was `auto` (no effort). cyril HANDLED: the load result's configOptions
  and the later update both flow through `ConfigOptionsUpdated` (`crates/cyril-core/src/protocol/domain_mediator/commands/session.rs:181-190`);
  until the update lands cyril's `/model` picker would list only `auto`.
- KAS-L7 (`syntheticUserMessageReason:"max-tokens-recovery"`): 0 hits in every lane capture — NOT-REACHED (no natural max-token stop occurred;
  forging one is off limits, trap 13). The changelog item "turn no longer ends early when the model spends its whole output budget on reasoning"
  is likewise NOT-REACHED.

