# Git and process crate reuse

## Scope and authority

Cyril production baseline: `4b970b1ed5d4cc8239a8012f131d6ff77df72e97`. This record retains the earlier policy/prior-art comparison and the subsequent library-first selection; it is not completed Round7 acceptance. Requester authorization is owned by `spec.md`, **Requester selection — git2 + Tokio direction and bounded-drain capture contract**; native prerequisite installation authorization is owned by `route.md`, **Execution authorization and caps**.

Selected: git2 as the single typed Git library, with ONE profiled explicit-argv CLI patch-production operation for ALL caller scopes and retained grep for demonstrated API gaps; Tokio for asynchronous diagnostics, with one subprocess/interprocess Windows lane including explicit batch where standard Tokio process handles fail the native identity/reader-ownership contract. No gix alongside, second scope grammar, std-batch adapter or automatic backend fallback. The package named `gix` is Gitoxide; it remains comparison history below.

## Reuse / deliberate-divergence receipt

Peer: GitComet `d1689a51f86ed2f337f276656b4fa91742396d42`, https://github.com/Auto-Explore/GitComet. Its tracked tree was clean when inspected; unrelated untracked files were untouched. GitComet is AGPL-3.0-only and declares Rust1.98.1. Borrow architectural ideas and upstream crate APIs, not peer source into MIT Cyril. Upstream gix0.88 separately declares MIT OR Apache-2.0 and Rust1.88.

All peer paths below are relative to the pinned GitComet root.

| Symbol/group | Existing evidence | Receipt |
|---|---|---|
| Native status | `crates/gitcomet-git-gix/src/repo/status.rs:118-160` uses gix `TreeIndex` and `IndexWorktree` items. | Useful typed data instead of porcelain parsing. Separate status lanes are not automatically the net target-to-worktree diff. |
| Native tree changes and statistics | `crates/gitcomet-git-gix/src/repo/log/commit_stats.rs:74-90,184-205` uses `diff_tree_to_tree` and blob Histogram diff. | Can remove tree-diff and ordinary line-stat subprocess/parsing work. Application mapping remains. |
| Unified patches | `crates/gitcomet-git-gix/src/repo/diff.rs:62-164` builds and parses installed-Git commands. | GitComet is not evidence that adopting gix eliminates the complete gather pipeline. |
| Commit to live worktree | `crates/gitcomet-git-gix/src/repo/log/repo_impl.rs:902-916` deliberately delegates to Git; `repo/submodules.rs:1129-1151` joins CLI status and numstat. | Directly relevant to Cyril's dirty/merge-base target. It is an operation-specific choice, not proof that a library cannot implement it. |
| Raw path boundary | `crates/gitcomet-git-gix/src/util.rs:1315-1334` uses Unix `OsStr` bytes and rejects unrepresentable Windows paths. | Confirms Cyril's identity/display separation. A Git package does not automatically solve caller pathspec or root-relative semantics. |
| Machine output profile | `crates/gitcomet-git-gix/src/repo/diff.rs:62-124` pins color and header settings. | Borrow explicit per-operation controls, not the exact list: it is not complete config isolation; `--` does not make a generated path literal; CR-at-EOL suppression is a presentation policy. |
| Statistics caps | `crates/gitcomet-git-gix/src/repo/log/commit_stats.rs:3-40` caps400 files/4MiB blobs and maps unreadable/binary cases to unknown. | Do not import UI caps or hidden-read-error behavior into review evidence. |
| Raw output versus display | `crates/gitcomet-git-gix/src/util.rs:67-102` keeps raw byte vectors separately from decoded activity. | Useful separation; main capture is uncapped. |
| Pipe lifetime | `crates/gitcomet-git-gix/src/util.rs:686-715,790-840` waits for reader EOF after leader exit and kills groups/trees on timeout/cancel. | Not Cyril's direct-child-only/no-descendant-EOF contract. |
| Executable probe | `crates/gitcomet-core/src/process/probe.rs:9-85` uses process-wrap groups/jobs, null stdin, EOF draining and64KiB retention. | Not an uncapped terminal snapshot primitive. |

GitComet's own CLI executor is handwritten `std::process::Command`. Its lock does contain transitive gix-command0.11 and process-wrap10.0.1; do not confuse a transitive helper with a typed application Git API. Its enabled gix `basic` feature includes `blob-diff` transitively.

## Candidate comparison and selected responsibilities

- **gix0.88:** typed refs/revisions/status/tree/blob operations without libgit2's C build. GitComet supplies concrete usage examples. Combined target-to-working-tree patch behavior still requires qualification, not naive concatenation of staged/unstaged changes.
- **git2 0.21:** selected safe Rust bindings over libgit2. Native discovery/revisions/dirty/whole-target documents remain; `Diff::from_buffer`/`Patch` supplies typed raw paths/statuses/stats/hunks/per-file extraction from one CLI patch. Its empty-addition boundary gap requires uniform standard-mail framing guarded by inserted-byte offsets and typed `DiffLine.content_offset` consumption checks; framing alone can disguise truncated deletion evidence and is not selected. This is bounded library adaptation, not another path/status/count/hunk parser or fallback. P7 preserves prior probe history; fresh guarded-adapter/native production qualification remains required.
- **subprocess1.2.1:** selected for ONE Windows native raw launch/pipe lane including explicit batch, not Git parsing or an EOF-bound communicator. Its owned overlapped pipe handles convert safely into existing interprocess2.4.3 Tokio byte streams. P8/P9 records native bounded-drain/resource-release proof and R9's exact absolute-spaced `.cmd` cases. The R8 std-batch alternative was proved but is not selected.
- **Tokio capture loop:** retain the existing Unix Tokio/Windows subprocess-interprocess owners. Bounded fair poll turns correct hot-stream/idle-stream throttling without blocking a ready stream or starving direct-child/deadline observation. Main's fresh Linux diagnostics matrix passes the unchanged32 MiB stdout-then-stderr/5s fixture and lifecycle controls; C10 reports process360/360. Native and final assembled acceptance remain separate.
- **gix-command0.11:** a process-preparation helper returning a normal child, not a typed status/statistics/patch API or lifecycle owner.
- **vcs-git0.12 / git-spawn0.3:** considered CLI wrappers, not selected; they do not remove the relevant typed Git semantics or establish Cyril's native diagnostics ownership.

The measured result selects **git2 plus only necessary CLI operations**. The isolated driver comparison in `evidence.md` P7 showed explicit fixture-local `diff=rust` selects a nested header in CLI but the outer header in libgit2; parsed CLI patches retain the CLI header. One profiled CLI patch-production operation therefore handles all caller scopes; typed git2 extraction replaces name-status/numstat/per-file CLI parsing. The earlier name-only selection plus literal narrowing/native patch production is superseded, with no generated per-file CLI, second backend or fallback. Git grep remains because git2 has no binding. `evidence.md` P7–P9 owns observations and source/report hashes; `design.md` owns packages/features and interfaces.

Facts symbol ownership, target auto-selection policy, artifact ordering, raw identity and domain error mapping remain Cyril responsibilities. A library migration preserves meaningful selected source/binary changes and functional semantics; Python serialization parity is not required by the current spec.

Verification reuses Git's own `--binary --full-index`, private index and tree operations for all full/per-file patch meaning: paired pre/post tree identities retain content, raw path, mode and deletion semantics while the real index is unchanged. Delete patch-ID/native-Python serialization proxies instead of extending a bespoke parser or adding empty-file/binary exceptions. Python retains metadata/types/order/symbols/usages/documents/outcomes responsibility. The initial pure-mode probe exposed premature temporary-index selection before ancestor construction; ordering the standard Git calls correctly resolved the fixture. The bounded probe and sensitivity receipts live in evidence.md; full gather/native qualification is not inferred.

## git2 build-time and runtime cost

Inspected git2 `0.21.0` and the `libgit2-sys 0.18.8+1.9.7` source returned for its compatible0.18 requirement:

- The git2 manifest has `default = []`; `ssh` and `https` are opt-in, and `vendored-libgit2` enables `libgit2-sys/vendored`. Local repository inspection needs neither network feature.
- With vendoring, the sys build compiles libgit2 C sources into a static library through `cc::Build`. Without it, it tries compatible system libgit2 via pkg-config, then falls back to bundled compilation. `LIBGIT2_NO_VENDOR` can force the system path even when vendoring was requested; release builds must control their environment.
- **Static bundled libgit2:** builders need the target's C compiler/linker, including MSVC/Windows SDK on Windows. Users do not need a separately installed libgit2 DLL/shared library. C code still executes inside Cyril, contributes binary size/native-code risk, and requires rebuilding/releasing Cyril for bundled libgit2 updates.
- **Dynamically linked system libgit2:** deployment must supply a compatible runtime shared library. Do not let ambient pkg-config accidentally determine release packaging.
- `libz-sys` is a mandatory compression dependency, not optional. Vendoring libgit2 does not make every dependency or the entire executable static. HTTPS/SSH add optional platform TLS/OpenSSL/libssh2 considerations. Inspect the final artifact's dependencies on each platform before claiming portable packaging.
- Cyril already requires a native C toolchain: root Cargo.toml enables rusqlite's `bundled` feature to compile libsqlite3. Vendored libgit2 adds another native library, not a new toolchain category. The documented missing `lib.exe` failure concerns Linux-to-MSVC cross-compilation, not native Windows builds. Reuse the existing Windows CI leg/VM toolchain for a selected libgit2 build and its behavior/linkage checks; missing prerequisites may be installed under the route authorization.

Sources:
- https://docs.rs/crate/git2/0.21.0/source/Cargo.toml.orig
- https://docs.rs/crate/libgit2-sys/0.18.8+1.9.7/source/build.rs
- https://docs.rs/crate/libgit2-sys/0.18.8+1.9.7/source/Cargo.toml.orig
- https://docs.rs/git2/0.21.0/git2/struct.Repository.html
- https://docs.rs/git2/0.21.0/git2/struct.Patch.html
- https://docs.rs/crate/gix/0.88.0/source/Cargo.toml.orig
- https://docs.rs/subprocess/1.2.1/subprocess/struct.Communicator.html
- https://docs.rs/crate/subprocess/1.2.1/source/Cargo.toml.orig

## Verification scope

The earlier standalone Rust1.94 EOF-ownership probe observed direct-child exit at1ms, unfinished readers then, and capture completion at2002ms after an inherited holder released both pipes. NUL/invalid-UTF8 and late descendant bytes survived. This was not the GitComet binary, not its tree-kill path, and not Windows qualification; the temporary probe was removed.

The corrected48-row subprocess1.2.1 native comparison **passed48/48** on Windows11 Pro10.0.26200 with Rust/Cargo1.94.0, MSVC19.44.35228.0 and CPython3.14.7. One native build and one unchanged matrix run:36 successful children matched complete stdout/stderr bytes, selected image/CRT argv/cwd/PATH/exit37;18 had valid cloneable NUL stdin at EOF and18 inherited the exact binary marker. Six file-not-found and six access-denied rows also matched. No dependencies were installed; the existing toolchain sufficed.

The original probe remains a recorded12/48 FAIL: it omitted stdout/stderr pipe configuration before `Exec::start`, so successful child receipts escaped to inherited controller output. The corrected source adds only `.stdout(Redirection::Pipe)` and `.stderr(Redirection::Pipe)` before launch; Cargo.toml, qualify.py and all matrix assertions remain byte-identical. Supplementary analysis of the failed run was not substituted for a passing run. Parent independently checked all48 row verdicts and36 successful byte/exit comparisons in the retained receipt.

Parent also inspected the complete PowerShell runner diff. Operational changes give the corrected probe separate root/archive/package names and an updated expected archive hash, and require explicit TransferBase instead of a default gateway/port. Both observed runs already supplied the same18974 transfer endpoint. Build/matrix invocation, Python/Rust selection and comparison assertions are unchanged. Thus the runner files are not byte-identical; the claim is that the comparison matrix is unchanged and the Rust launch correction is the two output-pipe calls, not that every execution-wrapper byte is identical.

Receipts: `local://native-subprocess-launch-evidence.json` and `local://native-subprocess-launch-corrected-evidence.json`; the latter and exact source/archive/runner/report/binary are also retained under `/home/dwalleck/.claude/tmp/cyril-s2hb-transfer-s1p5bhet/` with `capture-fixed` names. Corrected source SHA256 `87caf1ea62c0ba6ce275ac509b990597a0777ec01ef91607eb378ca63b36776d`; archive `9dd80e0ba0c87de8e12d8be058c66b7dcacaf5f241bf831a5f944dd9677c6b32`; controller `46cd2d942a15f983bf851ff939d4b842951cb566c53aef4db650273966984887`; exact native report `179c043dde8ad8424075f35d9d6cab420c655fc3a51b70a80951e8e40286ccb7`. Both owned Windows probe roots were removed after receipt retention; unrelated probes and transfer services were preserved.

This earlier48-row PASS covers only finite children, absolute selected paths, raw command-tail/CRT argv behavior, cwd, inherited PATH, stdin, exits/errors and byte capture. It does not qualify arbitrary environment overrides, native search/batch dispatch, descendant-held output, cancellation, libgit2 linkage or production diagnostics. Rust1.88 MSRV was not exercised. Later P8/P9 evidence in `evidence.md` independently qualifies the selected asynchronous capture APIs under the newly approved bounded-drain contract.

Historical policy-only smoke: `python3 .cyril-s2hb/oracles/check_shape.py --phase diagnostics --exact-base 5c1bf67c29b1eccba847d1ea185adec6c5ddf2a0` exited0 and reported `PASS C10: diagnostics ledger, dependencies, protected parents`. All11 owner counts were below their unchanged limits; only comment/error wording changed in that policy step. The later selected package set and cwd wiring now update the exact ledger/guard, with renewed assembled checks still pending. Prior runtime receipts remain scoped to their exact sources, not proof of package adoption or issue acceptance.
