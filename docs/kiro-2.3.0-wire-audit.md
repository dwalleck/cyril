# Kiro CLI 2.3.0 — ACP Wire Audit

> **📜 Historical release-specific snapshot.** This is the findings record from the 2.3.0 audit; it is not a current-state reference. For current wire shape see [`kiro-acp-protocol.md`](kiro-acp-protocol.md) (§ 11 documents changes through the current Kiro release). For the docs index and currency tracker, see [`README.md`](README.md).


> **Captured 2026-05-11** using `experiments/conductor-spike/` against today's backend. Binary-isolated test: same backend, two binaries (2.2.2 archived vs 2.3.0 freshly installed). Methodology details in [`docs/kiro-acp-protocol.md`](kiro-acp-protocol.md) and the `wire-audit-methodology` reference in user auto-memory.

**Released**: 2026-05-11 (S3 tarball; embedded changelog absent). Build hash `a44742ad7a3acc79eac7459c2986103a8ad2b756` dated 2026-05-07.

## Audit setup

Binary-isolated, same-backend test:

- **Reference**: 2.2.2 (archived at `~/.local/share/kiro-research/binaries/2.2.2/`) — `kiro-cli-chat` sha256 `317579c3…`
- **Capture**: 2.3.0 (archived at `~/.local/share/kiro-research/binaries/2.3.0/`) — `kiro-cli-chat` sha256 `96ba63cb…`
- **Tarball**: `https://desktop-release.q.us-east-1.amazonaws.com/2.3.0/kirocli-x86_64-linux.tar.zst` (sha256 `882a9150…`)
- **Wrappers**: `experiments/conductor-spike/conductor-wrapper-2.2.2.sh`, `conductor-wrapper-2.3.0.sh`
- **Harness output**: `experiments/conductor-spike/test_bridge-2.2.2.out`, `test_bridge-2.3.0.out`

## Exercised-wire result

**Zero field-level deltas across 14 common methods.** Same field paths, same counts. No methods present in only one binary. Field-differ output:

```
[_kiro.dev/commands/available (request/notif)] — no field deltas (17 fields)
[_kiro.dev/commands/execute (request/notif)] — no field deltas (4 fields)
[_kiro.dev/commands/execute (response)] — no field deltas (44 fields)
[_kiro.dev/commands/options (request/notif)] — no field deltas (3 fields)
[_kiro.dev/commands/options (response)] — no field deltas (5 fields)
[_kiro.dev/metadata (request/notif)] — no field deltas (6 fields)
[_kiro.dev/subagent/list_update (request/notif)] — no field deltas (2 fields)
[initialize (request/notif)] — no field deltas (7 fields)
[initialize (response)] — no field deltas (12 fields)
[session/new (request/notif)] — no field deltas (2 fields)
[session/new (response)] — no field deltas (9 fields)
[session/prompt (request/notif)] — no field deltas (3 fields)
[session/prompt (response)] — no field deltas (1 fields)
[session/update (request/notif)] — no field deltas (4 fields)
```

## Dormant method changes (from binary strings, not exercised by test)

Comparing extracted `_kiro.dev/*` strings between `kiro-cli-chat` 2.2.2 vs 2.3.0:

**Added in 2.3.0:**

- `_kiro.dev/mcp/governance_disabled` — notification with payload `{ apiFailure: boolean }`. The v2 TUI handler (`handleMcpGovernanceDisabled`) was already present in `tui.js` for 2.2.2 — 2.3.0 finally wires the agent side to emit it. Builds on the 2.2.2 "MCP governance enforcement in V2 TUI" release.
- `_kiro.dev/settings/list` — extension method to enumerate settings. Previously only `settings/set` existed. V2 TUI's `Qle::listSettings` already wired to call this.

**Removed in 2.3.0:**

- `_kiro.dev/agent/config_error` — string is gone from agent binary. v2 TUI still has `handleAgentConfigError` defensively, so removal is asymmetric.
- `_kiro.dev/session/list` — deprecated unstable session-listing extension fully removed.

Wire impact for cyril: today's bridge never observed any of these methods, so cyril is functionally unaffected. If cyril ever added `agent/config_error` handling it should drop it.

## Binary delta

| Binary         | 2.2.2 size | 2.3.0 size | Δ        |
|----------------|------------|------------|----------|
| `kiro-cli`     | 117.61 MB  | 117.86 MB  | +0.25 MB |
| `kiro-cli-chat`| 393.77 MB  | 395.18 MB  | +1.41 MB |
| `kiro-cli-term`| 85.73 MB   | 85.89 MB   | +0.16 MB |

Module-path symbol delta: 2455 → 2453 (-2 net; 36 added, 38 removed). The +1.4 MB on `kiro-cli-chat` is implementation density inside existing modules, not new module surface.

## Biggest finding: KAS TypeScript agent engine scaffolding

Hidden behind a new flag: `kiro-cli acp --agent-engine <ENGINE>` and `kiro-cli chat --agent-engine <ENGINE>`. Help text: *"Agent engine to use: 'rust' (default) or 'kas' (TypeScript KAS agent)"*. KAS adds its own `--mode <vibe|spec>`.

### Three success paths, only one fails by default

The error message reveals all three resolution paths plus the failure:

```
"Using KAS agent engine, server path override: "        ← KIRO_KAS_SERVER_PATH set (works today)
"Using KAS agent engine, embedded node: , server: "     ← compiled-in assets (not in this build)
"Using KAS agent engine, server resolved from @kiro/agent package"  ← npm pkg found (not public)
"KAS assets not embedded and KIRO_KAS_SERVER_PATH not set"          ← all three missing → error
```

The failure check is a precondition gate at `crates/chat-cli/src/cli/mod.rs:441` — before any extraction would happen. The "not embedded **and** not set" wording is an OR-gate between two parallel preconditions, both unsatisfied. Most plausible: a compile-time `--features kas-embedded` was off for the 2.3.0 build; the +1.4 MB binary growth is too small to actually contain a TypeScript agent runtime.

### Proven working with `KIRO_KAS_SERVER_PATH`

Probed empirically — the Rust binary actually spawns a node process when the env var is set:

```
$ KIRO_KAS_SERVER_PATH=/tmp/nonexistent.js kiro-cli acp --agent-engine kas
Error: Cannot find module '/tmp/nonexistent.js'    ← node WAS spawned
  at Module._resolveFilename (node:internal/modules/cjs/loader:1475:15)

$ KIRO_KAS_SERVER_PATH=/etc/hostname kiro-cli acp --agent-engine kas
/etc/hostname:1
zbook-ultra                                         ← node loaded the file as JS
ReferenceError: zbook is not defined
```

The host loop is fully functional. The only thing missing in the 2.3.0 build is the actual `acp-server.js` content. Anyone with a Node script implementing ACP can run it through 2.3.0's KAS host today.

### What 2.3.0 ships

- CLI argument plumbing (`--agent-engine`, `--mode`)
- **Working host loop** — Rust spawns `node --experimental-wasm-modules <server> --transport=stdio [--token-path=...]`
- Rust dispatch (`chat_cli::cli::execute_kas_acp`, `chat_cli::cli::chat::ChatArgs::resolve_agent_engine`)
- Asset extraction function (`chat_cli::embedded_tui::extract_kas_assets_if_needed`) — present but unused without embedded assets
- Token plumbing (`chat_cli::util::paths::kas_token_path`)
- TUI-side factory (`Ile` function in `tui.js` v2 bundle, already present since 2.2.1): `KIRO_AGENT_ENGINE=kas` switches to KAS class `vle`

What's missing: only the actual `acp-server.js` content. Same staging pattern as `AcpClient` scaffolding in 2.2.1 (still zero callers in 2.3.0).

### `@kiro/agent` NPM scope status

Checked 2026-05-12: `@kiro/agent` returns 404 on the public NPM registry, and `scope:kiro` returns 0 packages total. Either AWS-internal CodeArtifact only, or unreleased. The "Install @kiro/agent or set KIRO_KAS_SERVER_PATH" message in `tui.js` suggests public-NPM is the eventual distribution channel — just not flipped yet.

### KAS engine details (extracted from `tui.js` inside the binary)

KAS is a separate ACP dialect, not a drop-in alternative to the rust engine. Spawn command:

```
${KIRO_AGENT_PATH:-node} --experimental-wasm-modules ${SERVER_PATH} --transport=stdio [--token-path=...]
```

Server path resolution order:

1. `KIRO_KAS_SERVER_PATH` env var
2. Walk up 10 directories looking for `node_modules/@kiro/agent/dist/server/acp-server.js`
3. Hard-fail with "KAS agent not found. Install @kiro/agent or set KIRO_KAS_SERVER_PATH."

KAS-specific extension methods use `_kiro/` (single namespace) not `_kiro.dev/`:

- `_kiro/help`, `_kiro/agent/list`, `_kiro/clear`, `_kiro/plan`, `_kiro/session/delete`

KAS surfaces extension methods via `agentCapabilities._meta.kiro.extensionMethods[]` and only advertises a command if the agent declares the required methods. (Contrast: rust engine has hardcoded commands.)

KAS partial support (in 2.3.0 `tui.js`):

- `spawnSession` returns empty
- `listSettings` returns `{}`
- `terminateSession`, `setSetting` are no-ops
- `/agent` swap via `setSessionConfigOption({configId: "mode", value: agentName})` works
- Auto-sets `autopilot=on` on session start
- Honors `KIRO_MODE` env var

KAS-relevant env vars discovered in binary:

- `KIRO_AGENT_ENGINE`, `KIRO_AGENT_PATH`, `KIRO_KAS_SERVER_PATH`, `KIRO_KAS_TOKEN_PATH`, `KIRO_MODE`, `KIRO_TEST_MODE`

## New `/stats` slash command

Brand new in 2.3.0. Strings dump confirms: 2.2.2 slash command list ends `/guide`; 2.3.0 ends `/guide/stats`.

User-visible help: *"Show request IDs and timings for debugging slow turns"*. Usage: `/stats [N|save <filename>]`.

Backed by new Rust module:

- `chat_cli_v2::agent::acp::request_stats::{RequestRecord, RequestStats}` — `push`, `snapshot`
- `chat_cli_v2::agent::acp::acp_agent::AcpSession::record_request_stats`
- `chat_cli_v2::agent::acp::commands::stats::{save, show}`

### Wire format

Triggered `/stats` via `_kiro.dev/commands/execute` after a single one-word prompt. Response:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "success": true,
    "message": "1 request recorded",
    "data": {
      "stats": [{
        "request_id": "58e3ad65-e63d-42a0-9a49-55dc365f4cdf",
        "timestamp": "2026-05-12T02:06:03.463404776+00:00",
        "duration_ms": 1095.0620099999999,
        "ttfc_ms": 1094.8782079999999,
        "input_tokens": null,
        "output_tokens": null,
        "status_code": null,
        "had_tool_use": false,
        "error": null
      }],
      "summary": {
        "avg_ms": 1095.06,
        "p90_ms": 1095.06,
        "max_ms": 1095.06,
        "errors": 0
      }
    }
  },
  "id": "..."
}
```

`RequestRecord` populated fields: `request_id`, `timestamp`, `duration_ms`, `ttfc_ms`, `had_tool_use`. **`input_tokens` / `output_tokens` / `status_code` are `null`** as of today's backend — schema present, values not populated.

### Tokens-on-ACP status

This is the **first time** Kiro's ACP wire has carried token-named fields. Earlier audits established that:

- The push path (`_kiro.dev/metadata`) only carries `meteringUsage[]` credits + `turnDurationMs` + `contextUsagePercentage` — no tokens
- Even the SQLite session sidecar's `input/output_token_count` stays 0 while credits populate

2.3.0 doesn't fix the values, but it adds the **pull-path schema** (`/stats` command response). Identical staged-rollout pattern as `meteringUsage`: binary-side dormant for ≥1 release before the backend started populating in late April 2026. Strong signal a backend rollout is queued for token counts.

### Ordering and correlation (verified two-turn capture)

`/stats` array is **chronological — oldest first, newest last**. Confirmed by sending two prompts back-to-back, observing two `_kiro.dev/metadata` notifications with `turnDurationMs` 1096 then 1083, then calling `/stats`. The full response had `stats[0].duration_ms ≈ 1096` (TURN A) and `stats[1].duration_ms ≈ 1083` (TURN B), matching the `turnDurationMs` values and the 15-second wall-clock gap between the `timestamp` fields.

`/stats` with `args: {"value": "1"}` returns **only the most recent record** — same shape, but `stats[]` has length 1. The single record matched the last element of the full response (TURN B's `request_id` `b648bed5-…`). The `message` field changes accordingly: `"1 request recorded"` vs `"2 requests recorded"` — the count is the number returned, not the running total.

`request_id` is internal to `/stats` — it does NOT appear in `_kiro.dev/metadata`, `session/prompt` responses, or any other notification. There is no direct ID-based correlation. The available correlation signals across the wire are:

| Signal | Strength | Notes |
|---|---|---|
| `/stats 1` after `TurnCompleted` | **Strongest** — single record, guaranteed to be the turn we just finished | Recommended pattern |
| `turnDurationMs` ≈ `floor(duration_ms)` | Strong | 1096 vs 1096.54; 1083 vs 1082.80 — within 1 ms |
| `timestamp` (ISO-8601 with nanos) | Strong | Per-record `timestamp` aligns with notification arrival within ms |
| Array index after full `/stats` | Strong but requires running counter | Nth metadata notification ↔ `stats[N-1]` |

### Cyril integration sketch

If/when we want this data:

- After each `TurnCompleted`, send `BridgeCommand::ExecuteCommand { command: "stats", args: {"value": "1"} }`. The single returned record is the turn that just finished — no array-index or ID-correlation logic needed.
- Response data shape: `{ stats: RequestRecord[], summary: { avg_ms, p90_ms, max_ms, errors } }`. Bind with `Option<u64>` on `input_tokens` / `output_tokens` / `status_code` to tolerate today's nulls and tomorrow's populated values transparently. Add `#[serde(default)]` and avoid `deny_unknown_fields` — staged rollouts may add fields.
- A nice surface for cyril's toolbar: `ttfc_ms` (time-to-first-content) is tracked independently of `duration_ms`, so perceived-latency vs total-turn-time can be shown separately. In the verified capture, `ttfc_ms` was 1015 ms / 872 ms while `duration_ms` was 1096 ms / 1083 ms — meaningful gap.
- Watch for the backend rollout via same-binary-over-time capture (the *other* methodology axis); the moment `input_tokens` goes non-null in a fresh capture, the schema fully works without code changes on our side.

## Other notable Rust-side changes

- `chat_cli_v2::agent::acp::extensions::CompactionStatus` — REMOVED (likely moved/renamed)
- `chat_cli_v2::agent::acp::extensions::ModelNotFoundNotification` — REMOVED
- `chat_cli_v2::theme::{DEFAULT_THEME, Theme}` — REMOVED (v2 theme system gone)
- `chat_cli_v2::util::env_var::is_telemetry_disabled` — REMOVED
- `chat_cli::cli::agent::legacy::hooks::{LegacyHookTrigger, LegacyHookType}` — REMOVED. The "deserialize-only" legacy hook module added in 2.1.1 is now fully deleted; migration considered complete.
- `chat_cli::cli::chat::ChatArgs::resolve_agent_engine` — NEW. Engine resolution from CLI/env.
- `chat_cli_v2::agent::acp::acp_agent::synthesize_model_info` — NEW. Suggests model info fabrication when backend response is incomplete (esp. for `auto` model).
- `chat_cli_v2::api_client::delay_interceptor::RetryWarning` — NEW. May surface as `retry_warning` session update (v2 TUI bundle has handler).
- `chat_cli::mcp_registry::format_package_identifier` + `chat_cli_v2::mcp_registry::format_package_identifier` — NEW in both crates. MCP package registry formatting.

## `tui.js` unchanged

sha256 `228be7379b94a1d68eef7374c7598784e00f9db167f308b62cc4384df875520c` identical between 2.2.2 and 2.3.0. Launching `kiro-cli chat --tui` after upgrading did not trigger a bundle download — the embedded expected-hash matches. V2 TUI frontend is byte-identical.

## Cyril impact

**Safe to upgrade.** Cyril runs `kiro-cli acp` (default `--agent-engine rust`) and sees no field-level wire changes. The new dormant methods aren't exercised by cyril today and have no breaking shape if/when they appear.

**Potential follow-ups:**

- If we want MCP governance visibility, the new `_kiro.dev/mcp/governance_disabled` notification with `{ apiFailure: boolean }` payload is a 1-line decoder addition in `convert/kiro.rs`.
- If we want to expose `/stats` to cyril users, just add to the slash-command set — the response shape will be a regular `_kiro.dev/commands/execute` response.
- Future-watch: when KAS assets ship in a later release, cyril spawning `kiro-cli acp --agent-engine kas` would route into a wholly different ACP dialect (`_kiro/*` namespace, missing extension methods) — would need its own conversion path.

## Methodology check

This is the **binary-isolated, same-day, same-backend** axis. Both captures hit the same backend within ~6 minutes of each other. Backend rollouts attributable to the date range late-April → early-May 2026 (the metering field appearance) are present in both captures, eliminated as a variable. The `/stats` and KAS findings are pure binary-side changes.
