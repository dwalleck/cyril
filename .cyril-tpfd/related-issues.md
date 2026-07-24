# cyril-tpfd — related issues (prove-it-prototype step 0)

Searched rivets (`rivets list` + greps for sessionstart/precomputed/hook)
2026-07-23.

- **cyril-jiyn** (closed, PR #62) — parent. Shipped the hooks host with
  `respond_session_start` as an acknowledge-only `{results: []}` stub;
  this issue is that deferral. The host-arm A/B capture
  (`.cyril-jiyn/ab-results-host/result.json`) already records the live
  request shape: `_kiro/hooks/sessionStart {trigger: "sessionStart",
  sessionId}` fires under `{enabled: true}` on 2.13.0; the v2 arm drives
  none (winner-take-all).
- **cyril-qr6l** (open, discovered-from jiyn) — executeHook echo-integrity
  hardening. Adjacent (same responder file), not overlapping: sessionStart
  has no echo to verify (cyril fabricates the results itself).
- **cyril-n03f** (open) — agent-type hook actions in host mode. Overlaps
  conceptually: the carved producer shows SessionStart `askAgent` hooks
  become `originalType: "askAgent"` precomputed results — i.e. for
  SessionStart specifically, agent-type actions ARE deliverable via this
  issue's mechanism (appendix → content). n03f stays scoped to the other
  triggers.
- **cyril-2adk** (open) — registry hot-reload; orthogonal (load-time only
  here).
- **cyril-oiyt** (open) — hooks panel UI; downstream consumer of executor
  events, not touched by this issue.
- **cyril-mfkg** (open) — covenant doc re-sync; the carved
  `AcpPrecomputedHookResult` shape belongs in that sync (the curated doc
  names the type at §1a but never defines its fields).

No prior issue defines or verifies the `AcpPrecomputedHookResult` element
shape — that is this issue's blocker to resolve.
