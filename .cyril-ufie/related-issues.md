# Prior art for cyril-ufie (KAS-5b — terminal host-callback responders)

Searched rivets `kas` label + keyword `terminal|host|fs` (≤5 min).

## Direct dependency / seam
- **cyril-7bdu** (KAS-5a, fs half) — DONE. Built the HostIo seam this issue reuses.
  Artifacts in `.cyril-7bdu/`: PROVE-IT.md, falsifiable-design.md, budgeted-plan.md,
  host_callbacks_2.10.0.json (genuine KAS wire), fixtures/, initialize_result_2.10.0.json.
  KAS-5a prove-it already captured the **terminal** request shapes (same turn).
- **cyril-g9vt** (P3) — KAS host-io loop-mediation seam (ADR-0004 gate/transform point for fs+terminal).

## Terminal-relevant follow-ups already filed (don't re-file)
- cyril-8tq6 (P3) — translate WSL-internal paths for Windows host (applies to terminal cwd too)
- cyril-0v42 (P3) — atomic write_text_file (fs-only)
- cyril-ihj1 (P4) — bounded read (fs-only)
- cyril-mdbp / cyril-1116 — KAS context bar (unrelated)

## Key facts inherited from cyril-7bdu PROVE-IT (2.10.0, no drift):
- terminal/create {sessionId, command, args, cwd} → reply {terminalId}; cwd ABSOLUTE; permission-gated
- terminal/output {sessionId, terminalId} → reply {output, truncated, exitStatus:{exitCode}}
- terminal/wait_for_exit {sessionId, terminalId} → reply {exitStatus:{exitCode, signal}}
- terminal/release {sessionId, terminalId} → reply {}
- terminal/kill — not observed (build defensively per ACP)
- _kiro/terminal/shell_type {sessionId} → reply {shellType}
- ADR-0004 invariant: bridge loop must NOT await resolution; slow exec spawns OFF-LOOP.
