# Kiro CLI 2.10.0 — wire audit (diff vs 2.9.0)

**Analyzed:** 2026-06-29 · **Release:** 2.10.0, BUILD_HASH `a5da029503141c8033a11d7ee6569583d915cd90`, BUILD_DATE `2026-06-25T20:04:08Z` · **Tarball sha256** `551539f9beb55090d03a55eb7e46ee5c38185c9e4395b70afe14524cba6e684e` (matches AUR PKGBUILD pin `0db0d4c`).
**Method:** installed 2.10.0 binary (`~/.local/bin/`, archived to `~/.local/share/kiro-research/binaries/2.10.0/`) vs archived 2.9.0; same-day binary-isolated v2 surface capture (`probe-v2-surface-2.8.0.py`); `nm`+`rustfilt` module-path diff; binary-wide ACP-method-string diff; embedded-KAS sha-gate + `@kiro/agent` version comparison; doc-manifest delta. Single environment (this user's non-enterprise token).

**Verdict for cyril: SAFE — no code change required, safe to upgrade 2.9.0→2.10.0.** The v2 path cyril drives is wire-unchanged (24 cmds / 15 tools, zero new ACP methods). KAS/V3 is **byte-frozen** this release. The release's real substance is v2-Rust-internal (MCP hot-reload, subagent crew fail-fast/resilience) and never reaches the ACP wire.

---

## Changelog (announced, 2026-06-25)

```
Version 2.10.0 (2026-06-25)
  - Added: Hot-reload MCP servers when an agent config or mcp.json changes on disk —
           added, removed, or edited servers are reconciled (only changed servers
           restart) without losing conversation context.
  - Added: chat.disableInheritingDefaultResources to stop custom agents from inheriting
           default steering, skills, and AGENTS.md
  - Changed: Slash command and approval menus now show navigate, select, and cancel hints
  - Fixed: Various render bugs with the /chat menu picker
  - Fixed: Subagent crew tool no longer hangs when a stage fails — fails fast with an
           error instead of blocking indefinitely
  - Fixed: Improve subagent resilience: summary results are reliably delivered under load,
           and empty responses degrade gracefully
  - security: Harden Windows system-tool resolution against untrusted-search-path RCE (CWE-426).
```

Every item is either v2-Rust-internal, a frontend (tui.js) cosmetic, an agent-config/host concern, or a Windows-only security fix. None adds an ACP method, notification, command, or tool. (The embedded `version --changelog` returned this list directly — note it was reported "stale at 2.6.0" in 2.9.0; the installed 2.10.0 binary serves the current entry again.)

## KAS / V3 — **byte-frozen** (first time since the 2.7.1 landing)

The headline for the KAS-watching track: **the embedded KAS bundle did not change at all.**

- **`@kiro/agent` version: `0.3.299` in *both* 2.9.0 and 2.10.0.** The per-release bump streak since the KAS landing (0.3.224 @ 2.7.1 → 234 @ 2.8.0 → 257 @ 2.8.1 → 299 @ 2.9.0) **ended** — 2.10.0 is the first release to reship the prior bundle unchanged.
- **Embedded KAS bundle sha gate `81925c0995b5c1427b5d538e6a90ca2fdc4daffb786b09af749beaf7369d4e90` is present in both binaries.** That hash is the extractor's content gate ("on-disk sha ≠ embedded → re-extract"). Identical gate ⇒ the entire ~801MB extracted tree (`acp-server.js`, every `@kiro/acp-type-covenant` `.d.ts`, all `_kiro/*` handlers, the LangGraph orchestration graphs) is byte-identical.
- **No live KAS re-probe run, by design.** Byte-identity is *stronger* than a re-probe — it covers all of KAS, not just one path — and it transitively carries forward the 2.9.0 audit's full live characterization of 0.3.299 (handshake modes/configOptions/extensionMethods; `agent-subtask`/`agentSubtaskId` orchestration with no `list_update`; the 10-kind `session_info_update` taxonomy), captured same-day only 6 days ago. The only axis that could differ is the orthogonal **backend** axis, which is independent of this binary release. (If a live KAS handshake is ever wanted, `probe-kas-session-new-2.9.0.py` uses the direct-spawn free path — `node acp-server.js --transport=stdio`, FileAuthProvider, no token.)

**Implication:** the changelog's two subagent fixes ("crew fail-fast", "resilience under load") are therefore **not** in KAS — they land in the **v2 Rust engine** (see below). KAS's `OrchestrateSubAgent` was already one-shot/fail-fast (Kahn cycle-reject, `Promise.all` wave scheduler) and is unchanged.

## v2 (default `kiro-cli acp`) — real internal change, zero new wire surface

- **Exercised surface IDENTICAL to 2.9.0** (same-day binary-isolated capture, both binaries hitting today's backend):
  - **24 slash commands**: agent chat clear code compact context effort feedback goal guide help hooks knowledge mcp model paste plan prompts quit reply rewind stats tools usage
  - **15 tools**: code glob goal grep introspect knowledge read shell subagent todo_list tool_search use_aws web_fetch web_search write
- **Binary-wide ACP-method-string diff = zero new methods.** After normalizing for LTO string-adjacency glue, every apparent add/remove is the same base method with a different neighbor byte. `session/set_model` is present in both (still the unstable, unadvertised flag); `session/steer/*` in both. No new `kiro.dev/*`, `_kiro.dev/*`, `session/*`, `terminal/*`, or `fs/*` method.
- **`nm`+`rustfilt` module-path diff (+52 / −29 over 4245→4268)** — unlike 2.9.0, which was binary-identical to 2.8.1, the v2 Rust engine genuinely moved this release. Kiro-internal signal (the rest is AWS-SDK regen + image-codec churn + runtime dep bumps):

  | Module | Δ | Maps to |
  |---|---|---|
  | `agent::agent::mcp::reconcile` | **+** | MCP hot-reload (changelog #1) — host-internal server lifecycle, **not** an ACP method |
  | `chat_cli::agent::subagent` | **+** | v2 subagent crew fail-fast/resilience (changelog #5/#6) — behavioral |
  | `chat_cli_v2::agent::acp::subagent_tool` | **+** | ACP-facing subagent tool (still the same `subagent` wire tool) |
  | `chat_cli::cli::chat::tools::switch_to_execution` | **+** | goal→execution transition (internal; `goal` tool already on wire, no new tool) |
  | `chat_cli::cli::chat::line_tracker` | **+** | render/`/chat` picker fixes (changelog #3/#4) |
  | `chat_cli_v2::util::file_lock` | **+** | concurrency (likely the "summary delivered under load" resilience fix or mcp.json watcher) |
  | `agent_client_protocol_schema::agent::{Implementation, SessionModeId, SessionConfigValueId}` | **+** | acp-schema crate version bump (see below) |
  | `agent_client_protocol_schema::agent::{SessionModelState, SessionModeState}` | **−** | "" (reverses part of 2.9.0's reshape) |
  | `chat_cli::cli::agent::legacy::hooks`, `chat_cli_v2::cli::chat::legacy`, `chat_cli_v2::agent::acp::orchestration::types`, `chat_cli_v2::rollout`, `sacp::role::RemoteStyle` | **−** | legacy/dead-code cleanup |

### The one thing worth tracking: acp-schema dep churn

`agent_client_protocol_schema::agent` re-adds `SessionModeId`/`SessionConfigValueId`/`Implementation` and drops `SessionModelState`/`SessionModeState` — the inverse of 2.9.0's reshape. This is Kiro bumping its `agent-client-protocol-schema` dependency, not a behavior change (these are internal Rust type names; the JSON wire is serde-stable). It matters only because **cyril depends on the same crate family** (`agent-client-protocol` 0.10.2 / schema 0.11.2) and that crate is in active flux toward v1.0 (see `reference_sacp_acp_v1_merge`). Track it when planning cyril's own acp-dep upgrade; nothing to do now.

## Doc-manifest delta — nil

Embedded product-doc manifest **134 → 134 nodes, zero adds/removes, zero description changes**. The single signal is `tools/subagent.md` re-validated `2026-04-30 → 2026-06-22`, corroborating subagent/crew as the release's focus area. No unannounced features leaked (contrast 2.8.1, which pre-leaked `voice-mode`). Artifacts: `docs/kiro-docs-index-2.10.0-{82,118,merged}.json`.

## Binary sizes

| Binary | 2.9.0 | 2.10.0 | Δ |
|---|---|---|---|
| `kiro-cli` | 118,982,224 | 119,003,088 | +20,864 |
| `kiro-cli-chat` | 728,826,384 | 687,954,680 | **−40,871,704** |
| `kiro-cli-term` | 86,886,320 | 86,892,136 | +5,816 |

`kiro-cli-chat` **shrank ~41 MB**. The embedded KAS bundle is proven byte-identical (sha gate), so the shrink is entirely Rust-side — image-codec consolidation (the module diff removes `zip`/`zstd`/`flate2`/`ppmd`/`exr` encoder paths), legacy-code deletion (`legacy::hooks`, `cli::chat::legacy`, `orchestration::types`), and AWS-SDK regen. Not wire-relevant; a section-size breakdown is available on request if the exact attribution matters.

## cyril implications

- **Safe to upgrade. No code change.**
- **Subagent crew fail-fast (v2) is a free robustness win** for cyril's existing `SubagentTracker`/`crew_panel` (the `kiro.dev/subagent/list_update` path): a failed crew stage now reaches a terminal status promptly instead of leaving a perpetually-spinning row. Wire schema unchanged — worth a quick visual confirmation that cyril renders a failed/error stage gracefully, but no new field to handle.
- **MCP + agent-config hot-reload reflect in cyril for free** — and the re-advertisement *is* on the wire (corrected; see "Hot-reload — live-verified" below). `chat.disableInheritingDefaultResources` is a pure agent-config concern, off-wire.
- **acp-schema dep churn** — the only forward-looking item; fold into cyril's `agent-client-protocol` upgrade tracking, not this release.

## Hot-reload — live-verified (2026-06-29, `probe-hotreload-2.10.0.py`)

The 2.10.0-new piece (`agent::agent::mcp::reconcile`) is a kiro-internal **file watcher**; it adds no ACP method but **re-fires existing notifications cyril already handles**, so cyril reflects both hot-reloads for free, in real time, **while idle (no turn needed)**:

- **MCP:** rewriting `.kiro/settings/mcp.json` mid-session → ~3.5s later, idle, kiro fired `_kiro.dev/mcp/server_init_failure` **and** re-emitted `_kiro.dev/commands/available` carrying the **full updated `mcpServers` list** (replace semantics → removals propagate too). cyril maps both (`convert/kiro.rs` → `McpServerInitFailure`; `kiro.dev/commands/available` → `CommandsUpdated`, `app.rs:311`). (The docs' "next idle boundary between turns" undersells it — the watcher fires without a turn.)
- **Agents:** adding `.kiro/agents/probe-agent-2.json` mid-session → it appears in the live `_kiro.dev/commands/options` query for `/agent`. cyril's picker is pull-on-demand (no cached agent list), so it's always fresh.
- **Residual edge case (unreachable in practice):** `register_agent_commands` (`commands/mod.rs:200`) is insert-only and never prunes — but MCP servers contribute tools/prompts (not slash commands) and agent files are read live, so a hot-reload can't strand a removed slash command. Not worth changing.

Net: nothing to implement — 2.10.0's change rides notifications cyril already routes.

## Reproduction / artifacts

- Binaries archived: `~/.local/share/kiro-research/binaries/2.10.0/` (+ `BUILD-INFO`, `checksums.sha256`).
- v2 surface: `python3 experiments/conductor-spike/probe-v2-surface-2.8.0.py ~/.local/bin/kiro-cli-chat` (run against archived 2.9.0 the same day for isolation).
- Module diff: `nm <bin> | awk '$2~/[tT]/{print $3}' | rustfilt | grep -oE '^[a-z_][a-z0-9_]*(::…){2,}' | sed 's/::[a-z_].*$//' | sort -u`, then `comm`.
- KAS frozen proof: `strings -n8 <bin> | grep '@kiro/agent":"'` and `grep -c -a 81925c09…d4e90 <bin>` on both binaries.
- Doc-manifest: `python3 experiments/conductor-spike/extract_doc_manifest.py <bin> <out-prefix>`.
- Hot-reload wire proof: `python3 experiments/conductor-spike/probe-hotreload-2.10.0.py ~/.local/bin/kiro-cli-chat` (mutates `mcp.json` + adds an agent file mid-session, logs the inbound re-advertisement; does one trivial turn).

Methodology: `reference_kiro_wire_audit_methodology` (wire = binary × backend). Prior: `docs/kiro-2.9.0-wire-audit.md`.
