# cyril-jxmv — budgeted plan

Design: `.cyril-jxmv/design.md`, approved 2026-08-05 with all four open
decisions as recommended. One placement amendment made at planning time
(recorded in the design's C8 row rationale below and flagged in the PR):

> **C8 amendment**: the location is bound in `AgentProcess::spawn`
> (transport.rs), not `run_bridge`. Spawn receives the *final* argv after
> engine resolution, so binding there covers v2, KAS-free, KAS-wrapper, the
> `test_bridge` example, and every future caller by construction — a strict
> superset of the design's run_bridge claim, with ordering (bound before
> exec) guaranteed structurally. The design's Forbidden rules are unchanged.
>
> **REVERTED during slice 4** (audit trail, not deleted): impact analysis
> found a second production caller of `AgentProcess::spawn` —
> `terminal_io::TerminalRegistry::create` (KAS terminal host callback) —
> which would rebind the location from every terminal command's program
> mid-session, clobbering a WSL binding. The approved design's original
> placement (`run_bridge`, after `resolve_spawn_command`, before spawn)
> shipped instead. Slice 4's commit message carries the caller list.

Gates per slice (all must pass before the slice commits):
`cargo nextest run` · `cargo clippy --all-targets -- -D warnings` ·
`cargo fmt --check` · `cargo test --doc` · `bash .cyril-jxmv/probe.sh`
(prove-it oracle continuity — probe exercises ungated mechanism + Linux
identity, must stay AGREE-OK unchanged).

---

## Slice 1: AgentLocation type + pure resolution (heuristic ⊕ env override)

**Claim:** C4 (wsl-launcher detection), C5 (env override precedence).
**Oracle:** `.cyril-jxmv/falsifier-c4c6.py` expectations (Microsoft launcher
ground truth) — the Rust unit table mirrors the script's 14 rows; run the
script after implementing and diff verdicts.
**Stress fixture:** near-misses `wslkiro`/`wsl2`/`my-wsl-wrapper.exe`;
trailing-space `"wsl "` (literal, no trim); mixed-case full path
`c:\windows\system32\WSL.exe` — parsed with the manual both-separator split
ON LINUX (the `Path::file_name` cross-platform divergence bug class); env
grid: (`wsl`, `kiro-cli`)→Wsl, (`native`, `wsl`)→Native, (`WSL` case)→Wsl,
(``)→heuristic, (`banana`)→warn+heuristic.
**Loop budget:** basename split O(len(program)), once per resolution call;
production scale = 1 call per spawn. No collection loops.
**Files:** `crates/cyril-core/src/platform/path.rs`.
**Code (advisory):** `pub enum AgentLocation { Native, Wsl }`;
`pub fn resolve_agent_location(env: Option<&str>, program: &str) ->
AgentLocation` — env `eq_ignore_ascii_case` for `native`/`wsl`, empty=unset,
invalid → `tracing::warn!` + heuristic; heuristic = substring after last
`/` or `\`, `to_ascii_lowercase`, `strip_suffix(".exe")` optional, `== "wsl"`.
**Verification:**
- [ ] Unit tests pass (14-row table + env grid)
- [ ] Stress fixture rows produce expected outcome
- [ ] probe.sh still AGREE-OK
- [ ] Budgets hold (no loops beyond basename scan)

## Slice 2: process-global binding + the gate on to_native/to_agent

**Claim:** C1, C3, C7 (decision level), C9.
**Oracle:** the 6-cell decision matrix enumerated in the design (host ×
location), written in the test before the gate exists; probe.sh GATE section
(Linux identity through real entry points, unchanged).
**Stress fixture:** pure decision fn tested on all 6 cells — only
(windows=true, Some(Wsl)) activates; adversarial cells: (true, None) idle
(catches default-Wsl regression = today's bug surviving), (false, Some(Wsl))
idle (catches dropped-cfg gate). C9 source-scan test: walk
`CARGO_MANIFEST_DIR/src`, assert `CYRIL_AGENT_LOCATION` appears only in
`platform/path.rs`.
**Loop budget:** gate check O(1) atomic load per translation call
(production: 1 per fs callback, ≪ 10^3/turn). C9 scan: test-only,
O(files × bytes) ≈ 60 files × ~50KB ≈ 3MB reads, test-tier budget.
**Files:** `crates/cyril-core/src/platform/path.rs`.
**Code (advisory):** private `AtomicU8` (0 unset / 1 native / 2 wsl);
`pub fn bind_agent_location(program: &str)` — reads env
`CYRIL_AGENT_LOCATION` (the only env read, C9), resolves, stores
(last-spawn-wins), `tracing::info!` the outcome; `pub fn agent_location()
-> Option<AgentLocation>`; private pure `fn translation_active(windows:
bool, loc: Option<AgentLocation>) -> bool` = `windows && loc == Some(Wsl)`
with a `tracing::debug!` once-per-process on the `None`-on-windows cell;
`to_native`/`to_agent` become `if translation_active(cfg!(...),
agent_location()) { <existing arm> } else { identity }`.
`translate_paths_in_json` doc comment restated as fact (mechanics
unconditional; the gate lives in to_native/to_agent) — no precondition
language, so nothing to enforce.
**Verification:**
- [ ] Unit tests pass (6-cell matrix, scan test)
- [ ] Stress cells produce expected outcome
- [ ] probe.sh still AGREE-OK (GATE section identical)
- [ ] Budgets hold

## Slice 3: wiring fences in win_wsl_wiring.rs

**Claim:** C1, C2, C3, C7 (wiring level, real entry points, real process
state).
**Oracle:** expected outputs are the input strings themselves (identity
claims) and the pre-change corpus values (Wsl claims) — both written before
the code change lands in this binary.
**Stress fixture:** (a) Linux: `linux_translation_is_noop` extended —
`bind` via setter to Wsl FIRST, then assert identity for all corpus shapes
(catches a gate missing the cfg! term). (b) Windows child, nothing set:
identity for `C:\Users\u`, `/mnt/c/x`, `/home/u` (catches default-Wsl).
(c) Windows child, `CYRIL_AGENT_LOCATION=wsl` + `CYRIL_WSL_DISTRO=Ubuntu`:
existing 8tq6 assertions hold verbatim (catches over-gating). (d) Windows
child, `CYRIL_AGENT_LOCATION=wsl`, NO distro: drive translates, `/home/u`
passthrough (8tq6 unknown-distro semantics under explicit override).
Existing `env_distro_wiring_via_child_process` child gains
`CYRIL_AGENT_LOCATION=wsl` in its env (setup change only; assertions
untouched — this is C2's gated-on variant).
**Loop budget:** none added (child-process spawns, test-tier: 3 children).
**Files:** `crates/cyril-core/tests/win_wsl_wiring.rs`.
**Verification:**
- [ ] Unit tests pass (Linux legs locally; Windows legs statically audited
      — cfg-gated, per CI-triage rules)
- [ ] Stress children produce expected outcome
- [ ] probe.sh still AGREE-OK
- [ ] Budgets hold

## Slice 4: bind at the spawn choke point + engine-resolution fences

**Claim:** C6 (resolved-command derivation), C8 (bound before exec, both
engines, single call site — as amended above).
**Oracle:** `agent_location()` getter observed from test processes;
discovery.rs:228 argv construction read directly (independent of the
heuristic).
**Stress fixture:** (a) transport test: spawn with program `wsl` on Linux —
exec FAILS (no such binary), yet `agent_location() == Some(Wsl)` (binding
precedes exec; catches bind-after-spawn ordering). (b) spawn `sh` stub —
`Some(Native)`. (c) discovery test: `resolve()` with a fake `exists` yields
the node argv; feeding its program to `resolve_agent_location` → Native,
while the pre-resolve CLI program `wsl` → Wsl (divergence pair, catches
deriving from the CLI argv).
**Loop budget:** none added; bind is O(1) per spawn.
**Files:** `crates/cyril-core/src/protocol/transport.rs`,
`crates/cyril-core/src/protocol/kas/discovery.rs` (test mod only).
**Code (advisory):** first line of `AgentProcess::spawn`:
`crate::platform::path::bind_agent_location(cmd.program());`. The stale
"Windows spawns `wsl kiro-cli acp`" comment two lines below is
cyril-duz0's surface — not touched here.
**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixtures produce expected outcome (incl. failed-exec bind)
- [ ] probe.sh still AGREE-OK
- [ ] Budgets hold

## Slice 5: docs — CLAUDE.md agent-location conditionality

**Claim:** AC 5 (docs describe the conditionality; coordinates with
cyril-duz0, which keeps its own surfaces: transport comment, README,
Platform Constraints WSL-spawn claim).
**Oracle:** design.md core rule — the doc text must state the same
biconditional (active ⟺ windows ∧ wsl-location) and name
`CYRIL_AGENT_LOCATION`; checked by reading both side by side.
**Stress fixture:** n/a — docs-only slice (no logic; fixture exemption per
budgeted-plan step 4 scope "slices that implement logic").
**Loop budget:** n/a.
**Files:** `CLAUDE.md`, `.cyril-jxmv/design.md` (C8 amendment note).
**Verification:**
- [ ] Path-translation + Platform Constraints sections updated
- [ ] cargo gates still green (docs don't compile, but commit gate runs)

---

## Plan Self-Review

1. **Loops:** basename split O(len) ×1/spawn; atomic load O(1)/translation;
   C9 scan test-only O(3MB). No always-on loops; all ≪ budget. No gaps.
2. **Fixtures:** every logic slice has adversarial rows targeting a named
   bug class (cross-platform basename split, default-Wsl survival,
   dropped-cfg gate, over-gating, bind-after-exec ordering, CLI-argv
   derivation). No gaps.
3. **Doc-comment preconditions:** the one candidate
   (`translate_paths_in_json` "callers must consult") is rewritten as a
   statement of fact — no precondition shipped without enforcement. Unset
   location: documented behavior with runtime handling (identity + debug
   log), not a precondition. No gaps.
4. **Write targets:** all new output is `tracing` diagnostics (warn/info/
   debug → cyril.log); no stdout writes. No gaps.
5. **Tracker references:** cyril-duz0 (verified open, covers transport
   comment + README + Platform Constraints WSL claim), cyril-861q (filed
   this run, verified). No uncited deferrals. No gaps.

Claim coverage: C1(S2,S3) C2(S3) C3(S2,S3) C4(S1) C5(S1) C6(S4) C7(S2,S3)
C8(S4) C9(S2) — matches the design's 9-claim list exactly.
