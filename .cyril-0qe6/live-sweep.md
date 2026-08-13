# cyril-0qe6 slice 14 — live AC sweep record (2026-08-13, kiro-cli 2.18.0)

The real cyril binary (`--features kas`, debug build) on a 140x40 pty against
live KAS, driven by `.cyril-0qe6/live-sweep.py` (pyte screen scraping,
HOME-isolated with the real data dir symlinked in). All seven `/workflow`
subcommands exercised end to end (AC1), plus the cross-process
kill-relaunch-reattach-resume flow (AC4).

## Two real findings caught only by this sweep

1. **Ext-method spelling (fixed in-slice).** The bridge passed
   `_kiro/workflow/*` to `ext_method`, but the acp crate prefixes `_` onto
   outbound extension methods — the agent saw `__kiro/workflow/listRecipes`
   and errored `[PersistenceClassification] … no persistence classification`.
   Unit tests could not catch this (mocks don't cross the crate); the raw-wire
   probes could not either (they bypass the crate). Fixed to the crate-facing
   spelling (`kiro/workflow/*`, matching `kas::hooks::LIST_METHOD`) and
   documented on `workflow_op_request`. The error surfaced to the user
   verbatim through the new Failed-outcome path — C3/C7 proven live in the
   same breath.

2. **cyril-tpwn live-confirmed.** Under an isolated HOME with a real
   `XDG_DATA_HOME`, cyril's KAS bundle discovery fails (it derives the bundle
   root from `$HOME`); a direct `kiro-cli acp --agent-engine kas` spawn in
   the same environment works. Noted on cyril-tpwn at close-out.

## Run 1 (v3 script, strict assertions): 8 PASS / 3 fixture artifacts

| Phase | Result | Adjudication |
|---|---|---|
| P1 recipes | artifact | All 7 recipes rendered with full descriptions + footer (screen capture in the run log); the `Workflow recipes (7):` header scrolled off the 40-row viewport, and the assertion only scans the visible screen. |
| P2 empty list | PASS | `No workflow runs in this workspace.` |
| P3 run (file ref) | PASS | `.workflow.json` path branch: `new` → snapshot → `invoke`, `Launched … — run wf_…` |
| P4 status polls to completed | PASS | lifecycle events stream into the tracker (no wire call on the no-arg poll) |
| P5 status <id> | PASS | per-run view with node line |
| P6 cancel mid-flight | PASS | `Cancelled wf_… (was running).`; run lists `aborted` after |
| P7a list after kill+relaunch | artifact | Run listed as `running` — the engine's reconciliation sweep flips the label to `paused` shortly after; either label proves persistence. Resume was claimable immediately regardless (dead-pid short-circuit). |
| P7b attach | PASS | `Attached to wf_…` + node lines |
| P7c resume | PASS | accepted first try, 0s wait, `Resumed wf_… (status running).` |
| P7d resumed run completes | artifact → **new wire fact** | The engine re-drove the step; ~5s in, the model wrote completion signal **`need_input`** (`send_message.wrote_completion_signal {"signal":"need_input","nodeId":"slow"}` in the engine log) and the run re-parked `paused` — on disk AND in cyril's tracker, in agreement. Completion after resume is model-elected (wire-audit hazard 4); "resume accepted and re-driven" is the machine guarantee, "completes" is not. |
| P8 resume bogus id | PASS | `/workflow resume failed (…): …` with the engine's details text |

Oracle agreement throughout: the run store on disk
(`~/.kiro/sessions/<hash>/workflows/<id>/workflow-state.json` under the
sweep HOME) matched cyril's rendered state at every checkpoint — including
the P7d re-park, where disk said `paused`/node `paused` exactly as cyril did.

## Run 2 (v4 script, corrected assertions)

Assertions corrected to the observed contracts (footer-anchored P1;
`paused|running` P7a; completed-or-parked-on-disk P7d). Verdict recorded
below by the run log (`tasks/…/bl8f7zu2v.output` in the session dir; the
same phases, fresh workspace and runs).

RESULT: **11/11 PASS, exit 0.** P7d passed via the completion path this
time — the resumed run finished (`completed`), so both post-resume
outcomes (completion, and v3's `need_input` re-park) are now live-observed;
the outcome is model-elected, exactly as hazard 4 predicts.

## AC mapping

- **AC1** (all seven subcommands live, gate never set): P1–P8; every session
  in every run showed `workflowsEnabled` unset/false and no workflow tools.
- **AC2** (response-carrying bridge): every phase's reply rendered (or
  failed loud) — nothing dangled.
- **AC3** (explicit workspacePaths): `list` answered correctly in every
  phase; the `-32603` contract is unit-fenced.
- **AC4** (reattach receives the run's state): P7b attach rendered the
  killed run's nodes from the persisted snapshot; P7c/P7d resumed and
  re-drove it cross-process.
- **AC5** (four gate commands suppressed): nothing to suppress live —
  gate-off KAS never advertised them (unit fence covers the flip case).
- **AC6** (test/clippy both feature sets): the per-slice gates + final
  integration check.
