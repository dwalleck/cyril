# Fold, file, remember

## `RUN-STATE.md` template

Lives at `experiments/conductor-spike/audit-<NEW>/RUN-STATE.md`; created in step 0, ticked after
every step and lane. It is the resume point and the lanes' source of established facts.

```
# Audit <NEW> — run state (started <date>)
OLD: <ver>  NEW: <ver>  folds: <intermediate versions>  focus: <user focus>  tier: <constrained|full|minimal>

## Releases
| ver | build (UTC) | BUILD_HASH | tar sha256 | @kiro/agent | KAS class | TUI class | host class | headline |
|---|---|---|---|---|---|---|---|---|

## Established facts
- (byte identity results, installed==archived proofs, carve==self-extract, anything a lane must not re-derive)

## Steps
- [ ] 0 locate   - [ ] 1 acquire+carve   - [ ] 2 lanes   - [ ] 3 fold   - [ ] 4 file   - [ ] 5 remember+commit

## Lanes
| lane | agent | status | FINDINGS sections complete | leads forwarded to live |
|---|---|---|---|---|

## Dropped rows (lane finding -> reason not in the document)
```

## Evidence directory

```
experiments/conductor-spike/audit-<NEW>/
  RUN-STATE.md  feed-<ver>.json  frames-<ver>.json
  <lane>/FINDINGS.md + raw outputs          (lanes write here during step 2)
  <lane>-findings.md                        (step 3 copy, the committed write-up)
experiments/conductor-spike/
  static-<lane>-<topic>-<NEW>.py  probe-<engine>-<topic>-<NEW>.py  carve-frames-<NEW>.py
  kas-<topic>-<NEW>.jsonl  v2-<topic>-<NEW>.jsonl   (redacted, trimmed captures)
```

Raw lane outputs under `<lane>/` may stay untracked if large; the `-findings.md` copies, scripts,
inventories and captures are committed.

## Audit document

`docs/kiro-<NEW>-wire-audit.md`, mirroring the previous audit:

1. Title: `kiro-cli <NEW> wire audit — <headline> (covers <folded versions>)`; date, baseline link,
   scope sentence naming the lanes, **Verdict** line, the three findings that matter most, link to
   the evidence directory.
2. `## 1. Releases and artifacts`: the release table, hash/manifest proof, identity classes, frame
   sizes, changelog source.
3. `## 2. KAS <KAS_OLD> → <KAS_NEW>`: the delta table (item | ver | gate | live | cyril | issue), flags
   added/removed.
4. `## 3. Workflows (v3)`: control plane, auto-wake, approvals, recipe schema, bundled agents.
5. `## 4. v2 (Rust) engine and launcher`: delta table, rollout registry, env, settings, doc manifests.
6. `## 5. What Kiro's own TUI now sends and consumes`.
7. `## 6. Unannounced or dark features spotted`.
8. `## 7. Live results`: per leg, with capture file names; corrections to static rows called out.
9. `## 8. Issues`: new issues table (id | P | title) and the list of issues that received notes.
10. `## 9. Method notes for the next audit`: every trap hit this cycle (also appended to `traps.md`).

Every delta row names the cyril verdict (HANDLED, DROPPED, NOT-SENT) and the issue or `—`.

## Rivets

- Label every issue from the audit `kiro-<v>-<m>-<p>` (dots as dashes, e.g. `kiro-2-29-1`) plus the
  engine label `kas` or `v2`, and `w1` for workflow-track items. Triage label per
  `docs/agents/triage-labels.md` (`ready-for-agent`, `ready-for-human`, `needs-triage`).
- A finding that extends an open issue gets a **note** on that issue, not a new issue. Search
  `.rivets/issues.jsonl` by method name, field name and the previous audit's issue list first.
- Priority: P1 for a live-proven regression on cyril's current path or billed/unsolicited work; P2
  for a dropped field or missing route on the default engine; P3 for opt-in surface; P4 for
  cosmetic or TUI-only parity.
- Group a cycle's coverage work under one epic when there are more than about five related tickets;
  record blocking edges on files other work has open.
- Use the `rivets` CLI (`rivets create`, `rivets note add`, `rivets label add`); claiming an issue
  is a main-line commit. Commit `.rivets/issues.jsonl` after every mutation.

## Memory

Memory directory: `/home/dwalleck/.claude/projects/-home-dwalleck-repos-cyril/memory/` (a git repo
with a private remote; commit **and push**).

- `reference_kiro_<NEW>_diff.md` (frontmatter `type: reference`): releases covered, KAS versions,
  the findings that matter for cyril, corrections to static claims, links to the audit doc and
  evidence. Replace any `reference_kiro_<NEW>_pending_audit.md`.
- `MEMORY.md`: one line at the top of "References — kiro release diffs (newest first)":
  `- [<versions>](reference_kiro_<NEW>_diff.md) — <verdict>; <two or three hooks>`; remove the
  pending line.
- `reference_kiro_wire_audit_methodology.md`: an `Addendum <date> — <NEW> audit` with the cycle's
  new traps; mirror them into this skill's `traps.md`.

## Commit

Audit artifacts are main-line docs work: commit on `main` in the primary checkout as
`docs: kiro-cli <NEW> wire audit (<range>)`, with the evidence directory, new scripts and captures
in the same commit, and the rivets changes as `chore(rivets): ...` commits. Feature branches for
fixes come later from the filed issues, each in its own worktree.
