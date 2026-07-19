# cyril-jiyn — related issues + prior probe art (step 0)

Tracker searched 2026-07-19 (`jq` over `.rivets/issues.jsonl`, keyword: hook).

- **cyril-497j** (open, KAS-8) — hookConfirm modeling + Stop-hook confirm
  dialogs; its probe `probe-kas-hook-confirm-2.13.0.py` (all 6 verdicts PASS,
  commit ee1ce47) discovered the `v2:true` standalone-loader key this issue
  must decide on. Rendering side of the same feature; dependent link exists.
- **cyril-0wyn** (closed, PR #61) — hooksBlock system-prompt briefing is
  keyed on clientInfo.name→kiro-ide; cyril's honest name lands kiro-ide, so
  the model IS briefed (with IDE-flavored UI pointers). ADR-0006.
- **cyril-dn91** (open) — host callbacks gated by cargo feature vs bound
  engine; any new hooks responders must key off the bound engine per
  ADR-0001/0002 discipline (same trap the 0wyn advisory fenced).
- **cyril-g9vt** (open) — ADR-0004 loop-mediation seam for fs+terminal
  host-io; hooks responders share the host-executor layer (KAS-5) and should
  ride the same seam, not bypass it.
- **cyril-ctnv** (open) — upstream ask incl. keying hooks off capabilities.
- **cyril-mfkg** (open) — covenant re-sync; the decided hooks default must
  land in the covenant/audit docs per this issue's AC.

Prior probes (experiments/conductor-spike/):
- `probe-kas-hooks-host-2.7.1.py` — fired the full host path end-to-end
  2026-06-16 (list + executeHook + exit-2 preToolUse block).
- `probe-kas-hooks-enabled-2.7.1.py`, `probe-kas-hooks-usage-2.7.1.py` —
  handshake/usage variants.
- `probe-kas-hook-confirm-2.13.0.py` + `kas-hook-confirm-2.13.0.json` —
  2.13.0 v2-loader + hookConfirm capture (the ZERO-host-callbacks finding).

No duplicate of this implementation ticket. The open probe question is the
issue's own: enabled-only vs enabled+v2 A/B on 2.13.0 — do the two hook
models compose?
