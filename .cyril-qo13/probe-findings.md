# cyril-qo13 — prove-it-prototype findings (2026-07-02)

## Smallest question

> When the user picks option k on each real KAS `session/request_permission`
> request, what optionId does cyril's current pipeline put on the wire — and
> what does the correct reply look like?

## Probe

`crates/cyril-core/src/protocol/convert/probe_qo13.rs` — a `#[cfg(test)]`
module (the functions under test are `pub(crate)`). Replays all 10 permission
requests from the committed live trace
`experiments/conductor-spike/kas-live-session-trace-2.11.0.jsonl` through
cyril's REAL pipeline: serde parse into `acp::RequestPermissionRequest` →
`to_permission_options` → per selected index k: `PermissionResponse::from(kind)`
(what `UiState::approval_confirm` does, cyril-ui `state.rs:1345-1356`) →
`from_permission_response` → emitted wire optionId.

Run: `cargo test -p cyril-core probe_qo13 -- --nocapture` → `probe-output.txt`.

## Oracle

`oracle.py` — raw JSON text extraction (no cyril code, no acp crate) of what
the **reference client** (Kiro v3 TUI, recorded via `KIRO_ACP_RECORD_PATH`)
actually replied to the same requests, plus each request's raw option list by
index. Ground truth is anchored in reference-client *behavior*: it picked
non-first options on requests 2 (k=2), 3 (k=1), 10 (k=1), and the agent
demonstrably honored those picks (after id=3 → option-1 "Keep the blob, add
structured append" + id=4 → "Timestamp only" + id=5 → "New `note add`
subcommand", the agent designed exactly "a new `rivets note add <id>` command
that appends a UTC-timestamped entry ... without overwriting earlier content";
the fs_writes behind ids 6–9 all reached `status: completed`).

Run: `python3 .cyril-qo13/oracle.py` → `oracle-output.txt`.

## Agreement

Probe and oracle agree **item-by-item on all 10 requests × every option** that
the correct reply for pick k is `options[k].optionId`, and that cyril's parse
path preserves every option id in order (probe `picked_id` ≡ oracle
`correct reply for pick k`, byte-identical). On the distinct-kind fs_write
requests (6–9), cyril's *current* output also agrees with the oracle for all
16 pick-cases — standard tool approvals are correct today, as the issue claims.

## Confirmed disagreement (the filed bug — cyril-qo13 itself)

On the all-`allow_once` user_input requests (ids 1–5, 10), cyril's current
pipeline emits option-0 for **every** pick: 11 of 16 pick-cases WRONG.
Substrate-broken cause already filed as cyril-qo13; this probe is its
pre-design gate, so no new ticket.

## What I learned (not obvious before the probe ran)

1. **The consent `_meta` caveat is RESOLVED for `allow_once`**: the reference
   client itself omitted the consent `_meta` echo on requests 8/9 (present on
   6/7, all four `consentRound: 1` fs_writes), and all four writes completed
   identically → consent echo is *optional* for invocation-scope grants. The
   issue's control probe for that case is unnecessary.
2. user_input replies are **bare** `{"outcome": {"outcome": "selected",
   "optionId": ...}}` — no `_meta` at either level → the optionId fix needs no
   metadata plumbing for user_input.
3. `{"outcome": "cancelled"}` is a legal user_input reply: the reference client
   cancelled request 1 and KAS simply re-asked (request 2) — cyril's existing
   Esc→Cancel path is already wire-valid for these questions.
4. The acp crate (0.10.2) parses all KAS user_input requests losslessly — the
   probe would have panicked otherwise.

## Residual unknown (does not block the optionId fix)

Whether an `allow_always` grant needs the consent `_meta` echo
(`scope: "session"|"always"`) for KAS to persist the grant scope — the trace
contains no `always-accept` reply. Needs a live control probe at design time
(feedback_kiro_schema_vs_runtime) before implementing the consent-passthrough
half of the widened scope.

## Hard gate (falsifiable-design prerequisites)

- [x] Probe written, runs against the real codebase + real trace data
- [x] Oracle defined, produces output (`oracle-output.txt`)
- [x] Probe and oracle agree on a non-trivial slice (all 26 correct-reply
      cases across 10 requests; plus cyril-current agrees on all 16
      distinct-kind cases)
- [x] Learned something new: consent `_meta` is optional for allow_once
      (finding 1 above)
