# `docs/plans/` — Historical Implementation Plans

> **📜 Historical snapshots.** Each plan here was a point-in-time guide for implementing a specific feature. Many have been completed; some were superseded. Plans reference ACP wire shapes that may be stale relative to current Kiro releases — for current wire shape always consult [`../kiro-acp-protocol.md`](../kiro-acp-protocol.md).

## Provenance

Plans are date-prefixed and **immutable** after their work landed. Do not edit a plan to reflect changed reality — the dated filename is the contract that this is a snapshot.

For new work that needs a plan, create a new dated file alongside (e.g., `2026-06-15-foo-design.md`); reference older plans where relevant.

## Notable ACP-touching plans (potentially stale wire claims)

These plans contain wire-shape claims that may not match the current Kiro release. They were accurate for their dates but should not be used as current references:

| Plan | Date | Topic | Why ACP claims may be stale |
|---|---|---|---|
| `2026-04-02-v129-protocol-updates.md` | 2026-04-02 | Kiro v1.29.0 protocol updates | Wire shape has shifted across 2.0.x, 2.1.x, 2.2.x, 2.3.0, 2.4.x since. |
| `2026-04-09-protocol-parity.md` | 2026-04-09 | Protocol parity with Kiro CLI | Same — multiple Kiro releases since. |
| `2026-04-13-protocol-parity-remaining.md` | 2026-04-13 | Remaining protocol-parity tasks | Same. |
| `2026-04-03-extension-notifications-design.md` | 2026-04-03 | Extension notification handlers | Pre-dates 2.3.0/2.4.x extension additions (`mcp/governance_disabled`, `settings/list`, `_meta.trustOptions[]`, etc.). |
| `2026-04-02-subagent-support-design.md` | 2026-04-02 | Subagent support design | Pre-dates the empirical findings on `_session/spawn` (bare ACP path), `name`-as-label-not-mode, and the `Summarizing` tool_call result-delivery mechanism. |
| `2026-04-02-subagent-implementation.md` | 2026-04-02 | Subagent support implementation | Same. |

Other plans here are cyril-internal architecture work (v2 redesign, code intelligence, code-block rendering, etc.) and don't make ACP wire claims — they're useful as historical context but unaffected by Kiro releases.
