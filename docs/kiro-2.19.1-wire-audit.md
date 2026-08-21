# Kiro CLI 2.19.1 wire audit

**Audited:** 2026-08-21 (release date 2026-08-21). Patch release; binary `BUILD_HASH=5ac111d2` (built 2026-08-20), archived at `~/.local/share/kiro-research/binaries/2.19.1/` (origin sha verified; installed binary identical). tui.js snapshotted (`kiro-tui-2.19.1.js`, +7 KB). **KAS 0.46.1 → 0.48.0** — two more minors riding a patch; active development continues post-freeze.

**Cyril verdict: SAFE.** v2 wire is quiet (stall/retry/compaction string sets identical; `api.*` key set identical). All action is KAS-side, and everything degrades correctly by construction. One new opt-in wire surface worth adopting (cyril-2mo0).

## 1. NEW wire method: `_kiro/tools/content_chunk` — live shell-output streaming (unannounced)

The ride-along find, and the first `_kiro/*` + `_session/*` vocabulary expansion since 2.16.0's workflow emitter (110 → 111):

```json
{"method": "_kiro/tools/content_chunk", "params": {
  "sessionId": "…", "toolCallId": "…",
  "content": {"type": "content", "content": {"type": "text", "text": "<incremental shell output>"}}}}
```

- Emitted for a **running `execute` tool** in place of content-bearing `tool_call_update`s; the terminal update still carries full output. Chunks are sanitized, **redacted**, and coalesced server-side (`StreamCoalescer`, flush interval + max-bytes cap).
- **Client-gated**: `initialize.clientCapabilities._meta.kiro.streamingShellContent: true` (new member of the `textSearch`/`findFiles` capability family; default off → zero cyril impact today).
- **No shipped client consumes it**: tui.js 2.19.1 has 0 hits for the method and the capability — the emitter shipped ahead of any consumer (the mirror image of `_kiro/diagnostics/changed`, where the TUI subscribes to a non-existent emitter — still 0 in 0.48.0). Cyril can be the first client with live shell streaming → **cyril-2mo0**.

## 2. [V3] "Always allow" suppression — new consent fields (LIVE-PROBED)

New `src/acp/permission-options.ts`:

```js
canPersist      = consent && consent.persistableConsent !== false
canAlwaysAllow  = canPersist && consent.askType !== "explicit"
options = [ Allow(allow_once),
            …canAlwaysAllow ? Always allow(allow_always) : dropped,
            Deny(reject_once),
            …canPersist ? Always deny(reject_always) : dropped ]
```

`consentIsPersistable` verifies the candidate permission rule **round-trips** (rule derived from the command must match the command; check failure → degraded → false). When false, `_meta.kiro.consent` gains two new fields: `persistableConsent: false` and `persistableConsentReason` ("Kiro could not parse this command, so a saved rule would never match it. …", with a cmd.exe variant recommending PowerShell). This is the KAS analogue of v2 2.18.1's tar `trustOptions` suppression — but explanation-bearing and parser-driven rather than dangerous-flag-listed.

Live A/B (0.48.0, autopilot→off so approvals fire; probes committed): benign `echo hello`, command substitution + `eval`, a **multi-line command with a literal newline**, and a **heredoc** ALL drew the full 4-option set with clean parses (`triggeringResource` correctly extracted: `ls`, `echo first`, `cat`). The suppression was **not trippable with ordinary bash** — the parser is robust; the reason text naming cmd.exe suggests the practical trigger is mostly Windows. Confirmed consent shape when persistable: `{capability, resource, askType, triggeringResource, workspaceRoot}` — `persistableConsent` simply absent.

**Cyril impact: none required.** The approval overlay renders the offered `options` list dynamically (`state.rs show_approval` → windowed list), so a missing Always degrades correctly. Optional nicety whenever approval UI is next touched: surface `persistableConsentReason` so users know why Always is absent.

## 3. Other 2.19.1 items (verified where cheap)

- **Announced retry fixes** ([V3] EPIPE retry; truncated non-file tool-call retry loop): internal; no new wire vocabulary (`retry-wait` 2=2, watchdog strings still 0 in KAS — the stall window on v3 remains open, TurnStalled chip stays load-bearing).
- **[V3] initialize latency** (no longer waits on experiment resolution): timing-only. Related live observation: the model configOption's registry-settlement race is still visible — one probe's `set_config_option` response included `model` in the rebuilt list, another didn't (same day). KAS-4 consumers must stay update-driven.
- **Security: `grep_search`/`file_search` honor `.kiroignore`**: wiring of existing ignore machinery (string counts unchanged) into the search tools; agent-internal, no wire change.
- **`capturedOutput` still broken**: the extractor + transcript-fallback region is functionally byte-identical to 0.46.1 (only a bundler identifier rename) — the 2.19.0 audit § 14 finding stands unchanged in 0.48.0.
- **Doc manifest: zero drift** — identical path/title sets; 2.19.1 actually embeds a slightly *older* generation than 2.19.0 (2026-08-17 vs 08-19; `features/session-management.md` shows the pre-revalidation entry again). No new baselines committed.
- tmux/terminal-attribute fixes, `/knowledge` autocomplete + `rm` alias: TUI-side.

## 4. Artifacts

- Probes: `experiments/conductor-spike/probe-kas-perm-persist-2.19.1.py` (+ the second-arm variant is parameter tweaks of the same file).
- Captures (token-scrubbed): `experiments/conductor-spike/kas-perm-persist-2.19.1.jsonl`, `kas-perm-persist2-2.19.1.jsonl` — full permission frames incl. consent meta and 4-option lists; fixture material for approval tests.
- Issues: **cyril-2mo0** (streamingShellContent + content_chunk rendering).
- Archive: binaries + BUILD-INFO + SHA256SUMS at `~/.local/share/kiro-research/binaries/2.19.1/`; `tui-bundles/kiro-tui-2.19.1.js`.
