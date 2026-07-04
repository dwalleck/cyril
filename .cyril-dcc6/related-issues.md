# cyril-dcc6 — tracker prior art (prove-it-prototype step 0)

Searched rivets (all statuses) for discovery/auth/token/spawn/reap keywords, 2026-07-03.

| Issue | Status | Relevance |
|---|---|---|
| cyril-evwh | closed | KAS-1: built the existing `_kiro/auth/getAccessToken` responder (wrapper mode) + free-path discovery — the exact code dcc6 changes. Its responder reads the default SSO file; dcc6 repoints it at the sqlite store. |
| cyril-tbsk | closed | Explored `--auth=machine` / `KIRO_API_KEY` bypass — alternative auth tiers if callback path regresses. |
| cyril-taba | open P2 | "Auto-refresh kiro token file on stale before getAccessToken reply (wrapper mode)" — same problem, file-centric fix. The sqlite-backed responder likely **subsumes** it; resolve or fold when dcc6 lands. |
| cyril-0pms | open P3 | Child reaping for spawned KAS/kiro-cli — same spawn-lifecycle seam; bundle into dcc6's slice work if cheap. |
| cyril-0gke | open P3 | AgentProcess stderr never drained — touches the same spawn path. |
| cyril-6iek | open P2 | Engine identity fingerprinting at handshake — adjacent, not blocking. |
| cyril-l7tw | open P2 | Bridge error invisibility — the reason dcc6's "fast-error turn" looked like a silent instant end_turn. Fix stays separate. |

Key inheritance: `discovery.rs` (cyril-evwh) already has the pure `resolve()` + injected-`exists` test pattern; the versioned-dir fix extends it rather than replacing it.
