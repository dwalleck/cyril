# Kiro CLI 2.8.1 — wire audit (diff vs 2.8.0)

**Analyzed:** 2026-06-18 · **Method:** downloaded + SHA-verified 2.8.1 headless tarball (archived to `~/.local/share/kiro-research/binaries/2.8.1/`) vs archived 2.8.0; same-binary live v2 surface capture; `nm`+`rustfilt` symbol diff of `kiro-cli-chat`; carved + sha-validated embedded `tui.js` from both binaries; embedded `@kiro/agent` version probe; self-extracted KAS 0.3.257 bundle + covenant type diff. Single environment (this user's social/GitHub token, non-enterprise).

**Verdict for cyril: SAFE — nothing changed on the v2 path cyril drives.** 2.8.1 is a **TUI-only patch release** for the v2 engine (MCP OAuth clipboard, subagent-approval rendering, welcome link). cyril's default `kiro-cli acp` (v2) wire surface is identical to 2.8.0 — proven from three independent angles. **The only additive wire change is KAS-side and undocumented in the changelog:** the embedded KAS bundle jumped `@kiro/agent` 0.3.234 → 0.3.257, adding two new `_kiro/*` methods (`_kiro/sessions/changed`, `_kiro/hooks/setEnabled`).

Build: `BUILD_VERSION=2.8.1`, `BUILD_HASH=43ca6648d9e72889f9b499548df52a230d216456`, built 2026-06-17.

---

## Changelog (all four announced changes — every one TUI-side, V2 mode)

```
Version 2.8.1 (2026-06-17)
  - Fixed:   MCP OAuth in V2 mode now copies the authorization URL to the clipboard instead of failing silently
  - Fixed:   Subagent tool calls and approval prompts now appear in the main view during spec workflows
  - Changed: MCP OAuth panel now shows confirmation when URL is copied to clipboard
  - Changed: Welcome screen link updated to point to V3 documentation
```

All four are presentation/interaction fixes in the React/Ink TUI. None touch the ACP wire. (KAS bundle bump below is *not* in the changelog.)

## v2 (default Rust engine) — unchanged, proven three ways

1. **Exercised wire surface identical** (same-session capture, `probe-v2-surface-2.8.0.py` against both binaries):
   - **24 slash commands**: agent chat clear code compact context effort feedback goal guide help hooks knowledge mcp model paste plan prompts quit reply rewind stats tools usage
   - **14 tools**: code glob goal grep introspect knowledge read shell subagent todo_list **use_aws** web_fetch web_search write — `use_aws` still alive on v2.
2. **`kiro-cli-chat` Rust symbols byte-identical.** `nm`+`rustfilt` then filtered to kiro-relevant crates (`chat_cli_v2`/`kiro*`/`kas*`/`sacp*`): **2554 symbols, zero added, zero removed** (and zero module-path delta). This is a much cleaner signal than the strings-adjacency diff 2.8.0 had to fall back on — the backend agent/protocol code did not change at all.
3. **tui.js wire-method set identical**: 61 distinct `_kiro/*` / `kiro.dev/*` / `session/*` method strings in each bundle, zero delta. The `extNotificationHandlers` dispatch table and the `_message/send` / `_session/steer` / `_session/steer/clear` request senders are unchanged.

## The embedded `tui.js` bundle WAS rebuilt (but added no wire surface)

`tui.js` is embedded uncompressed in `kiro-cli-chat`, immediately followed by its sha256 trailer (self-validating carve). Both carved bundles validate against their trailers:

| Version | embedded `"version"` | sha256 | size |
|---|---|---|---|
| 2.8.0 | `2.8.0` | `0bd6de2bc79a1e131f64c926f15c6e88cdba389f827e9476dbb64679d45dc96d` | 12,249,527 |
| 2.8.1 | `2.8.1` | `db979570836a5de2a5a5af756c2571073347bd55c606430752ba1cd138c75b4f` | 12,250,644 (+1,117 B) |

Both archived to `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.8.{0,1}.js` (+ `.sha256` sidecars). The semantic changes (the rest of the char-level diff is minifier identifier-rename noise — one inserted component cascades short-variable reassignments across the file):

- **MCP OAuth clipboard fix** — new `copyOAuthUrl` handler, `pendingOAuthUrl`(`s`) state, a **Ctrl+Y** keybind, and an `"OAuth URL copied to clipboard"` confirmation toast (clipboard string count 41→46, `Copied` 1→2). Has both a v2 and a `kas` branch. This is the "fail silently" → "copy + confirm" fix.
- **Subagent approval rendering in main view** — React/Ink approval-queue components: `approvalQueue` (push `toolCall.toolCallId`), `pendingApproval`, `respondToApproval`/`cancelApproval`/`approvalMode` keyed by `sessionId`, a memoized approval component keyed by `toolCallId`, `sessionsWithApproval`. These surface per-subagent permission requests in the main viewport during spec workflows. **Wire is unchanged** — `session/request_permission`, tool-call `session/update`, and `kiro.dev/subagent/list_update` were always emitted; Kiro's own TUI just wasn't displaying them. cyril already renders subagent tool calls + approvals via its own pipeline, so this is a no-op for cyril.
- **Welcome link** — `https://kiro.dev/docs/cli/v3/` added (0→1), pointing at the new V3 docs.

## KAS / V3 — the real (undocumented) change: `@kiro/agent` 0.3.234 → 0.3.257

The chat binary grew ~335 KB; only ~1.1 KB of that is tui.js and the Rust symbols are identical. The remainder is the **embedded KAS bundle bump** — confirmed by the embedded version string (`"@kiro/agent":"0.3.234"` → `"0.3.257"`). Self-extracting the 2.8.1 bundle (`acp --agent-engine kas`, which works today — the `--v3` frontend gate is bypassed via the ACP path) and diffing the `@kiro/acp-type-covenant` against the archived 0.3.234:

**Covenant `_kiro/*` method catalog: 66 → 68. Two net-new methods, zero removed:**

- **`_kiro/sessions/changed`** (NEW `capabilities/sessions/` dir — the 31st capability; new `@kiro/agent` `session/session-roster-manager.d.ts`). A **connection-scoped session-roster delta** pushed to *observer* clients whenever the set of sessions on a connection changes, or a session's coarse status / title / description changes. State-convergent change-data-capture: `{ upserted: SessionRosterUpsert[], deleted: string[] }` where an upsert always carries `sessionId` and only the changed fields (first sighting = all fields), client merges. A `SessionRosterEntry` is `{ sessionId, title, description?, status, cwd, additionalDirectories? }`. Together with a single cold-start `session/list` it **replaces periodic `session/list` polling** for multi-session discovery + background-session status. Routes through `MultiplexStream` so it broadcasts to *every* client on the connection — i.e. a first-class **multi-client observer** primitive (the KAS analog of v2's `kiro.dev/subagent/list_update`, but for top-level peer sessions, not subagents). **Directly relevant to cyril's session-level-workflow / multi-observer direction.**

  **The roster deliberately excludes subagents — verified in the 0.3.257 source maps.** `SessionRosterManager.track()` is reached only via `trackSessionInRoster()`, which is called from exactly two sites: the ACP **`session/new`** and **`session/load`** handlers. KAS subagents are *not* created through `session/new` — they're spawned internally by the subagent tool (`createSubagentInvocationTools`, tools named `subagent/<agentId>`) and their live activity streams through the **parent** session's `session/update` as `subExecutionId`-tagged actions (`subExecutionId` → `agentSubtaskId` on the wire; *"Add subExecutionId if this tool call belongs to a sub-agent"* in `execution-message-adapter.ts`). The `SessionRosterEntry` carries no `parentSessionId`/subagent field — it is structurally a flat top-level list.

  **But subagents *are* real sessions — just not roster ones.** The covenant `CreatedReasonSchema = z.enum(['human','rewind','subagent','thread'])` and `SessionMetadata`/`SessionSummary` carry `parentSessionId` + `parentExecutionId`, so persisted subagent sessions exist with parent linkage and surface in **`session/list`** (the full history) — but *not* in the live `_kiro/sessions/changed` roster. Net asymmetry: `session/list` (cold-start) = all persisted sessions incl. `createdReason:'subagent'` *with* parent pointers; `_kiro/sessions/changed` (live) = top-level `session/new`+`load` only, flat, no parent. Subagent observability remains the separate `agentSubtaskId`/parent-stream path, unchanged by 0.3.257.
- **`_kiro/hooks/setEnabled`** (new `@kiro/agent` `hooks/set-hook-enabled.d.ts`; documented in covenant `capabilities/hooks/types.d.ts`). Client → agent request to **persist a hook's enabled state**: `{ hookId, enabled }` → `{ success, code?, error? }` (`code` ∈ `hook_not_found | not_file_backed | invalid_params | write_error`). The agent writes `enabled` into the hook's backing `.kiro/hooks/*.json` (the file is the source of truth for v2 enablement), the file watcher reloads and re-emits `_kiro/hooks/didChange`, and the client refreshes its tree from *that* — not from the response. Completes the hooks host-callback interface (which already had `list` / `executeHook` / `triggerHook` / `cancel` / `sessionStart` / `didChange` in 0.3.234).

**Other new KAS type file:** `acp/snapshot-uri.d.ts` — `kiro-snapshot:` URI build/parse helper (`{ snapshotId, originalPath, sessionId?, executionId? }`, supports a new `kiro-snapshot:/?...` form + a legacy `kiro-snapshot:///relativePath?...` form). Backs the `checkpoints: true` capability (rewind/checkpoint file snapshots).

**Type/map goldmine intact + grew:** `@kiro/agent/dist` `.d.ts` 661 → 664; covenant `.d.ts` 68 → 69 (31 capability dirs). Per-capability typed contracts keep shipping with full doc comments.

KAS init handshake shape unchanged (`extensionMethods`: `_kiro/knowledge`, `_kiro/codeIntelligence`, `_kiro/session/{context,compact,export,history}`; `sessionCapabilities {list, fork}` non-empty; `checkpoints`, `sessionList`, `policyNotifications` meta flags).

## Cyril impact

- **None on the current v2 path.** Stay on the default engine; `use_aws` + all v2 behavior unchanged. No cyril code change warranted by 2.8.1. Safe to upgrade `kiro-cli` 2.8.0 → 2.8.1.
- **KAS-track signal (the reason this release matters):**
  - **`_kiro/sessions/changed`** is a ready-made multi-session observer feed — a strong fit for the [session-level workflow](../README.md) / multi-client observer vision (`SessionTracker`). When the KAS converter lands (ROADMAP KAS-2/KAS-3), it should consume this roster CDC instead of polling `session/list`, and KAS-4's session UX can render the roster directly. Add `_kiro/sessions/*` to the [covenant reference catalog](kiro-kas-acp-covenant.md).
    - **This feeds the *session-level* (peer-session) path, NOT subagent rendering** — and that validates cyril's existing split. The roster only carries top-level sessions created via `session/new` (exactly cyril's session-level workflow engine's N peer sessions, per [session-level workflows](../README.md)); KAS subagent rendering (KAS-3) stays on its own channel — consume the focused session's `session/update` stream, group by `agentSubtaskId`/`subExecutionId`, render the DAG + review-loop state (the analog of the v2 `crew_panel`/`subagent/list_update` work). Don't wire subagents onto the roster — KAS keeps `SessionTracker` (roster-fed) and `SubagentTracker`/crew (parent-stream-fed) separate at the protocol level, same as cyril.
  - **`_kiro/hooks/setEnabled`** rounds out the hooks host-callback model already noted in the covenant memory; a future KAS hooks UI (enable/disable from a tree view) now has a wire verb. KAS-5 (fs/host-callback responder stage) should plan for the full hooks surface incl. `setEnabled` + `didChange`.
  - The KAS-2 converter's unknown-variant tolerance must accept the new `_kiro/sessions/changed` + `_kiro/hooks/setEnabled` (additive, no removals — same as the 2.8.0 `_kiro/safety/*` family).
- **Trajectory:** v2 is frozen-stable (two consecutive zero-backend-delta releases, 2.8.0 + 2.8.1); all real motion is in the fast-moving KAS bundle (0.3.224 → 0.3.234 → 0.3.257 across 2.7.1/2.8.0/2.8.1). KAS is where the wire is evolving — cyril's reverse-engineering attention should track the embedded `@kiro/agent` version every release, not just the changelog.

## Note on local KAS state

The installed launcher (`~/.local/bin/kiro-cli`) is still **2.8.0**; this audit self-extracted **0.3.257** into the shared `~/.local/share/kiro-cli/kas/` from the archived 2.8.1 binary for analysis. If the user stays on 2.8.0 and runs `--v3`, the version-gated extractor will re-extract 0.3.234; if they upgrade to 2.8.1 the on-disk bundle already matches. Benign either way.

## Reproduce

```sh
# fetch + verify (SHA from prod.download.cli.kiro.dev/stable/latest/manifest.json)
curl -sSLo /tmp/k281.tar.zst https://desktop-release.q.us-east-1.amazonaws.com/2.8.1/kirocli-x86_64-linux.tar.zst
echo "61e6a0ca88882f2f7a92769571d683cbd65ae5a1229f180b07d9c2c107d94aa6  /tmp/k281.tar.zst" | sha256sum -c

# the four announced (TUI-only) changes
kiro-cli version --changelog=2.8.1

# v2 surface identical (same-session), against each archived chat binary
python3 experiments/conductor-spike/probe-v2-surface-2.8.0.py ~/.local/share/kiro-research/binaries/2.8.0/kiro-cli-chat
python3 experiments/conductor-spike/probe-v2-surface-2.8.0.py ~/.local/share/kiro-research/binaries/2.8.1/kiro-cli-chat

# Rust backend symbols byte-identical (kiro-relevant)
diff <(nm .../2.8.0/kiro-cli-chat | rustfilt) <(nm .../2.8.1/kiro-cli-chat | rustfilt)   # -> empty for kiro/chat_cli_v2/kas/sacp

# the undocumented KAS bump
grep -aoP '"@kiro/agent":"[0-9.]+' ~/.local/share/kiro-research/binaries/2.8.{0,1}/kiro-cli-chat   # 0.3.234 vs 0.3.257
# new covenant methods after self-extracting 0.3.257 (acp --agent-engine kas init once):
cat ~/.local/share/kiro-cli/kas/node_modules/@kiro/acp-type-covenant/dist/capabilities/sessions/changed.d.ts
grep -A20 'HookSetEnabledParams' ~/.local/share/kiro-cli/kas/node_modules/@kiro/acp-type-covenant/dist/capabilities/hooks/types.d.ts
```
