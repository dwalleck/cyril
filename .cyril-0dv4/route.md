# Route: cyril-0dv4

Change: Fence that the `session/new` and `session/load` requests cyril builds always carry `mcpServers` as a JSON array (kiro-cli exits rc=0 with no stderr when the field is missing).
Date: 2026-09-27

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The fence's only premise is "the request cyril constructs serializes `mcpServers` today", and that is covered by in-repo evidence: (a) the pinned SDK — `agent-client-protocol =2.0.0` re-exports `agent-client-protocol-schema 1.5.0` (`Cargo.lock`), whose `v1::NewSessionRequest` (`src/v1/agent.rs:1011-1049`) and `v1::LoadSessionRequest` (`:1171-1213`) declare `pub mcp_servers: Vec<McpServer>` with NO `skip_serializing_if` (only the newer `additional_directories` field carries `#[serde(default, skip_serializing_if = "Vec::is_empty")]`), and both `::new()` constructors set `mcp_servers: vec![]`; (b) a frame recorded from cyril's own spawn path, `experiments/conductor-spike/v2-live-session-trace-2.11.0.jsonl` (`"dir":"out"`, `"method":"session/new"`, `"params":{"cwd":"/home/dwalleck/repos/rivets","mcpServers":[]}`). The motivating rc=0 claim (kiro-cli treats a missing `mcpServers` as malformed and exits rc=0 with no stderr) is attributed to `docs/kirocrew-acp-seam-findings.md` §4.1 / KiroCrew `_dispatch.py:70-76` and is NOT a premise the fence depends on: the fence asserts the field is present, which is required by the ACP schema regardless of how any one agent reacts to its absence. It was deliberately not re-measured here — a live `kiro-cli` spawn on this shared machine risks the single-use OIDC refresh race that logs the user out, and the requester scoped this change as a fence with no behavior change. | no |
| 2 | Structural module shape | Owner unchanged: `crates/cyril-core/src/protocol/domain_mediator/commands/session.rs` already owns session/new and session/load request construction and their serialization (`send_standard`). The change extracts the two inline request expressions into private constructor fns (`new_session_request`, `load_session_request`) and the `serde_json::to_value` step of `send_standard` into a private `standard_params` fn so a colocated test can exercise the exact production path; `send_standard`, `new_session`, `load_session` keep their signatures and semantics. No public interface, schema, seam, dependency direction, or responsibility move; no new types. Callers of the extracted expressions: none other than the two production sites (grep `NewSessionRequest\|LoadSessionRequest` under `crates/` — the other 7 hits are test fake-agent handlers that RECEIVE the request). Length gates: none found in the repository (`grep -rn "max.lines\|line.count\|wc -l\|too long\|length gate" scripts/ .github/` → no hits; no `clippy.toml`). File size 535 → ~610 lines including the test module; the growth is a colocated test plus ~15 production lines of extraction, no new responsibility cluster. | no |
| 3 | Production-scale risk | None: the extraction is a pure refactor of two request constructions and one `serde_json::to_value` call already executed once per session start. No latency, throughput, memory, concurrency, or data-volume dimension changes. | no |
| 4 | Explicit behavior | Given the cwd cyril was started with (after `platform::path::to_agent` translation), when the mediator builds the `session/new` request and serializes it with the production params serializer, then the params object carries key `mcpServers` whose value is a JSON array (empty allowed). Given a session id and that cwd, when the mediator builds the `session/load` request and serializes it the same way, then the params object carries key `mcpServers` whose value is a JSON array (empty allowed). No production behavior changes; a wire frame byte-identical to today is expected. The requester pre-decided: fence only, no new types; if omission is impossible via the SDK type the test stands as the tripwire against an SDK bump and says so in its comment; add the same fence for session/load because production constructs `LoadSessionRequest` (session.rs:144, cyril-rtrh family). | yes |

Unknown tests: none

## Selected route

Local — a colocated fence over an existing responsibility with no interface, schema, or ownership change.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict); requester decisions recorded in T4 |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict) |
| design.md | falsifiable-design | N/A — Local route: no design gate |
| plan.md | budgeted-plan | N/A — Local route: no plan gate |

Oracle checkpoint in `checkpointed-build`: N/A — Local route: checkpointed-build does not run

## Downstream sequence

none — implement with normal repository fix/TDD

## Terminal criterion

Local — focused behavioral verification: `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --all-features -- session::tests::` (the two fence tests), plus the mutation receipt: temporarily make production `standard_params` strip `mcpServers` from the serialized object (the closest production-side simulation of an SDK bump adding `skip_serializing_if = "Vec::is_empty"` to `mcp_servers`) → both tests red; restore → green. Full CI-mirroring gates are recorded in the PR body.

Result: 2026-09-27 | `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --all-features -E 'test(/commands::session::tests::/)'` | PASS

Checked source: branch `chore/cyril-0dv4-session-new-mcp-servers-fence` on top of `f9bc81d8` (main), linked worktree; environment Linux x86_64, Rust 1.94.0 (`rust-toolchain.toml`), worktree-local `target/`, `CARGO_TARGET_DIR` unset per invocation (the inherited value was an empty string). Logs were written under the session scratchpad (`focused-green-1.log`, `mutation-m1-red.log`, `mutation-m2-red.log`, `focused-green-restored.log`, `gate-*.log`, `gate-results.txt`), but that directory turned out to be shared with the eleven sibling agents of this batch and files in it get overwritten (a sibling's PR body replaced this change's `pr-body.md` while it was in use), so the log files are not a durable reference. The verdicts below were read directly from the runner's `gate-results.txt` whose header line named this branch and start time (`HEAD=f9bc81d8 BRANCH=chore/cyril-0dv4-session-new-mcp-servers-fence START=2026-09-27T00:32:05-05:00 … DONE END=2026-09-27T00:36:16-05:00`), and the counts below differ from every sibling's (this tree adds two cyril-core tests) — this record and the PR body are the evidence record.

- Baseline, unmodified production: 2/2 PASS (`new_session_request_always_carries_mcp_servers`, `load_session_request_always_carries_mcp_servers`).
- Mutation M1 — production `standard_params` removes `mcpServers` from the serialized object (the closest production-side simulation of an SDK bump adding `skip_serializing_if = "Vec::is_empty"` to `mcp_servers`; the SDK type itself cannot omit it): 2/2 FAIL, message `session/new params must carry \`mcpServers\` as an array (kiro-cli exits rc=0 with no stderr when it is missing); got None in {"cwd":...}` and the `session/load` twin.
- Mutation M2 — production `standard_params` sets `mcpServers: null` (present but not an array): 2/2 FAIL, `... got Some(Null) ...`.
- Restored (`grep MUTATION session.rs` → no hits): 2/2 PASS.

Full CI-mirroring gates on the restored tree, all PASS (`gate-results.txt`, real exit codes, no pipes): `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo nextest run --workspace --all-features`; `cargo test --doc --workspace --all-features`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo nextest run --workspace`; `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings`; `cargo nextest run -p cyril-core --no-default-features`. Counts are recorded in the PR body.

Final T2 recheck: `session.rs` 535 → 617 lines (the colocated test module is lines 562–617, i.e. 56 of the 82 added); still one responsibility owner, no interface change, no length gate exists in the repository — no new trigger. Cross-platform qualification is owned by the PR's Windows/macOS CI jobs, not claimed from these Linux runs.
