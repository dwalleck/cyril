# `docs/` — Index and Currency Tracker

This index catalogs every document in `docs/` along with its **currency status** against the current production Kiro release. The goal is reproducibility: anyone reading a doc should be able to tell at a glance whether it reflects current wire reality or is a historical artifact.

**Current Kiro production version (as of 2026-05-23):** `2.4.1`

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

This index is the source of truth for "is doc X current?" — if a doc's status is ✅ but the verified version is older than the current production version, it needs a re-verification pass.

## Catalog

### Protocol reference

| Doc | Status | Last verified | Purpose |
|---|---|---|---|
| [`kiro-acp-protocol.md`](kiro-acp-protocol.md) | ✅ Current | 2.4.1 (2026-05-23) | Canonical ACP wire reference. Sections 1–10 derived from tui.js 2.0.1 with line citations; § 11 documents changes through 2.4.1. The single source of truth for wire shape. |
| [`cyril-acp-coverage-vs-2.4.1.md`](cyril-acp-coverage-vs-2.4.1.md) | ✅ Current | 2.4.1 (2026-05-21) | Diff between what tui.js 2.4.1 handles and what cyril dispatches. Lists prioritized Tier 1/2/3 gaps. Rename without the version suffix when 2.5.x lands. |

### Per-release wire audits (historical)

| Doc | Status | Release | Findings summary |
|---|---|---|---|
| [`kiro-2.3.0-wire-audit.md`](kiro-2.3.0-wire-audit.md) | 📜 Historical | 2.3.0 (2026-05-11) | Same-day binary-isolated diff vs 2.2.2: dormant `_kiro.dev/*` method additions (`mcp/governance_disabled`, `settings/list`), `/stats` schema with null token values, KAS engine scaffolding. |
| [`kiro-2.4.1-wire-audit.md`](kiro-2.4.1-wire-audit.md) | 📜 Historical | 2.4.1 (2026-05-21) | Same-day binary-isolated diff vs 2.3.0: `/effort` + `/rewind` slash commands, `_meta.trustOptions[]` on permission requests, model-conditional `effort` field on metadata, `rawOutput.items[].Text\|Json` tagged-union variants, `Summarizing` tool_call subagent-result mechanism, KIRO_ACP_RECORD_PATH built-in recorder. |

When a new release ships, add a `kiro-X.Y.Z-wire-audit.md` here; do NOT amend prior audits.

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
