# cyril-jxmv — falsifiable design: key path translation off agent location

Probe basis: `.cyril-jxmv/findings.md` (probe/oracle AGREE-OK, 2026-08-05).
The design extends the probe's facts and contradicts none of them.

## Purpose

On Windows, `to_agent`/`to_native` currently translate unconditionally
(`cfg!(target_os = "windows")`), assuming a WSL-hosted agent. The default
agent command is the native `kiro-cli acp` (kiro-cli.exe from the MSI), so
the outbound session cwd is corrupted (`C:\…` → `/mnt/c/…`, probe fact 1) and
inbound `/`-rooted paths corrupt when a distro is configured (probe fact 2).
Fix: translation activates only when the **resolved spawn command** routes
through the WSL launcher; a native agent gets identity passthrough exactly
like Linux.

## Core rule

```
translation active ⟺ cfg!(windows) ∧ agent_location() == Wsl
agent_location    = env CYRIL_AGENT_LOCATION override, else wsl-launcher
                    detection on the RESOLVED spawn command's program
```

## Architecture / placement (step 2c)

- **Owner: `cyril-core/src/platform/path.rs`** — new `AgentLocation` enum
  (`Native | Wsl`), a private process-global `AtomicU8`
  (`Unset | Native | Wsl`, last-spawn-wins), `pub fn set_agent_location`,
  `pub fn agent_location`, and the pure
  `pub fn resolve_agent_location(env: Option<&str>, program: &str) ->
  AgentLocation`. Policy and mechanism stay in the one module that owns
  translation; no other crate consumes the type, so `types/` would be a
  gratuitous layer hop.
- **Call site: `run_bridge` (protocol/bridge.rs)** — exactly one call,
  after `resolve_spawn_command` returns and before `AgentProcess::spawn`,
  on `spawn_command.program()`. Engine-independent by construction (sits
  after the engine match), which is what makes probe fact 4 (KAS free
  replaces the argv) hold for free.
- **Detection is manual-split, not `Path::file_name`**: basename = substring
  after the last `/` OR `\`, ASCII-lowercased, optional `.exe` stripped,
  exact `== "wsl"`. `Path::file_name` would split `C:\…\wsl.exe` correctly
  on Windows but return the whole string on Linux — cross-platform-divergent
  detection is untestable on Linux CI (the cyril-xi4a/selection-matrix
  lesson: fully fake, component-wise-identical behavior everywhere).
- **Gate scope: `to_native` + `to_agent` only** (the policy entry points —
  the two production call sites, probe fact 3). `win_to_wsl`, `wsl_to_win`,
  and `translate_paths_in_json` remain ungated mechanism: `translate_paths_
  in_json` has zero production callers (probe fact 5) and translates
  OS-independently today; gating it would churn its test corpus for no
  behavioral consumer. Its doc comment gains a "callers must consult
  `agent_location()`" note.
- **Unset reads = `Native` (identity) + one debug log.** All production
  translation happens after spawn, which always sets location; unset occurs
  only in tests/misuse. Identity is the do-no-harm default and matches the
  shipped default agent command.
- **Forbidden:** KAS host-io (`host_io.rs`, `kiro_fs.rs`, `terminal_io.rs`)
  must NOT re-derive location or read the env var — they keep calling
  `to_native` and inherit the gate. `convert/` and `cyril-ui` never see the
  type. The setter is never called from per-engine arms. `CYRIL_AGENT_
  LOCATION` is read in path.rs only.
- **New seam: none** — extend-existing behind the current `to_native`/
  `to_agent` interface; no `design-an-interface` run needed.

## Input shapes (step 2)

1. **Program string** (resolved spawn command): bare native (`kiro-cli`,
   `sh`, `node`), `.exe` native (`kiro-cli.exe`), bare launcher (`wsl`),
   launcher with ext (`wsl.exe`, case variants), full launcher path
   (`C:\Windows\System32\wsl.exe`, case variants), full native path
   (`/usr/bin/node`, `C:\Program Files\nodejs\node.exe`), near-misses
   (`wslkiro`, `my-wsl-wrapper.exe`, `wsl2`), trailing-space (`"wsl "` —
   literal, no trim, CYRIL_WSL_DISTRO precedent). Empty program:
   unrepresentable (`AgentCommand` non-empty invariant) — out of scope.
2. **Env override `CYRIL_AGENT_LOCATION`**: unset / `native` / `wsl` /
   case variants (`Native`, `WSL`) / empty (= unset, `nonempty_distro`
   precedent) / invalid (`banana`) → warn + fall back to heuristic.
3. **Location state at read time × host OS**: {Unset, Native, Wsl} ×
   {windows, other} — six cells, all claimed (C1/C2/C3/C7).
4. **Engine resolution**: v2 verbatim / KAS free (argv replaced with node)
   / KAS wrapper (program preserved) (C6).
5. **Path shapes** through the gate: unchanged mechanics, covered by the
   existing corpus (probe + cyril-8tq6 fences); the gate only decides
   *whether* mechanics run (C1/C2 reuse the corpus shapes).
6. Exotic wrapper (script internally invoking wsl): undetectable from argv
   — served by the env override (C5), documented; no auto-detection
   (negative space #2).

## Removed-invariant sweep (step 2b)

The change is subtractive: it removes "on Windows, every boundary path is
translated." What that invariant silently guaranteed:

- *WSL agents keep working*: preserved by construction — `wsl`-prefixed
  commands classify `Wsl` and take today's exact code path (C2, C4).
- *`to_native_checked`'s absolute check*: judged on the agent path
  pre-translation via `has_root()` (host_io.rs:201) — gate-off changes only
  the translation half; a native agent's `C:\…` has root on Windows. Safe,
  one-sentence reason recorded here.
- *Linux identity*: never depended on the removed invariant (cfg! arm), but
  gains a new hazard — a buggy gate keyed on location alone would translate
  on Linux when location=Wsl. Fenced explicitly (C3).

## Claims and falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| C1 | With location=Native on Windows, `to_agent`/`to_native` return input byte-identical for every corpus shape (drive, POSIX, UNC, ext-prefix) | Windows child-process fence, location unset AND explicitly Native, assert identity on corpus shapes; buggy impl caught: gate inverted, or default=Wsl | expected = input strings themselves (no computation) | unit-CI | pending | `win_wsl_wiring.rs::native_agent_no_rewrite` (new, cfg windows) |
| C2 | With location=Wsl on Windows, behavior is byte-identical to today (drive unconditional; `\\wsl$` per cyril-8tq6 distro rules) | run the existing win_wsl_wiring WSL fences with the gate ON (child env `CYRIL_AGENT_LOCATION=wsl`); zero edits to existing path.rs unit corpus; buggy impl caught: gate blocks the Wsl arm or reorders distro checks | pre-change test corpus (written before this design) | unit-CI | pending | existing `env_distro_wiring_via_child_process` + gated-on variant; existing path.rs unit tests unmodified |
| C3 | On non-Windows, translation is identity for ALL location states incl. explicitly-set Wsl | extend `linux_translation_is_noop` to set location=Wsl first; buggy impl caught: gate keyed on location without cfg! | expected = input strings | unit-CI | pending | `win_wsl_wiring.rs::linux_translation_is_noop` (extended) |
| C4 | `resolve_agent_location` heuristic: Wsl iff basename (manual split both separators, ASCII-lc, optional `.exe` strip) == `wsl`; near-misses Native | table of 14 real + adversarial programs (falsifier-c4c6.py) | Microsoft launcher ground truth (System32\wsl.exe, CreateProcess .exe append, NTFS case-insensitivity), annotated per row | 2m | **passed** (falsifier-c4c6.py, FALSIFIER-PASSED) | path.rs unit `resolve_agent_location_*` table mirroring the script |
| C5 | Env override wins over heuristic; `native`/`wsl` ASCII-case-insensitive; empty = unset; invalid → warn + heuristic | unit table: (env, program) grid incl. (`wsl` env, `kiro-cli` prog) → Wsl and (`native` env, `wsl` prog) → Native; buggy impl caught: heuristic-first ordering, or invalid silently → Native (sentinel-default violation) | grid expectations enumerated in design (this table) | unit-CI | pending | path.rs unit `env_override_*` |
| C6 | Location derives from the RESOLVED spawn command: KAS free with `wsl`-prefixed CLI argv classifies Native (argv replaced by node); v2 & wrapper preserve the program | pre/post-resolve divergence (falsifier-c4c6.py, passed); Rust side: unit on `discovery::resolve` output fed to `resolve_agent_location`; buggy impl caught: deriving from pre-resolve CLI argv | discovery.rs:228 argv construction (read directly, not via the heuristic) | 2m/unit | **passed** (script half) | unit `kas_free_resolved_command_is_native` + bridge integration (C8's test asserts via getter) |
| C7 | Unset location on Windows = identity + one debug log | Windows child fence with nothing set, corpus passthrough asserted; buggy impl caught: default=Wsl (today's behavior surviving) | expected = input strings | unit-CI | pending | same fence as C1 (unset half is a distinct child) |
| C8 | `run_bridge` sets location after `resolve_spawn_command`, before spawn, both engines, exactly once | Linux bridge integration: spawn_bridge with `sh` stub → after SessionCreated, `agent_location()==Native` (was Unset at process start); buggy impl caught: set only in v2 arm (structural: single call site after the match — review-enforced Forbidden), or set skipped entirely | `agent_location()` getter observed from the test process, vs process-start Unset | unit-CI | pending | bridge test `spawn_sets_agent_location` |
| C9 | `CYRIL_AGENT_LOCATION` is read only in path.rs | source-scan test: walk `crates/*/src`, count occurrences outside platform/path.rs == 0; buggy impl caught: host-io reading the env to re-derive location | filesystem contents via CARGO_MANIFEST_DIR (independent of any cyril code) | unit-CI | pending | `path.rs` unit `env_var_confined_to_this_module` |

Cheapest falsifier (C4+C6 script): **run and passed** before this design was
presented — `.cyril-jxmv/falsifier-c4c6.py`, output `FALSIFIER-PASSED`.

Per-claim distinctness: C1/C7 are separate child processes with named tests;
C2's gated-on child is distinct from C1's; C3–C9 each name their own test.

## Negative space (what this deliberately does not do)

1. **No CLI flag** (`--agent-location`): env-only override. Settled choice —
   the ticket offered "CLI flag or env"; env matches the existing
   `CYRIL_WSL_DISTRO` surface and needs no clap churn. Not deferred work.
2. **No detection of exotic wrappers** (scripts that invoke wsl internally):
   undetectable from argv; `CYRIL_AGENT_LOCATION=wsl` is the documented
   escape hatch.
3. **No change to translation mechanics**: drive-mount rules, `\\wsl$` UNC
   semantics, distro resolution (cyril-8tq6) untouched; `translate_paths_
   in_json` stays ungated mechanism with zero production callers.
4. **No multi-bridge/per-session location**: process-global, last-spawn-wins;
   cyril runs one bridge per process today. (`_kiro/workflow` peer sessions
   ride the same bridge/agent, same location.)
5. **Not fixing cyril-861q** (wrapper version probe targets argv[0] — filed
   during this run) nor the open Windows host-io family: cyril-lwpm (auth
   store in WSL), cyril-f2fv (UNC cwd terminal verify), cyril-trkw (distro
   auto-detect). cyril-duz0 keeps its own docs surfaces; this PR touches only
   the path-translation sections of CLAUDE.md (AC 5 coordination).

## Docs (AC 5)

CLAUDE.md "Path Translation" + "Platform Constraints" gain the agent-location
conditionality: translation active only for `wsl`-launcher spawn commands or
`CYRIL_AGENT_LOCATION=wsl`; native Windows agents = passthrough like Linux.
Worded to complement, not preempt, cyril-duz0's broader doc fixes.

## Open decisions for approval

1. **Env var name/shape**: `CYRIL_AGENT_LOCATION=native|wsl` (recommended —
   names the fact, matches `CYRIL_WSL_DISTRO` family) vs
   `CYRIL_PATH_TRANSLATION=on|off|auto`.
2. **Unset-read default = Native** (identity, recommended) vs preserving
   today's translate-by-default. Native matches the shipped default agent
   command; unset never occurs on the production path (spawn always sets).
3. **Gate scope**: `to_native`/`to_agent` only (recommended);
   `translate_paths_in_json` stays ungated mechanism (zero production
   callers).
4. **C9's source-scan test**: include (recommended — mechanical fence for the
   env-confinement rule) or drop as over-fencing.
