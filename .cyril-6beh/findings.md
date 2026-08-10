# Prove-it findings

## Smallest question

Can live Kiro 2.16.2, with `workflowsEnabled: false`, emit the two previously unobserved terminal workflow statuses required by the signed specification: `failed` and `aborted`?

## Probe

`.cyril-6beh/probe.py` runs two isolated, credential-safe arms against the archived 2.16.2 `kiro-cli-chat` binary:

1. **Failed** — a normal gate-off parent session creates and invokes a one-step run, while the host deliberately returns an expired token to the step session. Kiro emits `node_complete.status = "failed"` and `run_complete.status = "failed"` with the authentication-refresh failure recorded as `failureReason`.
2. **Aborted** — the existing RPC-sweep harness is rewritten only to remove `workflows.enabled`; its throwaway run is paused, resumed, and cancelled. Kiro emits `run_complete.status = "aborted"` with `finalState.status = "aborted"`.

The wrapper changes temporary copies of existing audit harnesses, never production code. Captures:

- `.cyril-6beh/terminal-failed-2.16.2.jsonl`
- `.cyril-6beh/terminal-aborted-2.16.2.jsonl`

(2026-08-09: both captures relocated to `crates/cyril-core/tests/fixtures/kas/workflow/`.)

Both captures were scanned for credential-bearing fields. The failed capture records no credential response; the aborted capture uses the repository's `<redacted>` convention. No unredacted credential value is present.

## Oracle

`.cyril-6beh/oracle.sh` independently uses `jq`, not the Python probe logic, to unwrap both raw and audit-envelope JSONL records, select `_kiro/workflow/run_complete`, count terminal statuses, and fail unless both required statuses occur at least once.

Observed final run:

```text
probe:  {"aborted-arm": ["aborted", "aborted"], "failed-arm": ["failed", "failed"]}
oracle: {"failed": 2, "aborted": 2}
```

Probe and oracle agree item-for-item on four terminal frames. The signed live-evidence gate passes.

## What I learned

A KAS workflow step-session host failure is promoted cleanly into the workflow lifecycle: an expired-token refresh rejection produces both `node_complete failed` and `run_complete failed` while the gate remains off. A model-elected `send_message` fault is therefore unnecessary for deterministic failed-run evidence. Cancellation is also confirmed gate-off on 2.16.2, correcting the audit's earlier “non-invoke mutating verbs unverified” coverage gap.

## Evidence boundary

This probe proves terminal status emission and payload availability. It does not prove every failure cause, `completionSignalSource = "status_update"`, or live `node_paused`; those remain outside the signed live gate and retain source-derived fixtures.

## Review-feedback decisions

| # | Finding | Reviewer | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|---|
| 1 | Replay test ignores the independent four-source expectation | CoreLifecycleReview | Bug | Yes — `CYRIL_WORKFLOW_ORACLE_EXPECTED=/dev/null` still passed, then the environment-aware test failed against the four-source oracle | Accept | Absorb into `cyril-6beh`: honor the environment oracle and fold every committed replay source once and twice |
| 2 | `node_start` discards snapshot descriptor metadata | CoreLifecycleReview | Bug | Yes — `snapshot_reconciled_node_start_preserves_descriptor_metadata` failed because `descriptor()` became `None` | Modify | Absorb into `cyril-6beh`: retain snapshot identity and metadata whenever the structural kind is compatible; rebuild only for a changed kind |
| 3 | Exact `run_start` after snapshot is logged as a conflict | CoreLifecycleReview | Bug | Yes — `snapshot_reconciled_active_run_start_exact_repeat_is_silent` captured `reason="active_run_start_conflict"` | Modify | Absorb into `cyril-6beh`: retain the incarnation's original opening forest across reconciliation; use snapshot root children only for snapshot-seeded runs, while keeping the public current-plan view snapshot-owned |
| 4 | Four-source replay rejects the live repeat/watch capture | CoreLifecycleReview | Bug | Yes — the Rust adapter required nonexistent watch `handlerName` and recipe-only repeat fields from `finalState`, while the raw capture, wire audit, and independent Python folder agree on `agentName` plus sparse runtime nodes | Accept | Correct the design and manifest first; split strict plan descriptors from sparse runtime snapshots, preserve observed watch cursor/terminal JSON, and require Rust/Python projection equality across all four sources |
