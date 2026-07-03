# Kiro CLI 2.11.0 — wire audit (diff vs 2.10.0)

**Analyzed:** 2026-07-02 · **Release:** 2.11.0, BUILD_HASH `c231a655bcc626f024dd1f67b49687e83c9e73f1`, BUILD_DATE `2026-07-02T01:30:40Z` · **`kiro-cli-chat` sha256** `654f63784f5fe1fc51f0b9878871e9e4e5b31078fc6feb9d14f596e0a6623deb` (archived `~/.local/share/kiro-research/binaries/2.11.0/`).
**Method:** installed 2.11.0 binary vs archived 2.10.0 — same-day binary-isolated v2 surface A/B (`probe-v2-surface-ab-2.11.0.py`, field-path set diff over init/session-new/all `_kiro.dev/*`); `nm`+`rustfilt` module-path diff; binary-wide ACP-method-string diff; embedded-KAS sha-gate + `@kiro/agent` version diff + direct-spawn A/B on both extracted bundles; doc-manifest delta; tui.js carve. Live method probes on the extracted 0.8.0 bundle with 0.3.299 controls. Single environment (this user's non-enterprise token).

**Verdict for cyril: SAFE — no code change required, safe to upgrade 2.10.0→2.11.0.** The v2 path cyril drives is wire-frozen (24 cmds / 15 tools, zero new ACP methods, field-path-identical). KAS/V3's `@kiro/agent` jumped `0.3.299 → 0.8.0`, but that is a **version renumber, not a rewrite** (+0.7% bundle bytes; 0.4–0.7 skipped) whose only new wire surface is **four additive methods**, all of which cyril already tolerates by dropping them to `Ok(None)`.

---

## Changelog (announced, 2026-07-01)

```
Version 2.11.0 (2026-07-01)
  - Added: /mcp auth, /mcp cancel-auth, and /mcp logout commands to force OAuth
           re-authentication, abort pending auth flows, and remove stored
           credentials for remote MCP servers
  - Added: Keyboard shortcuts in the MCP panel status view: ^A force auth,
           ^X abort auth, ^R remove stored credentials
  - Changed: /usage now shows prepaid "Additional credits" packs in place of the
           legacy post-paid overages section, and hides the progress bar for
           users without a usage cap
  - Fixed: kiro_planner re-entry getting stuck on a nonexistent dummy tool after
           a plan-to-execute handoff
  - Fixed: Improve subagent summary tool compliance via positional priority and
           stronger prompt instructions
  - Fixed: [V3] Agent now correctly reports the active model when asked
  - Fixed: Agent configs with unknown fields (e.g. permissions) no longer fail to
           load in --legacy-ui mode
  - Fixed: Subagent result silently dropped when the summary tool was cancelled
           before executing during an error teardown
  - Fixed: MCP stdio servers no longer spawn visible console windows on Windows
```

Every item maps to a v2-Rust-internal change, a tui.js/frontend concern, or a prompt/graph-level fix inside the KAS bundle. None adds an ACP method, notification, command, or tool on cyril's v2 path. (Hidden `version --changelog=2.11.0` == the public list.)

## v2 (default `kiro-cli acp`) — wire FROZEN

- **Exercised surface field-path-IDENTICAL to 2.10.0** (same-day A/B, both binaries hitting today's backend). Diffed field-path sets of `initialize` resp, `session/new` resp, and every `_kiro.dev/*` notification (`commands/available`, `metadata`, `subagent/list_update`, `mcp/server_initialized`) — all identical.
  - **24 slash commands**: agent chat clear code compact context effort feedback goal guide help hooks knowledge mcp model paste plan prompts quit reply rewind stats tools usage
  - **15 tools**: code glob goal grep introspect knowledge read shell subagent todo_list tool_search use_aws web_fetch web_search write
- **`nm`+`rustfilt` module-path diff** maps 1:1 onto the changelog, no new wire:
  - `chat_cli::mcp_client::oauth_util::start_authorization` **+** → `/mcp auth|cancel-auth|logout` (TUI-level MCP OAuth plumbing, **not** an ACP method)
  - `chat_cli::cli::chat::cli::usage::usage_renderer::{format_billing_rate,format_cost_with_currency}` **−** → `/usage` prepaid-credits rework
  - `chat_cli::cli::agent::wrapper_types::{alias_schema,tool_settings_schema}` **−** → agent-config unknown-fields fix (`--legacy-ui`)
  - `chat_cli::cli::chat::cli::persist::build_session_entries` **+**, `orchestration::types::SessionStatus` gains serde `Serialize` (off-wire persistence)
  - Telemetry (off-wire, AWS metrics): net-new `kiro_cli_user_logged_in_total` (attr `credential_kind`) + `kiro_cli_ui_mode_{session_started,changed,default_changed}_total` (lite-mode rollout instrumentation — suggests a default-flip is being measured); `bedrock_stream_inter_token_latency`. Removed cost-estimation metrics (`estimated_cost_usd`, `token_economics_records`) pair with the prepaid-credits change.
- **CORRECTION — `chat_cli::util::system_info::which_mwinit` is NOT new.** It appeared in the `nm` diff, but the cached static `is_mwinit_available::MWINIT_AVAILABLE` exists in 2.10.0 too; 2.11.0 merely outlined the helper into a visible symbol. It runs `which mwinit` (Amazon-internal Midway detection) feeding the signin `&from_amazon_internal=true` param and the `is_internal_amazon` telemetry gauge attr — both pre-existing. *(Lesson: an `nm`-visible symbol ≠ new functionality; check the prior binary's statics/strings.)* CLI flag surface identical.

## KAS / V3 — `@kiro/agent 0.3.299 → 0.8.0` = version RENUMBER + 4 additive methods

**The bundle un-froze** (2.10.0 was byte-frozen), but the jump is a renumber, not a rewrite.

- Bundle sha gate `afa13285…` (2.9.0/2.10.0) → `05e941ed…` (2.11.0). Bundle bytes 19,426,245 → 19,564,098 (**+0.7%**). `package.json` dep blocks byte-identical except version strings. Self-version `0.8.0`; **no 0.4/0.5/0.6/0.7 KAS strings → 0.4–0.7 were skipped** (renumber, decoupled from the tiny code delta).
- **Audit-lesson correction to the first-pass read.** The "dep overhaul / covenant gone / dist collapsed" framing was WRONG — 0.3.299 already had the single-bundle `dist/server/acp-server.js`, the `@agentclientprotocol/sdk` dep, Cedar/z3, and only `@kiro/agent` under `node_modules/@kiro/`. The apparent difference was comparing against the older **flat** extraction (`kas/node_modules/`, 0.3.234-era) vs the newer **per-version** dirs (`kas/<cliver>-<sha>/`). Always diff versioned-dir vs versioned-dir. Covenant (`@kiro/acp-type-covenant`) is still bundled-in and authoritative; the MiniLM embedding engine (renamed `EmbeddingEngine`→`TransformersModelLoader`) pre-existed.
- **Method surface: `_kiro/*` 73 → 76, ZERO removed; `_session/*` +1.** Live A/B (`probe-kas-commands-tools-2.9.0.py` direct-spawn, both bundles same-day) is otherwise byte-identical — command set, tool set, `orchestrate_subagent` schema, launch contract (`node --experimental-wasm-modules acp-server.js --transport=stdio`) unchanged. KAS-3 subagent model FROZEN (`agentSubtaskId`/`agent-subtask`/no-`list_update` stable). Auth providers byte-identical (profileArn gotcha still applies).

### The 4 new methods — live-probed (0.3.299 controls all fail `PersistenceClassification` → genuinely new)

| Method | Dir | Contract (probed) |
|---|---|---|
| `_session/steer/clear` | client→agent req | `{sessionId}` → `{cleared:true, messageIds:[…]}`; broadcasts `session_info_update` kind `steering_cleared {messageIds}`; empty queue = no-op `[]`; missing/unknown sessionId → -32603. Bumps steering epoch (drops in-flight `_session/steer` append), persists a reload-safe boundary marker. **Does NOT abort the turn** (unlike cancel). |
| `_kiro/sandbox/applyConfig` | client→agent req | `{configId,value}` both required strings (else -32602), engine-global (no sessionId) → `{}`. configIds `sandbox`/`sandboxNetworkMode`/`mcpSandboxing`. **Unknown configId/value = silent warn + `{}` no-op.** |
| `_kiro/knowledge/indexingStarted` | agent→client notif | `{sessionId, name, fileCount}` (fire-and-forget; no client cap needed). |
| `_kiro/knowledge/indexingCompleted` | agent→client notif | `{sessionId, name, status, itemCount?}` (`itemCount` only on `status:"success"`). |

Live steer→clear lifecycle confirmed: `_session/steer {sessionId, message}` → `{queued:true, messageId:"steer-<uuid>"}` + `steering_queued`; then clear returned that exact id + `steering_cleared`. (End-to-end "model omits the cleared instruction" NOT verified — turn 403'd on a stale SSO token; wire lifecycle is conclusive.)

### Knowledge indexing — same engine, new declarative entry point

- **Not a new knowledge subsystem.** The KAS `/knowledge` command (`_kiro/knowledge` subcommands show/add/search/remove/…), the store/registry, BM25 (`fast`) + semantic (`best`) index, and MiniLM embedding all pre-exist in 0.3.299. New in 0.8.0 (all `old=0`): `reconcileAgentKnowledge`/`syncAgentKnowledgeBases` (agent-config-declared bases), `KnowledgeBaseResourceSchema`/`resources.knowledgeBases[]`, the `indexing{Started,Completed}` notifications, and `storeForSession` (per-agent-identity store). Manual `/knowledge add` reports progress inline + polled via `show`; the push notifications are exclusive to the agent-declared auto-sync path.
- **Trigger** (live): bind a custom agent declaring `resources.knowledgeBases[]` via `set_config_option {configId:"mode", value:"<agentId>"}` (custom file agents appear as mode values) → fires in `syncActiveAgentKnowledge` at session setup / mode switch.
- **Retrieval is pull-based/tool-gated** — the agent reads a KB only by calling `knowledge search`; KB content is not auto-injected. The tool's "when to use" description + the KB's name/description drive discovery. Must-have context belongs in steering, not a KB.
- **`fast`=BM25 local, instant. `best`=MiniLM (`Xenova/all-MiniLM-L6-v2`) via `@huggingface/transformers`, `allowRemoteModels=true`, empty cache → first use DOWNLOADS ~90MB from the HF hub to `~/.kiro/models/` (`$KIRO_KNOWLEDGE_MODEL_CACHE_DIR` override); not bundled.** LIVE-verified: fresh full download → `indexingCompleted status=success itemCount=3`; a download killed mid-fetch leaves a truncated `model.onnx` (79MB vs 90MB) that the next run treats as cached and **fast-fails** → `status=failed` (108ms `loadEmbeddingModel.error`); download in flight → `indexingStarted` with **no Completed**. ⇒ **three real completion states** (success+itemCount / failed / never-Completed) — plus a silent ~90MB HF network dependency that fails offline/air-gapped.

## Doc-manifest delta (82 → 83; +118 manifest unchanged)

- **ADDED** (unannounced in changelog): `settings/disable-inheriting-default-resources.md` — doc entry for `chat.disableInheritingDefaultResources`. (The *setting* shipped in 2.10.0; its doc-manifest entry trails one release.)
- Re-validated: `slash-commands/effort.md` (→2026-07-01), `tools/summary.md` (→2026-06-25, matches the subagent-summary fixes).

## tui.js

- Changed: 12.32MB → 12.61MB, carved sha `35301f81…` → `16a97c21…`. `/mcp cancel-auth` etc. confirmed TUI-side. Embedded `version:"2.4.0"` string is a decoupled internal package version — identify by carved sha only. Carved+archived for 2.10.0 and 2.11.0.

## Cyril impact — NO wire addition required

- **Upgrade-safe as-is.** cyril's KAS engine drops unknown `_kiro/*` frames to `Ok(None)` (test `kas_engine_drops_unknown_ext_frame`, KAS-2a/cyril-j16p); `session_info_to_notification` sends non-`turn_end`/`context_usage` kinds to `None`. So all four new methods (and the `steering_queued`/`cleared` kinds) drop gracefully — no crash, no capability-handshake change. The knowledge *tool* renders as an ordinary `tool_call`.
- **Optional, feature-driven additions only:** knowledge-indexing progress display (receive-side; rivets **cyril-45ld**, design sketch at `experiments/knowledge-panel-render/`); `_session/steer/clear` as part of the K1/KAS-steering track (rivets **cyril-vgcm**); `_kiro/sandbox/applyConfig` send (elective, likely out of scope).

## Repro pointers

- Binaries: `~/.local/share/kiro-research/binaries/2.11.0/` (+ BUILD-INFO, checksums). KAS extractions: `~/.local/share/kiro-cli/kas/2.{10,11}.0-<sha>/`.
- Probes (all `experiments/conductor-spike/`): `probe-v2-surface-ab-2.11.0.py`, `probe-kas-steer-clear-sandbox-2.11.0.py`, `probe-kas-steer-clear-behavior-2.11.0.py`, `probe-kas-knowledge-indexing-2.11.0.py`; logs under `logs/*-20260702*`. Doc-manifest: `docs/kiro-docs-index-2.11.0-*.json`.
