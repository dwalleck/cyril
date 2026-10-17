---
name: kiro-release-audit
description: >-
  Audit a kiro-cli release against cyril: acquire and carve the binaries, diff the KAS (v3) bundle,
  Rust host and TUI bundle, run paired live captures, fold into docs/kiro-<ver>-wire-audit.md, file
  rivets issues, update memory. Use when the user asks whether a kiro-cli version has been assessed,
  to audit a new or upgraded kiro-cli, what changed in a release, or to resume an audit that stopped
  (usage limit, lost context). Trigger on "kiro-cli <version>", "release audit", "wire audit",
  "update audit", "has <ver> been assessed", "KAS <0.66.x>".
---

# Kiro release audit

You orchestrate; lanes investigate. **OLD** is the last audited version (newest
`docs/kiro-*-wire-audit.md`), **NEW** the target release; intermediate releases fold into NEW's audit.
Every byte a lane produces lands in the repo working tree under
`experiments/conductor-spike/audit-<NEW>/` so a run cut off mid-lane resumes from disk (the
scratchpad vanishes on account or session changes; the 2.29.1 audit lost all five lane briefs that way).

Read [`references/traps.md`](references/traps.md) before step 1 and hand it to every lane: each
trap there once produced a wrong finding.

## 0. Locate

Run `scripts/audit-state.sh`. It prints the installed version, the archive, OLD, the latest
manifest version, the per-version changelog (`feed.json`) for every release after OLD, the
`@kiro/agent` version per carve, existing `kiro-<ver>` rivets labels, and `RUN-STATE.md` if an audit
is in flight.

- `RUN-STATE.md` present: resume at its first unchecked item. A lane whose `FINDINGS.md` has every
  section is done; a lane with a partial file restarts from that file.
- Absent: create `experiments/conductor-spike/audit-<NEW>/` and `RUN-STATE.md` from the template in
  [`references/fold-and-file.md`](references/fold-and-file.md).

Done when the release table (version, build date, KAS version, changelog headline) covers every
release after OLD and `RUN-STATE.md` exists.

## 1. Acquire and carve

For each release after OLD: `scripts/acquire.sh <ver>`, then `scripts/carve-frames.py` on its
`kiro-cli-chat`. Then collapse by **byte identity**: `diff -rq` the KAS carves, `cmp` the TUI
bundles and the three host binaries across all releases, and against the installed binaries and
the self-extracted KAS tree. Procedure and the archive layout:
[`references/acquire-and-carve.md`](references/acquire-and-carve.md).

Done when `RUN-STATE.md` records, per release, the tarball sha256, `BUILD-INFO`, the `@kiro/agent`
version, and its identity class for KAS, TUI and host. Identical artifacts are audited once.

## 2. Lanes

Size the run before spawning. Five parallel lanes exhausted a session limit in about fifteen
minutes on 2026-10-11, producing nothing. Tiers:

| tier | when | shape |
|---|---|---|
| **constrained** (default) | budget unknown or shared | static lanes one at a time, priority order `kas-core`, `kas-workflow`, `tui`, `host`; then `v3-coverage` and the live lane together, the live lane carrying every static lead |
| **full** | the user asks for it and the session is fresh | four static lanes in parallel plus the live lane; `v3-coverage` once the static lanes return; forward leads to the live lane as lanes return |
| **minimal** | a patch release whose KAS and TUI bytes match OLD | host lane, `v3-coverage`, plus one live sweep pair |

The user's stated focus reorders the static lanes and deepens their question lists; the user's
2026-10-11 ask was v3 backend, workflows, unreleased features, which is the default order above.

Each lane is a background agent with the brief template in
[`references/lanes.md`](references/lanes.md): scope, inputs, reusable scripts, the question list,
the `FINDINGS.md` section contract, and the live-lane protocol (one lane owns auth; legs run
serially; `HOME` isolated, `XDG_DATA_HOME` real). Lanes write only inside their own
`audit-<NEW>/<lane>/` directory and append to `FINDINGS.md` as they go. Spot-check each hand-back
(lanes.md, Orchestration) and tick `RUN-STATE.md`.

Done when every lane's `FINDINGS.md` has all contract sections, every delta row carries
STATIC or LIVE and HANDLED, DROPPED or NOT-SENT with a cyril `file:line`,
`experiments/conductor-spike/check-citations.py` passes on every `FINDINGS.md`, `v3-coverage` has
ranked the flip blockers, and the live lane has run `sweep-new-fields.py --diff` over each OLD/NEW
capture pair, `unread-fields.py` exits 0 on the NEW captures (every never-read wire field fixed,
allowlisted with a reason, or filed), and the cyril-on-KAS leg has run (a user-visible verdict per
flip blocker).

## 3. Fold

Write `docs/kiro-<NEW>-wire-audit.md` in the section order given in
[`references/fold-and-file.md`](references/fold-and-file.md), opening with a one-line verdict
(SAFE for cyril, or the regression) and the three findings that matter most. Copy each lane's
`FINDINGS.md` to `audit-<NEW>/<lane>-findings.md`.

Done when every delta row from every lane is either in the document or listed under "dropped"
with its reason, and every changelog line is reconciled to a location or marked not exercised.

## 4. File

Open rivets issues for each "proposed issue" row and add notes to existing issues the finding
touches, following the labelling and note-versus-issue rules in `fold-and-file.md`. Commit
`.rivets/issues.jsonl`.

Done when every proposed row has an issue id or an existing-issue note, and the document's Issues
section lists them.

## 5. Remember and commit

Write `reference_kiro_<NEW>_diff.md` in the memory directory (replacing any
`reference_kiro_<NEW>_pending_audit.md`), add the index line, append the cycle's new traps to the
methodology memory and to `references/traps.md` here, then commit and push the memory repo. Commit
the audit document, evidence directory and scripts on `main` in the primary checkout as a `docs:`
commit.

Done when `git status` shows nothing from the audit uncommitted in either repository and
`RUN-STATE.md` is fully ticked.
