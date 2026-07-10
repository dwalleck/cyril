# cyril-6iek — related issues (prove-it step 0)

Tracker sweep 2026-07-09 (keyword: fingerprint, engine identity, v3, handshake, sess_).

| Issue | Status | Relation |
|---|---|---|
| cyril-j16p | closed (PR #32) | KAS-2a walking skeleton — established the `sess_` session-id shape and `turn_end` semantics this issue's fingerprints ride on. |
| cyril-dn91 | open | "KAS host callbacks gated by cargo feature, not bound engine — implement ADR-0001 capability accessors or amend the ADR." Adjacent: cyril-6iek puts *detection* outside the `kas` feature; dn91 is about *callbacks*. Not blocking, but the design should not preempt dn91's resolution. |
| cyril-ykkc | open | "Local gate never compiles kas-gated code — mandate `--features kas` in the verification loop." Directly relevant to this branch's own gates: run both feature lanes per slice. |
| cyril-5db7 | open | KAS discovery/auth type-shape cleanup — same code neighborhood (`kas::auth`, spawn gate); avoid scope creep into it. |
| cyril-9akh | open | v2 notification-vs-response ordering race — tangential; mentions turn-completion signaling differences that motivated engine-keyed turn-end. |

**Prior-decision conflict found:** `crates/cyril-core/src/types/agent_engine.rs:60-71` — the D7 parse-table
test *deliberately* asserts `"v3".parse::<AgentEngine>()` is an **error**, with the comment "v3 is the
kiro-cli flag, not cyril's selector value." cyril-6iek asks to accept `v3` as an alias for Kas.
This is a recorded-decision reversal, flagged as an open decision for the design pause.

No existing issue covers wire fingerprinting itself — cyril-6iek is the first.
