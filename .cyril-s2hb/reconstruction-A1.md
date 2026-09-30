Scope and evidence: I derived the staged set with git diff --cached --stat/name-status in the target worktree, then inspected only the four changed production paths plus relevant production/test callsites. The staged .cyril-s2hb artifacts, README, and CI workflow were excluded from the reconstruction. No build, test, lint, formatter, or state-changing command was run.

1. Changed module interfaces and responsibilities

- `crates/cyril-core/src/lib.rs:1-7` adds `pub mod review;`. This is a module-registration/export interface only; it owns no prefix behavior.
- `crates/cyril-core/src/review/mod.rs:11-16` defines the public `ShellDialect` value interface with four variants: `Posix`, `Fish`, `Pwsh`, and `WindowsPowerShell`.
- `crates/cyril-core/src/review/mod.rs:19-37` defines the public `PrefixError` error interface. It distinguishes current-executable lookup, canonicalization, relative paths, non-UTF-8 paths, shell-active characters, surviving POSIX backslashes, and unsupported Windows verbatim/device prefixes.
- `crates/cyril-core/src/review/mod.rs:40-95` defines `CrtoolPrefix` and its public interface:
  - `current(dialect)` obtains `env::current_exe`, canonicalizes it, and delegates to `from_executable` (`:45-55`).
  - `from_executable(executable, dialect)` validates and spells an absolute path without resolving it (`:57-95`).
  - `as_str()` exposes the generated string (`:91-95`).
- `crates/cyril-core/src/review/mod.rs:99-120` owns shell-character validation. It rejects ASCII quote, dollar, backtick, and control characters for all dialects; it additionally rejects U+201C/U+201D/U+201E for PowerShell and rejects a surviving backslash on non-Windows (`:99-120`).
- `crates/cyril-core/src/review/mod.rs:124-143` owns Windows-native path spelling. It uses typed `Path` components, strips only supported verbatim disk/UNC markers, converts separators to forward slashes in the caller at `:78-87`, and rejects unsupported verbatim/device prefixes (`:124-143`).
- `crates/cyril-core/src/protocol/kas/host_shell.rs:96-117` shows the existing `HostShell` responsibility: retain the resolved private `ShellKind` and executable, and resolve a runnable host shell. Its existing command/rendering responsibility is visible at `:166-243`, with `command()` producing a `ShellCommand` launch plan.
- `crates/cyril-core/src/protocol/kas/host_shell.rs:156-164` adds only `HostShell::review_shell(&self) -> crate::review::ShellDialect`, an exhaustive projection of private `ShellKind`; it does not generate the prefix or validate paths.
- `crates/cyril-core/src/protocol/bridge.rs:30-43` extends `BridgeHandle` with `review_shell: Option<ShellDialect>` and a public `review_shell()` copy getter. The getter's documented value is the same host-shell dialect resolved for the bridge's KAS session (`:41-43`).
- `crates/cyril-core/src/protocol/bridge.rs:210-225` populates the value once after `resolve_host_shell(&config)`: KAS maps the already-resolved `HostShell` through `HostShell::review_shell`; non-KAS builds set it to `None` (`:215-225`).
- `crates/cyril-core/src/protocol/bridge.rs:67-82` and `:148-165` initialize test/channel-created handles with `review_shell: None`; the existing bridge channel ownership and notification/permission/source/completion interfaces are otherwise unchanged.

2. Responsibility clusters and separation rule

[OBSERVED] The changed code forms four clusters:

- Prefix construction/validation: `review/mod.rs:40-143`. This cluster accepts a path plus an explicit dialect and returns a string/error. It owns the fixed output grammar: POSIX/Fish use `"..." crtool`, while both PowerShell variants use `& "..." crtool` (`:74-87`).
- Host-shell discovery and terminal execution: `protocol/kas/host_shell.rs:96-117` resolves the executable and private kind; `:166-243` renders arbitrary terminal commands and launch arguments. `protocol/kas/terminal_io.rs:167-191` consumes that `HostShell` and calls `shell.command(...)` to construct the process launch plan.
- Dialect adaptation: `host_shell.rs:156-164` translates the private `ShellKind` vocabulary to the public `ShellDialect` vocabulary. The existing KAS wire vocabulary remains separately owned by `wire_name()` at `:166-171`.
- Bridge lifecycle/transport: `bridge.rs:210-230` resolves the startup shell, creates channel state, and projects the dialect onto `BridgeHandle`; the bridge then owns the async/thread/channel lifecycle. The shell resolution policy itself remains in `resolve_host_shell` at `bridge.rs:306-331`.

[OBSERVED separation rule] The prefix module consumes only a path and a small value dialect; the KAS HostShell owns discovery and arbitrary terminal command rendering; the bridge carries the resolved dialect but does not repeat path validation or shell resolution. This is evidenced by the review module imports at `review/mod.rs:4-7`, the HostShell projection at `host_shell.rs:156-164`, and the bridge assignment at `bridge.rs:215-225`. No source in the changed module makes the review module responsible for ACP, terminal spawning, or bridge channels.

[UNDETERMINED] No approved design/requirements were read, so this is a source-derived boundary description, not a conformance judgment.

3. Dependency direction and concrete adapters

- `cyril-core/src/lib.rs:1-7` exposes `review`, making the value API available as `cyril_core::review`.
- `review/mod.rs:4-7` depends only on `std::env`, `std::io`, `std::path`, and `thiserror`; `crates/cyril-core/Cargo.toml:33` confirms `thiserror` is an existing core dependency. The new module does not import `protocol`, `bridge`, `HostShell`, ACP, or Tokio.
- `protocol/kas/host_shell.rs:156-164` depends downward on the public `review::ShellDialect` enum. The concrete adapter is the exhaustive `ShellKind -> ShellDialect` match, preserving four source kinds even though `wire_name()` collapses both PowerShell kinds to the wire token `powershell` (`host_shell.rs:166-171`).
- `protocol/bridge.rs:215-225` depends on that adapter and stores only `Option<ShellDialect>`, not a `HostShell`. Thus the bridge boundary does not expose the private executable or shell implementation.
- The source-resolution context is feature-gated: `protocol/mod.rs:1-15` makes `kas` private and conditional; `protocol/client.rs:7-11` aliases `ResolvedHostShell` to `Option<HostShell>` only under `kas` and uses a non-KAS marker struct otherwise. `bridge.rs:306-331` returns `None` for V2 and a resolved HostShell only for KAS; `bridge.rs:217-225` consequently returns no dialect on non-KAS builds.
- There is no reverse edge in the changed code: `review/mod.rs` does not import HostShell or bridge, and `HostShell`/bridge are the only production users of `ShellDialect` found in the in-repo production tree.

4. Pass-through modules and hypothetical seams

- `lib.rs:6` is a pure registration/export pass-through.
- `HostShell::review_shell` at `host_shell.rs:156-164` is a pure projection adapter. It contains no new shell behavior beyond enum translation.
- `BridgeHandle::review_shell` at `bridge.rs:41-43` is a stored-value getter; `spawn_bridge` at `:215-225` forwards the value from the already-resolved host shell. It is transport/wiring, not a second resolver.
- `BridgeHandle::split` remains a five-element consuming split at `bridge.rs:91-108`; it does not return the dialect. Therefore the actual seam is temporal: a caller must read `review_shell()` before calling `split()`. The primary App consumer currently calls `bridge.split()` directly at `crates/cyril/src/app.rs:403-404`, and no in-repo production call to `review_shell()` was found in the searched crate tree.
- `CrtoolPrefix::current` is a convenience path from process environment to the deterministic formatter (`review/mod.rs:45-55`). `CrtoolPrefix::from_executable` at `:58-89` is the concrete deterministic seam for callers/tests that already possess a path; it validates/spells and deliberately does not resolve the path.
- No trait, backend selector, shell evaluator, process adapter, or injected environment abstraction was added in the changed production code. The only actual process execution associated with this module is a Unix unit-test smoke path at `review/mod.rs:347-360`; production `CrtoolPrefix` returns data.
- [UNDETERMINED] A downstream crate outside this repository could consume the public getter or prefix API, but such callers cannot be established from this source tree.

5. Protected-parent growth: wiring versus new responsibility

Source-only assessment of the changed parents:

- `crates/cyril-core/src/lib.rs:6`: one-line module export. Wiring only; no new responsibility in the parent.
- `crates/cyril-core/src/protocol/kas/host_shell.rs:156-164`: one narrow enum projection beside existing resolution/rendering code. Wiring/adapter only; prefix grammar, path validation, and executable discovery remain outside this parent (`:96-117`, `:166-243`).
- `crates/cyril-core/src/protocol/bridge.rs:37`, `:41-43`, `:82`, `:164`, and `:216-225`: field, getter, constructor defaults, and startup assignment. Wiring/transport only; no new bridge loop, channel protocol, process lifecycle, or shell resolver was introduced. The bridge takes the resolved shell from the existing `resolve_host_shell` path (`:306-331`).
- `crates/cyril-core/src/review/mod.rs:1-143`: this is the sole new responsibility-bearing module. It owns the prefix grammar, errors, shell hazard policy, and platform path spelling. Its tests occupy `:145-365` within the same new source file.
- No changes were staged in `protocol/client.rs` or `terminal_io.rs`; their existing ownership remains the `ResolvedHostShell` type alias (`client.rs:7-11`) and actual host command launch-plan construction (`terminal_io.rs:167-191`).

No verdict is made about whether any parent is allowed to grow under an unseen design; the classification above is whether the observed additions are wiring or behavior.

6. Tests reaching past interfaces

New review tests (inside the new module) exercise the public value API and the concrete generated command:

- `review/mod.rs:150-153` defines a local `prefix` helper around `CrtoolPrefix::from_executable`, and `:195-221` tests all four dialects and the expected POSIX/PowerShell prefix forms.
- `review/mod.rs:223-248` checks apostrophes and safe Unicode quote punctuation; `:250-268` checks PowerShell-only typographic double-quote rejection; `:270-315` checks relative paths, shell hazards, surviving backslashes, and non-UTF-8 paths.
- Windows-only tests at `review/mod.rs:318-345` exercise verbatim drive/UNC normalization and unsupported device-prefix rejection.
- The Unix smoke test at `review/mod.rs:347-360` passes the generated `as_str()` to `/bin/sh -c`, so it reaches beyond string shape into native shell consumption. This is a test callsite, not a production dependency.

Existing tests that reach past a public interface into private/concrete implementation details:

- `protocol/kas/host_shell.rs:539-565` has `assert_shell` that inspects private `HostShell.kind` and `HostShell.executable`; `:569-584` directly constructs private `ShellKind`/`HostShell::new` and calls `wire_name()`.
- `protocol/kas/host_shell.rs:842-877` constructs private shells and calls private `render_command`; `:910-1033` directly inspects `command()` launch arguments and runs installed shell plans. These tests do not exercise the new `review_shell()` projection.
- `protocol/bridge.rs:474-477` and `:499-502` call public `spawn_bridge` and then `split()`, but do not read the new getter.
- `protocol/bridge/tests/current_runtime_contract/saturation.rs:194-195` uses `BridgeHandle::for_tests_with_command_rx()` and its sender; `:258-260` uses crate-private `create_channel_pair()` and `split()`. `protocol/bridge/tests/harness.rs:665-667` likewise constructs the crate-private channel pair and immediately splits it.
- The test constructors themselves hard-code the new field to `None` at `bridge.rs:67-82` and `:148-165`; no existing test callsite was found that asserts a KAS-resolved `Some(ShellDialect)` or calls `BridgeHandle::review_shell()`.

[OBSERVED] In the searched in-repo production/tests tree, the new bridge getter appears only at its definition (`bridge.rs:41-43`) and the producer-side field assignment (`bridge.rs:217-225`); there is no current test or production consumer call. [UNDETERMINED] External downstream use is not knowable from this repository.

7. Reconstructed ledger: `path | responsibility | interface owner`

| path | responsibility | interface owner |
|---|---|---|
| `crates/cyril-core/src/lib.rs:1-7` | Register/export the new core module; no prefix logic | `cyril_core::review` module export, with behavior owned by `review/mod.rs` |
| `crates/cyril-core/src/review/mod.rs:11-16,40-95` | Public shell dialect and safe `crtool` executable-prefix value construction | `ShellDialect`, `CrtoolPrefix::{current,from_executable,as_str}` |
| `crates/cyril-core/src/review/mod.rs:19-37,99-143` | Typed construction failures, shell-hazard validation, and Windows/POSIX spelling rules | `PrefixError` and private validation/spelling helpers |
| `crates/cyril-core/src/protocol/kas/host_shell.rs:96-117,166-243` | Resolve the startup host shell and render/launch arbitrary terminal commands; unchanged existing owner | private `HostShell`/`ShellKind`, `wire_name`, `command` |
| `crates/cyril-core/src/protocol/kas/host_shell.rs:156-164` | Adapt the resolved private shell kind into the review value vocabulary | `HostShell::review_shell` |
| `crates/cyril-core/src/protocol/bridge.rs:30-43,67-82,148-165` | Store/expose an optional dialect on the bridge handle and preserve `None` in synthetic handles | `BridgeHandle::review_shell` |
| `crates/cyril-core/src/protocol/bridge.rs:210-225,306-331` | Resolve once at bridge startup and project the existing KAS shell into the handle; return `None` for V2/non-KAS | `spawn_bridge` plus existing `resolve_host_shell` |
| `crates/cyril-core/src/protocol/client.rs:7-11` (unchanged supporting owner) | Define the feature-dependent `ResolvedHostShell` boundary consumed by bridge/domain startup | `ResolvedHostShell` type alias/marker |
| `crates/cyril-core/src/protocol/kas/terminal_io.rs:167-191` (unchanged supporting owner) | Consume the resolved `HostShell` to construct terminal process launch plans | `TerminalRegistry::new/create` and `HostShell::command` |

[UNDETERMINED] This ledger is reconstructed from production ownership and callsites only. It does not claim that every public interface has an in-repo caller, nor does it compare the code against any omitted design/specification.