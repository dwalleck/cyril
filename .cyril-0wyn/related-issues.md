# cyril-0wyn — related issues (prior art, step 0)

Tracker searched 2026-07-18 (`jq` over `.rivets/issues.jsonl`, keywords:
clientInfo, client_info, persona, allowlist, agentInfo).

- **cyril-jiyn** (open, KAS-7) — hooks handshake + host responders. Coupled: the
  hooksBlock system-prompt briefing is gated on `clientInfo.name → kiro-ide`,
  while jiyn's machinery is gated on `_meta.kiro.hooks`. Cross-linked 2026-07-18
  (related dep). The name decision and KAS-7 must land coherently.
- **cyril-evwh** (closed, KAS-1) — engine selection + getAccessToken responder.
  This issue is the *other* KAS-1 handshake knob; evwh's probe infra
  (KAS spawn incantation) is reusable here.
- **cyril-nhzw** (closed) — `_meta.kiro.settings` handshake. Same initialize
  envelope; settings marshalling is orthogonal to the name but shares the
  handshake-assembly code path.
- **cyril-6iek** (closed) — engine fingerprinting at handshake (bridge.rs
  init_mismatch). Any name change must not confuse the fingerprint check.
- No prior issue proposes or decides a clientInfo.name value. No duplicate.

Sources of the claims under probe: docs/kiro-2.13.0-wire-audit.md §clientInfo;
cyril-0wyn description + TRIAGE 2026-07-18 addendum (hooksBlock carve).
