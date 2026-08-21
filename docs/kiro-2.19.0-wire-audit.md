# Kiro CLI 2.19.0 wire audit

**Audited:** 2026-08-20 (release date 2026-08-18). Binary `BUILD_HASH=60eb656b3201c6e0a5b800c3b9dde677e0e33396`, archived at `~/.local/share/kiro-research/binaries/2.19.0/` (sha256 verified against the origin manifest; the installed `~/.local/bin/kiro-cli` is byte-identical). tui.js snapshotted to `tui-bundles/kiro-tui-2.19.0.js`.

**Headline:** the **KAS freeze is over** — `@kiro/agent` jumps **0.38.7 → 0.46.1** (eight minor versions at once; bundle 21.7 MB → 23.1 MB) — yet the `_kiro/*` + `_session/*` extension-method vocabulary is **byte-stable at 110 strings**. Meanwhile the **"v2 wire frozen" doctrine is broken this release**: the Rust engine gains new session-update kinds (`stream_stall_notice` new, `stream_discarded` new, `retry_warning` pre-existing but newly emitted broadly), new `api.*` settings, mid-stream agent-layer retry, and mid-turn auth refresh. KAS's wire deltas are field-level only: `messageId` on `turn_start`/`turn_end` frames, `kind:"retry-wait"` on workflow `node_paused`, resource-identity `_meta.kiro.resource` stamps, `instanceStatus` on session/list rows.

**Cyril verdict: SAFE.** Happy-path wire on both engines is unchanged (live-verified). The new v2 kinds are warn-logged and dropped by `convert/kiro.rs` today (`unhandled kiro.dev/session/update variant`) — graceful by construction. Follow-ups filed: **cyril-svi2** (render `stream_stall_notice`/`retry_warning`), **cyril-9weh** (`stream_discarded` → roll back uncommitted streaming text; silent-duplication hazard), **cyril-f7tm** (WorkflowTracker `retry-wait` pause discrimination).

**Engine split (the load-bearing fact):** every timeout in the changelog — stream-idle watchdog, `api.timeout`, `api.subagentTimeout` — is **v2-engine-only**. The KAS 0.46.1 bundle has ZERO occurrences of `streamIdle`/`subagentTimeout`/`api.timeout`/`idleDeadline` (its only `api.*` string is `api.aws`), and no new turn-level timer on the prompt path. **A backend stall on KAS still never ends the turn** — the bh7g window stays open on the default (`chat.agentEngine: "v3"`) engine, and cyril's 30 s `TurnStalled` chip remains the only stall signal there.

---

## 1. Stream-idle watchdog (v2) — LIVE-CONFIRMED, full wire contract

2.19.0 adds an idle watchdog on the model response stream: warn at 60 s, cancel at 300 s, tunable via `api.streamIdleSoftTimeout` / `api.streamIdleHardTimeout` (settings keys in `~/.kiro/settings/cli.json`; **units are seconds**; unset by default — no schema entry, honored at runtime).

Probe method: fake `HOME` with `cli.json` `{"api.streamIdleSoftTimeout":1,"api.streamIdleHardTimeout":2}`, 68 KB prompt to inflate prefill TTFB past 2 s. Traces: `watchdog-v2-{control,wd1,hard}-trace.jsonl` (session scratchpad; key frames reproduced below).

### Soft timeout → new extension notification

At exactly soft-timeout seconds of stream silence (TTFB counts — the timer runs from request start, before the first SSE event):

```json
{"jsonrpc":"2.0","method":"_kiro.dev/session/update","params":{
  "sessionId":"…",
  "update":{"sessionUpdate":"stream_stall_notice","message":"Still working, model is thinking..."}}}
```

New inner kind `stream_stall_notice` riding the existing `_kiro.dev/session/update` ext envelope (same envelope as `tool_call_chunk` and the steering echoes). Deliberately **not** a typed `session/update` variant — the schema crate's `SessionUpdate` enum has no catch-all, so a typed addition would hard-fail old deserializers; the ext path degrades gracefully.

### Hard timeout → abort + retry ×3 → JSON-RPC error

With hard=2 s and TTFB > 2 s, the observed sequence (v2-hard trace, all timestamps from prompt send):

```
+1.00s  stream_stall_notice "Still working, model is thinking..."   (soft, attempt 1)
+2.01s  stream_stall_notice "Response timed out - retrying"         (hard fires → retry)
+3.01s  stream_stall_notice "Still working, model is thinking..."   (soft, attempt 2)
+4.01s  stream_stall_notice "Response timed out - retrying"         (hard → retry)
+5.01s  stream_stall_notice "Still working, model is thinking..."   (soft, attempt 3)
+6.01s  session/prompt ERROR response:
        {"code":-32603,"message":"Internal error",
         "data":"Encountered an error in the response stream: The stream timed out receiving the response after 2000ms"}
+6.01s  _kiro.dev/metadata {sessionId, turnDurationMs: 6005}
```

So the contract on watchdog exhaustion is: **3 stream attempts** (initial + 2 retries, each with its own soft warn + hard cancel), then the turn ends with a **JSON-RPC error response** to `session/prompt` (`-32603`, human-readable `data` string) — *not* a result with a `stopReason`. A trailing `_kiro.dev/metadata` still arrives (long-standing `turnDurationMs` field — present since 2.3.0, not a delta).

Static corroboration (symbols, all new in 2.19.0): watchdog in `agent::agent::agent_loop` (`with_idle_timeouts`/`next_idle_deadline`/`handle_idle_deadline`/`end_current_stream`); `StreamTimeoutSource {IdleWatchdog, SdkRecv}`; `StallRetryOutcome {Recovered, Exhausted}`; API error variant `StreamTimeout {timeout_source}`; internal events `StreamStalled`/`StreamStallContinuation`/`StreamStallRecovery`. On **Recovered** the turn simply continues (metric `kiro_cli_stream_stall_recovery_seconds` = hard-cancel → first event of the retried stream); only **Exhausted** ends the turn (the −32603 observed live). No new `stopReason` values; no new *standard* `session/update` kinds — the three notice kinds live only in the Kiro dialect union, matching the live-observed `_kiro.dev/session/update` envelope. Two hard-tier message variants exist — "Response timed out - retrying" (observed live; no partial output) and "Response timed out - discarding the stalled response and retrying" — and `stream_discarded` correspondingly fires only when rendered partial output existed (why the live TTFB-stall capture showed none; stall telemetry carries a `partial_output` y/n label). Notable guards: `track_mcp_stall_signals` feeds MCP activity into the watchdog (long-running MCP tools don't count as stalls); soft ≥ hard disables the soft tier ("stream-idle soft timeout is >= the hard timeout; disabling the soft warning tier"). The v1 (non-ACP) engine got the same treatment (`pre_headers_timeout_error`, "the response stream exceeded the idle deadline").

The "retry banner" of the changelog is these same `stream_stall_notice` messages — **retry state is ACP-visible**, distinguished only by message text ("Response timed out - retrying"), not by a structured field. Do not key logic on the prose; treat any `stream_stall_notice` as "agent still alive, stream degraded".

### Consequences for the turn-stall model (bh7g / cyril-14ou)

- The bh7g finding "a backend stall never ends the turn" is **repaired at the source for the v2 engine**: a dead stream now self-cancels at ≤ 300 s × 3 attempts (≈ 15 min worst case with defaults) and the turn ends with an error response. The stall *window* still exists (up to that bound), so cyril's 30 s `TurnStalled` chip remains useful — it fires long before kiro's own 60 s soft notice.
- Cyril today: `convert/kiro.rs` unknown-variant arm → `Err(Protocol)` → `client.rs` logs `warn!("malformed extension notification")` and drops. No crash, no user signal.
- Follow-up (rivets): parse `stream_stall_notice` → notification → render as an activity chip; it is the engine's *authoritative* stall/retry signal and composes with (or refines) the synthetic TurnStalled detector. Also verify the prompt-error path clears `is_busy` and surfaces the `data` string (it should — bridge error-notification invariant).

### KAS side — RESOLVED: no watchdog exists in KAS

Same probe against `--agent-engine kas` (settings in fake-HOME `cli.json`): a 3.78 s ACP-visible TTFB gap produced **no notice and no cancel** — and the static diff explains why: the 0.46.1 bundle contains zero occurrences of `streamIdle`/`watchdog`/`softTimeout`/`hardTimeout`/`api.timeout`/`subagentTimeout` in either version, and no new turn-level timer on the prompt path. The only new `setTimeout` deadline anywhere is the 15 s session-title LLM abort. KAS's mid-turn resilience remains the pre-existing **silent** stream-recovery (`empty_response_retry`/`truncated_response_retry`/`stream_error_retry`, all default true, byte-identical to 0.38.7, nothing on the ACP wire; terminal failures carry `retryErrorType` inside the −32000 error data — also pre-existing).

### `stream_discarded` (v2, static) — the mid-stream-retry duplication hazard

A second new v2 kind, `stream_discarded` (binary strings 0→3; tui.js handler 0→4: "Stream discarded notice received; dropping rendered partial"): when a mid-stream reset triggers the new agent-layer retry, the client is told to **drop the partially rendered stream** before the retried attempt re-streams. tui.js rolls back; cyril appends `streaming_text` with no rollback → silent duplicated prefix if ignored (cyril-9weh). Envelope inferred from string-table co-location with `_kiro.dev/session/update` — confirm with a capture before shipping the fix.

### `retry_warning` (v2, structured, pre-existing kind — broader emission)

tui.js validates exactly: `{attempt:number, maxAttempts:number, delaySecs:number, message:string}` — the retry banner's feed. The kind and its emitter (`delay_interceptor.rs`) pre-exist 2.18.1; 2.19.0 adds a "Connection dropped, retrying in " site and agent-layer mid-stream retry ("scheduling transient stream retry at the agent layer", new). On exhaustion the turn error message is suffixed `(failed after N attempts)`. In the TUI, any non-`retry_warning`/`stall_notice` stream event clears the banner — a sensible display contract for cyril's chip too (cyril-svi2).

## 2. KAS 0.46.1 — freeze over, wire stable (live-verified)

- `@kiro/agent` 0.38.7 → **0.46.1**. Only 4 files differ in `dist/`: `acp-server.js` (+1.36 MB), `acp-server.externals.json` (**+`node:sqlite`** — matches the new `ExperimentalWarning: SQLite` at boot), `multiplex-stream.d.ts`(+`.map`).
- `_kiro/*` + `_session/*` method vocabulary: **110 = 110, zero diff.**
- **Turn-end ordering re-verified live** on a real turn: `session_info_update kind:turn_completion` (credits/elapsedTime/requestIds) → `_kiro/sessions/changed` → `session_info_update kind:turn_end {stopReason:"end_turn"}` → `session/prompt` response **7 ms later**. Both terminals, turn_end first — the cyril-14ou model holds unchanged.
- Boot notification storm unchanged in kind: `_kiro/governance/state`, `_kiro/mcp/status`, `_kiro/powers/items_changed`, `_kiro/progressive_context/items_changed`, `_kiro/sessions/changed`, `_kiro/steering/documents_changed`, `_kiro/tools/didChange` (NB `_kiro/policy/changed` did not appear in this probe's storm; it was listed in the 2.10.0 storm — unverified whether gated or timing).
- `session_info_update` kinds observed this audit: `user_message_id_assigned`, `focus_update` (carries live `title`), `turn_start`, `context_usage`, `turn_completion`, `turn_end`.
- `initialize` result: `agentInfo` **absent** on KAS (v2 sends it since 2.1.0); `sessionCapabilities {list:{}, fork:{_meta.kiro.messageId:true}}`.

### KAS field-level wire deltas (static, 0.38.7 → 0.46.1)

- **`turn_start`/`turn_end` frames gain `messageId`** (`${executionId}-turn-start`, `${persistId}-turn-end`; observed live in this audit's traces). Purpose per doc comment: relay replay dedup ("the persisted turn_end row's id, so a relay dedups the row's replay against this live frame"). `broadcastTurnEnd(stopReason, messageId, stopDetails)` — a `stopDetails` spread also exists. Additive; cyril's `turn_end` parse reads only `kind`+`stopReason` and is unaffected.
- **Workflow in-place transient retry** (the sleeper change — cyril-f7tm): new `src/workflow/step-error-classifier.ts` classifies step errors (`interruption|throttle|transient-network|transient-service|permanent`); `withTransientRetry` re-drives the SAME step session in place. Before each wait it emits `node_paused` with **new payload field `kind:"retry-wait"`** and reason "Throttled, retrying in Ns (attempt X/Y)." — the node's persisted status stays `running`; the event is the only client-visible signal — then **re-emits `node_start` for the same node/session** after the sleep. Delays `[10,20,60,120]` s (throttle) / `[10,20,60]` s (network), env overrides `KIRO_WORKFLOW_THROTTLE_RETRY_DELAYS_SEC` / `KIRO_WORKFLOW_TRANSIENT_RETRY_DELAYS_SEC`. Retry-wait pauses are excluded from transcript persistence but delivered live. 0.38.7 paused immediately on transient errors with no in-place retry. Cyril's tracker: `node_start` merge-not-append already tolerates the re-emission; the pause-kind discrimination is cyril-f7tm (zd8u's "immediacy keys off node_paused" now needs the discriminator).
- **Steering session-scoping** (the "[V3] steering docs and slash commands no longer leak across sessions" fix): new `src/steering/session-scope-filter.ts` — `SteeringManager.sessions` is now `Map<sessionId,{workspacePaths}>`; filter-on-read (`client`-source and `global`-scope docs always visible; workspace docs only when under a session root) applied at turn-time assembly, `_kiro/steering/documents_changed` pushes (now per-session filtered), and slash-command enumeration. Unknown/destroyed session degrades to global-only (fail-closed). The untagged "fileMatch/manual no longer auto-injected by glob match" fix is **NOT in KAS** (that logic is byte-identical) — it's v2/tui-side.
- **Resource-identity stamps**: `_kiro/steering/documents_changed` docs and progressive-context items now carry `_meta.kiro.resource {resourceType: steering|skill|agent, source:{origin: bundled|client|user|workspace(+root)|cloud(+provenance)|power}}` — additive provenance cyril could surface.
- **`session/list` rows** gain `_meta.kiro.instanceStatus` (`active|provisioning|suspended|failed`) for cloud sessions; row shape otherwise unchanged.
- **Model settlement hardening** (the KAS half of the cold-registry fix): `pinSessionModelId` contract flips from "undefined when cold — callers apply their own `auto`" to "**never synthesize a model id the registry did not serve**" (GovCloud rationale: `auto` isn't registered in every partition); new `ModelRegistryUnavailableError`, `settleSessionModel` with classified retries (credential failures terminal within a turn; `MODEL_SETTLE_RETRY_BACKOFF_MS=100`, 5 s refresh-wait ceiling, cancellable). `buildModelConfigOption` (omit when empty) unchanged; `setSessionConfigOption` still applies any value unvalidated — the "[V3] /model keeps active model on unavailable pick" guard is picker/TUI-side, not KAS-side.
- **MCP SDK 1.x → 2.0 swap**: `@modelcontextprotocol/sdk` removed; `client`/`core` 2.0.0 added. Modern protocol revision `2026-07-28` negotiated via a `server/discover` probe (default mode stays `legacy`). New `elicitation-fallback.ts` auto-cancels `elicitation/create` when no handler is active — KAS won't hang on eliciting servers.
- **Agent-profile schema relaxation** ("[V3] null accepted"): `description`, `model`, `welcomeMessage` change `.optional()` → `.nullish()` in `JsonAgentFileSchema` (`prompt` already was).
- **`_kiro/diagnostics/changed`: still ZERO occurrences in 0.46.1** — tui.js's pre-subscription (2.18.1 watch item) still points at an emitter that does not exist. Watch stays open.
- **`node:sqlite` is NOT a sessions DB** — sole consumer is the new `spec/symbolic/store/sqlite-persistence.ts` (code-analysis store, below). Session titles use the JSON store + a new `FileStampCache` summary cache.

## 3. Model configOption: late delivery IS the cold-registry fix (KAS, live)

- `session/new` → `configOptions` ids `[mode, autopilot, contentCollection]` — **`model` absent** (registry not yet loaded), same presentation as the 2.17.0 "transiently absent" watch item.
- At first prompt, a `config_option_update` arrives with the **full rebuilt list including `model`** (observed `currentValue: "auto"`).
- **KAS-4 consequence** (Config options + modes UX — corrected from "KAS-5", which is the fs/terminal host-callback milestone): a cyril model picker for KAS must consume `config_option_update` (and tolerate `model` missing at session/new), not snapshot the `session/new` response. This amends KAS-4's earlier guidance that the initial `session/new` snapshot is the only way to learn starting config state — still true for `mode`/`autopilot`, but `model` can be cold-absent and arrive only via the later update. The [V3] "empty model list right after startup" fix manifests as this late push.

## 4. AI session titles (KAS) — field old, generation new, **LLM part ships DARK**

- `session/list` reply (live): `{sessions:[{sessionId, cwd, title, updatedAt, _meta.kiro{agentMode, createdAt, source, executionTarget{kind}, status}}]}`. The `title` field predates 2.19.0 (documented on `_kiro.dev/session/list` since the 2.4.1 coverage doc).
- **Generation mechanism** (new `src/session/session-title.ts` + `services/session-title-service.ts`): on FIRST prompt, `deriveSessionTitle` sets a deterministic placeholder (strips filler prefixes like "can you"/"help me", max 6 passes, 80-char cap), then `kickoffLlmSessionTitle` fire-and-forgets an LLM upgrade (model `simple-task`, 15 s abort, dropped if the user renamed meanwhile). The LLM half is **gated dark** — `FeatureKey.SESSION_TITLE_LLM` default false, server-ramped; env overrides `KIRO_FEATURE_SESSION_TITLE_LLM_ENABLED` / kill-switch `KIRO_DISABLE_SESSION_TITLE_LLM=true`. System prompt: "3 to 6 words. Never more than 8. … Title Case … Output ONLY the title".
- This exactly explains the live observation: both probe sessions kept the **verbatim first prompt** as title ("Reply with exactly: OK" has no filler prefix to strip; the LLM upgrade never ran).
- Wire: `setSessionTitle` rides the pre-existing focus-update pipeline — `session_info_update` `kind:focus_update` with `title` mirrored to the **top-level `title` field** of the update object (observed live), plus `_kiro/sessions/changed` roster pings. User-rename precedence (`titleSetByUser`) unchanged. Workflow step sessions get `«workflowName» · «nodeId» #«iteration»` titles.
- tui.js side: picker rows render `title (sessionId[0:8])`; new local `dashboard-meta.json` stores user title overrides with a `runIfFreshUntouched` guard; new `/sessions rename <name>`; dashboard inline rename calls `renameSessionById` only on the v3 engine. v2 keeps its sidecar title (`~/.kiro/sessions/cli/{uuid}.json`).

## 5. Items probed and cleared (no cyril impact)

- **Unknown slash commands** ("refused instead of sent to the model"): over ACP, `session/prompt` with `/definitelynotacommand hello` still reaches the **model** (live: model-authored polite refusal streamed as normal chunks, `end_turn`). The refusal is TUI-side input handling only. Cyril's own slash layer stays authoritative; unknown text keeps flowing as prompt.
- **v2 happy path**: byte-familiar `agent_message_chunk` → 2 × `_kiro.dev/metadata` (context %, then `turnDurationMs`) → response `{stopReason:"end_turn"}`. The v2 freeze holds on the happy path.
- **Auto-compaction on overflow**: cyril already parses `kiro.dev/compaction/status` (started/completed/failed) from the `/compact` era; automatic triggering reuses that modeled path. **One new status value: `unsummarized_dropped`** (0→4 hits; inserted between `completed` and `failed`) — the wire surface of "recover from a context overflow even when compaction itself overflows, by dropping the oldest history". Cyril's unknown-status arm warn-drops it today (small follow-up: cyril-0f4e). Supporting internals pre-exist (`ContextRecoveryAttempt {final_attempt}`, `CompactionRetry`); new field `from_history_reduction`.
- **`kiro-cli settings list`** shows only *set* keys; the new `api.*` keys have no defaults written and read back "No value associated" until set.

## 6. Settings keys (2.19.0)

| Key | Engine | Unit | Default | Live-verified |
|---|---|---|---|---|
| `api.streamIdleSoftTimeout` | **v2 only** | seconds | 60 | ✅ notice at exactly the set value |
| `api.streamIdleHardTimeout` | **v2 only** | seconds | 300 | ✅ cancel+retry; error message echoes `2000ms` for value 2 |
| `api.timeout` | **v2 only** (wall-clock override) | — | 3600 s streaming | not probed |
| `api.subagentTimeout` | **v2 only** (crew idle deadline) | — | 3600 s | not probed live |

Settings descriptions from the binary: "Stream idle soft timeout in seconds; warn after this much stream silence (number)" / "Stream idle hard timeout in seconds; abandon the stream after this much silence (number)". Watchdog log strings: "response stream has stalled past the soft idle threshold" / "response stream exceeded the hard idle threshold, abandoning it". Retry-exhaustion terminal error text: "The response stream repeatedly stalled (s without data) and retries were exhausted. Try again, or split the work into smaller steps." New telemetry: `stream_stall_count` / `stream_stall_retries` on turn completion.

**v2 crew stall handling** (`api.subagentTimeout` mechanics, from binary strings): "crew stage stalled: cancelling it; sibling stages keep running", "crew group stalled: no live stages and no completion within the deadline window", "Pipeline finished with N stage(s) cancelled by the idle deadline", "subagent stall window is smaller than the probe round-trip budget; detection timing will be approximate" — per-stage idle cancellation with sibling survival. KAS has no equivalent.

`api.subagentTimeout` details (static): registry description "Per-subagent idle deadline in seconds; **resets on that child's progress; 0 disables**"; ms env override `KIRO_SUBAGENT_STALL_TIMEOUT_MS`. SessionManager grew activity tracking (`NotifySessionActivity`, `NotifySessionHumanWait {waiting}`, `GroupLastActivity`, `HumanWaitClearGuard`) — progress resets the deadline and **pending human approval pauses it**. Cancellation surfaces through the **existing** `kiro.dev/subagent/list_update` status vocabulary (failed/terminated — no new statuses) plus a model-visible message: "This subagent was cancelled: it made no observable progress for a full idle window (…, setting api.subagentTimeout / env KIRO_SUBAGENT_STALL_TIMEOUT_MS). Sibling subagents were not affected. Work this subagent completed before cancellation … may have partially applied…". **No cyril tracker change needed** — existing status handling covers it.

`api.timeout` pre-existed as a registered setting ("API request timeout in seconds"); 2.19.0 only changed the streaming default to 3600 s.

New tui.js settings keys: `chat.enableWorkflows`, `chat.keybindings.toggleSessionDashboard`, `chat.sessionDashboard.groupBy`, `chat.specReview.mouseEnabled`.

## 7. Multiplex / serve-mode drift (static)

`multiplex-stream.d.ts` gains `subscribeClientToSession(sessionId, clientId)` — fixes a broadcast-drop race where the creator of a `session/new` wasn't subscribed during the interval between the id existing agent-side and the response emitting; events settling in that window (MCP server reaching `connected`, a power's server joining) were dropped with no catch-up. Doc comment references an internal `docs/client-scoped-messaging.md`. The `kiro-cli serve` ws-mux (2.18.0, undocumented) is being actively hardened.

## 8. tui.js delta (2.18.1 → 2.19.0, 12.95 MB → 13.11 MB)

- **Slash-command queueing** ("run automatically: right away if read-only, otherwise at turn end") is **effect-based, not name-based**. Immediate set = commands whose effect is in the panel/read-only effect set (`showContextPanel`, `showHelpPanel`, `showUsagePanel`, `showMcpPanel`, `showToolsPanel`, `showHooksPanel`, `showKnowledgePanel`, `showFeedbackUrl`, `showThemeMenu`, `showGoalPanel`, `showChangelogPanel`, `showSessionId`, `showStatsPanel`, `verbosityConfig`, `updateTitle`) → `/feedback /help /context /usage /mcp /tools /stats /hooks /knowledge /changelog /session-id /theme /title /verbosity`, plus bare `/goal` (with args = queued) and `/stats <u32>`. Arg-sensitive four: only `context show`, `mcp list`, `knowledge show` stay immediate. Everything mutating (`/model /effort /agent /clear /compact /rewind …`) queues into `queuedMessages` (editable via Ctrl+X tray) and runs via `processQueue()` at turn end. If cyril mirrors mid-turn command semantics, this effect-set is the contract.
- **Unknown slash refusal is TUI-side only** — live-confirmed over ACP that `/nonexistent` still reaches the model as chat (§ 5).
- **New KAS-gated session dashboard**: `/sessions` opens a full dashboard (rename/bulk-delete/stale-cleanup/grouping) on the v3 engine only ("Session dashboard is available on the V3 (KAS) engine only"), backed by `_kiro/session/rename` + `_kiro/session/delete` — both RPCs **already existed dormant in KAS 0.38.7** (in the stable 110-method vocabulary); tui.js only now calls them. Both accept `sessionSource: "remote"|"local"` (cloud-session aware).
- Retry banner: store `retryStatus` renders `retry_warning`/`stall_notice` messages under the spinner; cleared by any other stream event.
- `_kiro/*` string set 51 → 53: only additions are `_kiro/session/delete`, `_kiro/session/rename`. `kiro.dev/*` set identical (36).
- `/config` gains `kasOnly` flag; command→handler map otherwise byte-identical.

## 9. Embedded doc-manifest delta — ZERO drift

2.19.0 manifests: 103 + 138 docs (union 162), counts identical to 2.18.1; `generated_at` 2026-08-19. **Zero docs added/removed/retitled.** Single field-level change in the whole corpus: `features/session-management.md` revalidated (2026-04-24 → 2026-08-12), keywords +`"slash commands"`, +`"mid-turn"`. No dark features this cycle (contrast 2.18.1's +13 incl. the dark v2 `session` tool).

## 10. Unexpected finds

- **A symbolic program-analysis subsystem ("c2s" = code-to-spec) is growing inside KAS**: sqlite-persisted store (functions/call-edges/invariants/contracts/refinement-types/taint/alias/escape/bi-abduction tables, schema v2), a new model-visible `c2s_query` tool joining 9 pre-existing `c2s_*` tools, PBT executor, repair loop, semantic agent — plus `z3-solver` in node_modules. Spec-mode formal-analysis ambitions; explains most of the bundle growth alongside the MCP 2.0 vendoring.
- **New server-ramped A/B experiment gates**: `session_title_llm`, `fta_vibe` (post-implementation functional-task-alignment validator in vibe mode), `user_agent_refactoring_enabled` (derive backend UA from ACP `clientInfo` — telemetry-relevant for cyril's clientInfo).
- Usage-limit errors compute `actions` (`viewUsage`/`enableOverages`/upgrade) riding the −32000 error data (continuation of 2.17.0 overage vocab).
- ws-mux/serve mode: only the `subscribeClientToSession` race fix (§ 7); no new flags, no listener auth — no promotion sign this release.
- The token store key for IdC accounts is `kirocli:odic:token` — "odic" [sic] for OIDC, in `auth_kv` of `data.sqlite3`.

## 11. Rust v2 binary delta (nm + strings, kiro-cli-chat)

- **Wire-surface strings**: `kiro.dev/*` / `_kiro.dev/*` — **no additions, no removals**. New session-update kinds exist only in the Kiro dialect union (§ 1). No new stopReason values. Dialect schema field addition: `AgentExecutionUserMessageQueued` gained `content` (queued steering echoes now carry their text — cyril already reads `content` for this family since cyril-vgcm, so this fills a field we already parse).
- **New modules**: `agent::agent::error_recovery` (transient retry: `is_transient_network_error`, `parse_retry_after`, `transient_backoff`; classes throttling/network/timeout/server_error/other), agent-loop idle machinery (§ 1), subagent deadline machinery (§ 6), `chat_cli_v2::auth::refresh_coordinator` — **mid-turn auth refresh** with a cross-process `.refresh.lock`, `Model::refresh_auth` inside the agent loop, and `resolve_kas_token_for_callback_with_refresh` (the KAS `_kiro/auth/getAccessToken` bridge now force-refreshes — pattern to mirror in cyril's KAS-1 auth handler).
- **Removed**: v1 TUI command modules (`changelog`, `clear`, `experiment`, `lite`, `paste`, `reply`), `record_automatic_retries` telemetry (→ `record_transient_retry`).
- **Telemetry expansion (cyril-relevant)**: new metrics `kiro_cli_stream_stall_total/idle_seconds/retry_total/recovery_seconds`, `kiro_cli_subagent_deadline_expired_total`, `kiro_cli_transient_retry_total`, `kiro_cli_turn_failure_total` (`turn_failure_reason` ∈ context_limit/model_error/tool_error/execution_limit/internal_error), crash receipts, process-health gauges — and attributes **`acp_client` / `acp_client_name`** with `session_interface` ∈ {interactive_cli, noninteractive_cli, **external_acp**}. **Cyril sessions are now a tracked dimension in AWS dashboards** (unless telemetry is disabled — see the acp-telemetry reference).
- **New env vars**: `KIRO_SUBAGENT_STALL_TIMEOUT_MS`, `KIRO_SESSION_LOAD_TIMEOUT_MS`, `KIRO_AGENT_CONFIG_DIR`, `KIRO_DISABLE_SESSION_SEARCH_INDEX`, `KIRO_TEST_SESSIONS_ROOT`, `KIRO_PERF_TESTS`, `KIRO_VOICE_SUPPORTED`, `KIRO_VOICE_SERVER_URL` (voice moving to a server/remote-helper model). KAS-side: `KIRO_FEATURE_SESSION_TITLE_LLM_ENABLED`, `KIRO_DISABLE_SESSION_TITLE_LLM`, `KIRO_WORKFLOW_THROTTLE_RETRY_DELAYS_SEC`, `KIRO_WORKFLOW_TRANSIENT_RETRY_DELAYS_SEC`.
- **Unavailable-tool fast-fail** (changelog): internal only — new `tool_index::filter_specs_by_allowed_tools` + pre-existing `InvalidToolUse`/`ToolValidationError` events; no new wire strings.
- Session persistence churn: `collect_all_sessions_all_cwds`, `delete_kas_session`, `list_sessions_impl`, session search index toggle — the `/sessions` surface growing into cross-cwd management.

## 12. Follow-ups filed this audit

| Issue | What | Why |
|---|---|---|
| cyril-svi2 | Render `stream_stall_notice` + `retry_warning` | Engine-authoritative stall/retry signal; integrate with TurnStalled chip |
| cyril-9weh | Handle `stream_discarded` → roll back uncommitted streaming text | Silent duplicated-prefix hazard on v2 mid-stream retry |
| cyril-f7tm | WorkflowTracker: discriminate `node_paused kind:"retry-wait"` | zd8u renderer keys immediacy off node_paused; two meanings now |
| cyril-0f4e | Parse compaction `unsummarized_dropped` | History-loss recovery currently invisible |
| cyril-w0vy (note) | 2.19.0 watchdog does NOT cover the security-filter wedge | Watchdog guards an open stream; w0vy's stream ends normally |

## 13. Request-layer audit (`KIRO_DUMP_REQUESTS`, KAS only — added on follow-up)

The first pass audited only the ACP wire; this leg dumps the agent→backend model requests (`conversationState`) — same-day A/B: host 2.19.0 with KAS 0.46.1 vs 0.38.7 pinned via `KIRO_KAS_SERVER_PATH`, same prompt.

- **Eight versions of KAS churn ≈ near-frozen model-facing request contract.** conversationState keys, `userInputMessage` fields, the `<EnvironmentContext>` content wrapper, and the system-prompt delivery (rides `history[0].userInputMessage`, primed by `history[1].assistantResponseMessage: "I will follow these instructions."`) are all identical. Only two deltas:
  - **`read_files` REMOVED from the model's tool list (17 → 16)** — corrects the static leg's inference that the tool "survives" (its registration strings do; its model exposure does not). The system prompt's context-gatherer paragraph drops the `read_files` mention accordingly. No cyril impact (tool roster is agent-internal), but it is a real capability removal.
  - System prompt +286 chars: documents steering `inclusion: auto` front-matter (name/description auto-match) — pairs with the steering fixes.
- **The dark title-LLM feature verified end-to-end**: with `KIRO_FEATURE_SESSION_TITLE_LLM_ENABLED=true` (env passthrough through the host spawn works), the session's first prompt triggers a **second, separate model request** — fired ~60 ms *before* the main turn call — whose shape is: titling instructions as `history[0]` user message ("3 to 6 words. Never more than 8…"), **prefilled assistant turn `"Title:"`** (response priming), current message literally `"Continue"`, no tools, no modelId in the dump. It returned 200 and the `session/list` title became the LLM output (`'OK'`) instead of the verbatim prompt. When AWS ramps this experiment, every session gains +1 model call on first prompt (credit impact unmeasured — the dump carries no metering).
- **Dumper corrections to the 2.14.2-era notes**: the dump envelope is `{invocation {agentName, executionId, chatId}, request.conversationState, response {conversationId, metadata {httpStatusCode, requestId, attempts, totalRetryDelay}, headers}}` in BOTH 0.38.7 and 0.46.1 — response/retry accounting is included, and the system prompt IS visible (via history[0]). Still absent: `additionalModelRequestFields`, `profileArn`.
- Remaining request-layer gap: **v2 has no dumper** — v2 request auditing (e.g. whether a watchdog stall-retry resends an identical payload) needs TLS interception or a fresh AWS prompt-log pull; the July prompt-log corpus predates 2.19.0.
- Artifacts: `experiments/conductor-spike/reqdump-kas-{0.46.1-main,0.38.7pin-main,0.46.1-titlecall}-2.19.0.json` (no auth material; request dumps carry no tokens).

## 14. Live workflow run (`_kiro/workflow/*` re-verification — added on follow-up)

A two-step recipe (bundled `wf-coder`, exact-output prompts, `{{s1.output}}` chaining) driven gate-off on 0.46.1, plus a same-day 0.38.7-pinned A/B (`KIRO_KAS_SERVER_PATH`). Probe: `experiments/conductor-spike/probe-kas-workflow-live-2.19.0.py`.

**Event contract: unchanged.** Gate-off `_kiro/workflow/new` + `invoke` still work (`_meta.workflowsEnabled: false` echoed); ordering matches the W1 model exactly — `run_start` → `node_start` ×2 per step (scheduled, then with `sessionId`) → `node_complete` → … → `run_complete`; payload keys per kind are identical to the documented nine-kind table, **no new fields on the happy path** (an organic `node_paused` carries `reason` only — no `kind`, consistent with `retry-wait` being the sole kind-bearing case). `run_complete status:"paused"` non-terminal trap still true. Step-session titles live-confirmed as `«workflow» · «nodeId»` (`audit-min · s1`) via `session/list`; step transcripts persist the `…-turn-start`/`…-turn-end` messageId records.

**Finding: `{{step.output}}` capture is BROKEN in practice — and not newly.** s1 answered `ALPHA` as text then signaled `send_message(severity:success)` (the pattern the engine's own appended protocol produces); `node_complete.capturedOutput` and `finalState.capturedOutputs` came back `""`. Same on the 0.38.7 pin, and every committed 2.16.0/2.16.2 capture in this repo also shows `capturedOutput:""` — every observation we hold is empty. Root cause visible in the 0.46.1 bundle: `extractCapturedOutput` matches `role:"bot"` messages with `entries[].type:"text"`; the in-turn view yields nothing for a speak-then-signal turn, and the **new 0.46.1 transcript fallback** (whose doc comment describes this exact bug and promises to keep `{{id.output}}` non-empty) reads persisted records shaped `{payload:{type:"assistant", content}}` — no `role`, no `entries` — so it can never match either; live it returned `""` with no `captured_output_fallback_failed` warning. Downstream consequence observed live: s2's prompt interpolated the hole, the model asked for clarification (`send_message need_input`) and the run parked (`node_paused` → `paused` → `run_complete paused`) — while the 0.38.7-pin s2 happened to comply and the run completed (model roulette, not a binary delta).

Consequences: (a) workflow authoring must not build data flow on `{{id.output}}`/`{{previous.output}}` — the `artifacts` path registry with input-derived paths is the reliable channel (the kiro-workflow-authoring skill's templating section and setup-step idiom updated accordingly); (b) cyril-zd8u should not spend UI on `capturedOutput` — it is empty in every observed run; (c) `send_message` severity → completion mapping re-verified (`success` → completed, `need_input` → node pause).

## 15. Artifacts

- Probes: `experiments/conductor-spike/probe-v2-watchdog-2.19.0.py` (parameterized engine/soft/hard/prompt — reusable for future watchdog checks), `probe-kas-surface-2.19.0.py`, `probe-kas-workflow-live-2.19.0.py` (two-step gate-off workflow run; reusable capture re-verifier).
- Workflow captures (token-scrubbed): `kas-workflow-live-{0.46.1,0.38.7pin}-2.19.0.jsonl`.
- Captures (token-scrubbed): `experiments/conductor-spike/{v2-watchdog-control,v2-watchdog-soft,v2-watchdog-hard,kas-watchdog,kas-surface,v2-slash}-2.19.0.jsonl`. `v2-watchdog-soft` shows the notice at exactly the configured soft seconds during a completing turn; `v2-watchdog-hard` is the full 3-attempt exhaustion; `kas-watchdog` is the KAS negative (3.78 s gap, no notice).
- Doc-manifest baselines: `docs/kiro-docs-index-2.19.0-{103,138,merged}.json` (identical counts to 2.18.1; see § 9).
- Archive: binaries + BUILD-INFO + SHA256SUMS at `~/.local/share/kiro-research/binaries/2.19.0/`; `tui-bundles/kiro-tui-2.19.0.js` (+`.sha256`).
