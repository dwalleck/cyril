# Evidence: cyril-j7um

## Premise checklist

| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | An OS-released exclusive file lock can enforce one Unix runtime owner without a stale PID sentinel. | While one child holds the lock, does a second nonblocking acquisition fail, and does acquisition succeed after the holder is killed? | PASS |
| P2 | Separate SQLite stores in WAL mode retain committed schema metadata and reopen after abnormal runtime termination. | After killing a child with an uncommitted write transaction, do both stores reopen in WAL mode with the last committed schema version intact? | PASS |
| P3 | A Unix runtime directory and socket can be tightened to owner-only modes. | Does the production-shaped endpoint fixture report mode `0700` for its directory and `0600` for its socket? | PASS |
| P4 | Cyril's current installation layout guarantees that a companion runtime is a sibling of `current_exe`. | N/A — the issue requires an absolute executable path but does not require a sibling-layout premise; executable resolution and an explicit test override are design/checkpoint concerns, and no runtime artifact exists yet. | N/A — claim about the feature to be designed, not existing-system behavior |
| P5 | A maintained safe Rust API can create a Tokio Windows named pipe with an explicit current-user-only security descriptor and remote clients disabled, without unsafe code in Cyril. | Does an isolated Rust 1.94 probe compile current-process SID retrieval, protected DACL construction for that exact SID, `interprocess::PipeListenerOptions.accept_remote(false)`, and `create_tokio_duplex` for `x86_64-pc-windows-msvc`? | PASS |
| P6 | Bounded length-prefixed JSON plus a constant-time 256-bit credential comparison can distinguish authorized requests from the required invalid input classes. | Do independent Rust and Python mechanisms agree for valid health, missing/invalid auth, malformed/oversized frames, unknown operations, and unsupported versions? | PASS |
| P7 | A safe cross-platform process wrapper can bound forced shutdown and reap the runtime's descendants. | On Unix, does killing a wrapped child group finish within two seconds and remove its grandchild, and does the same probe compile the Windows Job Object branch? | PASS |
| P8 | OS-appropriate default data/runtime paths resolve to the exact issue-defined locations. | N/A — the issue defines the mapping; resolution, validation, and test environment injection belong to design/checkpoint rather than an existing resolver. | N/A — specified feature behavior |

## Data

- Source: production-shaped
- Shape: a temporary canonical-style data root containing two real SQLite stores, one process-lifetime ownership lock, and one Unix-domain socket; a process-group fixture containing a child and grandchild; and framed JSON requests covering every M0 authentication/version/size/operation class. The stores contain only the M0 singleton schema-version row and use WAL mode. The Windows leg compiles only the IPC security and Job Object mechanisms in an isolated probe package, avoiding the workspace's unrelated bundled-SQLite linker constraint.
- Safety: temporary roots were created outside the repository and operator data directories; probe processes were self-spawned and bounded; no Cyril config, home data store, installed executable, running Cyril process, or production state was read or mutated.

## Probe

- Files: `probe.py`, `probe-platform/Cargo.toml`, `probe-platform/src/main.rs`
- Mechanism: Python drives child processes through `fcntl.flock`, kills lock and SQLite transaction holders, reopens both databases through Python's SQLite binding, and reads Unix mode bits through `pathlib`. The independent Rust package exercises constant-time framed-request classification, `process-wrap` process-group containment with a real grandchild, and compilation of `process-wrap::JobObject` plus current-user SID retrieval and `interprocess`'s safe security-descriptor-aware Tokio pipe API.
- Runs: `./.cyril-j7um/probe.py <temporary-root>`; `cargo run --quiet --manifest-path .cyril-j7um/probe-platform/Cargo.toml`; `cargo check --manifest-path .cyril-j7um/probe-platform/Cargo.toml --target x86_64-pc-windows-msvc`

## Oracle

- Files: `probe-oracle.sh`, `probe-platform-oracle.py`, and source contracts from `interprocess` 2.4.3, `win-security-identifier` 0.2.0, `process-wrap` 9.1.0, and Microsoft Named Pipe Security.
- Mechanism: independent system tools repeat the lock lifecycle through `flock(1)` and inspect the SQLite/socket fixture through `sqlite3(1)` and `stat(1)`. A separately implemented Python parser uses `hmac.compare_digest` and `start_new_session`/`killpg` to compute the framing and process-tree answers through different libraries and control flow. For Windows, the independent source oracle is `PipeListenerOptions.security_descriptor` + `create_tokio_duplex` in <https://docs.rs/interprocess/2.4.3/x86_64-pc-windows-msvc/interprocess/os/windows/named_pipe/struct.PipeListenerOptions.html>, `JobObject` in <https://docs.rs/process-wrap/9.1.0/process_wrap/tokio/struct.JobObject.html>, and Microsoft's statement that a supplied named-pipe security descriptor controls access while the default descriptor is broader than user-only at <https://learn.microsoft.com/windows/win32/ipc/named-pipe-security-and-access-rights>.
- Runs: `./.cyril-j7um/probe-oracle.sh <temporary-root>`; `./.cyril-j7um/probe-platform-oracle.py`; source/API comparison by the URLs above.

## Comparisons

| ID | Probe output | Oracle output | Verdict |
|----|--------------|---------------|---------|
| P1 | `blocked_while_held=true`; `reacquired_after_kill=true` | `lock.blocked_while_held=true`; `lock.reacquired_after_kill=true` | PASS |
| P2 | initial/reopened journals: memory=`wal`, knowledge=`wal`; reopened versions: memory=`1`, knowledge=`1` after killing an uncommitted writer | `sqlite.memory.journal=wal`; `sqlite.knowledge.journal=wal`; memory version=`1`; knowledge version=`1` | PASS |
| P3 | root mode=`0o700`; socket mode=`0o600` | root mode=`700`; socket mode=`600` | PASS |
| P5 | Windows target check completed with safe current-user SID retrieval, protected DACL construction for that explicit SID, and `PipeListenerOptions.accept_remote(false).create_tokio_duplex`; the Job Object branch also compiled under Rust 1.94 | `win-security-identifier` reads `TokenUser`; `interprocess` passes the descriptor into `CreateNamedPipeW`; Microsoft documents descriptor-based access checks and warns that the default DACL is broader; `process-wrap` documents Windows Job Object containment | PASS |
| P6 | valid=`ok`; missing/invalid auth=`unauthorized`; malformed=`malformed_frame`; oversized=`frame_too_large`; unknown=`unknown_operation`; version 2=`unsupported_version` | The independent Python implementation produced the same seven outcomes | PASS |
| P7 | `kill_completed_within_two_seconds=true`; `grandchild_reaped=true`; mechanism=`process_group`; Windows `JobObject` branch compiled | Python `start_new_session`/`killpg` independently completed within two seconds and reaped the grandchild; `process-wrap` source maps the Windows wrapper to a Job Object | PASS |

## Validated / learned

- P1: Validated prior understanding — an active child excludes a contender and OS process teardown releases ownership after `SIGKILL`, so no manually deleted stale PID sentinel is needed on the probed Unix host.
- P2: Validated prior understanding — WAL selection persists and a killed uncommitted transaction does not replace the committed schema version; both independent stores reopen at version 1.
- P3: Validated prior understanding — explicit tightening yields owner-only directory and Unix socket modes on the probed Unix host.
- P5: New learning — Tokio's own safe `ServerOptions::create` passes null security attributes and cannot prove user-only local access, but `win-security-identifier` safely retrieves the current process user SID and `interprocess` 2.4.3 safely accepts an owned security descriptor; the isolated Windows-target probe compiled a protected DACL naming only that SID with remote clients disabled.
- P6: Validated prior understanding — two implementations with different JSON, constant-time comparison, and control-flow libraries agree on all seven framed-request classes.
- P7: Validated prior understanding — safe process-group containment reaped a real descendant within the two-second bound on Unix, the independent oracle agreed, and the corresponding safe Windows Job Object branch compiles.

## Related issues

- Consulted: `cyril-j7um` (the M0 contract) and parent epic `cyril-ct0y`. One bounded tracker search also found downstream memory milestones `cyril-n3j7`, `cyril-3dqf`, `cyril-s7gn`, `cyril-nxq5`, and `cyril-y91y`; they consume the M0 runtime later and provide no prior implementation evidence for P1-P7.
- Filed: none — probe and oracle agree; no underlying-system defect or deferred work was discovered.
