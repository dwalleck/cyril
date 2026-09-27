# Review decisions: cyril-0dv4 (PR #138)

Round 1 — independent review of PR #138 (Opus, xhigh; relayed by the batch orchestrator on 2026-09-27). The reviewer approved with nits and reproduced the refactor equivalence, the SDK field shape (schema 1.5.0 `src/v1/agent.rs` L1024-1026 / L1173-1175 with no `skip_serializing_if`; `additional_directories` L1021 / L1186 skip-when-empty) and mutation M1. No active `plan.md` exists (Local route), so repairs use the repository's normal reproduce → fix → focused-verification path per the stage's "outside the workflow" rule.

Landing-impact order: F1 first (`blocking`), then F2 (`non-blocking`).

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Bug claim: the fence's doc comment (`session.rs` `assert_mcp_servers_is_array`, "omitted list would silently un-pool the resumed session") inverts its source — `docs/kirocrew-acp-seam-findings.md` §4.1 says an EMPTY `mcpServers` on `session/load` is applied and un-pools the session, i.e. the state cyril sends today and the fence permits; and the shared assertion clause "kiro-cli exits rc=0 with no stderr when it is missing" is sourced only for `session/new` (§4.1 row for `session/new`), yet the helper prints it for `session/load` too. Fix claim: reword the load paragraph (key required by the ACP schema; even an EMPTY list un-pools per §4.1, tracked by cyril-rtrh, NOT guarded here) and make the rc=0 clause `session/new`-only; re-run M1 so the receipt shows the new messages. | Opus xhigh review (via orchestrator) | Verified | Direct reading: `docs/kirocrew-acp-seam-findings.md:267` (`session/new` row: missing field → rc=0, no stderr) vs `:270` (`session/load` row: "an empty `mcpServers` is *applied*, un-pooling the session for life"); pre-repair comment at `session.rs:575-578` said "omitted list"; pre-repair helper at `session.rs:592-593` hard-coded the rc=0 clause for every caller. | Accept | Doc comment rewritten with a `session/new` paragraph and a separate `session/load` paragraph (schema-required key; EMPTY list un-pools per §4.1; deliberately not guarded; resume contract → cyril-rtrh). Helper now takes `why_required: &str`; `session/new` passes `"kiro-cli exits rc=0 with no stderr when it is missing"`, `session/load` passes `"required by the ACP schema; kiro-cli re-initializes the session's MCP servers from it"`. Applied in `crates/cyril-core/src/protocol/domain_mediator/commands/session.rs` (`tests::assert_mcp_servers_is_array` and both call sites). Receipt below. | blocking — the delivered fence's stated rationale contradicted its cited source, and the shared message attributed a `session/new`-only failure mode to `session/load`; the predicate is unchanged, so this is a comment/message repair, not a behavior change. On the tracker pointer: cyril-rtrh's own text names the Crew resume contract (`cwd`, `mcpServers:[]`, `_meta` session_file, `modes` check) rather than the literal "re-declare a non-empty list"; the cross-reference to cyril-rtrh comes from cyril-0dv4's own description, so the comment cites it as the resume-contract owner, not as an explicit un-pooling issue. |
| F2 | Bug claim: `route.md` T2 says "the other 7 hits are test fake-agent handlers that RECEIVE the request", but one of the 7 SENDS (`crates/cyril-core/src/protocol/sdk_runtime/tests/topology.rs:181`, `.send_request(acp::NewSessionRequest::new(...))`). Fix claim: say "test-only (6 fake-agent handlers, 1 topology test sender)". | Opus xhigh review (via orchestrator) | Verified | The routing-stage grep (`grep -rn "NewSessionRequest\|LoadSessionRequest" crates/ --include='*.rs' \| grep /tests`) listed 7 hits: `topology.rs:151` handler, `topology.rs:181` sender, `serial.rs:528/866/1065` handlers, `bridge/tests/harness.rs:275` and `:445` handlers — 6 handlers + 1 sender. | Accept | `route.md` T2 row reworded to "test-only: 6 fake-agent handlers that receive the request, 1 topology test sender at `protocol/sdk_runtime/tests/topology.rs:181`"; the PR body's Evidence bullet carries the same correction. | non-blocking — a record nit with no code impact; fixed in the same round because the route record is the change's evidence record. |

## Review errors

None found. The reviewer's anchors (`session.rs` ~574-578; schema 1.5.0 L1021/L1024-1026/L1173-1175/L1186) match the pre-repair tree and the pinned SDK source as read during routing.

## Repair re-review

N/A — the round changed only a doc comment, an assertion message (predicate unchanged), and a route-record sentence; no production behavior or fence discrimination changed. The message change is proven at both call sites by the M1 receipt below rather than by a second reviewer.

## Receipts (post-repair revision)

Checked source: this branch after the F1/F2 repairs (parent `08cd2579`, production code byte-identical to `82cf962a`'s — round 2 touches only the `#[cfg(test)] mod tests` block of `session.rs` and `route.md`). Environment: Linux x86_64, Rust 1.94.0, worktree-local `target/`, `env -u CARGO_TARGET_DIR` on every cargo call. Scratch logs are `cyril-0dv4-round2-*.log` in the (shared) session scratchpad; this record is the durable evidence.

Focused command for every fence run: `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --all-features -E 'test(/commands::session::tests::/)'`.

**F1 fence-message receipt** (shared helper's message changed → proven at both call sites):

- M1 — production `standard_params` removes `mcpServers` from the serialized object (same injected fault as round 1): **2/2 FAIL**.
  - `session/new params must carry \`mcpServers\` as an array (kiro-cli exits rc=0 with no stderr when it is missing); got None in {"cwd":"/home/dwalleck/.claude/tmp"}`
  - `session/load params must carry \`mcpServers\` as an array (required by the ACP schema; kiro-cli re-initializes the session's MCP servers from it); got None in {"sessionId":"sess_fence-0dv4","cwd":"/home/dwalleck/.claude/tmp"}`
- Restored (`grep -c MUTATION session.rs` → 0): **2/2 PASS** (`new_session_request_always_carries_mcp_servers`, `load_session_request_always_carries_mcp_servers`).
- M2 (present-but-`null`) is retained from round 1 (`route.md` Terminal criterion): the predicate `is_some_and(Value::is_array)` is unchanged, so its round-1 red/restored-green still applies; only the message clause changed and M1 above prints the new clause at both call sites.

**Round-2 verification** (the set the orchestrator specified for this round), all PASS on the restored tree:

- `cargo fmt --all -- --check` → PASS
- the two fences (command above) → PASS, 2 passed / 1073 skipped
- `cargo clippy -p cyril-core --all-targets --all-features -- -D warnings` → PASS

**Evidence disposition:** the round-1 full gate set on `82cf962a` (eight commands, all PASS, counts in `route.md` and the PR body) is retained for every crate and feature leg: no production path, fixture, dependency, or configuration changed; the only changed proof artifact is the two fences' message text, re-proven above; `session.rs`'s test module carries no feature gating, so the all-features clippy re-run covers the same test code that the default and no-default legs compile.
