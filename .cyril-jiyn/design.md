# cyril-jiyn — design: KAS-7 hooks host (advertisement + responders + the v2 decision)

Status: DRAFT — awaiting the HARD PAUSE decisions. Probes: `findings.md`
(A/B both arms MATCH vs the buildSessionHooks carve).

## Purpose

Make cyril the hook host on the KAS engine: advertise `_meta.kiro.hooks` at
initialize, own a hook registry loaded from the user's `.kiro/hooks/*.json`,
and implement the host responders (`_kiro/hooks/{list,executeHook,
sessionStart}` + the `cancel`/`didChange` inbound notifications) — the org
write/exec-policy interception point (exit-2 preToolUse block, wire-verified
2.7.1 and source-continuous on 2.13.0).

## The decision the probe reframed (for the pause)

The issue hypothesized the host model and the `v2:true` standalone loader
"may compose — probe to confirm". **They do not** (findings Q1: winner-take-
all per session at `buildSessionHooks`; live A/B: the v2 arm drove ZERO host
callbacks while KAS executed the disk hook in its own process). So the
choice is a true either/or per session:

- **host** (`{enabled:true}`): cyril executes hooks, sees every trigger, can
  block preToolUse — the strategic org-policy gate. Users' on-disk hooks run
  ONLY through cyril's registry.
- **kas** (`{enabled:true, v2:true}`): KAS's own loader executes on-disk
  hooks agent-side with confirm dialogs (cyril-497j renders them); cyril
  never sees hook execution — no gate, no audit.
- **off**: no advertisement; no hooks anywhere.

**Proposed**: a three-value config knob `[agent] kas_hooks = "host" | "kas"
| "off"`, **default `host`** — parity with kiro-cli's own behavior (hooks
are user-authored config, executed by the tool that reads them), and the
default that preserves cyril's reason for implementing KAS-7 at all.

## Architecture

1. **`KasHooksMode` enum** (cyril-core types, `present_as.rs` pattern):
   `Host` (default) | `Kas` | `Off`; TOML `kas_hooks`; invalid values follow
   the house whole-file-default posture.
2. **Advertisement**: the KAS engine's `client_capabilities()` composes the
   hooks key per mode into `_meta.kiro` (sibling of the nhzw `settings`
   key): Host → `{enabled:true}`, Kas → `{enabled:true, v2:true}`, Off →
   absent. The v2 engine's capabilities are untouched by the knob (bound-
   engine keying, the cyril-dn91 trap).
3. **`HookRegistry`** (cyril-core, KAS layer): loads workspace
   `<cwd>/.kiro/hooks/*.json` + global `~/.kiro/hooks/*.json` once at
   bridge startup. Parses `{version, hooks:[{name, trigger, matcher?,
   action}]}`; maps file-side PascalCase triggers (`UserPromptSubmit`,
   `Stop`, `PreToolUse`, `PostToolUse`, `SessionStart`) to the wire's
   camelCase (`promptSubmit`, `agentStop`, `preToolUse`, `postToolUse`,
   `sessionStart`) — probe finding Q2; unmappable triggers, unknown action
   types, and unparseable files are skipped with a warn, never aborting the
   load. runCommand/command actions only (agent-type: cyril-n03f).
4. **Responders** (KAS client layer, beside the fs/terminal host-io):
   - `list {trigger, toolId?, toolTags?}` → registry hooks matching trigger
     and (when a matcher exists) matcher-vs-toolId; unknown trigger → empty.
   - `executeHook {hookId, hookName, command, userPrompt, timeout?,
     operationId?}` → spawn the command async (never blocking the bridge
     loop) with `USER_PROMPT` in env, cwd = workspace, default timeout 60s;
     reply `{output: combined stdout+stderr, exitCode: real, cancelled}`.
     Abort handles registered by `operationId`; children `kill_on_drop`
     (the ba5x/lw67 lessons).
   - `sessionStart` → execute sessionStart-registered hooks, reply
     `{results:[...]}` (empty when none).
   - Notifications: `cancel` aborts the matching operationId's child;
     `didChange` is consumed and logged (registry reload: cyril-2adk).
5. **Docs**: the KAS wire-audit hooks section + covenant doc record the
   decided default and the A/B no-composition result (AC requirement;
   the full covenant re-sync remains cyril-mfkg).

## Input shapes

- **Knob**: `host` | `kas` | `off` | absent (→ default) | invalid string
  (→ whole-file default posture). Engine axis: V2 (knob inert) | Kas.
- **Hook sources**: workspace dir present/absent × global dir
  present/absent; empty dirs; multiple files; multiple hooks per file;
  duplicate names across files (both served; ids namespaced by file stem).
- **Hook entries**: trigger ∈ 5 mappable PascalCase names | unknown;
  matcher present (hit/miss vs toolId) | absent; action `command` |
  `agent` (skip+warn) | unknown (skip+warn).
- **`list` requests**: 4 turn triggers × toolId present/absent; unknown
  trigger.
- **`executeHook`**: exit 0 / nonzero / exit 2 / timeout (default and
  param-override) / cancelled mid-run; output on stdout, stderr, both.
- **Notifications**: cancel with known/unknown operationId; didChange.
- Out of scope: `workspacePaths` param on list (single-workspace cyril
  passes one cwd; multi-root is not a cyril concept today) — one-sentence
  justification: cyril sessions have exactly one cwd.

## Removed-invariant sweep (2b)

Additive at the protocol layer (new responders, new advertisement key). One
capacity-shaped risk checked: a synchronous executeHook implementation
would serialize the bridge loop for up to 60s per hook — not an invariant
removal but a new blocking hazard; claim 13 pins the non-blocking property.
No lock, guard, ordering, or uniqueness property is removed.

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|---|---|---|---|---|---|
| 1 | KAS 2.13.0 `{enabled:true}` drives host callbacks; `{enabled:true,v2:true}` drives zero and executes disk hooks agent-side; no per-session composition | the A/B probe, expectations pre-registered | carved `buildSessionHooks` (source vs live behavior) | 20m | **passed** (both arms MATCH) | **manual (release-audit)**: re-run `probe-hooks-ab-2.13.0.py` per release (checklist line lands in this PR; needs pause approval like 0wyn claim 2) |
| 2 | Knob→advertisement mapping: Host→enabled, Kas→enabled+v2, Off→absent — on the KAS engine only | unit over the 3×2 (mode×engine) matrix under `--features kas` | the covenant §2 key shapes (doc, not the impl) | 5m | pending | unit `kas_hooks_advertisement_matrix` (catches: v2 always sent; off still advertising; cfg-keying — the dn91 trap via the V2 cells) |
| 3 | Invalid `kas_hooks` TOML value → whole-config warn + defaults | unit: `kas_hooks = "both"` + other valid keys → all defaults | `Default` impl vs parsed struct | 5m | pending | unit `invalid_kas_hooks_falls_back_to_default_config` |
| 4 | Registry loads workspace+global files, maps PascalCase→camelCase, and skips invalid files/triggers/actions with a warn without aborting | unit fixtures: valid+invalid JSON+unknown trigger+agent action in one load | fixture files on disk (tempdir) vs loaded set | 10m | pending | units `hook_registry_loads_and_maps`, `hook_registry_skips_invalid_without_aborting` (catches: whole-load abort on one bad file; identity trigger mapping) |
| 5 | `list` returns exactly the trigger-matching hooks, honoring matcher-vs-toolId; unknown trigger → empty, not error | unit: registry with matcher/non-matcher hooks queried across triggers | hand-enumerated expected sets | 10m | pending | unit `hooks_list_filtering` (catches: matcher ignored; unknown-trigger panic/error) |
| 6 | `executeHook` runs with `USER_PROMPT` env and workspace cwd | unit: command printing `$USER_PROMPT` + `pwd`, assert reply output | the OS (subprocess output) | 10m | pending | unit `execute_hook_env_and_cwd` |
| 7 | Reply carries combined stdout+stderr and the REAL exit code — 0, 1, and 2 verbatim | unit: three commands (`echo`, `sh -c 'echo e >&2; exit 1'`, `exit 2`) | the OS exit codes | 10m | pending | unit `execute_hook_real_exit_codes` (catches: bool-success mapping, stderr dropped) |
| 8 | The preToolUse block contract: an exit-2 hook reply is `{output, exitCode:2, cancelled:false}` verbatim (agent-side blocking wire-verified 2.7.1; source-continuous 2.13.0) | unit on the reply shape for an exit-2 command | covenant §10 block semantics + the 2.7.1 capture | 10m | pending | unit `pre_tool_use_exit2_block_contract` (the AC's named regression test) |
| 9 | Default timeout 60s, `timeout` param honored; expiry kills the child and the reply says so | unit with a short override (e.g. 500ms) + `sleep 30` command; child liveness via `ps -o stat=` | the OS process table | 15m | pending | unit `execute_hook_timeout_kills` (catches: timer without kill; reply lying about completion) |
| 10 | `cancel {operationId}` aborts the running hook: reply `{cancelled:true}` and the child is reaped; unknown operationId is a warn no-op | unit: `sleep 30` hook + cancel; `ps -o stat=` liveness; then cancel a bogus id | the OS process table | 15m | pending | unit `execute_hook_cancel_reaps` (catches: the lw67 bug class — cancel during pending wait as silent no-op) |
| 11 | `sessionStart` executes sessionStart hooks and replies `{results:[...]}` (empty when none registered) | unit: registry with/without a sessionStart hook | covenant §1a response shape | 10m | pending | unit `session_start_results` |
| 12 | `didChange` is consumed without error | unit: inject the notification through the converter | converter dispatch table | 5m | pending | unit `hooks_did_change_consumed` |
| 13 | A running hook never blocks the bridge: a second responder call completes while a `sleep` hook runs | integration on the in-process harness: concurrent shell_type (or fs) call during a slow executeHook | wall-clock ordering of the two replies | 30m | pending | integration `slow_hook_does_not_block_loop` (catches: synchronous spawn on the LocalSet) |
| 14 | Audit doc records the decided default + the no-composition A/B | grep for the decided mode + "do not compose" in the hooks section | `.cyril-jiyn/` artifacts | 2m | pending | manual (docs; review-time — pause approval requested) |

Cheapest falsifier: claim 1 — already run and passed (the A/B). Every
pending fence names its buggy implementation in-line. Distinctness: each
claim has its own named test or grep.

## Negative space

1. **No hot-reload**: the registry loads once per bridge; file-watching is
   cyril-2adk (verified: filed this session). The agent re-queries `list`
   per trigger, so a future reload needs no protocol change.
2. **No agent-type hook execution** in host mode — skipped with a warn;
   cyril-n03f (verified: filed).
3. **No hooks-panel UI** for the KAS registry — cyril-oiyt (verified:
   filed; distinct from cyril-uw20's responsiveness work).
4. **No blending of models**: `kas` mode is a knob value, not a default,
   and never combines with host responders (probe: composition is
   impossible upstream).
5. **No output-size cap** on hook output in v1 — the timeout is the bound;
   revisiting belongs with the registry work if real hooks prove chatty
   (settled rationale: hooks are user-authored short commands; the 60s
   timeout bounds runaway processes, and `terminal/create outputByteLimit`
   precedent (cyril-1rpv) covers the pattern if needed).
6. **No client-side createHook / hook-authoring** — cyril reads what the
   user wrote; authoring UX stays with Kiro's tools.

## Open decisions for the pause

1. **Default mode**: `host` (recommended — the org-policy gate is KAS-7's
   point; parity with kiro-cli executing user-authored hooks) vs `off`
   (conservative: no hook execution until opted in) vs `kas`.
2. **Ship the three-value knob now** (recommended) vs fixed `host`-only.
3. **Claims 1 and 14 fences are `manual (release-audit)` / review-time** —
   need your explicit approval per the fence rule.
4. **sessionStart hooks execute** (recommended) vs empty-reply stub.
