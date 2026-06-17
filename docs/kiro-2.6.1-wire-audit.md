# Kiro CLI 2.6.1 — ACP Wire Audit

**Analyzed:** 2026-06-09/10 · **Method:** same-day binary isolation (cyril path `kiro-cli acp`, no conductor) vs archived 2.5.1 with the backend held constant, plus `nm`+rustfilt symbol diff, method-string diff, and self-validated `tui.js` carve. Covers the two missed releases **2.6.0** and **2.6.1**.

**Verdict for cyril:** **SAFE TO UPGRADE. No cyril code changes required.** Every 2.5.1→2.6.1 change is either dormant on cyril's default path (KAS auth, still not embedded) or a v2-TUI-only feature that never crosses ACP (voice, surveys, LSP code-intelligence, MCP registry fetch). The live cyril-path capture shows **zero exercised wire deltas** vs 2.5.1.

---

## Attribution: everything landed in 2.6.0

2.6.1 is a thin patch on 2.6.0. Same-module symbol counts (2.5.1 → 2.6.0 → 2.6.1):

| Module / method | 2.5.1 | 2.6.0 | 2.6.1 |
|---|---|---|---|
| `chat_cli_v2::auth::kas_token` family | 1 | 6 | 6 |
| `kas_token::handle_kas_auth_ext_method` | 0 | 1 | 1 |
| `agent::agent::permissions::evaluate_url_permission` | 0 | 1 | 1 |
| `acp::session_manager::deep_merge` | 0 | 1 | 1 |
| `agent::agent::mcp::registry` / `refresh_mcp_registry` | 0 | 3 | 3 |
| `acp::commands::guide` (symbol) | 1 | 0 | 0 |

2.6.1 adds only `acp::commands::rewind::format_tokens` over 2.6.0; the rest of the 2.6.0→2.6.1 module "delta" is monomorphization churn. **So all real changes are 2.6.0 changes**, and the 2.5.1→2.6.1 isolation captures them cumulatively.

---

## 1. New binary capabilities — all dormant or TUI-only

### KAS auth ext method `_kiro/auth/getAccessToken` (NEW, dormant)
New module `chat_cli_v2::auth::kas_token` with `handle_kas_auth_ext_method`, `resolve_kas_token_for_callback`, `profile_arn_from_db`, surfacing the wire literal **`_kiro/auth/getAccessToken`** (note the `_kiro/` namespace, not `_kiro.dev/` — matches the KAS dialect). This is auth plumbing for the KAS TypeScript agent engine.

**Dormant for cyril:** `kiro-cli acp --agent-engine kas` still errors **`KAS assets not embedded`**. cyril runs the default v2 engine, which never invokes this handler. When KAS eventually ships embedded, cyril will need to *respond* to this server→client request (it carries an auth token callback) — tracked as future work, not a 2.6.1 gap.

### URL permissions `evaluate_url_permission` + `anchor_regex` (NEW, no wire-shape change observed)
New shell/URL permission evaluation (`agent::agent::permissions::evaluate_url_permission`, `permissions::anchor_regex`, `shell_permission::decider::build_rules`). This governs whether URL-bearing tool calls need approval. cyril renders `session/request_permission` generically (Yes/Always/No), so any new URL-permissioned tool is handled with no code change. No new *required* field observed on the permission request between 2.5.1 and 2.6.1.

### MCP registry (NEW, TUI-side)
`agent::agent::mcp::registry`, `chat_cli::mcp_registry::to_v2_registry_response`, `handle_refresh_mcp_registry`. The v2 TUI now fetches an MCP server registry over HTTP ("Fetching MCP registry from:", "Fetched and validated N servers"). This is a TUI feature; it does not add an ACP method cyril consumes.

### v2-TUI-only features (out of scope for cyril)
The carved `tui.js` package.json + store reveal an **Aperture feedback survey** system (`/feedback`, session/plan/implement surveys), MCP-registry + OAuth panels, and a TUI bump to **`@agentclientprotocol/sdk` 0.19.0** / **`@kiro/agent` 0.3.45**. cyril does not run `tui.js`; none of these cross the ACP wire.

### Voice input — wired in the TUI but inert in this build
`tui.js` has voice-input plumbing (hold-Space PTT + a special-cased `c==="voice"` dispatch; store fields `voiceStop/Cancel/Level/AutoSubmit/PartialText`, model-download UX). It is **not** flag- or setting-gated — but it spawns an external helper `kiro-cli voice [--ptt]` (`q9e()` → `KIRO_CLI_PATH ?? "kiro-cli"`), and the shipped 2.6.1 binary has **no `voice` subcommand** (`kiro-cli voice` → `unrecognized subcommand`; only `voice-serve`/`voice-cloud-setup` exist, and those route oddly through the kiro-cli↔kiro-cli-chat router). `/voice` is also absent from the slash-command registry. So voice is **partially-shipped/in-progress**, not a usable feature in this build — and v2-TUI-only regardless (zero ACP/cyril relevance). Env knobs: `KIRO_CLI_PATH`, `KIRO_VOICE_SERVER_URL`, `KIRO_PTT_HOLD_MS`.

### LSP code-intelligence — pre-existing, NOT reimplemented
The `/code` command and the `code_agent_sdk` LSP subsystem are **not new** — `/code` is in the byte-identical 23-command set, and the symbol bulk is flat across versions (`code_agent_sdk` 2103→2112, `code_intelligence` 38→37, `lsp`-anything 2025→1986, `acp::commands::code` 16→16; deltas are monomorphization noise). The **only** 2.6.0 addition is two additive helpers — `CodeIntelligence::lsp_init_warnings` (a `tracing`-instrumented accessor) and `lsp_warnings_for_workspace` — that surface LSP *init warnings* (e.g. "LSP failed to start for workspace X"). No new ACP wire literal (`lspInitWarning`/`initWarnings` absent from the binary), no distinct `tui.js` token; folded into the existing code panel / `initErrors`. Zero cyril impact.

---

## 2. Live cyril-path capture — zero exercised deltas

`cargo run --example test_bridge -- --agent-command kiro-cli acp` against installed 2.6.1, diffed against an identical same-day 2.5.1 run:

| Surface | 2.5.1 | 2.6.1 |
|---|---|---|
| `commands/available` | 23 cmds | **identical 23 cmds** |
| `session/new` | 5 modes, 14 models, opus-4.8 | identical |
| `MetadataUpdated` shape | ctx / metering / tokens / effort | identical |
| metering post-turn | `Some(0.0196)` | `Some(0.0112)` (value varies, field present) |
| `/stats` `input_tokens`/`output_tokens` | `null` | `null` — unconditional (see below) |
| `/effort` under haiku | 0 options | 0 options |
| "missing contextUsagePercentage" warns | 4 | 4 (pre-existing) |

The 23-command set (`agent chat clear code compact context effort feedback guide help hooks knowledge mcp model paste plan prompts quit reply rewind stats tools usage`) is **byte-identical** across versions.

### `/stats` token counts still `null` — confirmed unconditional
`input_tokens`/`output_tokens` were probed under three turn types on 2.6.1, all `null`:

| turn | `had_tool_use` | `input_tokens` | `output_tokens` |
|---|---|---|---|
| haiku, no tool (sweep) | false | `null` | `null` |
| Opus 4.8 + effort=high, reasoning | false | `null` | `null` |
| Opus 4.8, forced shell tool (both request rows) | true / false | `null` | `null` |

So it is **not** model- or turn-type-conditional (unlike `/effort`, which is model-conditional). The field is emitted as `null` (= "not provided"), not `0` — a pending **backend** rollout, unaffected by the binary. `duration_ms`/`ttfc_ms`/`request_id`/`had_tool_use` populate correctly; only the token fields are dark. Reproduce: `probe-stats-tokens-2.6.1.py`.

### `/guide` is NOT removed (static hypothesis overturned)
The `agent::acp::commands::guide::execute` symbol vanished in 2.6.0, which *looked* like a command removal. The live `commands/available` still advertises **`guide`** ("Get help with Kiro CLI features from the guide agent"). A symbol disappearing under LTO/refactor ≠ a wire removal — this is exactly why the methodology pairs the binary diff with a live capture.

### Thinking parity (Opus + effort → `agent_thought_chunk`) — DIRECTLY CONFIRMED
A corrected probe (`probe-stats-tokens-2.6.1.py`, model+effort sent as **requests with `sessionId`**) on installed 2.6.1: model→`Model changed to claude-opus-4.8`, effort→`Effort set to high`, then the turn streamed **19 `agent_thought_chunk` + 35 `agent_message_chunk`**, `stop=end_turn`. Thinking works on 2.6.1, identical to 2.5.1. (The earlier "0 thoughts" probe runs were two stacked probe bugs: model/effort sent as notifications, *and* `commands/execute` missing `sessionId` — sending it as a request without `sessionId` makes the binary exit. cyril includes `sessionId` and sends requests, so it's unaffected; the thinking modules are also byte-unchanged 2.5.1→2.6.1.)

---

## 3. Collateral finding (not a cyril blocker)

**sacp-conductor 11.0.0 cannot proxy kiro-cli-chat 2.6.1** — `kiro-cli-chat acp` via conductor dies at ACP init with *"server shut down unexpectedly"*; the archived 2.5.1 binary proxies cleanly the same day. cyril's direct ACP path is unaffected. This is a blocker for the **conductor-integration** project (proxy-stage layer) — conductor needs a version bump or the init handshake change in 2.6.0 needs investigating before conductor can sit in front of 2.6.x.

---

## Reproduce

```sh
# Live cyril-path capture (no conductor)
cargo run --example test_bridge -- --agent-command kiro-cli acp > /tmp/tb-2.6.1.out 2>&1
# Symbol/module diff
nm ~/.local/share/kiro-research/binaries/2.5.1/kiro-cli-chat | rustfilt \
  | grep -oE 'agent::acp::commands::[a-z_]+' | sort -u   # vs same on 2.6.1
# tui.js self-validating carve: start offset of 3rd "#!/usr/bin/env bun", trim at trailing 64-hex sha
```

Artifacts: `experiments/conductor-spike/test_bridge-2.6.1.out`, `logs/conductor-2.5.1-2026-06-09.log`, `logs/probe-thinking-2.{5,6}.1-notif*.log`; binaries + BUILD-INFO at `~/.local/share/kiro-research/binaries/2.6.{0,1}/`; `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.6.1.js`.
