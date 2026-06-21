# `docs/` — Index and Currency Tracker

This index catalogs every document in `docs/` along with its **currency status** against the current production Kiro release. The goal is reproducibility: anyone reading a doc should be able to tell at a glance whether it reflects current wire reality or is a historical artifact.

**Current Kiro production version (as of 2026-06-21):** `2.8.1`

**Two engines, two canons.** Kiro ships two ACP engines (see [`ROADMAP.md`](ROADMAP.md) "KAS engine integration track"):
- **v2 (default Rust engine)** — canonical wire reference is [`kiro-acp-protocol.md`](kiro-acp-protocol.md) (§ 11 changelog). The v2 *exercised* wire surface has been **frozen-stable since 2.4.1** (per-release audits 2.5.0 → 2.8.1 all report zero v2 wire delta), so that doc remains current for v2 despite its 2.4.1 "last verified" line.
- **KAS (`--agent-engine v3`/`kas`)** — canonical is a **two-doc pair**: [`kiro-kas-acp-covenant.md`](kiro-kas-acp-covenant.md) for the `_kiro/*` **type contract** (authoritative — extracted from the `@kiro/acp-type-covenant` package; "where it and an audit differ, the covenant wins") + [`kiro-2.7.1-wire-audit.md`](kiro-2.7.1-wire-audit.md) for **live behavior + cyril-impact strategy**. Runtime behavior the covenant can't hold (server `--auth` modes, fs-callback gating) lives in the **latest** per-release audit — currently [`kiro-2.8.1-wire-audit.md` § KAS runtime behavior](kiro-2.8.1-wire-audit.md#kas-runtime-behavior--live-capture-2026-06-21-addendum). KAS is where the wire is actively evolving; track the embedded `@kiro/agent` version every release.

## Status legend

| Status | Meaning |
|---|---|
| ✅ **Current** | Verified against the current Kiro production version. Refresh after each kiro-cli release that ships ACP changes. |
| 📜 **Historical** | Release-specific snapshot. Accurate for its dated release; not a current-state reference. Keep as provenance. |
| 🧪 **Research artifact** | Extracted data (captures, embedded prompts, schemas). Tied to a specific binary version; preserved for reproducibility, not maintained. |
| ⚠ **Stale** | Out of date and likely misleading. Should be updated or removed. |
| 📋 **Design / planning** | Forward-looking proposal. Not a reference of current state. |

## How to update currency status

After each Kiro release:

1. Run a fresh wire capture (see `experiments/conductor-spike/` and the `KIRO_ACP_RECORD_PATH` env var documented in `kiro-acp-protocol.md` § 11.6).
2. Compare against `kiro-acp-protocol.md` § 11 ("Changes since 2.0.1") for known additions; section-by-section if you're auditing.
3. If new wire changes appear, write a per-release `kiro-X.Y.Z-wire-audit.md`, then mirror the deltas into a new sub-section of § 11 in the canonical protocol doc.
4. Update the "Current Kiro production version" line above and bump the "Last verified" header on each affected doc.
5. Audit docs from prior releases stay as 📜 historical — don't rewrite them in place.
6. **KAS changes** don't go into the v2 protocol doc. Type-shape changes → update [`kiro-kas-acp-covenant.md`](kiro-kas-acp-covenant.md) (re-extract the covenant package after `acp --agent-engine v3` self-extracts the new bundle). Runtime/gating behavior (auth modes, callback negotiation, new `_kiro/*` that fired live) → the new per-release `kiro-X.Y.Z-wire-audit.md`, in a dated "KAS runtime behavior" section. The ROADMAP should *link* these, not restate them.

This index is the source of truth for "is doc X current?" — if a doc's status is ✅ but the verified version is older than the current production version, it needs a re-verification pass.

## Catalog

### Protocol reference

| Doc | Status | Last verified | Purpose |
|---|---|---|---|
| [`kiro-acp-protocol.md`](kiro-acp-protocol.md) | ✅ Current (v2) | 2.4.1; v2 wire unchanged through 2.8.1 | Canonical **v2-engine** ACP wire reference. Sections 1–10 derived from tui.js 2.0.1 with line citations; § 11 documents changes through 2.4.1. Single source of truth for the **v2** wire shape (frozen-stable since 2.4.1 — confirmed by per-release audits). |
| [`cyril-acp-coverage-vs-2.4.1.md`](cyril-acp-coverage-vs-2.4.1.md) | ✅ Current | 2.4.1 (2026-05-21) | Diff between what tui.js 2.4.1 handles and what cyril dispatches. Lists prioritized Tier 1/2/3 gaps. Rename without the version suffix when 2.5.x lands. |

### KAS engine reference (canonical for `--agent-engine v3`/`kas`)

| Doc | Status | Last verified | Purpose |
|---|---|---|---|
| [`kiro-kas-acp-covenant.md`](kiro-kas-acp-covenant.md) | ✅ Current (KAS) | 0.3.257 (2026-06-18) | **Authoritative `_kiro/*` type contract**, extracted from the `@kiro/acp-type-covenant` package. Method catalog, `KiroClientMeta` handshake, `AgentSettings`, Trust-v2, host-callback signatures. Where it and an audit differ, **the covenant wins**. |
| [`kiro-2.7.1-wire-audit.md`](kiro-2.7.1-wire-audit.md) | ✅ Current (KAS) | 2.7.1 (2026-06-16) | The big KAS **live-behavior + cyril-impact** companion (the KAS landing). Strategic narrative + host-callback map + spec workflow; defers to the covenant on exact shapes. The reference most ROADMAP KAS milestones cite. |
| [`kiro-subagent-tool-schemas.md`](kiro-subagent-tool-schemas.md) | ✅ Current | v1/v2/kas (2026-06-19) | Full JSON schemas for Kiro's subagent tools across all three engines (`agent_crew` v2 vs `orchestrate_subagent`/`agent-subtask` KAS). Source for KAS-3 rendering. |

### Per-release wire audits (historical, newest first)

| Doc | Status | Release | Findings summary |
|---|---|---|---|
| [`kiro-2.8.1-wire-audit.md`](kiro-2.8.1-wire-audit.md) | 📜 Historical (but holds latest KAS runtime behavior) | 2.8.1 (2026-06-18 + 2026-06-21 addendum) | v2 SAFE/unchanged (proven 3 ways). KAS bundle `@kiro/agent` 0.3.234→0.3.257: new `_kiro/sessions/changed` (multi-client observer roster CDC), `_kiro/hooks/setEnabled`. **§ KAS runtime behavior (2026-06-21)** holds the live auth free-path + three-mode fs gating (the facts the covenant can't carry). |
| [`kiro-2.8.0-wire-audit.md`](kiro-2.8.0-wire-audit.md) | 📜 Historical | 2.8.0 (2026-06-17) | v2 unchanged; V3/KAS promoted to advertised `--v3`; engine flag renamed `kas`→`v3`. New KAS `_kiro/safety/*` Infrastructure Safety gate. |
| [`kiro-2.7.1-wire-audit.md`](kiro-2.7.1-wire-audit.md) | — see KAS reference above | 2.7.1 (2026-06-16) | The KAS landing (assets embedded, self-extracting). Elevated to KAS canon — listed under "KAS engine reference". |
| [`kiro-2.7.0-wire-audit.md`](kiro-2.7.0-wire-audit.md) | 📜 Historical | 2.7.0 (2026-06-12) | Queue steering (`_session/steer[/clear]`, three `steering_*` variants), `/goal` cmd + `goal` tool; tui.js `_kiro.dev/*`→`_kiro/*` migration complete. Wire reference for ROADMAP K1. |
| [`kiro-2.6.1-wire-audit.md`](kiro-2.6.1-wire-audit.md) | 📜 Historical | 2.6.0/2.6.1 (2026-06-09) | Zero cyril-path delta. Dormant `_kiro/auth/getAccessToken` (KAS auth), `evaluate_url_permission`, MCP registry, voice/LSP (tui.js only). |
| [`kiro-2.5.0-wire-audit.md`](kiro-2.5.0-wire-audit.md) | 📜 Historical | 2.5.0 (2026-05-28) | Thinking crosses ACP as `agent_thought_chunk` (Opus+effort precondition); subagent review-loop fields on `list_update`; `--token-path` auth plumbing. |
| [`kiro-2.4.1-wire-audit.md`](kiro-2.4.1-wire-audit.md) | 📜 Historical | 2.4.1 (2026-05-21) | `/effort` + `/rewind`, `_meta.trustOptions[]`, model-conditional `effort` on metadata, `rawOutput.items[].Text\|Json`, `Summarizing` mechanism, KIRO_ACP_RECORD_PATH recorder. |
| [`kiro-2.3.0-wire-audit.md`](kiro-2.3.0-wire-audit.md) | 📜 Historical | 2.3.0 (2026-05-11) | Dormant `_kiro.dev/*` additions (`mcp/governance_disabled`, `settings/list`), `/stats` null tokens, KAS engine scaffolding. |

When a new release ships, add a `kiro-X.Y.Z-wire-audit.md` here; do NOT amend prior audits. If it carries KAS runtime/gating findings, add them in a dated "KAS runtime behavior" section of that audit (per step 6 above).

### Research artifacts

Provenance for prior reverse-engineering work. Tied to specific binary versions; not refreshed.

| Doc | Status | Version | Purpose |
|---|---|---|---|
| [`kiro-ide-agent-extension.md`](kiro-ide-agent-extension.md) | 🧪 Research | IDE 0.12.155 | Analysis of the Kiro IDE's bundled `@kiro/agent` extension. Companion to 2.3.0 CLI audit. Establishes that the IDE-side agent library predates the CLI's KAS scaffolding. |
| `kiro-embedded-agent-{1,2,3}.md` | 🧪 Research | 2.0.x | Embedded system prompts extracted from kiro-cli-chat binaries. |
| `kiro-embedded/*.md` | 🧪 Research | 2.0.x | More extracted embedded prompts (default agent, planner, knowledge-system). |
| `kiro-acp-capture-2.1.0.json` | 🧪 Research | 2.1.0 | Captured JSON-RPC frames. |
| `kiro-agent-schema-2.1.1.json` | 🧪 Research | 2.1.1 | Extracted agent schema. |
| `kiro-changelog-*.{json,txt}` | 🧪 Research | various | Extracted changelogs (use `kiro-cli version --changelog=all` to regenerate). |
| `kiro-docs-index-2.8.1*.{md,json}` | 🧪 Research | 2.8.1 | Extracted embedded doc-manifest index from `kiro-cli-chat` (two manifests in 2.8.1; baseline + extractor output). Diff `documents[]` per release to catch unannounced features (per CLAUDE.md doc-manifest addendum). |

### Design proposals

| Doc | Status | Purpose |
|---|---|---|
| [`workflow-engine-design.md`](workflow-engine-design.md) | 📋 Design | Workflow engine inspired by Pi's `context-workflow` extension. Has an inline "Empirical corrections (2026-05-23)" section reflecting wire-probe findings — read both the main design and the corrections section together. |

### Project direction

| Doc | Status | Purpose |
|---|---|---|
| [`ROADMAP.md`](ROADMAP.md) | ✅ Current | Phased path from current Kiro-focused implementation toward vendor-neutral platform status. New non-trivial work should land in a numbered phase. |
| [`plans/`](plans/) | 📜 Historical | Dated implementation plans for completed and in-flight work. Each is a snapshot from its date — references to wire shapes may be stale. See `plans/README.md` for context. |

## Verifying this index

Run from the repo root:

```sh
# Every top-level docs/ file should appear in the catalog (or be covered by a
# glob entry like `kiro-embedded-*.md` or `kiro-changelog-*.{json,txt}`):
ls docs/*.md docs/*.json docs/*.txt 2>/dev/null | xargs -n1 basename | sort
```

Cross-check the listing against the catalog above. Any file that exists but isn't represented (by name OR by glob pattern) is either undocumented (add a row) or stale (delete it). New protocol-touching files should get a row before merging; new historical artifacts can use the existing glob entries.
