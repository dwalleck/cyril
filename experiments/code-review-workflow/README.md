# code-review-max — a KAS workflow port of Claude Code's max-effort `/code-review`

A multi-agent, recall-mode code review that runs on Kiro's KAS workflow engine
(kiro-cli ≥ 2.16.0, verified on 2.22.0 / KAS 0.66.0). One peer session per
finder angle, one **fresh** session per candidate verification, a gap sweep,
then ranked findings and one Conventional Comments review comment per finding. Source prompt: the
assembled max-effort `/code-review` prompt (`code-review-assembled-max.md` in the separate
`claude-code-system-prompts` repo).

## Files

| Path | Role |
|---|---|
| `.kiro/workflows/code-review-max.workflow.json` | the recipe — **generated**, do not hand-edit |
| `experiments/code-review-workflow/build_recipe.py` | recipe generator; the angle texts live here |
| `.kiro/agents/cr-finder.md` | finder: one angle → `candidates/<angle>.json` (read-only otherwise) |
| `.kiro/agents/cr-verifier.md` | verifier: ONE candidate off a queue → `verdicts/<id>.json` |
| `.kiro/agents/cr-clerk.md` | bookkeeping: runs `crtool.py`, decides duplicates and rank order |
| `.kiro/agents/cr-commenter.md` | writes one Conventional Comments review comment per reported finding |
| `.kiro/code-review/crtool.py` | every mechanical transformation (`gather merge shard collate finalize`) |
| `experiments/code-review-workflow/run_review.py` | raw-ACP driver: zero-credit validate, run, retry, per-session stats |
| `experiments/code-review-workflow/compare_baseline.py` | line a run up against a baseline review's table |
| `experiments/code-review-workflow/review_policy.py` | the driver's pure decisions: data dir, crtool command, input checks, permission policy |
| `experiments/code-review-workflow/selftest_crtool.py` | offline cross-platform self-test (CI runs it on all three OSes) |

## Running it

```sh
python3 experiments/code-review-workflow/build_recipe.py            # regenerate (--shards N, --split-cleanup)
python3 experiments/code-review-workflow/run_review.py --workspace . --validate-only   # zero credits
python3 experiments/code-review-workflow/run_review.py --workspace <ws> --install \
        --target '<base>...<head>' --scope crates \
        --context-file review-context.txt \                 # which docs are authoritative for this change
        --check-cmd 'cargo clippy --workspace --all-targets --message-format=short -- -D warnings'
python3 experiments/code-review-workflow/check_run.py <rundir> <trace.jsonl>          # health-check a finished run
uv run --script experiments/code-review-workflow/selftest_crtool.py                  # offline self-test, any OS
```

`--context` names the documents a verifier must consult before confirming: the change's own spec/design docs and
the repo's protocol references. `--check-cmd` is run ONCE by the driver before the workflow starts, so a slow build
never sits inside an LLM tool call; both are optional.

The workspace must be checked out **at the diff's head** — finders read the
surrounding code from the working tree, and `crtool gather` records a warning
in `manifest.json` when it is not. For a historical range, use a throwaway
detached worktree (`git worktree add --detach ../cyril-wt-review-x <head>`) and
`--install` to copy the recipe, agents and crtool into it. Results land in
`<ws>/.code-review/<timestamp>/{findings.json,report.md}`.

## Platforms and Python

Linux, macOS and native Windows; plain Python ≥ 3.10 or [uv](https://docs.astral.sh/uv/). Every script carries
PEP 723 inline metadata with no dependencies, so `uv run --script <file>.py` needs no project or venv, and
`python3 <file>.py` (`python` on Windows) works the same.

- **How KAS launches crtool** is the workflow's `crtool` input — a machine fact, not a recipe fact. The driver's
  `--runner auto` (default) uses `uv run --script .kiro/code-review/crtool.py` when `uv` is on PATH, else
  `python .kiro/code-review/crtool.py` on Windows / `python3 …` elsewhere; `--crtool-cmd` sets it exactly. Anyone
  starting the recipe another way (cyril's `/workflow run`) must pass `crtool=…` too.
- **On Windows KAS runs commands in PowerShell** (when the client advertises no terminal it picks its own local
  shell: `pwsh`, else `powershell`). So every `crtool` argument in the recipe is double-quoted — single quotes mean
  nothing to cmd.exe, and a bare `a,b,c` becomes an array in PowerShell — and run paths are written with forward
  slashes, which bash, pwsh, cmd, Python and node all accept.
- **kiro-cli's data directory** (auth store + KAS bundles) is `%LOCALAPPDATA%\Kiro-Cli` on Windows,
  `$XDG_DATA_HOME/kiro-cli` (default `~/.local/share/kiro-cli`) elsewhere; `KIRO_DATA_DIR` overrides. The driver
  isolates `USERPROFILE` as well as `HOME` on Windows (node's home directory), kills the server tree with
  `taskkill /T`, and compares permission paths case-insensitively.
- **Inputs are checked before anything spawns.** `rundir` is written with forward slashes once, where it is
  created. `rundir`, `target` and `scope` may not be empty or contain a quote (including `“ ” „`, which PowerShell
  reads as `"`), a backtick, a backslash, a control character, or a `$` a shell would expand — `$` is allowed only
  before `/`, so a `//wsl$/<distro>/…` workspace works. A `--crtool-cmd` must be one simple command naming
  `crtool.py` in the platform's shell syntax: on Windows a quoted program needs PowerShell's `& "exe"` call form;
  under bash `&` is refused. `--runner auto` checks that the chosen `python` actually runs (on Windows it is often
  the Microsoft Store alias) and stops with a hint when it does not.
- **Callers that never set `crtool`** (KAS inputs have no defaults, and `workflow/new` does not validate templates):
  the clerk and commenter agents fall back to uv, else `python`/`python3`, when a command starts blank or with a
  literal `{{crtool}}`.
- **`--retry`/`--resume` keep the run's stored inputs**, so a new `--crtool-cmd`/`--runner` is ignored with a warning.
  The driver keeps its own copy in `<rundir>/_driver-inputs.json` (crtool command, target, scope): the permission
  policy needs the exact crtool command, and a retry reads it from there. `--auto-recover` forwards `--target` and
  `--scope` too.
- **macOS**: kiro-cli's data is looked for under `~/Library/Application Support/kiro-cli` (Rust's
  `data_local_dir`), falling back to the XDG location when only that holds an auth store. The auth store is opened
  read-only, so a lookup never creates an empty one. Because kiro-cli resolves that directory from `HOME`, the
  driver links the real one into the isolated HOME; `XDG_DATA_HOME` stays real for uv. Not tested on a Mac with
  kiro-cli; CI covers the tooling only.
- **Checked in CI**: the *Code Review Tooling* job runs `selftest_crtool.py` under uv on ubuntu, windows and macOS —
  on every push to main, and on a PR only when it touches `experiments/code-review-workflow/`, `.kiro/` or `ci.yml`
  (the job holds a scarce macOS slot). It checks the permission policy against known bypasses, runs every recipe
  `crtool` line through every shell on the runner (pwsh 7 and Windows PowerShell 5.1 on Windows) into an argv echo
  that must match what the policy parsed, then runs each line for real and every `crtool` subcommand end to end.

## Shape (20 of the engine's 20 step nodes)

```
setup                                  cr-clerk     crtool gather → manifest, per-file patches, facts/
parallel[allSettled]  8 × find-*       cr-finder    one peer session per angle
dedup                                  cr-clerk     crtool merge → decisions.json → crtool shard
parallel[allSettled]  3 × repeat(verify-K)  cr-verifier   queue drain (the runs below used 4, before the comment step)
sweep                                  cr-finder    gaps only; reads the already-raised list
repeat(verify-sweep)                   cr-verifier
ballots                                cr-clerk     crtool ballots → two more votes for refuted + conventions claims
parallel[allSettled]  2 × repeat(verify-rK) cr-verifier   the second and third votes, blind to the first
rank                                   cr-clerk     crtool collate (2-of-3 tally) → ranking.json → crtool finalize
comment                                cr-commenter one Conventional Comments comment per reported finding → crtool comments
```

With the comment step the verify phase runs 3 shards, not 4 (4 would be 21 nodes). The recipe is AT the cap: adding a step means removing one (`--shards 3` frees a node; `--split-cleanup` no longer fits).

### Three accuracy mechanisms (added after run 2)

1. **Authorities outside the code.** Run 2's #1 finding was a real code mechanism that the change's own spec had
   decided on purpose, with evidence that made the failure impossible — and the verifier, scoped to `crates/`, never
   saw the spec. Now `crtool gather` lists `change_docs` in the manifest (documents the SAME diff touched, whatever the
   review scope), the `context` input names the repo's protocol references, and the verifier must check both before
   CONFIRMED: decided-and-impossible → REFUTED; intended-but-still-fails → still a finding; trigger depends on the
   other side and no authority covers it → PLAUSIBLE at most.
2. **2-of-3 where one vote was unstable.** Identical inputs agreed on only 82 % of verdicts, and every REFUTED
   conventions claim flipped. `crtool ballots` mints two ballot ids (`C29.v2`, `C29.v3`) for each first-round REFUTED
   and each conventions claim; they drain through the unchanged verifier protocol in two parallel loops, and
   `collate` tallies: REFUTED needs two votes, so does CONFIRMED, a split is PLAUSIBLE, a lone vote stands (except a
   lone REFUTED, which is kept as PLAUSIBLE). The report prints every vote record.
3. **Code intelligence computed once, not per session.** `facts/usages-N.txt` maps each symbol the diff adds or
   changes to its usages (`git grep -w`, same language, shrunk to at most four reads; common names are counted, not
   listed; symbols used nowhere else are called out). `facts/diagnostics.txt` is the repo's check command, run once
   by the driver on exactly the reviewed tree. KAS's own LSP tool would cost a language-server cold start in each
   of ~50 sessions and returned nothing here; this costs milliseconds and is the same for every session.

### The three adaptations from the Claude Code prompt

1. **Per-candidate verification is a data-dependent fan-out; the DAG is static.**
   A `repeat` loop is used as a queue-drain iterator: each iteration is a fresh
   peer session that takes ONE id off `queues/queue-K.json`, writes a verdict,
   and rewrites the queue; `fileCheck {jsonPath: done, value: true}` ends the
   loop. The step cap counts *nodes*, not *executions*. The stop condition is
   evaluated after the body, so an empty shard costs one no-op session.
2. **There is no orchestrator model.** The engine is a deterministic scheduler,
   so dedup, ranking and the 15-finding cap are explicit steps.
3. **Files are the only data channel.** `{{id.output}}` captures a step's LAST
   message — for a step that writes a file and signs off, that is the sign-off
   (cyril-srp6). `rundir` is a client-minted input; nothing reads a capture.

### LLMs emit decisions, scripts move data

A candidate's text is written once, by the finder that raised it. Later steps
emit only small decision files (`decisions.json`: duplicate pairs;
`ranking.json`: an id order; one verdict per candidate) and `crtool.py`
applies them. Recall mode is enforced in code, not just asked for in prompts:
a missing/unparseable verdict collates as `UNVERIFIED` and is **kept**; an
unranked finding is appended, never dropped; an exhausted verify loop uses
`onMaxIterations: continue`.

## Engine facts established here (kiro-cli 2.22.0 / KAS 0.66.0, 2026-09-20)

- **8-wide `parallel` works.** All eight finder sessions start within the same
  second; no concurrency cap or throttle was hit. (Bundled recipes only use 2.)
- **`repeat` gives a fresh session per iteration** — confirmed live, not just
  from the 2.16.0 capture: `verify-loop-1/iter-0` and `/iter-1` are distinct
  sessions, and `loop_iteration.stopConditionMet` tracks the queue file.
- **Model cascade: step `modelId` > agent `model` > parent session (`auto`).**
  Agent-level `model`/`effortLevel` in a `.md` agent's frontmatter IS honored by
  workflow step sessions (probe: agent haiku → `claude-haiku-4.5`; same agent +
  step `modelId` → `claude-sonnet-5`; control → `auto`). Observable per session
  in `config_option_update`. With `auto`, the wire never names the concrete
  model the router picked.
- **Load-time validation covers nested agents but NOT unknown input templates.**
  A bogus agent three levels deep (`parallel → repeat → step`) is rejected with
  a named error; `{{no_such_input}}` in a nested prompt is ACCEPTED. The skill's
  `validate_workflow.py` catches it — run the offline validator, don't rely on
  `_kiro/workflow/new` for templates.
- **`initialState.root` is a lazy runtime tree.** `parallel`/`repeat` nodes have
  no children until they start, so it cannot be used to count the compiled plan.
- **Every tool call raises `session/request_permission`** (`askType: implicit`)
  in a fresh HOME — reads included: 282 requests for 19 sessions on run 1, a 1:1
  ratio with tool calls. Consent shape: `_meta.kiro.consent {capability:
  fs_read|fs_write|shell, resource, workspaceRoot}`. The agent-file tool id
  `execute_bash` surfaces on the wire as `toolId: run_command`, and running a
  command raises a NESTED `fs_read` consent on its cwd — deny that and the
  command is rejected. An interactive client (cyril) would face an approval
  storm; pre-authorizing via agent `permissions.rules` is untested.
- **KAS's code-intelligence (LSP) tool is client-gated, and the gate's LOCATION is the trap.** Tool id `code`
  ("Code Intelligence": tree-sitter + per-language LSP clients from `<ws>/.kiro/settings/lsp.json`; ops
  `search_symbols lookup_symbols get_document_symbols goto_definition find_references hover diagnostics`, plus the
  mutating `pattern_rewrite rename_symbol format_code`). The session factory builds it only when
  `clientMeta.settings.codeIntelligence` is `{enabled: true}` (an OBJECT; bare `true` reads as off), and
  `clientMeta` is **`initialize.params.clientCapabilities._meta.kiro`** (covenant §2) — NOT `params._meta.kiro`.
  A top-level `_meta` is accepted and silently ignored, which cost this session five probes and a wrong
  "unreachable via the launcher" conclusion. Verified live: with the correct placement `code` joins the parent's
  tool list on BOTH the launcher (`kiro-cli acp --agent-engine kas`) and the direct `node acp-server.js
  --transport=stdio --auth=acp-callback` spawn; with the wrong placement, `session/new._meta`, or the host setting
  `chat.enableCodeIntelligence=true`, it does not. Driver: `--kiro-setting codeIntelligence` (+ `--direct`).
  **In a workflow step** (agent `tools: [read_file, code]`, gate sent once at initialize — engine-created step
  sessions inherit it): the tool is present and its consent is a read capability, so the driver's policy allows it.
  The **tree-sitter half works instantly** — `search_symbols`, `get_document_symbols`, `pattern_search` (it found
  the `refresh_powers_panel` call site). The **LSP half returned nothing**: `goto_definition`, `find_references`
  and `hover` came back empty on 50 calls over 160 s with no error or "starting" status, although rust-analyzer WAS
  spawned (own pid, the worktree's pinned 1.94.0 toolchain, reaped with the KAS process group). Cause undetermined
  (cold index vs position encoding); KAS logs nothing about the LSP lifecycle even at `KIRO_LOG_LEVEL=debug`.
  Structural problem regardless: every peer session spawns its OWN language server, so one review = ~50
  rust-analyzer cold starts, 8 at once. For this workflow, a deterministic callers map from `crtool gather` is the
  better source of reference data; `code` is worth granting only for its tree-sitter operations.
- **Hooks stay off** unless the client advertises
  `clientCapabilities._meta.kiro.hooks`; this driver does not, so the repo's
  `clippy-on-stop` hook never fires in step sessions.
- A step that is denied a permission signals `warning` → the run pauses; the
  driver's nudge (`session/prompt` to the step session) + `_kiro/workflow/resume`
  path does restart the step (observed once, by accident).

## Driver permission policy

Unattended, but not blanket-approve: `fs_read` only inside the workspace,
`fs_write` only inside the run directory. `shell` is an allowlist checked by
construction, not a blacklist of dangerous characters: an optional
`cd <workspace> && `, then this run's exact crtool command, then a step
subcommand (`gather`, `merge`, `shard`, `facts`, `ballots`, `collate`,
`finalize`, `comments` — never `diagnostics`, which runs an arbitrary command)
whose first argument is the run directory, then only plain words or safely
double-quoted strings. Redirects, newlines, subexpressions, chaining and
substitution cannot be spelled that way. Every field that carries the command
(consent resource, `_meta.kiro.command`, `rawInput.command`/`cmd`) must pass; a
request with none is denied. Everything else is denied and logged.

## Results — PR #122 pre-review head (`ab65e7a31ae7...88187579`, scope `crates`, 23 files / 107 KB)

Baseline: a max-effort Claude Code `/code-review` of the same commit (20 findings), kept as a local, uncommitted
`PR122-code-review.md` at the repo root — pass any review with the same summary-table shape to `compare_baseline.py`.
Matching was done by reading pairs, not by `compare_baseline.py` leads alone (its leads include false ones).

| | run 1 | run 2 |
|---|---|---|
| models | `auto` everywhere | finder `claude-opus-5`/high, verifier + clerk `claude-sonnet-5` |
| finder prompt | original (restraint-weighted) | recall-tuned ("plausible mechanism, not conviction"; read real source) |
| wall clock | 8.8 min | 39.4 min |
| step sessions | 19 | 50 (9 finder, 38 verifier, 3 clerk) |
| raw → deduped (+sweep) | 5 → 5 (+2) | 61 → 33 (+6) |
| verdicts | 6 CONFIRMED, 1 PLAUSIBLE | 28 CONFIRMED, 7 PLAUSIBLE, 3 REFUTED, 1 UNVERIFIED |
| baseline findings matched | 3 / 20 | **14 / 20** (#1 2 3 4 5 8 9 10 12 13 16 18 19 20) |
| peak context per session | max 9.1 %, mean 5.6 % | max 19.4 % (sweep), mean 6.4 % |
| permission requests | 282 | 1032 (8 denied by policy) |

Run 2 misses: #6 (no `+N more`), #7 (`· steering` truncated), #11 (re-spelled push below log level),
#14 (two unfailable fences in `state.rs`), #17 (unconditional registration), #15 (`CLAUDE.md` — outside `--scope crates`).

**Known defects in run 2 — fix before relying on it:**

- **Its #1-ranked finding is one the baseline RETRACTED.** C02 (dropped `sessionId` → cross-session catalog
  overwrite) was CONFIRMED because the code-level mechanism is real. It is harmless because the catalog is
  process-global — powers are a per-user install, and every session's push carries the same set. That fact is
  NOT in the wire audit (`docs/kiro-2.20.1-wire-audit.md:147` says the frame is session-scoped and carries
  `sessionId`, which at face value *supports* C02). It is in the change's own design record,
  `.cyril-v19o/spec.md:118` ("Process-global; latest push wins") and `evidence.md` P5 — inside the PR, readable in
  the worktree, but outside `--scope crates`, and nothing told the verifier such a record exists. The C02 verifier
  read 2 source files and no docs. Fix: a recipe input naming the change's design/spec docs and the repo's
  protocol authorities, passed to verifiers — "intended, on this evidence" is what separates a documented
  decision from a bug. A wire-audit pointer alone would not have caught this one.
- **The LLM-maintained queue dropped one id** (S04: 6 queued, 5 iterations, queue ended `done`). `crtool collate`
  caught it (`UNVERIFIED`, kept, flagged) so the finding was not lost — but 1 of 39 queue rewrites was wrong.
  Fix: make the queue immutable and derive "pending" from which `verdicts/<id>.json` exist; the only mutation
  left is flipping `done`.
- **The clerk reached for ad-hoc shell 4 times** (`python3 -c`, `grep`) because `all.json` (78 KB) and
  `verified.json` (121 KB) exceed the 30 k tool-result cap. The driver's policy denied them and the clerk fell
  back to ranged reads. Fix: have `crtool merge`/`collate` also emit a one-line-per-candidate digest sized for
  a single read.
- **Dedup left one triple unmerged** (C04/C14/C19, all the transport-fence ordering at `powers.rs:73`) — two
  wasted verifications, no lost finding.
- **Angle drift.** Under recall pressure the "wrapper/proxy" finder ran a general review. Costs duplicates
  (61 → 33), not findings; multi-angle corroboration (`also_flagged_by`, up to 6 of 8 angles) falls out as a
  free confidence signal.

## Results — the three accuracy mechanisms (replays of run 2's 61 candidates, sonnet only, 2026-09-20)

| | replay 1 (before) | replay 2 (context + votes + facts) | mini replay (sharpened rule, 6 contested candidates) |
|---|---|---|---|
| dropped `sessionId` (run 2's #1; baseline RETRACTED it) | CONFIRMED | PLAUSIBLE, ranked #7, spec + wire audit cited | REFUTED 2-1, quoting spec.md:118 / evidence P5 |
| catalog not reset on `/new` (baseline #10, PLAUSIBLE) | CONFIRMED | REFUTED 3-0 on the spec's "process-global" decision | REFUTED 2-1 |
| popup-height bug (real; the author later fixed it) | CONFIRMED | **REFUTED 3-0** — arithmetic verified, refuted as "documented" | CONFIRMED + `by_design` note |
| hand-copied overlay quintet (baseline #20) | CONFIRMED | **REFUTED 2-1** — one example disproved, finding discarded | CONFIRMED + `by_design` note |
| `#![allow]` in test modules (conventions) | CONFIRMED | CONFIRMED 3-0 | REFUTED 2-1 |
| verdicts | 36 C / 2 P / 0 R | 29 C / 4 P / 6 R, 9 balloted, 0 outcomes changed by vote | 2 C / 4 R, 4 balloted, all 4 split 2-1 |

What this shows:

- **Authorities work, and cut both ways.** The false #1 is gone and two claims the design record genuinely settles are
  refuted with quotes. But replay 2's verifiers also refuted a real bug whose arithmetic they had just confirmed, because
  a doc called it accepted. The rule is now "can the wrong outcome occur" — never "was it on purpose" — and an accepted
  limitation stays a finding carrying a `by_design` note. The mini replay confirms the sharper rule on exactly those cases.
- **Voting stabilises noise, not bias.** Three verifiers with the same prompt made the same over-refutation 3-0. After the
  rule was sharpened every balloted candidate split 2-1, so the vote decided real disagreements. It does NOT make runs
  reproducible: the `#![allow]`-in-tests claim went 3-0 CONFIRMED in one run and 2-1 REFUTED in the next, because the
  rules file itself is ambiguous about test code.
- **Facts are cheap and were uncontroversial**: 78 symbols in 4 pages, clippy clean in 24 s, no verifier claimed a build
  failure. Too small an effect on this PR to measure.

Hazards found while running these — all fixed in the driver/crtool, none in the mechanisms themselves:

- **The hourly token dead window.** `kiro-cli user whoami` renews only AFTER expiry; KAS rejects a token with <180 s
  left. Crossing an hour boundary failed every in-flight step. The auth callback now waits the window out off-thread.
- **Concurrent renewal logs the user out.** Two threads ran `whoami` at the instant of expiry; the refresh token is
  single-use, the loser was rejected, kiro-cli deleted the token row. Now: ONE renewer, post-expiry, locked, backed off.
- **KAS turn stall**: both in-flight sessions went silent mid-run for 20 min with no error or watchdog frame. The driver's
  idle timeout (`--idle-min`) ends the run resumably; `--resume` (a rehydrated interrupted run is `paused`) or `--retry`
  (terminal failed) continues from a NEW server process without redoing completed steps — both proven live.
- **Two path-confusion bugs in the ballot loops**: a verifier read another loop's `C32.v3.json` as its own `C32.v2`
  (→ one verdict directory per queue), then a loop resolved the relative `verdicts/r1` against `queues/` (→ absolute
  `verdict_dir`; `collate` and `check_run` now find a verdict wherever it sits under the run, and flag strays).

## Results — full 20-node run (2026-09-20, `full-20260920-201458`, same PR #122 range)

One pass, no recovery needed: **50.9 min, 64 step sessions** (9 opus finders 66 session-min, 51 sonnet verifiers 57
session-min, 4 clerk steps 3 min), peak context per session max 19.3 % / mean 6.5 %, 1,346 permission requests answered
by policy. It crossed a token-hour boundary at minute 40 cleanly (one renewal, login intact). `check_run`: every verdict
in place, all three votes present for all 6 balloted candidates, nothing UNVERIFIED.

| | run 2 (17 nodes) | full run (20 nodes) |
|---|---|---|
| raw → deduped (+ sweep) | 61 → 33 (+6) | 55 → 32 (+7) |
| verdicts | 28 C / 7 P / 3 R / 1 unverified | 36 C / 1 P / 2 R, 11 kept findings marked `by design` |
| baseline findings matched (by reading pairs) | 14 / 20 | 12 / 20, plus #10 found and refuted 3-0 on the design record |
| …which ones | #1 2 3 4 5 8 9 10 12 13 16 18 19 20 | #2 3 5 **6 7** 8 12 13 16 18 19 20 |
| the dropped-`sessionId` false alarm | CONFIRMED, ranked **#1** | CONFIRMED `[by design]`, ranked #10 |
| votes | — | 6 balloted: 4 unanimous, 2 split 2-1, one outcome changed (C→R) |

What it says:

- **Recall is finder-bound and varies run to run.** This run found #6 and #7 (both via the gap sweep), which run 2
  missed, and did not raise #1, #4 or #9, which run 2 found. The union of the two runs covers 16 of 20. Verification
  lost nothing real in this run; the misses were never raised.
- **The top of the report is now trustworthy-looking but crowded.** #1 is the `/help` bug corroborated by all 8 angles;
  the false alarm fell from #1 to #10 with a `by design` label. But dedup left four copies of the paste-guard finding
  at the identical `app.rs:1610` unmerged — slots 6–9 of 15. `crtool merge` now flags same-location clusters in the
  digest (verified offline on this run's candidates: all 11 clusters flagged; not yet run live).
- **The sharpened rule swung toward CONFIRMED** (36 of 39). "Real, and the author documented it as accepted" now
  survives with a label instead of being refuted — right for the popup bug, debatable for the `sessionId` claim, which
  the mini replay REFUTED 2-1 and this run CONFIRMED `by design`. Same candidates, different sessions, different call.
- **The facts pack was barely used**: 2 of 8 finders opened it, the cross-file tracer never did (21 greps instead),
  finder greps fell only 143 → 122. A pointer in the agent prompt is not enough; it belongs in the step prompt of the
  angle that needs it.
- Still off-script: the rank clerk tried `python3 -c` once (denied, completed anyway); the sweep queue was written twice.

## Review comments — the final step (Conventional Comments)

Every reported finding ends as a postable review comment in the [Conventional Comments](https://conventionalcomments.org/)
format — `<label> [decorations]: <subject>` + discussion — in `<rundir>/comments.json` (`file`, `line`, `label`,
`decorations[]`, `subject`, `discussion`, rendered `body`, `source`), `comments.md`, a `comment` field on each
`findings.json` entry, and a section appended to `report.md`.

- `crtool finalize` writes `comments/brief-N.txt`: one block per reported finding with everything a commenter needs
  (verdict, votes, category, angles, summary, failure scenario, finder + verifier evidence, the by-design quote), so the
  model never hunts for verdict files.
- `cr-commenter` (one sonnet session, ~2 min, ~7 % context) writes `comments/<id>.json` — a label, decorations, a
  subject under 100 characters and a 2–5 sentence discussion ending in the smallest fix — or `{"duplicate_of": id}`
  when a finding is the same defect at the same place as a better-ranked one.
- `crtool comments` validates and renders: label must be in the spec's vocabulary; decorations are lowercase words;
  `blocking` is downgraded on anything not CONFIRMED, accepted by design, or carrying a label the spec makes
  non-blocking by nature (`nitpick`, `thought`, `note`, `praise`), and those get an explicit `non-blocking`. A finding
  with no usable model-written file gets a **template comment** — every finding always has a comment.
- Run just this step over a finished run for the price of one session:
  `build_recipe.py --only comment --out /tmp/c.json` then `run_review.py --recipe /tmp/c.json --rundir <finished run>`.

Live on the full run's 15 findings: 12 comments written + 3 marked duplicates (the three copies of the paste-guard
finding collapsed to one comment), labels `issue` ×11 / `question` ×1, **4 `blocking`** (the `/help` bug, empty `name`,
steering-marker truncation, paste guard), 3 `issue (test)`, the two by-design findings rendered `question (non-blocking)`
and `issue (non-blocking)`. The first attempt marked 10 of 13 `blocking` until the agent prompt said plainly that
blocking is scarce and that a defective test or an unusual-input edge case does not earn it.
