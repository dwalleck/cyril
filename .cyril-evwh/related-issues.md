# Prior art / related issues — KAS-1 (cyril-evwh)

Searched rivets (KAS track) + ROADMAP. Findings:

- **cyril-atjw** (KAS-0, ✓ closed, PR #29 just merged) — the seam KAS-1 hangs
  off: `AgentEngine{V2,Kas}` enum, `engine_for()` gate (Kas → "not available
  yet"), `Engine` trait, `kas` cargo feature (empty), `--agent-engine` flag +
  `[agent] engine` config. KAS-1 fills `engine_for(Kas)` with a real spawn.
- **cyril-nhzw** (KAS, open, blocked by KAS-1) — the SETTINGS half of the
  `_meta.kiro` (KiroClientMeta) handshake (`AgentSettings`). Explicitly carved
  out of KAS-1. Do NOT pull settings into this spec.
- **cyril-j16p** (KAS-2a, open, depends on KAS-1) — walking-skeleton turn
  (converter arms + turn-end + busy-guard rework). The demo. Consumes whatever
  spawn shape KAS-1 lands. Its acceptance ("select KAS, authenticate, send a
  prompt") is satisfied by EITHER spawn shape.
- **cyril-7bdu** (KAS-5, open) — host I/O fs/terminal responders. Same
  server→client-request plumbing the auth responder would use; possible shared
  responder seam.
- **cyril-3zy4 / cyril-3ald** (KAS-8) — governance/safety gates. Downstream.
- **cyril-nk4o** (KAS-8) — `_kiro/mcp/*`. Downstream.
- **cyril-5et2** (KAS-2b) — context-usage breakdown. Downstream.

No prior ticket re-litigates KAS-1's spawn-shape decision — it is open and is
the first thing this interrogation must pin.

## Key source docs (already-researched, not to be re-probed)
- `docs/kiro-2.8.1-wire-audit.md` § "KAS runtime behavior — live capture
  (2026-06-21)": the 5-tier auth-provider priority + two-spawn-shape contract +
  fs three-mode gating. Authoritative for auth-mode behavior.
- `docs/ROADMAP.md` KAS-1 (~line 191) + Open Tension #7.
- `docs/kiro-kas-acp-covenant.md` — KiroClientMeta handshake, getAccessToken
  type contract.
- memory `reference_kiro_kas_launch_contract` — `node --experimental-wasm-modules
  acp-server.js --transport=stdio|ws`; @kiro/agent not on npm.
