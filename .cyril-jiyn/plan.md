# cyril-jiyn — budgeted plan

Design: `.cyril-jiyn/design.md` (approved: default `host`, knob now, manual
fences ok, sessionStart executes). Gates per slice: `cargo test --workspace`
(default AND `--features kas`), `cargo clippy --all-targets -- -D warnings`
(both builds), `cargo fmt --check` — real exit codes, echo-verified.

Global notes: no stdout writes anywhere (diagnostics via `tracing`); no
load-bearing doc-comment preconditions are introduced (registry/list/executor
fns are total over their inputs; invalid input → skip-with-warn or error
reply, enforced in code, per-slice below). Loop budgets: every new loop is
O(hook files) or O(registered hooks) — production scale ≤ dozens; nothing
approaches 10^6 ops or 10^3 syscalls. The only spawned processes are
user-authored hook commands, bounded by the 60s default timeout.

## Slice 0: absorb cyril-jmjb — `SpawnConfig` bundles the spawn-knob clump

**Claim:** enabler (design architecture; closes cyril-jmjb — verified filed
2026-07-19): `spawn_bridge(agent_command, config: SpawnConfig, cwd)` where
`SpawnConfig {engine, kas_spawn, present_as}` (kas_hooks joins in slice 2a);
`Default` = (V2, Free, Cyril).
**Oracle:** existing test suite green both builds — behavior byte-identical;
the probe-A wire capture re-run must produce the identical initialize frame.
**Stress fixture:** the existing identity fences (`client_info_*`,
`advisory_matrix`) — a mis-threaded field (e.g. present_as dropped in the
bundling) fails them; plus probe-A byte-compare.
**Loop budget:** none. **Wall budget:** n/a.
**Files (atomic signature ripple, justified as in cyril-0wyn slice 4):**
`crates/cyril-core/src/protocol/bridge.rs` (struct + signatures),
`crates/cyril/src/main.rs`, `crates/cyril/examples/test_bridge.rs`,
`crates/cyril/examples/l7tw_death_probe.rs`, and the four kas smoke tests —
each caller shrinks to `SpawnConfig { engine: …, ..Default::default() }`.

**Verification:**
- [ ] Full suite green, both builds
- [ ] Probe-A re-capture byte-identical to `probe-a-post-impl-capture.jsonl`
- [ ] prove-it oracle unaffected
- [ ] Budgets hold (trivially)

## Slice 1: `KasHooksMode` enum + `[agent] kas_hooks` config field

**Claim:** 3 (invalid value → whole-file default posture) + config halves
of 2. `Host` (default) | `Kas` | `Off`; TOML `"host"|"kas"|"off"`.
**Oracle:** TOML literals in fixtures (not the enum's own serialization).
**Stress fixture:** `kas_hooks = "both"` (plausible user guess for the
composition that doesn't exist) → whole-config defaults with other valid
keys reverted; `"Host"` case variant → same; absent → `Host`.
**Loop budget:** none. **Wall budget:** n/a.
**Files:** `crates/cyril-core/src/types/kas_hooks.rs` (new),
`crates/cyril-core/src/types/config.rs`,
`crates/cyril-core/src/types/mod.rs` (registration line).

**Verification:**
- [ ] Units pass incl. `invalid_kas_hooks_falls_back_to_default_config`
- [ ] Stress fixture expected outcomes
- [ ] Oracle agrees
- [ ] Budgets hold

## Slice 2a: `kas_hooks` joins `SpawnConfig`; main.rs plumbs it

**Claim:** plumbing half of 2. **Oracle:** compile + existing suite (an
unused-field bug is caught in 2b's matrix). **Stress fixture:** n/a — pure
field addition; adversarial coverage lands in 2b (combined-slice rationale:
this slice exists only to keep 2b two-file).
**Loop budget:** none. **Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (SpawnConfig field +
Default), `crates/cyril/src/main.rs` (config.agent.kas_hooks).

**Verification:** suite green both builds; budgets trivially hold.

## Slice 2b: advertisement matrix — engine carries the mode, meta merges hooks

**Claim:** 2. `engine_for(engine, &SpawnConfig)` → `KasEngine { hooks_mode }`;
`kas/settings.rs` gains `kiro_client_meta(hooks_mode)` assembling
`_meta.kiro = {settings: …, hooks?: …}`: Host → `{enabled:true}`, Kas →
`{enabled:true, v2:true}`, Off → key absent. V2Engine untouched.
**Oracle:** covenant §2 key shapes (doc) — asserted against serialized JSON
of the built capabilities, not the constructor's own enums.
**Stress fixture:** the 3×2 mode×engine matrix under `--features kas`; the
V2 cells catch cfg-keying (dn91 trap); the Off cell asserts the `hooks` key
is ABSENT (not `{enabled:false}` — sentinel rule); Host cell asserts `v2`
key ABSENT (not false).
**Loop budget:** none. **Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/engine.rs`,
`crates/cyril-core/src/protocol/kas/settings.rs` (+ the `engine_for` call
site in bridge.rs — one line, counted honestly).

**Verification:**
- [ ] `kas_hooks_advertisement_matrix` passes under `--features kas`
- [ ] Off/Host absent-key asserts pass
- [ ] Oracle: serialized JSON vs covenant shapes
- [ ] Budgets hold

## Slice 3: `HookRegistry` — load, map, skip-warn

**Claim:** 4. Loads `<cwd>/.kiro/hooks/*.json` + `~/.kiro/hooks/*.json`
once; parses `{version, hooks[]}`; PascalCase→camelCase trigger map
(`UserPromptSubmit→promptSubmit`, `Stop→agentStop`, `PreToolUse→preToolUse`,
`PostToolUse→postToolUse`, `SessionStart→sessionStart`); skip-with-warn:
unparseable file, unknown version, unknown trigger, non-command action
(agent-type → cyril-n03f, verified filed); ids namespaced `<stem>:<name>`.
**Oracle:** fixture files on disk (tempdir) hand-enumerated vs loaded set.
**Stress fixture:** one load containing: valid file with 2 hooks (one with
matcher), invalid-JSON file, unknown-trigger hook, agent-action hook, and a
duplicate hook name in a second file — expected: 3 loaded (2 + the
duplicate under its distinct id), 3 skip warns, load does NOT abort.
**Loop budget:** O(files × hooks-per-file), production ≤ dozens; one
readdir + one read per file (syscalls ≪ 10^3).
**Wall budget:** startup-only, sub-ms at scale.
**Files:** `crates/cyril-core/src/protocol/kas/hooks.rs` (new),
`crates/cyril-core/src/protocol/kas/mod.rs` (registration).

**Verification:**
- [ ] `hook_registry_loads_and_maps`, `hook_registry_skips_invalid_without_aborting`
- [ ] Stress fixture expected counts
- [ ] Oracle agrees
- [ ] Budgets hold

## Slice 4: `list` filtering

**Claim:** 5. `list(trigger, toolId)` → hooks whose wire trigger matches;
matcher (regex) present → include only when it matches toolId (absent
toolId → matcher hooks excluded); unknown trigger → empty vec, never error.
**Oracle:** hand-enumerated expected sets per query.
**Stress fixture:** registry with {matcher `fs_.*` hook, no-matcher hook}
queried as (preToolUse, toolId="fs_write") → both; (preToolUse,
toolId="execute_bash") → no-matcher only; (preToolUse, None) → no-matcher
only; ("bogusTrigger", _) → empty. Catches: matcher ignored (first query
equals second), matcher-on-missing-toolId panic.
**Loop budget:** O(registered hooks) per call, ≤ dozens.
**Wall budget:** per-trigger call path, µs.
**Files:** `crates/cyril-core/src/protocol/kas/hooks.rs`.

**Verification:** `hooks_list_filtering`; fixture expected sets; budgets hold.

## Slice 5a: executor — spawn, env, cwd, output, real exit codes

**Claims:** 6, 7, 8. Async spawn (tokio, `kill_on_drop`) with `USER_PROMPT`
env + workspace cwd; reply `{output: stdout+stderr combined, exitCode:
real, cancelled:false}`; exit 2 passes through verbatim (the block
contract).
**Oracle:** the OS — subprocess stdout/exit codes.
**Stress fixture:** three commands: `printf "$USER_PROMPT"; pwd` (env+cwd),
`sh -c 'echo out; echo err >&2; exit 1'` (combined output + real nonzero),
`sh -c 'echo DENY; exit 2'` (block contract — catches bool-success mapping
and exit-code clamping).
**Loop budget:** none beyond output collection O(bytes), timeout-bounded.
**Wall budget:** test commands < 1s each.
**Files:** `crates/cyril-core/src/protocol/kas/hooks.rs`.

**Verification:** `execute_hook_env_and_cwd`, `execute_hook_real_exit_codes`,
`pre_tool_use_exit2_block_contract` (the AC's named fence); budgets hold.

## Slice 5b: executor — timeout + cancel + reap

**Claims:** 9, 10. Default timeout 60s, `timeout` param override; expiry
kills the child, reply marks it (cancelled:true per covenant exitCode
semantics — exact shape pinned against the covenant .d.ts during the
slice); abort registry keyed by `operationId`; cancel aborts + reaps;
unknown operationId → warn no-op.
**Oracle:** the OS process table (`ps -o stat=`, the portable-liveness
pattern from feedback memory).
**Stress fixture:** `sleep 30` hook with 500ms timeout override → child
dead (ps) + reply within ~1s (catches timer-without-kill); `sleep 30` +
cancel(operationId) → `{cancelled:true}` + child dead (catches the lw67
class: cancel during pending wait as silent no-op); cancel("bogus") → warn,
no panic, no effect on the running hook.
**Loop budget:** none. **Wall budget:** tests ≤ ~3s total (timeouts are
sub-second overrides).
**Files:** `crates/cyril-core/src/protocol/kas/hooks.rs`.

**Verification:** `execute_hook_timeout_kills`, `execute_hook_cancel_reaps`;
liveness asserts pre- and post-kill; budgets hold.

## Slice 6: sessionStart execution + results

**Claim:** 11. `sessionStart` runs the registry's sessionStart hooks via
the 5a executor and replies `{results:[…]}` per the covenant
`AcpPrecomputedHookResult` shape (pinned against the covenant .d.ts inside
the slice); empty registry → `{results:[]}`.
**Oracle:** covenant .d.ts shape + OS output of the fixture hook.
**Stress fixture:** registry with one sessionStart hook (`echo started`) +
one promptSubmit hook — reply contains exactly ONE result (catches
trigger-filter bypass running every hook at session start).
**Loop budget:** O(sessionStart hooks). **Wall budget:** session-start
path; fixture < 1s.
**Files:** `crates/cyril-core/src/protocol/kas/hooks.rs`.

**Verification:** `session_start_results`; one-result assert; budgets hold.

## Slice 7: wire dispatch — method routing + notifications

**Claims:** 12 + the wiring half of 5-11. Method consts (acp-stripped, the
`SHELL_TYPE_METHOD` pattern): `kiro/hooks/{list,executeHook,sessionStart}`
requests routed in `client.rs::handle_ext_request` to the registry/executor;
`kiro/hooks/{cancel,didChange}` notifications: cancel → abort registry,
didChange → consumed + logged. Registry held by KiroClient (the terminals-Rc
pattern), constructed at bridge startup from cwd when the engine is KAS and
mode is Host.
**Oracle:** the 2.7.1/2.13.0 wire captures (method strings as KAS actually
sends them) — not cyril's own consts.
**Stress fixture:** dispatch table test: each method string routes to its
responder; an unknown `kiro/hooks/x` falls to the existing unknown-ext
path (not a panic); cancel with an in-flight operation aborts it
end-to-end through the dispatch (not just the unit-level abort).
**Loop budget:** none (match arms). **Wall budget:** per-request, µs.
**Files:** `crates/cyril-core/src/protocol/client.rs`,
`crates/cyril-core/src/protocol/kas/hooks.rs`.

**Verification:** `hooks_dispatch_routes`, `hooks_did_change_consumed`;
budgets hold.

## Slice 8: non-blocking integration fence

**Claim:** 13. A `sleep`-hook `executeHook` in flight does not serialize the
bridge: a concurrent `shell_type` ext request completes while the hook runs.
**Oracle:** wall-clock ordering of the two replies (tokio time, not
implementation internals).
**Stress fixture:** in-process fake-agent harness (the bridge.rs test rig):
fire executeHook (`sleep 2` hook) then immediately shell_type; expected:
shell_type reply arrives < 500ms while executeHook resolves ~2s later. A
synchronous spawn implementation fails the ordering.
**Loop budget:** none. **Wall budget:** the test itself ~2.5s.
**Files:** `crates/cyril-core/src/protocol/client.rs` (only if routing
needs a test seam) / test module in `kas/hooks.rs` or `bridge.rs` tests —
2-file cap respected at implementation time.
**Verification:** `slow_hook_does_not_block_loop`; ordering asserts.

## Slice 9: docs — decided default + checklist fence

**Claims:** 14 + claim 1's approved manual fence. ROADMAP KAS-7 entry
records the decided default (`host`), the knob, and the no-composition
finding; `experiments/conductor-spike/README.md` checklist gains the
per-release `probe-hooks-ab-2.13.0.py` re-run line (glob-update note, like
the 0wyn line).
**Oracle:** grep — `kas_hooks`, `host`, "do not compose" in ROADMAP KAS-7;
probe filename in the README checklist; against `.cyril-jiyn/` artifacts.
**Stress fixture:** the grep MUST find the no-composition statement (a lazy
edit that only names the default fails it — the composition finding is the
load-bearing correction to the stale milestone text).
**Loop budget:** none. **Wall budget:** n/a.
**Files:** `docs/ROADMAP.md`, `experiments/conductor-spike/README.md`.

**Verification:** claim-14 grep (3 patterns) + checklist grep; budgets hold.

## Plan Self-Review

1. **Loops:** registry load O(files×hooks), list O(hooks), sessionStart
   O(hooks) — all ≤ dozens at production scale; syscalls bounded by file
   count at startup. No gaps.
2. **Fixtures:** S0 byte-compare (mis-threading), S1 "both"/case variants,
   S2b absent-key + dn91 cells, S3 mixed-validity load, S4 matcher/no-toolId
   cells, S5a exit-code clamp + stderr drop, S5b timer-without-kill + lw67
   cancel class, S6 trigger-filter bypass, S7 unknown-method fall-through +
   end-to-end cancel, S8 synchronous-spawn ordering, S9 lazy-edit grep. All
   adversarial. No gaps.
3. **Doc-comment preconditions:** none load-bearing introduced; invalid
   inputs are handled in code (skip-warn / error replies / no-op warns) —
   each named in its slice. No gaps.
4. **Write targets:** `tracing` only (diagnostic); replies are protocol
   responses, not stdout. No gaps.
5. **Tracker references:** cyril-jmjb (absorbed, S0 — update at close-out),
   cyril-2adk (hot-reload), cyril-n03f (agent-type actions), cyril-oiyt
   (panel UI) — all verified existing this session. No gaps.

Claim coverage: 1→probe(passed)+S9 fence; 2→S1/S2a/S2b; 3→S1; 4→S3; 5→S4;
6/7/8→S5a; 9/10→S5b; 11→S6; 12→S7; 13→S8; 14→S9. Complete.
