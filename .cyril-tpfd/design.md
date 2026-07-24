# cyril-tpfd — falsifiable design: execute SessionStart hooks + precomputed context

## Purpose

Replace the acknowledge-only `respond_session_start()` stub with real
execution: run the registry's SessionStart `runCommand` hooks and package
their output as `AcpPrecomputedHookResult[]`, which KAS injects into the
session's first user prompt as `<HOOK_INSTRUCTION>` blocks. Shape and
consumption semantics are probe-verified (`findings.md`: carve on
2.13.0+2.14.1, live injection MATCH 2026-07-23).

## Architecture (small)

- `HookAction` gains `#[serde(default)] timeout: Option<u64>` (seconds,
  per the kasHookFileSchema); threaded to `HookDef.timeout`.
- `execute_hook` splits into a typed core — `HookRunOutcome::{Completed
  {output, exit_code}, SpawnFailed, TimedOut}` — plus a thin wire adapter
  preserving today's `executeHook` replies exactly. sessionStart
  packaging consumes the typed outcome (no string-matching on error
  text).
- `respond_session_start(registry, cwd)` becomes async: `list
  ("sessionStart", None)` → execute sequentially in registry order, each
  under its own timeout (default 60s) with `USER_PROMPT=""` → package
  per the carved KAS producer semantics: include iff Completed with
  non-empty `stdout || stderr` (exit code NOT filtered — KAS parity);
  skip SpawnFailed/TimedOut/empty with a warn.
- Element shape: `{id, name, hookId, originalType: "runCommand",
  content}`, `id == hookId` == registry id (`file-stem:name`), and
  `originalType` is always the literal `"runCommand"` (unknown values
  throw `assertNever` inside the agent — carved constraint).
- Harness repair (same subsystem, this branch): the per-release fence
  `.cyril-jiyn/probe-hooks-ab-2.13.0.py` `token()` sends the profile
  row's `.arn` (the object-verbatim bug killed every probe turn).

## Input shapes

- Registry sessionStart hooks: **0** (claim 8), **1** (claim 3), **many**
  (claim 2), **mixed with other triggers** (claim 1).
- Hook output: **stdout-only**, **stderr-only**, **both** (stdout wins,
  claim 4), **empty** (claim 6). Non-UTF8 output: out of scope — the
  shared executor already `from_utf8_lossy`s it (jiyn behavior,
  unchanged).
- Exit status: **0**, **non-zero** (claim 5), **spawn-fail**,
  **timeout** (claim 6).
- `action.timeout`: **present** (claim 7), **absent** → 60s (claim 7),
  **0** — honored verbatim (degenerate: hook always times out, warn
  makes it visible; one-line accepted).
- Params `{trigger, sessionId}`: ignored — the method name is the
  discriminator and cyril's registry is workspace-global; out of scope
  (matches `respond_execute`'s posture of not validating `hookId`).
- `kas_hooks` mode: only `host` can receive the callback (advertisement
  gates it — jiyn A/B); `kas`/`off` out of scope.
- SessionStart hook WITH a `matcher`: excluded by `list(trigger, None)`
  semantics (nothing to match at session start). Accepted divergence,
  degenerate config; debug-logged at sessionStart membership
  (`session_start_hooks` — generic list filtering stays silent, since
  matcher exclusion under a real `toolId` is routine wire semantics).

## Subtractive sweep

Additive: a stub gains real work; no lock, ordering, or uniqueness
property is removed. The one relaxed property — the sessionStart reply
is no longer instant — is bounded per-hook by claim 7's timeout and
covered for loop-liveness by claim 10.

## Claims

1. The responder executes exactly the registry's `sessionStart`-trigger
   hooks — hooks registered for other triggers do not run.
2. Multiple sessionStart hooks execute sequentially in registry order,
   and reply `results` order equals execution order.
3. Every included element has exactly the keys `{id, name, hookId,
   originalType, content}` with `id == hookId` == registry id, `name` ==
   hook name, and `originalType == "runCommand"` always.
4. `content` is stdout when stdout is non-empty, else stderr — never a
   concatenation.
5. A hook exiting non-zero with output is still included (KAS parity:
   the carved producer has no exit-code filter).
6. Empty-output, spawn-failed, and timed-out hooks are skipped with a
   warn, and sibling hooks still appear in a well-formed reply.
7. Per-hook execution timeout is `action.timeout` seconds when present,
   else 60s, parsed into `HookDef`.
8. With zero sessionStart hooks the reply is `{results: []}` — stub
   parity, byte-compatible with the shipped behavior.
9. `USER_PROMPT` is set (to the empty string) in sessionStart hook
   environments.
10. The client RPC loop keeps serving other requests while a sessionStart
    hook runs.
11. `executeHook` wire replies are unchanged by the typed-outcome
    refactor (success/spawn-fail/timeout/cancel all byte-shape
    identical).
12. With the `.arn` fix, fence-probe KAS turns complete (the harness no
    longer poisons `profileArn`).

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| 1 | trigger filtering | registry w/ sessionStart + preToolUse marker hooks; responder runs → only sessionStart marker exists, 1 element | marker files on disk | 10m | pending | `session_start_runs_only_session_start_hooks` |
| 2 | order | two hooks echoing A then B → results `[A, B]` | reply array order vs registry order | 5m | pending | `session_start_results_in_registry_order` |
| 3 | element shape | key-set equality + literal originalType assert on a real run | carved producer field set (probe-carve-shape.sh output, independent of cyril) | 5m | **passed (live)** — shaped-arm reply consumed end-to-end 2026-07-23 | `session_start_element_shape_matches_carve` |
| 4 | stdout-precedence | hook `echo out; echo err >&2` → content == "out\n" | carved `stdout \|\| stderr` semantics; live KAS producer | 5m | **passed (carve)** — both bundles | `session_start_content_stdout_precedence` |
| 5 | non-zero included | hook `echo boom >&2; exit 3` → element present, content "boom\n" | carved producer (no exit filter) | 5m | pending | `session_start_nonzero_exit_still_included` |
| 6 | skip-not-fail | trio: empty-output + timeout(1s sleep 30) + healthy → reply has exactly the healthy element | reply structure + elapsed | 15m | pending | `session_start_skips_empty_and_timeout` |
| 7 | timeout parse | file `timeout: 2` on `sleep 1 && echo ok` → included (catches secs-as-millis); absent → HookDef reports 60s | file schema (kasHookFileSchema: seconds) | 10m | pending | `session_start_skips_empty_and_timeout` (carries the secs-as-millis catch; built-time fold-in, recorded in review-decisions P2) + `hook_def_default_timeout` |
| 8 | stub parity | empty registry → `{results: []}` | existing test (pre-dates change) | 1m | **passed** — `session_start_acknowledges_empty_results` green on branch | same test, retargeted at the async responder |
| 9 | USER_PROMPT set-empty | hook `printenv USER_PROMPT && echo SET` — unset env: printenv fails → empty output → element absent; set-empty: element present | printenv exit semantics (distinguishes unset from empty) | 10m | pending | `session_start_user_prompt_env_empty` |
| 10 | non-blocking | shell_type resolves <2s while a 3s sessionStart hook runs | tokio timing at resolution (jiyn claim-13 pattern) | 15m | pending | `slow_session_start_does_not_block_loop` |
| 11 | executeHook unchanged | existing four executeHook fences pass after the refactor | pre-existing tests (written against the old code) | 1m | pending | `execute_hook_real_exit_codes` + `execute_hook_timeout_kills` + `execute_hook_cancel_reaps` + `pre_tool_use_exit2_block_contract` |
| 12 | harness fix | fixed `token()` → KAS turn completes | live KAS (KRS accepts the request) | 0m | **passed** — today's control+shaped arms completed with the identical fix | manual — per-release fence probe (pre-approved pattern from jiyn: re-run both arms per release) |

Cheapest falsifiers already run and passed: #3 (live shaped-arm), #4
(carve, both bundles), #8 (existing test), #12 (today's live arms).

## Negative space

1. **No `askAgent` packaging.** The registry loads `runCommand` actions
   only; agent-type actions remain cyril-n03f — with a close-out note
   that the carve answers n03f's open investigation for SessionStart
   (precomputed results ARE the wire path; content = the prompt text).
2. **No cancellation** of in-flight sessionStart executions — the wire
   carries no `operationId` for this method; the per-hook timeout is the
   only bound.
3. **No hooks-authority briefing.** The live probe showed an unbriefed
   model may refuse injected `HOOK_INSTRUCTION` content; the steering
   mitigation is queued at `.cyril-tpfd/to-file.md` item 2 (filed at
   close-out; jsonl stays off this branch per parallel-session rule).
4. **No registry hot-reload** — cyril-2adk.
5. **No precomputed results for non-sessionStart triggers** — KAS's ACP
   provider only ever requests sessionStart (carved guard).
6. **No stdout/stderr interleaving fidelity** — we reuse the executor's
   captured pipes; KAS parity requires only stdout-else-stderr.

## Open decisions for the pause

- **D1 — failed-hook output parity.** A hook that exits non-zero but
  prints output is INCLUDED (claim 5), exactly matching KAS's own
  producer. Stricter filtering (exclude on non-zero) would diverge from
  engine behavior. Recommend: parity.
- **D2 — harness fix scope.** Fix `.cyril-jiyn/probe-hooks-ab-2.13.0.py`
  in this branch (claim 12) since it's the load-bearing per-release
  fence and the bug provably poisoned its turns. Recommend: yes.

## Approval

APPROVED at hard pause 2026-07-23 (user: "approved"). D1 = parity
(non-zero-exit hooks with output are included, matching the KAS
producer). D2 = yes (fence-probe token() fix ships in this branch).
