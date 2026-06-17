# Related issues (prior-art scan)

Scanned rivets + ROADMAP. Time-boxed.

- **cyril-f2g8** — K1a — Queue-steering wire + state plumbing. **This spec's subject.**
- **cyril-bm1j** — K1b — Queue-steering TUI UX. Direct dependent. Consumes everything K1a builds (bridge commands, notification variants, converter arms, state fields). Defines the actual user-facing UX (Enter-while-busy, `/steer`, toolbar chip, local echo). **Everything UX is K1b, not K1a.**
- **cyril-atjw** — KAS-0 — Engine trait + v2 port. Dependent. Will later absorb K1a's three new `convert/kiro.rs` arms into the `V2Engine` impl at negligible cost (ROADMAP sequencing note, 2026-06-16). K1a ships *before* KAS-0.
- ROADMAP **K1** track (docs/ROADMAP.md:90-119) — full milestone breakdown K1a/K1b/K1c + non-goals.
- Wire reference: **docs/kiro-2.7.0-wire-audit.md** (esp. lines 9-31, 61) — the authoritative-but-internally-contradictory source on the steering wire shape.
- Probe: **experiments/conductor-spike/probe-steer-goal-2.7.0.py** — captured the original semantics; log no longer on disk.
- Memory: `project_cyril_steering_roadmap.md`, `reference_kiro_2_7_0_diff.md`.

No prior decision is being re-litigated. K1a is greenfield plumbing for an already-decided track.
