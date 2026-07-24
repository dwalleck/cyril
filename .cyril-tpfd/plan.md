# cyril-tpfd — budgeted plan

Claims from design.md (1–12) → 6 slices. Gates per slice: `cargo test -p
cyril-core --features kas`, `cargo clippy --all-targets --all-features --
-D warnings`, `cargo fmt --check`, real exit codes (`&& echo SLICE-N-OK`).

## Slice 1: parse `action.timeout` into `HookDef`

**Claim:**          7 (parse half): timeout seconds when present, else 60s.
**Oracle:**         kasHookFileSchema ("timeout must be >= 0 seconds") — the carve; a file with `timeout: 2` must yield a 2s duration, absent yields 60s.
**Stress fixture:** hook file with `timeout: 0` (degenerate, must parse and be honored verbatim) and one with `timeout` absent alongside — catches an `unwrap_or`-on-parse-failure collapse and a required-field regression breaking existing files.
**Loop budget:**    none (field threading; existing load loop unchanged, O(hooks) at ~1–20 hooks).
**Wall budget:**    n/a (load-time only).
**Files:**          `crates/cyril-core/src/protocol/kas/hooks.rs`

`HookAction` gains `#[serde(default)] timeout: Option<u64>`; `HookDef`
gains `timeout: Option<u64>` + `effective_timeout() -> Duration`
(`map_or(60s, from_secs)`). Existing files without the field must load
unchanged (serde default).

**Verification:**
- [ ] `hook_def_default_timeout` (absent → 60s) + present/zero parse fences pass
- [ ] Existing registry fences pass untouched
- [ ] Budgets hold

## Slice 2: typed `HookRunOutcome` core + wire adapter

**Claim:**          11: executeHook wire replies byte-shape unchanged.
**Oracle:**         the four pre-existing executeHook fences (`execute_hook_real_exit_codes`, `execute_hook_timeout_kills`, `execute_hook_cancel_reaps`, `pre_tool_use_exit2_block_contract`) — written against the OLD code, they are the independent contract.
**Stress fixture:** exactly those fences: they cover success/exit-2/timeout/cancel; a refactor that reorders fields, drops `cancelled:false`, or maps spawn-fail differently fails them.
**Loop budget:**    none (restructure only).
**Wall budget:**    n/a.
**Files:**          `crates/cyril-core/src/protocol/kas/hooks.rs`

`enum HookRunOutcome { Completed { output: String, exit_code: i32 },
SpawnFailed { message: String }, TimedOut }`; `run_hook_command(...) ->
HookRunOutcome` holds the tokio spawn/timeout logic; `execute_hook`
becomes the thin wire adapter mapping outcomes to today's exact JSON
(incl. spawn-fail `{output: "hook failed to spawn: ..", exitCode: 127}`,
timeout `{cancelled: true, exitCode: 124}`, signal-death 137 warn).

**Verification:**
- [ ] All four fences pass unmodified
- [ ] clippy/fmt clean
- [ ] Budgets hold

## Slice 3a: pure packaging — outcomes → `AcpPrecomputedHookResult[]`

**Claims:**         2 (order), 3 (shape), 4 (stdout-precedence), 5 (non-zero included), 6 (skip semantics, pure half).
**Oracle:**         the carved producer semantics (`probe-carve-shape.sh` output, independent of cyril) + the live-accepted element from `probe-sessionstart-live.py` (KAS consumed exactly that JSON on 2026-07-23).
**Stress fixture:** outcome set [Completed{out+err, exit 0}, Completed{stderr-only, exit 3}, Completed{empty, exit 0}, SpawnFailed, TimedOut, Completed{stdout, exit 0}] → exactly elements [0-stdout-only, 1-stderr, 5] in that order. Catches: combined-output copy-paste from executeHook, exit-code filter, failure aborting the batch, reordering.
**Loop budget:**    O(hooks) at ~1–20 hooks — trivial.
**Wall budget:**    n/a (pure).
**Files:**          `crates/cyril-core/src/protocol/kas/hooks.rs`

`fn package_session_start_results(runs: Vec<(&HookDef, HookRunOutcome)>)
-> Vec<serde_json::Value>`: per D1 (parity) include iff
`Completed` and `!(stdout-else-stderr).is_empty()`; content = stdout if
non-empty else stderr (the executor must expose stdout/stderr separately
— extend `Completed` to carry both, adapter combines for executeHook);
element `{id, name, hookId, originalType: "runCommand", content}`;
SpawnFailed/TimedOut/empty → `warn!` + skip.

NOTE: slice 2's `Completed.output` must therefore be
`{stdout: String, stderr: String}` — the executeHook adapter combines,
the sessionStart packer picks. Slice 2 implements it that way from the
start.

**Verification:**
- [ ] `session_start_element_shape_matches_carve` (key-set equality + originalType literal, expected element copied from the live-accepted probe JSON)
- [ ] `session_start_content_stdout_precedence`, `session_start_nonzero_exit_still_included`, `session_start_packaging_skips_and_orders` pass
- [ ] Budgets hold

## Slice 3b: async responder — list → execute → package → reply

**Claims:**         1 (trigger filter), 6 (integration half), 7 (behavior half), 8 (stub parity), 9 (USER_PROMPT).
**Oracle:**         marker files on disk (claim 1); printenv exit semantics (claim 9 — distinguishes unset from set-empty); wall-clock at fixture scale (claims 6/7); the pre-existing stub test (claim 8).
**Stress fixture:** registry dir with FOUR files: sessionStart marker hook, preToolUse marker hook (must NOT run), sessionStart `timeout: 1` on `sleep 30` (skipped fast), sessionStart `timeout: 2` on `sleep 1 && echo ok` (included — fails if seconds are misread as millis). Expected: only sessionStart markers, reply has exactly the two producing elements, elapsed < 10s.
**Loop budget:**    O(sessionStart hooks) sequential executions, each ≤ its timeout; production ~1–5 hooks × ≤60s — bounded by user config, session-start-only (not always-on). Justified: parity with KAS's own sequential executor.
**Wall budget:**    worst case Σ timeouts at session start; degenerate configs are the user's own (visible via warns). Fixture scale: <10s.
**Files:**          `crates/cyril-core/src/protocol/kas/hooks.rs`, `crates/cyril-core/src/protocol/client.rs`

`respond_session_start(registry: &HookRegistry, cwd: &Path) ->
acp::Result<acp::ExtResponse>` async: `list("sessionStart", None)`; for
each, resolve the `HookDef` (list returns JSON — instead add
`registry.session_start_hooks() -> Vec<&HookDef>` to avoid
JSON-roundtripping our own registry), `run_hook_command(cmd, "", cwd,
def.effective_timeout())`, package, reply. client.rs:405 gains
`.await` + args. USER_PROMPT set to `""` (claim 9): doc-comment states
"sessionStart has no prompt; env var present-but-empty" — sanity-hint
level, enforced by fence not runtime check (violation = fence failure,
not silent wrong output).

**Verification:**
- [ ] `session_start_runs_only_session_start_hooks`, `session_start_skips_empty_and_timeout`, `session_start_timeout_seconds_not_millis`, `session_start_user_prompt_env_empty` pass
- [ ] `session_start_acknowledges_empty_results` retargeted (async) and passing
- [ ] Budgets hold at fixture scale (<10s)

## Slice 4: non-blocking fence

**Claim:**          10: RPC loop serves other requests while a sessionStart hook runs.
**Oracle:**         tokio timing captured at resolution (jiyn claim-13 pattern — `slow_hook_does_not_block_loop` precedent): a concurrent future resolves in <2s while a 3s hook runs.
**Stress fixture:** 3s sessionStart hook + concurrent cheap future; timing captured AT RESOLUTION, not after join (the exact bug jiyn's P2 review caught in its first version).
**Loop budget:**    none (test only).
**Wall budget:**    test ≤ ~4s.
**Files:**          `crates/cyril-core/src/protocol/kas/hooks.rs` (test mod)

**Verification:**
- [ ] `slow_session_start_does_not_block_loop` passes
- [ ] Budgets hold

## Slice 5: harness repair (D2)

**Claim:**          12: with `.arn` extraction, fence-probe turns complete.
**Oracle:**         live KAS accepted the identical fix today (tpfd control+shaped arms completed; `.cyril-tpfd/live-results/result-*.json`).
**Stress fixture:** n/a — live-verified today; the probe is itself the per-release manual fence (pre-approved pattern from jiyn, design table row 12).
**Loop budget:**    none.
**Wall budget:**    n/a.
**Files:**          `.cyril-jiyn/probe-hooks-ab-2.13.0.py`, `experiments/conductor-spike/README.md`

Port the fixed `token()` (parse profile row JSON, extract `.arn`);
README fence note gains one line: A/B `prompt_completed` was poisoned by
the profileArn-object harness bug until 2026-07-23 — LIST/EXEC/marker
conclusions unaffected.

**Verification:**
- [ ] `python3 -m py_compile` on the probe
- [ ] README note present

## Slice 6: docs — audit + roadmap reflect shipped execution

**Claim:**          closes the loop on design "Purpose" (no new behavior claim; docs-only).
**Oracle:**         grep: wire-audit hooks section no longer says sessionStart is acknowledge-only; ROADMAP KAS-7 deferred-list drops tpfd.
**Stress fixture:** n/a (docs slice; the greps above are the check).
**Loop budget:**    none.
**Wall budget:**    n/a.
**Files:**          `docs/kiro-2.7.1-wire-audit.md`, `docs/ROADMAP.md`

Wire-audit hooks bullet: replace "sessionStart is acknowledged
`{results: []}`; executing … is cyril-tpfd" with the shipped behavior +
the carved element shape (fields + assertNever constraint + stdout-else-
stderr) — the shape's covenant-doc home stays cyril-mfkg (verified open;
its scope covers the hooks types re-sync). ROADMAP KAS-7: move tpfd from
deferred to shipped.

**Verification:**
- [ ] Both greps pass; docs build (n/a) — review by eye

## Plan Self-Review

1. **Loops:** slice 3a packaging O(hooks)≈20 — trivial; slice 3b sequential execution O(hooks × timeout) bounded by user config, session-start only, KAS-parity justified. No always-on loops. No gaps.
2. **Fixtures:** slice 1 zero/absent timeout (parse collapse); slice 2 the four adversarial executeHook fences (refactor drift); slice 3a mixed-outcome batch (combined-output, exit-filter, abort-on-failure, reorder bugs); slice 3b four-file registry (trigger leak, secs-as-millis, timeout-abort); slice 4 timing-at-resolution (the jiyn P2 bug class); slices 5/6 live-verified/greps. No happy-path-only fixtures.
3. **Doc-comment preconditions:** slice 3b's "USER_PROMPT present-but-empty" = sanity-hint, enforced by fence. The packaging fn documents "include iff Completed non-empty" = load-bearing, enforced by the function's own match (no caller precondition). No unenforced contracts.
4. **Write targets:** all tracing output = diagnostic (stderr via tracing, existing config); wire replies = data (JSON-RPC channel). No new stdout/stderr writes.
5. **Tracker refs:** cyril-n03f (askAgent, verified), cyril-2adk (hot-reload, verified), cyril-mfkg (covenant shape home, verified open), to-file.md queue (ship-skill-sanctioned close-out filing). No unresolved deferrals.

Claim coverage: 1→3b, 2→3a, 3→3a, 4→3a, 5→3a, 6→3a+3b, 7→1+3b, 8→3b,
9→3b, 10→4, 11→2, 12→5. All 12 covered.
