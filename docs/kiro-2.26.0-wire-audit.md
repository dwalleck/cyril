# kiro-cli 2.26.0 wire audit — workflows go GA (covers 2.24.1, 2.25.0, 2.26.0)

Audited 2026-09-29. Baseline: 2.24.0 ([kiro-2.24.0-wire-audit.md](kiro-2.24.0-wire-audit.md)).
Focus: the 2.26.0 headline "**[V3] Enables multi-step asynchronous workflows with
reusable recipes. `/settings` → features to enable**" — did the `_kiro/workflow/*`
ACP surface cyril drives (ADR-0011, ROADMAP W track) change?

**Verdict: SAFE for cyril's workflow control plane.** GA is a launcher rollout-segment
flip, not an engine change. The KAS gate is byte-identical logic, cyril's gate-off
`new → invoke` path runs to `completed` on KAS 0.66.15, and the lifecycle event
vocabulary, payload keys, double `node_start` and ordering match the 2.24.0 capture.
New surface is additive: one gated notification, one optional `resume` param, one
`finalState` key.

## 1. Releases and artifacts

| ver | build | `@kiro/agent` (KAS) | notes |
|---|---|---|---|
| 2.24.1 | `0e0f0e19` 2026-09-25 | 0.66.8 (byte-identical to 2.24.0) | no embedded changelog entry |
| 2.25.0 | `118eed59` 2026-09-28 | 0.66.11 | +`recipes_changed`, +`_kiro/memory/*` |
| 2.26.0 | `15d7f349` 2026-09-29 | 0.66.15 | workflows rollout `internal → all` |

Tarballs are `.tar.xz` (a versioned `manifest.json` does not exist — 403/non-JSON);
2.26.0 verified against `latest/manifest.json` sha256 `3b0e528a…`, and its
`kiro-cli-chat` is byte-identical to the installed `~/.local/bin` copy. AUR pins
`.tar.zst`, so 2.24.1/2.25.0 have no independent pin. KAS carved with the 2.24.0
recipe (first zstd frame of `kiro-cli-chat`) into
`~/.local/share/kiro-research/kas-carves/<ver>/`; TUI bundles carved to
`tui-bundles/kiro-tui-{2.25.0,2.26.0}.js`.

## 2. Where "GA" actually lives

Host rollout registry (`static-rollout-2.24.0.py` over the three binaries):

| release | `workflows` row |
|---|---|
| 2.24.0, 2.25.0 | `treatment=100 segment=internal` — "All internal users on the KAS engine can opt in via chat.enableWorkflows in /settings features." |
| **2.26.0** | `treatment=100 segment=all` — "Generally available to all users on the KAS engine; opt in via chat.enableWorkflows in /settings features." |

The TUI computes `workflowsAvailable = rollout("workflows")` and
`enabled = available && chat.enableWorkflows` (default `false`) — both unchanged
since 2.24.0. The ACP server never consults the rollout registry
([2.22.0 audit § 4b](kiro-2.22.0-wire-audit.md)), so nothing about GA reaches
`acp --agent-engine kas` except what a client sends.

Registry also grew 19 → 21 in **2.25.0**: `background_execution` (0 % internal
nightly, dark — "Opt in anywhere with KIRO_ENABLED_FEATURES"; drives a
`backgroundExecution` KAS session setting) and `memory_controls` (0 % internal
nightly, `/memories` browsing).

## 3. KAS gate — unchanged

Same resolver in 0.66.8 / 0.66.11 / 0.66.15:

```js
workflowsEnabled = settings.workflows?.enabled ?? persistedSessionMeta.workflowsEnabled ?? false
```

`settings` = `session/new` `_meta.kiro.settings`. What the 2.26.0 TUI sends when the
toggle is on: `{workflows:{enabled}, goal:{enabled}, workflowNotifications:{enabled,
delivery?}}` (`goal` rides the same toggle). `_kiro/session/setWorkflowNotificationDelivery`
is a **TUI-internal** extension-runtime call (present in the 2.24.0 TUI; zero
occurrences in KAS) — not wire surface.

## 4. Static delta, KAS 0.66.8 → 0.66.15

* `_kiro/workflow/*` literals 24 → **25**: `+recipes_changed` (0.66.11). All 24 prior
  methods/events retained. TUI registers the same 9 lifecycle kinds + `recipes_changed`;
  node types `{step, sequence, repeat, parallel, watch}` unchanged. Same 7 bundled
  recipes (autoresearch, feature-pipeline, goal, investigate, publish-pr, ralph,
  semantic-review-multi-model).
* `_kiro/memory/{create,delete,get,list,update}` (0.66.11) — KAS agent memory
  (rollout `memory` still internal nightly); overlaps cyril-memory.
* **`_kiro/workflow/resume` += optional `extendRepeat: {nodeId, additionalIterations}`**
  (positive int) — grants more iterations to a `repeat` that parked on
  `onMaxIterations: "pause"` (value pre-existing), then resumes. Errors as
  `-32603`-class `"_kiro/workflow/resume: <reason>"`. Model-side twin:
  `update_workflow` action `extend_repeat`.
* Watch handlers: `+kind:"shell"` alongside `command`/`file`/`url`; the command
  watcher now asks permission through the **parent session's** tool-approval path
  (`execute_bash`, operationId `workflow-watch-<wf>-<node>-<n>`) — i.e. a
  `session/request_permission` attributed to the parent, relevant to cyril-z4eo.
* Parent-authority hardening (log ids only, no payload change observed):
  `node.parked_awaiting_parent_session` — a step that must ask the user while its
  parent session is not loaded in this agent process emits a plain `node_paused`
  with a prose `reason` ("…continues when the session loads…"), classified
  `transient-error`. Relevant to cyril's reattach-on-demand model: a detached run can
  park on this.
* Checkpoint restore wiring (`workflow.checkpoint.*`) and a new
  `session_info_update` kind `checkpoint_restore_availability`.

## 5. Live — two legs on the installed 2.26.0 launcher (KAS 0.66.15)

Probe: `experiments/conductor-spike/probe-kas-workflow-channels-2.26.0.py`
(`WF_GATE=off|on`, restate style, temp `HOME`, real `XDG_DATA_HOME`, serial legs).
Captures `kas-workflow-channels-06615-restate-gate{off,on}-2.26.0.*`.

| | gate off (cyril) | gate on (TUI shape) |
|---|---|---|
| `session/new` `_meta.workflowsEnabled` | `false` | `true` |
| `new` → `invoke` → `run_complete` | **completed**, 15.2 s | completed, 17.8 s |
| capture `{{s1.output}}` / artifact file | ALPHA / ALPHA | ALPHA / ALPHA |
| `listRecipes` | 9 recipes | same 9 |
| `recipes_changed` | **none** | 1, after a recipe file was written mid-session |
| permission requests | 2 | 2 |

Event vocabulary and payload keys vs `kas-workflow-channels-0668-restate-2.24.0`:
identical except `run_complete.finalState.memoryConfig {mode, reflection}` (also on
`initialState` and `session/new` `_meta`). Double `node_start`, per-step
`turn_completion → turn_end → node_complete`, and parent
`focus_update.activity {turnActive, runningWorkflows, pausedWorkflows}` unchanged.

`recipes_changed` payload: `{sessionId, workspacePaths[], recipes[]}` where each
row is exactly a `listRecipes` row (`name, description, inputs, plan[], source,
builtIn, _meta.kiro.resource`). Full catalog, not a delta. Not emitted for the
session-start baseline refresh — only on a real change.

Other new frames seen (non-workflow, all drop safely in cyril today —
`convert/kas.rs` unknown `session_info_update` kinds fall to `_ => None`; cyril
does not consume `_kiro/session/notify`; no `deny_unknown_fields` on any ACP path):

* `session_info_update` kind **`checkpoint_restore_availability`**
  `{availability:{allowed:false, reason:"Checkpoint reverts are disabled for running workflows."}}`
  on each step session (and re-enabled after).
* `_kiro/session/notify` += **`sender: "step"`** (a step's `send_message` relayed to
  the parent, alongside `nodeId`, `workflowId`, `severity`, `callerSessionId`).
* `user_message_chunk` `_meta.kiro.timestamp`; `context_usage` breakdown `+memory`.
* `agentCapabilities.sessionCapabilities.close` — `session/close` advertised.

## 6. Consequences for cyril

1. **Nothing in the wire contract breaks.** ADR-0011 (never set the gate) still
   holds at GA; the client-owned `/workflow` control plane works unchanged.
   **But see § 8:** a long-standing KAS auto-wake gives every cyril-invoked run
   an unsolicited, billed model turn on the main session (cyril-lki9).
2. **Gate-off costs `recipes_changed`.** If cyril wants live recipe-catalog
   refresh it must either set `workflows.enabled` (which also hands the model
   `run_workflow` et al. — the thing ADR-0011 avoids) or keep polling
   `listRecipes`. Decision, not a bug.
3. `/workflow resume` could expose `extendRepeat` for loops parked on max
   iterations.
4. Approvals queue (cyril-z4eo): watch-node shell commands now raise permission
   requests on the **parent** session, not a step session.
5. Reattach: runs can park with `node_paused` "awaiting parent session" when the
   parent isn't loaded — the renderer should show the prose reason (no `kind`
   discriminator, unlike `retry-wait`).
6. Unverified, worth a probe: 2.26.0 "Stale tool approvals are now rejected
   instead of honored" — a late answer from cyril's queued approvals may now be
   refused.

## 7. Changelog (embedded)

2.24.1: none embedded. 2.25.0 and 2.26.0: see `kiro-cli version --changelog=<ver>`;
2.26.0's `[V3]` security items (per-file approval for hook writes/LSP edits, MCP
consent, untrusted shell starts each time, stale approvals rejected) are
approval-policy changes in KAS; 2.25.0 notably fixes "Loading a v2 or classic
session over ACP on the v3 engine now replays its history".

## 8. Addendum (same day) — `/workflow new` authoring and parent auto-wake

Issues filed from this section: **cyril-lki9** (auto-wake bug, P1) and
**cyril-0asq** (authoring design, blocked on lki9).

### 8a. The TUI's creation menu is a canned prompt

The V3 TUI's `/workflow` subcommands are `new | list | run | retry` (plus
`pause | resume | status | cancel`). All except `new` map onto `_kiro/workflow/*`.
**`/workflow new <description>` sends a plain `session/prompt`** (tui.js `jCn`),
extracted verbatim to
`experiments/conductor-spike/tui-workflow-new-prompt-2.26.0.txt` (bundle sha256
`df77898749d0f3fb…`). It tells the model to delegate to the bundled
`wf-workflow-creator` agent, have it validate via `save_workflow_definition` and
return the full JSON, then write `.kiro/workflows/<name>.workflow.json`. If the
creator is unavailable, the prompt falls back to authoring and running
`validate_workflow` itself. The creator and the other 8 `wf-*` agents, plus the
validate/save tools, exist only when `workflowsEnabled` is true.

Row visibility: `/settings → Features → Workflows` shows only when
`Qn("workflows") && engine === "kas"`. It is **hidden on V2**, and the
"relaunch with --v3" hint is `KIRO_INTERNAL`-only.

### 8b. Live: authoring (`probe-kas-workflow-new-2.26.0.py`, `analyze-workflow-new-2.26.0.py`)

| | gate on (TUI) | gate off (cyril) |
|---|---|---|
| delegation | `run_workflow{workflowPath:"agent://wf-workflow-creator"}`: **async** run | Sub-agent `wf-workflow-creator` → **failed** (unregistered) |
| turns on main | 2 (1 client prompt + **1 server wake**) | 1 |
| JSON handoff | `send_message` carried a 204-char summary only; JSON only in `capturedOutputs.run` (2,905 chars) | n/a |
| recovery | wake turn: `send_message` to the finished step (dead letter), `validate_workflow` itself, Write File (permission request mid-wake) | wrote the file **unvalidated**, reported success |
| `listRecipes` | `summarize-notes` **valid** | `summarize-notes` **INVALID**: `inputs.notes_dir: Expected string, received object (+4 more issues)` |
| elapsed | 109 s | 117 s |

`listRecipes` returns `validationError` per recipe **with the gate off**, so
cyril can verify authored recipes without the gate.

### 8c. Live: KAS auto-wakes the parent on every terminal run

`probe-kas-workflow-channels-2.26.0.py` with `WF_GATE=off WF_TAIL=60`, i.e. a run
started exactly as cyril starts one (`new` with `parentSessionId` = main session,
then `invoke`). At `run_complete: completed` **+0.0 s**, KAS opened a turn on the
main session (`turn_start`). The model read two files and streamed a results
summary, then `turn_end` at +13.1 s. There was **no client `session/prompt`**, the
injected wake text never reaches the client (no `user_message_chunk`), and there
is no prompt response; `turn_end` is the only end marker.

Static (0.66.15, present at least since 0.66.0 / 2.22.0): `autoWakeParentOnComplete`
fires on any terminal `run_complete` whose parent resolves
(`parentSessionId ?? finalState.parentSessionId ?? registry`). The default
`wakePolicy.decide` **always** returns `wake:true`: "A workflow you launched
("X") completed/failed/aborted. Review its results and continue if you were
waiting on it.", with softer text when the user stopped or completed the run. It
loads an unloaded parent on demand. `rewakeWithInfo` also wakes an idle session
when a step's `send_message` arrives. No client setting disables either path.
Earlier audits missed this because every workflow probe stopped at `run_complete`.
