export const meta = {
  name: 'sweep-bugs',
  description: 'Fix a confirmed batch of small rivets bugs in parallel worktrees, then independently verify each PR',
  whenToUse: 'After /sweep-bugs has selected a batch, applied the smallness test, and gotten human confirmation. Pass the confirmed batch as args. Does NOT select, confirm, or merge — those stay with the human.',
  phases: [
    { title: 'Fix', detail: 'one worktree-isolated agent per bug: red first, minimal fix, fence, full gate, PR' },
    { title: 'Verify', detail: 'independent per-PR check: diff scope, fence plausibility, gate re-run' },
  ],
}

// ---------------------------------------------------------------------------
// args: the CONFIRMED batch. Selection + the smallness test + the one mandatory
// pause + `rivets update -s in_progress` all happen in the main loop BEFORE this
// runs — a workflow script cannot ask a human anything.
//
//   args = {
//     baseline: "1123 tests pass, clippy+fmt clean @ <sha>",   // required
//     bugs: [
//       { id, title, branch, scope, locus, criteria, notes? },
//       ...
//     ]
//   }
// ---------------------------------------------------------------------------

const batch = Array.isArray(args?.bugs) ? args.bugs : []
const baseline = args?.baseline ?? 'unknown — re-establish it yourself before trusting any red'

if (!batch.length) {
  log('No bugs passed in args.bugs — nothing to sweep. Expected {baseline, bugs:[{id,title,branch,scope,locus,criteria}]}.')
  return { fixed: [], kickedBack: [], failed: [], note: 'empty batch' }
}
if (batch.length > 4) {
  log(`WARNING: ${batch.length} bugs passed. The skill caps a sweep at 2-4 — more than that and subagents start colliding. Running all ${batch.length} anyway; shrink next time if PRs collide.`)
}

log(`Sweeping ${batch.length} bug(s): ${batch.map(b => b.id).join(', ')}`)

// Constraints every fixing agent needs verbatim — it cannot discover these.
const CONSTRAINTS = `
## Workflow (strict order)
1. Create your branch in this worktree: \`git checkout -b <BRANCH>\`
2. **RED FIRST**: reproduce the bug with a FAILING test BEFORE touching product
   code. Run it and SEE it fail. If you cannot reproduce it, STOP and report
   outcome "cannot-reproduce" with the evidence — do not fix blind.
3. **MINIMAL FIX** at the root cause. The failing test becomes the regression
   fence — name it after the bug CLASS, not the issue id.
4. **IMPACT ANALYSIS**: if you change a function's signature/name/semantics:
   - \`tethys index\` first (add \`--rebuild\` if it reports a stale schema)
   - \`tethys callers "<Type::method|fn>" --lsp\`  — QUALIFIED names only; a bare
     method name errors \`not found: symbol\`.
   - Use \`--lsp\`, NOT \`--exclude-speculative\`: they are mutually exclusive, and
     measured on this repo 2026-08-02 the non-lsp tiers MISS cross-module
     production callers entirely (they returned only test callers for
     \`UiState::show_picker\` and dropped the sole production caller of
     \`parse_options_response\`). Blast radius is a RECALL question.
   - \`rg\` is the authority. If tethys contradicts rg, rg wins.
   - TRAP: if the bug is IN tethys's own resolver/call-edge logic, tethys cannot
     analyze its own change — use rg only.
   - Callers you surface but deliberately do NOT fix go in your report, never
     silently dropped.
5. **FULL GATE with real exit codes** — never pipe, pipes return the LAST stage's
   status and will mask a failure:
   - \`cargo nextest run --workspace > /dev/null 2>&1 && echo NEXTEST-OK\`
   - \`cargo clippy --all-targets -- -D warnings > /dev/null 2>&1 && echo CLIPPY-OK\`
   - \`cargo fmt --check > /dev/null 2>&1 && echo FMT-OK\`
   - \`cargo test --doc --workspace > /dev/null 2>&1 && echo DOCTEST-OK\`
   A MISSING \`*-OK\` echo IS A FAILURE. Verify all four before committing.
6. **COMMIT**: conventional, ONE lowercase scope, NO commas. CI regex is
   \`^(feat|fix|docs|style|refactor|perf|test|build|ci|chore)(\\([a-z][a-z0-9-]*\\))?!?: .{3,}\`
   Subject cites the rivets id, e.g. \`fix(picker): <what> (<ID>)\`.
7. Push the branch and \`gh pr create\`. Body sections: **Repro** (the failing test
   and what it showed), **Cause**, **Fix**, **Fence**. Do NOT use \`Closes\` —
   reference the rivets id in prose (GitHub cannot close rivets ids).

## Hard constraints
- **NEVER touch \`.rivets/\`** — the tracker belongs to the orchestrator. Side
  issues you discover go in your report, not the tracker.
- Code discipline (CLAUDE.md, clippy-enforced workspace-wide): zero \`.unwrap()\`
  in non-test code, zero \`let _ =\` discarded Results, zero \`#[allow(...)]\`, zero
  sentinel values. NEVER weaken a lint to make code compile.
- Crate boundaries: \`cyril-core\` imports no UI crate and no ratatui/crossterm;
  \`cyril-ui\` never imports \`agent-client-protocol\` nor knows about ACP/JSON-RPC;
  only \`protocol/convert/\` imports \`acp::\`.
- **ESCAPE HATCH**: if the fix needs a design decision, a schema change, or edits
  across multiple subsystems, STOP and report outcome "needs-design" with your
  assessment and evidence instead of forcing a "small" fix. A kicked-back bug is
  a GOOD outcome; a disguised feature merged without a design is not.
- Do NOT merge your PR. Merges require human approval.
`

const FIX_SCHEMA = {
  type: 'object',
  required: ['id', 'outcome'],
  properties: {
    id: { type: 'string' },
    outcome: { enum: ['fixed', 'cannot-reproduce', 'needs-design', 'gate-failed'] },
    prUrl: { type: 'string' },
    prNumber: { type: 'integer' },
    branch: { type: 'string' },
    commitSha: { type: 'string' },
    filesTouched: { type: 'array', items: { type: 'string' } },
    fenceTests: { type: 'array', items: { type: 'string' } },
    redEvidence: { type: 'string', description: 'what the failing test showed BEFORE the fix' },
    gates: {
      type: 'object',
      properties: {
        nextest: { type: 'boolean' }, clippy: { type: 'boolean' },
        fmt: { type: 'boolean' }, doctest: { type: 'boolean' },
      },
    },
    callersSurfacedNotFixed: { type: 'array', items: { type: 'string' } },
    discoveredNotFixed: { type: 'array', items: { type: 'string' } },
    escapeHatchReason: { type: 'string' },
  },
}

const VERIFY_SCHEMA = {
  type: 'object',
  required: ['id', 'verdict', 'reasons'],
  properties: {
    id: { type: 'string' },
    verdict: { enum: ['ready-to-merge', 'needs-changes', 'reject'] },
    reasons: { type: 'array', items: { type: 'string' } },
    touchesRivets: { type: 'boolean', description: 'true if the diff touches .rivets/ — an automatic reject' },
    fencePresent: { type: 'boolean' },
    fenceWouldHaveFailedBefore: { type: 'boolean', description: 'judged by reading the fence against the pre-fix code' },
    scopeCreep: { type: 'boolean' },
    gateReRun: { type: 'boolean', description: 'did YOU re-run the gate rather than trusting the report' },
  },
}

const fixPrompt = (b) => `Fix ONE bug in the \`cyril\` Rust workspace (repo dwalleck/cyril). You are in an
isolated git worktree branched from an up-to-date \`main\`.

BASELINE (already verified — any red you see is YOURS): ${baseline}

## The bug: ${b.id} — ${b.title}

${b.locus ? `**Where it lives:**\n${b.locus}\n` : ''}
${b.criteria ? `**Acceptance criteria:**\n${b.criteria}\n` : ''}
${b.notes ? `**Notes:**\n${b.notes}\n` : ''}
**Branch to create:** \`${b.branch}\`
${b.scope ? `**Expected file scope:** ${b.scope} — going meaningfully beyond this is a signal to take the escape hatch.` : ''}

${CONSTRAINTS.replace('<BRANCH>', b.branch)}

Return the structured result. \`outcome\` must be one of: fixed / cannot-reproduce /
needs-design / gate-failed. Set every gate boolean honestly — a gate you did not
run is \`false\`, not \`true\`.`

const verifyPrompt = (fix, b) => `Independently verify a bug-fix PR. You are a SKEPTIC: assume the fixing agent's
self-report is optimistic and check it yourself. Do NOT merge anything.

Bug: ${b.id} — ${b.title}
PR: ${fix.prUrl || '(none reported)'}   branch: ${fix.branch || b.branch}
The fixing agent CLAIMED: outcome=${fix.outcome}, fences=${(fix.fenceTests || []).join(', ') || 'none reported'}

Check, in this order:
1. \`gh pr diff ${fix.prNumber || ''} --name-only\` — does the diff touch \`.rivets/\`?
   That is an AUTOMATIC reject; the tracker belongs to the orchestrator.
2. Read the actual diff. Is there scope creep beyond this one bug? Refactors,
   drive-by renames, unrelated files?
3. Find the named fence test(s) and READ them. Would they genuinely have FAILED
   against the pre-fix code? A fence that passes both before and after is not a
   fence. Reason about it against the diff — do not take the report's word.
4. Re-run the gate YOURSELF in a checkout of that branch:
   \`cargo nextest run --workspace > /dev/null 2>&1 && echo NEXTEST-OK\`
   \`cargo clippy --all-targets -- -D warnings > /dev/null 2>&1 && echo CLIPPY-OK\`
   \`cargo fmt --check > /dev/null 2>&1 && echo FMT-OK\`
   A missing echo is a failure. Never pipe — a pipe returns the last stage's exit
   status and will report success on a failed command.
5. Check code discipline in the diff: no \`.unwrap()\` in non-test code, no
   \`let _ =\` discarded Results, no \`#[allow(...)]\`, no weakened lints.
6. Note any bot review comments on the PR (\`gh api repos/dwalleck/cyril/pulls/<n>/comments\`).

Verdict: ready-to-merge / needs-changes / reject. Give concrete reasons — cite
file:line. Do NOT merge, and do NOT push fixes; report only.`

// Pipeline, not a barrier: each bug's verify starts the moment its own fix lands.
// A barrier here would idle the fast fixes until the slowest finished.
const results = await pipeline(
  batch,
  (b) => agent(fixPrompt(b), {
    label: `fix:${b.id}`,
    phase: 'Fix',
    schema: FIX_SCHEMA,
    isolation: 'worktree',
  }),
  (fix, b) => {
    if (!fix) return null
    // Only PRs are worth verifying; kickbacks go straight to the report.
    if (fix.outcome !== 'fixed' || !fix.prUrl) return { fix, verify: null }
    return agent(verifyPrompt(fix, b), {
      label: `verify:${fix.id}`,
      phase: 'Verify',
      schema: VERIFY_SCHEMA,
    }).then(verify => ({ fix, verify }))
  },
)

const done = results.filter(Boolean)

const dropped = batch.length - done.length
if (dropped > 0) {
  log(`${dropped} bug(s) produced NO result (agent died or was skipped) — they are neither fixed nor kicked back. Do not read this run as covering them.`)
}

const fixed = done.filter(r => r.fix?.outcome === 'fixed')
const kickedBack = done.filter(r => r.fix && (r.fix.outcome === 'needs-design' || r.fix.outcome === 'cannot-reproduce'))
const failed = done.filter(r => r.fix?.outcome === 'gate-failed')

for (const r of kickedBack) {
  log(`KICKED BACK ${r.fix.id}: ${r.fix.outcome} — ${r.fix.escapeHatchReason || 'no reason given'}`)
}
for (const r of fixed) {
  const v = r.verify
  log(`${r.fix.id}: PR ${r.fix.prUrl} — verify=${v ? v.verdict : 'NOT VERIFIED'}`)
}

return {
  summary: {
    requested: batch.length,
    fixed: fixed.length,
    kickedBack: kickedBack.length,
    failed: failed.length,
    noResult: dropped,
  },
  // Merging is NOT this workflow's to do — human approval is required.
  readyForHumanMergeDecision: fixed
    .filter(r => r.verify?.verdict === 'ready-to-merge')
    .map(r => ({ id: r.fix.id, pr: r.fix.prUrl, fences: r.fix.fenceTests })),
  needsAttention: fixed
    .filter(r => !r.verify || r.verify.verdict !== 'ready-to-merge')
    .map(r => ({ id: r.fix.id, pr: r.fix.prUrl, verdict: r.verify?.verdict ?? 'unverified', reasons: r.verify?.reasons ?? [] })),
  kickedBack: kickedBack.map(r => ({ id: r.fix.id, outcome: r.fix.outcome, reason: r.fix.escapeHatchReason })),
  failed: failed.map(r => ({ id: r.fix.id, gates: r.fix.gates })),
  sideIssuesToFile: done.flatMap(r => (r.fix?.discoveredNotFixed ?? []).map(d => ({ from: r.fix.id, issue: d }))),
  callersSurfacedNotFixed: done.flatMap(r => (r.fix?.callersSurfacedNotFixed ?? []).map(c => ({ from: r.fix.id, caller: c }))),
}
