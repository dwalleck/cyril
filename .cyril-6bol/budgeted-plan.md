# Budgeted plan: KAS host shell

Approved design: `.cyril-6bol/falsifiable-design.md`. Cheapest falsifier: expansion-aware probe/oracle `AGREEMENT` on 2026-08-01.

Checkpoint revision (requester-approved 2026-08-01): strict KAS lint proved the original domain-only Slice 2 could not stand alone without dead production types. The coherent resolve-and-report seam therefore merges original Slices 2–4 and 7–9; it keeps their summed budgets and gates, and leaves rendering, launch construction, and execution as independent checkpoints.

Final checkpoint revision (requester-approved 2026-08-02): strict KAS lint requires the crate-private renderer and launch-plan APIs to gain their production `terminal/create` caller in the same checkpoint. Original Slices 5, 6, and 10 therefore merge into Slice 5 with their summed budgets and all fixtures/oracles unchanged.

Final budget accounting: the approved cyril-6bol checkpoint is 136 net production lines and 300 net test lines against the Slice 2 checkpoint, within the merged ≤150/≤300 limits. The operator-pipeline lifecycle fence discovered `cyril-2z9g`; its separately tracked fix adds 77 production lines and 29 test lines (including the bridge cancellation fence), within a local ≤80/≤30 bug-fix budget.
Pre-PR review corrections add 40 net production lines and 119 net test lines against the completed checkpoint, within a local ≤45/≤120 review-fix budget. The production delta covers effective-access and absolute-path resolution, correct PowerShell status finalization, and one shared create-time output drain; the tests are the minimum regression fences for each reproduced failure.
Every slice ends in one conventional commit. “Prototype oracle” below means:

```sh
diff <(.cyril-6bol/probe.py | jq -S .) <(.cyril-6bol/oracle.sh | jq -S .)
```

The prototype is a standalone empirical witness, not the Cyril executable. Therefore every slice also builds `cyril` with the KAS feature and runs its named regression fence against compiled production code; the standalone comparison revalidates the independent premise. Both are required for the checkpointed binary/oracle gate.

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
- [x] Config unit tests pass
- [x] Stress table returns the expected `Option<String>` / fallback cells
- [x] Prototype oracle agrees
- [x] Loop and wall budgets hold

## Slice 2: Resolve and report one immutable host shell

**Claim:** C1–C5/C10 — KAS validates the raw configured value before channels/thread/process, resolves exactly one runnable Unix or Windows shell, and uses the retained snapshot to emit only `posix`, `fish`, or `powershell`; V2 keeps the real `None` absence and performs no host-shell work.

**Oracle:** Hand-authored Unix/Windows expected-path matrices and KAS normalized-token table from the signed spec, plus a fake agent marker and serialized ext response independent of the resolver branches.

**Stress fixture:** Enumerate all four kinds and every automatic/explicit platform branch: absent/empty/stale/unknown Unix `$SHELL`, spaces/Unicode, bounded PATH Bash, pwsh PATH/Program Files priority, signaled+runnable Windows PowerShell, poisoned `COMSPEC`, invalid/wrong-platform config, invalid KAS pre-thread failure, V2 absence, missing callback snapshot, and exact response JSON.

**Code budget:** ≤355 production lines and ≤480 test/callsite lines across the merged original slice budgets.

**Loop budget:** PATH lookup is O(p), `p ≤ 256`, with no nested loops; fixed Windows probes add at most two metadata calls. One resolver call per KAS bridge and zero per V2 bridge.

**Wall budget:** One KAS startup resolution, ≤250 ms on local paths; invalid KAS returns before thread/process creation; V2 adds zero resolver wall time. No polling or re-resolution.

**Files:**
- `crates/cyril-core/src/protocol/kas/host_shell.rs` (new)
- `crates/cyril-core/src/protocol/kas/mod.rs`
- `crates/cyril-core/src/protocol/kas/terminal_io.rs`
- `crates/cyril-core/src/protocol/client.rs`
- `crates/cyril-core/src/protocol/bridge.rs`
- `crates/cyril/src/main.rs`

**Preconditions / output:** Kind construction stays private. Executable availability is a runtime correctness probe. `PSModulePath` only permits probing Windows PowerShell; it is not proof. The startup seam is the only production producer of `Option<HostShell>`; callback absence is refused. Resolution failures carry `HostShellError` as the source of `InvalidConfig`.

**Verification:**
- [x] Full fake Unix and Windows matrices match fixed paths/kinds/errors
- [x] PATH search stops at 256 probes and production selection has zero `COMSPEC` reads
- [x] Invalid KAS creates no thread/agent marker; V2 carries `None`
- [x] Ext routing serializes the retained normalized family exactly
- [x] Strict KAS tests/clippy, default full gate, prototype oracle, and wall/loop budgets pass

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
- [x] Renderer matrix unit tests pass
- [x] 64 KiB fixture remains within the O(n+b) budget
- [x] Expansion-aware prototype oracle agrees
- [x] TDD inversion: generic quote-all and unquote-all mutations each fail distinct assertions

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
- [x] Launch-plan unit tests pass
- [x] Profile marker count is exactly 1; failure-side-effect count is exactly 1
- [x] Installed-shell exit-42 fixtures pass where available
- [x] Prototype oracle agrees; no retry/wall budget holds

## Slice 10: Execute terminal requests through the bound shell

**Claim:** C6/C7/C8/C9 — terminal create rejects empty input, uses the shell command plan, and preserves existing lifecycle while adding native operators, pure variable expansion, profile behavior, and native failure status.

**Oracle:** Filesystem/output/exit assertions independent of registry internals; direct shell invocations provide expected command output and exit status.

**Stress fixture:** Empty command returns `-32602` and creates no id; spaced literal plus `| tr`; pure environment variable; controlled profile marker; non-empty nonexistent command returns an id then native non-zero status; cwd/env/stderr; release/cancel of a delayed shell pipeline leaves no delayed marker; concurrent slow/fast terminals retain unique ids and non-blocking wait.

**Code budget:** ≤50 production lines and ≤120 lifecycle-test lines, replacing obsolete direct-spawn assertions rather than layering duplicates.

**Loop budget:** Request env application remains O(e), `e ≤ 128`; rendering remains O(n+b), `n ≤ 256`, `b ≤ 65,536`; no nested loops and ≤128 env mutations per create.

**Wall budget:** Create returns within the existing fast bound; stress waits are each ≤5 s. No profile retry and no additional subprocess beyond the selected shell.

**Files:**
- `crates/cyril-core/src/protocol/kas/terminal_io.rs`

**Preconditions / output:** Empty command runtime check is load-bearing and survives release builds. Terminal stdout/stderr/exit are ACP data; spawn/wait errors are ACP diagnostics. Existing terminal identifiers, output snapshots, and kill/release status contracts are retained; process ownership now covers the selected shell's full tree under `cyril-2z9g`.

**Verification:**
- [x] Terminal unit/lifecycle tests pass
- [x] Every stress cell produces the prewritten error/output/status/marker result
- [x] Expansion-aware prototype oracle agrees
- [x] Create latency, env-loop, and renderer budgets hold

## Plan self-review

### Loops

No gaps. Startup PATH scan is O(p), bounded to 256 entries / ≤258 probes. Rendering is O(n+b), bounded to 256 tokens / 64 KiB. Request env application remains O(e), bounded to 128 values. No nested or always-on loops are introduced.

### Fixtures

No gaps. Each slice names a plausible failing implementation: serde erasure, token drift, stale/unknown `$SHELL`, COMSPEC poisoning, quote-all/unquote-all, profile suppression/retry, dropped config propagation, post-spawn validation, repeated resolution, and direct-argv/lifecycle regression.

### Doc-comment preconditions

No gaps. Runnable shell selection and empty command are load-bearing runtime checks. The private startup constructor and callback guards enforce the engine/shell pairing; `None` means V2 absence rather than a sentinel. Profile-once is enforced by a single launch path with no retry. Sanity-only assumptions are not presented as caller preconditions.

### Output targets

No gaps. ACP replies and terminal stdout/stderr/status are data. Configuration/resolution/spawn/wait failures are returned/traced diagnostics. No new stdout diagnostic exists.

### Tracker references

No design deferrals appear in this plan. `cyril-1rpv` and `cyril-3lh8` remain named only in the approved design’s negative space and were verified during design. Implementation exposed and fixed the separately tracked process-tree bug `cyril-2z9g`; spec review absorbed the existing pre-wait pipe-drain issue `cyril-r3t6`; the final runtime smoke exposed and filed the unrelated one-shot prompt bug `cyril-0ffy`.

### Claim coverage

No gaps: C1 → slices 1/2; C2–C5 → 2; C6 → 5/10; C7 → 6/10; C8 → 2/10; C9 → 6/10; C10 → 1/2.
