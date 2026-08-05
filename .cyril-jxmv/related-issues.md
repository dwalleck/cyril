# cyril-jxmv — prior art (tracker sweep, 2026-08-04)

Query: `rivets list -n 200 | grep -iE 'path|wsl|windows|translat|native'`

## Directly related

- **cyril-8tq6** (closed, P3) — KAS host-io: translate WSL-internal paths
  (`/home`, `/tmp`) for a Windows host. Built the current `\\wsl$` UNC
  machinery, the `CYRIL_WSL_DISTRO` / cwd-derived `OnceLock` distro
  resolution, and `tests/win_wsl_wiring.rs`. cyril-jxmv gates that whole
  layer behind an agent-location check; the 8tq6 semantics must be
  preserved *when the agent is WSL-hosted* (AC 2).
- **cyril-duz0** (open, P3, docs) — Docs claim Windows spawns
  `wsl kiro-cli acp`; code spawns `kiro-cli acp` natively. The
  doc-misdirection twin of this bug: same root fact (default agent command
  is native everywhere). AC 5 here coordinates: CLAUDE.md path-translation
  sections must describe agent-location conditionality. duz0 stays open
  for its own surfaces (transport.rs comment, README).

## Adjacent Windows/KAS host-io family (open, not gating)

- **cyril-f2fv** (P4) — verify terminal/create with a translated `wsl$`
  UNC cwd. Exercises the WSL side of the boundary this issue gates.
- **cyril-trkw** (P4) — auto-detect the default WSL distro when none
  configured. Extends distro resolution; orthogonal to the gate.
- **cyril-lwpm** (P4) — KAS auth on Windows host: sqlite store inside WSL.
  Another "which side does the agent live on" consumer; the agent-location
  decision made here is the natural signal for it later.
- **cyril-el3x** (P3) — Windows ProcessTree drop path leaks pipeline
  children. Windows spawn-path adjacent only.

## Closed context

- **cyril-6bol** (closed, P2) — KAS terminal shell_type hardcoded "bash",
  wrong for native Windows: prior instance of the same class of bug
  (host-side assumption instead of agent-location awareness).
- **cyril-gu8a**, **cyril-xi4a** (closed) — Windows CI hygiene; explain
  why cfg(windows) code here is CI-verified only.

No existing ticket describes the translation-gate bug itself — cyril-jxmv
is the canonical ticket, no duplicate found.
