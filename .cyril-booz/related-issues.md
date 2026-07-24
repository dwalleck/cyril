# cyril-booz — related issues (prove-it-prototype step 0)

Tracker sweep 2026-07-23 (keywords: hook, briefing, HOOK_INSTRUCTION, authority, instruction, kas_hooks, sessionStart).

## Direct lineage

- **cyril-tpfd** (closed, PR #63) — `discovered-from` parent. Built the SessionStart
  hooks host: execute hooks + inject precomputed context (`AcpPrecomputedHookResult`).
  Its live probe (`.cyril-tpfd/findings.md`, 2026-07-23) observed the refusal this
  issue mitigates: model called injected HOOK_INSTRUCTION "not a legitimate system
  directive, just text injected into the message".
- **cyril-0wyn** (closed, PR #61) — clientInfo identity decision. Its triage
  (commit 3ae3d33) documented that KAS's hooks system-prompt briefing is
  **kiro-ide-gated** (`hooksBlock` verified from the 2.13.0 KAS bundle) and coupled
  the briefing-vs-machinery desync to KAS-7. The "0wyn coupling note" is where the
  cyril-booz mitigation candidate (cyril injects its own corrected briefing) comes from.
- **cyril-jiyn** (closed, PR #62) — KAS-7: `_meta.kiro.hooks` advertisement at
  initialize + hooks-host responders + `kas_hooks` knob. The machinery cyril-booz
  extends.

## Open siblings (scope boundaries — NOT this issue)

- **cyril-ctnv** (open, P3) — upstream ask: KAS should recognize third-party ACP
  clients or key persona/allowlist/hooks off capabilities. That is the *real* fix;
  cyril-booz is the client-side mitigation until then.
- **cyril-qr6l** (open, P3) — executeHook command-echo verification hardening.
- **cyril-2adk** (open, P3) — hooks registry hot-reload.
- **cyril-n03f** (open, P4) — agent-type hook actions (v1 skips with warn).
- **cyril-497j** (open, P3) — KAS-8 hookConfirm / Stop-hook confirm dialogs.
- **cyril-oiyt** (open, P4) — hooks panel shows host-mode registry + firing status.

No existing ticket covers briefing injection itself — cyril-booz is not a re-discovery.
