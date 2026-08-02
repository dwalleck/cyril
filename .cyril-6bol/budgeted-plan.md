# Budgeted plan: KAS host shell

Approved design: `.cyril-6bol/falsifiable-design.md`. Cheapest falsifier: expansion-aware probe/oracle `AGREEMENT` on 2026-08-01.

Every slice ends in one conventional commit. “Prototype oracle” below means:

```sh
diff <(.cyril-6bol/probe.py | jq -S .) <(.cyril-6bol/oracle.sh | jq -S .)
```

## Slice 1: Preserve shell configuration for semantic validation

**Claim:** C1/C10 — `[agent] shell` is absent or an exact external string; unknown strings survive TOML parsing so only KAS startup validates them, while wrong-typed TOML keeps the established whole-file fallback.

**Oracle:** Serialize/parse the `[agent]` table independently as `toml::Value`; compare the raw `shell` cell with `AgentConfig.shell`.

**Stress fixture:** Parse absent, `auto`, all four explicit shell strings, `cmd`, `future-shell`, and wrong-type `shell = 7`. Expected: strings survive exactly; absent is `None`; wrong type produces whole-file defaults.

**Code budget:** ≤20 production lines and ≤45 test lines.

**Loop budget:** No new loops. The test table is O(8) at test time only.

**Wall budget:** No always-on phase; config deserialization remains one startup read.

**Files:**
- `crates/cyril-core/src/types/config.rs`

**Preconditions / output:** The semantic validator’s “string input” precondition is enforced by serde type checking; invalid type uses the existing runtime fallback. Config warnings remain diagnostics through tracing, not stdout data.

**Verification:**
- [ ] Config unit tests pass
- [ ] Stress table returns the expected `Option<String>` / fallback cells
- [ ] Prototype oracle agrees
- [ ] Loop and wall budgets hold

## Slice 2: Define the closed shell domain and wire vocabulary

**Claim:** C5 — POSIX, fish, PowerShell 7, and Windows PowerShell are the only resolved kinds and map to exactly `posix`, `fish`, or `powershell`.

**Oracle:** Hand-authored KAS normalized-token table from the signed spec and issue evidence.

**Stress fixture:** Enumerate every kind and assert the full list of emitted tokens equals `[(posix,posix),(fish,fish),(pwsh,powershell),(powershell,powershell)]`; assert no `bash`/`cmd` token.

**Code budget:** ≤45 production lines and ≤35 test lines.

**Loop budget:** No production loops. Test enumeration is O(4).

**Wall budget:** No I/O or always-on phase.

**Files:**
- `crates/cyril-core/src/protocol/kas/host_shell.rs` (new)
- `crates/cyril-core/src/protocol/kas/mod.rs`

**Preconditions / output:** Kind construction stays private; callers receive only a resolved `HostShell`. Wire names are data returned to the ACP serializer.

**Verification:**
- [ ] Host-shell unit tests pass
- [ ] Wire matrix emits the three exact normalized tokens
- [ ] Prototype oracle agrees
- [ ] Loop and wall budgets hold

## Slice 3: Resolve Unix shells from a substitutable host seam

**Claim:** C2 — automatic Unix resolution uses a supported runnable `$SHELL`, otherwise runnable PATH Bash; explicit Bash/fish is platform-checked and never substituted.

**Oracle:** Expected-path table written from the signed Unix decision matrix, independent of resolver branches.

**Stress fixture:** Fake host cases: `$SHELL` unset/empty; runnable `/bin/zsh`; runnable path containing spaces/Unicode; stale `/bin/fish`; unknown `/bin/nu`; PATH Bash present/absent; explicit Bash/fish present/absent; explicit PowerShell. Expected paths/tokens/errors are fixed before implementation.

**Code budget:** ≤50 production lines and ≤70 table-test lines.

**Loop budget:** PATH lookup O(p) metadata probes, `p ≤ 256` entries at production scale, so ≤256 startup syscalls and O(256) comparisons once per KAS startup. No nested loops.

**Wall budget:** One startup resolution; ≤250 ms on local host paths in the stress harness. No polling.

**Files:**
- `crates/cyril-core/src/protocol/kas/host_shell.rs`

**Preconditions / output:** Executable availability is load-bearing and gets a runtime probe; stale paths cannot produce a resolved shell. Errors are diagnostics carried in `HostShellError`.

**Verification:**
- [ ] Unix resolution unit tests pass
- [ ] Full adversarial Unix matrix matches the oracle table
- [ ] Prototype oracle agrees
- [ ] PATH loop stays ≤256 probes in the counting fake

## Slice 4: Resolve Windows PowerShell without COMSPEC

**Claim:** C3 — native Windows resolution is PATH pwsh → Program Files pwsh → signaled+runnable Windows PowerShell → error; cmd/COMSPEC and Unix shells never resolve.

**Oracle:** Explicit Windows expected-path table plus production-source search proving the resolver contains zero `COMSPEC` reads.

**Stress fixture:** Fake Windows host with: both pwsh locations; Program Files only; `PSModulePath` plus runnable Windows PowerShell; signal without executable; PowerShell executable without signal; only cmd plus poisoned `COMSPEC`; no shells; each explicit value and missing executable.

**Code budget:** ≤50 production lines and ≤80 table-test lines.

**Loop budget:** One PATH scan O(p), `p ≤ 256`; two fixed-location probes; ≤258 startup metadata probes, below 10³ syscalls. Environment lookups are O(1).

**Wall budget:** One startup resolution; ≤250 ms in the counting fake. No polling.

**Files:**
- `crates/cyril-core/src/protocol/kas/host_shell.rs`

**Preconditions / output:** `PSModulePath` is evidence to try Windows PowerShell, not proof of executability; the executable probe is a runtime correctness check. Resolution errors are diagnostics.

**Verification:**
- [ ] Windows resolution unit tests pass on every host through the fake seam
- [ ] Poisoned COMSPEC fixture still errors
- [ ] Production resolver source has zero COMSPEC reads
- [ ] Prototype oracle agrees and probe-count budget holds

## Slice 5: Render literal, operator, and pure-variable tokens

**Claim:** C6 — literals remain one argument; the closed operator set and only pure family-valid environment-variable forms remain syntax.

**Oracle:** Fixed rendered strings plus installed-shell outputs from `.cyril-6bol/oracle.sh`; oracle does not use the Rust renderer.

**Stress fixture:** Empty/multi args; spaces/newline/Unicode; single quotes; duplicate values; every operator; operator-looking literal (accepted syntax); POSIX `$NAME`/`${NAME}`; fish `$NAME`; PowerShell `$env:NAME`/`${env:NAME}`; embedded `prefix-$HOME`, glob, and `$(command)` expected literal. Add 256 tokens totaling 64 KiB.

**Code budget:** ≤50 production lines and ≤90 matrix-test lines.

**Loop budget:** One pass O(n+b), `n ≤ 256` tokens and total bytes `b ≤ 65,536`; ≤65,792 character/token operations, zero syscalls, one output allocation.

**Wall budget:** Per terminal creation but CPU-only; ≤2 ms for the 64 KiB stress fixture in debug tests.

**Files:**
- `crates/cyril-core/src/protocol/kas/host_shell.rs`

**Preconditions / output:** Empty command is load-bearing and is rejected at runtime before rendering. Rendered command text is internal process data, never printed.

**Verification:**
- [ ] Renderer matrix unit tests pass
- [ ] 64 KiB fixture remains within the O(n+b) budget
- [ ] Expansion-aware prototype oracle agrees
- [ ] TDD inversion: generic quote-all and unquote-all mutations each fail distinct assertions

## Slice 6: Build profile-aware launch plans with exit fidelity

**Claim:** C7/C9 — launch flags load one non-interactive login/profile startup with no retry, and PowerShell preserves external exit codes greater than one.

**Oracle:** GNU/fish/Microsoft invocation contracts plus direct installed-shell exit status; expected argv is hand-authored per kind.

**Stress fixture:** Assert exact argv for all four kinds; controlled zsh/fish profile marker emits once; failing profile writes a side-effect marker once and exits; PowerShell wrapper source clears stale `$LASTEXITCODE` and preserves simulated 42. Platform-gate actual executable runs, but compile and pure argv assertions on every host.

**Code budget:** ≤50 production lines and ≤90 test lines.

**Loop budget:** Reuses Slice 5’s O(n+b) renderer; no additional production loops.

**Wall budget:** One shell startup per terminal command; no retry and no additional process. Test fixture timeout 5 s per installed family.

**Files:**
- `crates/cyril-core/src/protocol/kas/host_shell.rs`

**Preconditions / output:** “One profile startup” is enforced by one command construction and zero retry path. Shell stdout/stderr are terminal data; resolver/launch errors are diagnostics.

**Verification:**
- [ ] Launch-plan unit tests pass
- [ ] Profile marker count is exactly 1; failure-side-effect count is exactly 1
- [ ] Installed-shell exit-42 fixtures pass where available
- [ ] Prototype oracle agrees; no retry/wall budget holds

## Slice 7: Carry raw shell configuration to bridge startup

**Claim:** C1/C10 — the binary passes `[agent] shell` into `SpawnConfig`; default/struct-update callers remain absent, and V2 treats every value as inert.

**Oracle:** Exhaustive `AgentConfig` consumption in `main` plus a hand comparison of `SpawnConfig::default()` and explicit values.

**Stress fixture:** Build `SpawnConfig` with absent, auto, and invalid strings; clone it across the spawn seam; assert no value is dropped. Compile every integration smoke struct literal using `..Default::default()`.

**Code budget:** ≤20 production lines and ≤35 test lines.

**Loop budget:** No new loops.

**Wall budget:** No new work beyond moving one optional startup string.

**Files:**
- `crates/cyril/src/main.rs`
- `crates/cyril-core/src/protocol/bridge.rs`

**Preconditions / output:** No value-level validation occurs in `main`; bridge startup owns semantic errors. Existing startup errors remain stderr/returned diagnostics.

**Verification:**
- [ ] Main/core compile all targets
- [ ] SpawnConfig propagation fixture preserves each raw value
- [ ] Prototype oracle agrees
- [ ] No loop or wall increase

## Slice 8: Gate KAS startup and bind the resolved shell to KasEngine

**Claim:** C1/C4/C10 — KAS resolves before channels/thread/process and constructs `KasEngine` only with the resulting shell; V2 performs zero host-shell probes.

**Oracle:** Fake agent marker file plus a counting `HostEnvironment`; engine kind/capability output remains the existing independent witness.

**Stress fixture:** Invalid KAS value and unavailable explicit shell must return `InvalidConfig` with zero host thread/agent markers; valid KAS probes exactly once; V2 with `cmd` probes zero times and reaches its in-process handshake unchanged.

**Code budget:** ≤50 production lines per file and ≤80 test lines total.

**Loop budget:** Resolver bounds are Slice 3/4; startup adds no loop. Exactly one resolver call per KAS bridge.

**Wall budget:** Invalid KAS returns before thread creation; valid KAS adds ≤250 ms once; V2 adds 0 resolver wall time.

**Files:**
- `crates/cyril-core/src/protocol/bridge.rs`
- `crates/cyril-core/src/protocol/engine.rs`

**Preconditions / output:** KasEngine’s host-shell precondition is enforced structurally by its constructor/field; no `Option` or sentinel shell exists inside KasEngine. `InvalidConfig` is returned to the process startup caller.

**Verification:**
- [ ] Bridge/engine unit and harness tests pass
- [ ] Invalid KAS creates zero markers; valid KAS resolver count is 1; V2 count is 0
- [ ] Prototype oracle agrees
- [ ] Startup wall and probe budgets hold

## Slice 9: Share one shell snapshot across client response and registry

**Claim:** C4/C5/C8 — KiroClient obtains the bound KasEngine shell once, constructs the terminal registry with it, and both ext response and terminal create cross that registry seam.

**Oracle:** Capture the serialized ext response and spawned executable/argv from one registry; compare both against the original `(path, token)` tuple retained by the test.

**Stress fixture:** Resolve a fake shell, mutate the fake environment/PATH afterward, then route `_kiro/terminal/shell_type` and create a terminal. Expected: original token and executable remain; V2 ext request remains method-not-found/disabled.

**Code budget:** ≤45 production lines per file and ≤75 test lines total.

**Loop budget:** No new loops; one `Rc` clone and one immutable shell clone at bridge construction.

**Wall budget:** No re-resolution; response is serialization-only and create adds no discovery I/O.

**Files:**
- `crates/cyril-core/src/protocol/client.rs`
- `crates/cyril-core/src/protocol/kas/terminal_io.rs`

**Preconditions / output:** A terminal callback requires a KAS engine with a bound shell; the engine variant makes that load-bearing precondition representable without a sentinel. ACP responses are data; impossible callback mismatches return ACP errors.

**Verification:**
- [ ] Client and registry unit tests pass
- [ ] Environment-poison snapshot fixture retains original path/token
- [ ] Prototype oracle agrees
- [ ] Zero resolver calls after startup

## Slice 10: Execute terminal requests through the bound shell

**Claim:** C6/C7/C8/C9 — terminal create rejects empty input, uses the shell command plan, and preserves existing lifecycle while adding native operators, pure variable expansion, profile behavior, and native failure status.

**Oracle:** Filesystem/output/exit assertions independent of registry internals; direct shell invocations provide expected command output and exit status.

**Stress fixture:** Empty command returns `-32602` and creates no id; spaced literal plus `| tr`; pure environment variable; controlled profile marker; non-empty nonexistent command returns an id then native non-zero status; cwd/env/stderr; release/cancel of a delayed shell pipeline leaves no delayed marker; concurrent slow/fast terminals retain unique ids and non-blocking wait.

**Code budget:** ≤50 production lines and ≤120 lifecycle-test lines, replacing obsolete direct-spawn assertions rather than layering duplicates.

**Loop budget:** Request env application remains O(e), `e ≤ 128`; rendering remains O(n+b), `n ≤ 256`, `b ≤ 65,536`; no nested loops and ≤128 env mutations per create.

**Wall budget:** Create returns within the existing fast bound; stress waits are each ≤5 s. No profile retry and no additional subprocess beyond the selected shell.

**Files:**
- `crates/cyril-core/src/protocol/kas/terminal_io.rs`

**Preconditions / output:** Empty command runtime check is load-bearing and survives release builds. Terminal stdout/stderr/exit are ACP data; spawn/wait errors are ACP diagnostics. Existing tracked-child lifecycle is unchanged.

**Verification:**
- [ ] Terminal unit/lifecycle tests pass
- [ ] Every stress cell produces the prewritten error/output/status/marker result
- [ ] Expansion-aware prototype oracle agrees
- [ ] Create latency, env-loop, and renderer budgets hold

## Plan self-review

### Loops

No gaps. Startup PATH scan is O(p), bounded to 256 entries / ≤258 probes. Rendering is O(n+b), bounded to 256 tokens / 64 KiB. Request env application remains O(e), bounded to 128 values. No nested or always-on loops are introduced.

### Fixtures

No gaps. Each slice names a plausible failing implementation: serde erasure, token drift, stale/unknown `$SHELL`, COMSPEC poisoning, quote-all/unquote-all, profile suppression/retry, dropped config propagation, post-spawn validation, repeated resolution, and direct-argv/lifecycle regression.

### Doc-comment preconditions

No gaps. Runnable shell selection and empty command are load-bearing runtime checks. KasEngine construction encodes the bound-shell invariant. Profile-once is enforced by a single launch path with no retry. Sanity-only assumptions are not presented as caller preconditions.

### Output targets

No gaps. ACP replies and terminal stdout/stderr/status are data. Configuration/resolution/spawn/wait failures are returned/traced diagnostics. No new stdout diagnostic exists.

### Tracker references

No deferrals appear in this plan. `cyril-1rpv` and `cyril-3lh8` remain named only in the approved design’s negative space and were verified during design.

### Claim coverage

No gaps: C1 → slices 1/7/8; C2 → 3; C3 → 4; C4 → 8/9; C5 → 2/9; C6 → 5/10; C7 → 6/10; C8 → 9/10; C9 → 6/10; C10 → 1/7/8.
