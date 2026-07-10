# cyril-6iek — pre-PR review decisions (/code-review vs main, 2026-07-09)

**Spec axis:** conforms — zero findings. All four acceptance criteria verified
independently by the reviewer (both test lanes re-run); every plan deviation matched a
pre-approved entry in `build-audit.md`.

**Standards axis:** zero hard violations; five judgement calls, each verified against
the code before deciding:

| # | Finding | Decision | Rationale |
|---|---|---|---|
| 1 | `fingerprint.rs` imports `acp` / Kiro quirk outside `convert/` | **reject** | The rule's intent is containment inside the protocol layer — `bridge.rs`, `engine.rs`, `client.rs` already import `acp` there. Fingerprinting is handshake *verification*, not dialect *conversion*; `convert/kiro.rs` would split it from its only consumer. Design placement was pause-approved. |
| 2 | Duplicated mismatch-stop hunk ×3 in `bridge.rs` | **accept — fixed** | Extracted `notify_fingerprint_stop(tx, at, reason)`; call sites keep their own control flow (`return` at handshake, `break` in the loop). Three sites can no longer drift. |
| 3 | `Option<String>` reasons instead of a typed thiserror struct | **reject** | `BridgeDisconnected { reason: String }` is the existing channel contract (all cyril-l7tw reasons are prose). A typed mismatch struct has zero consumers today — speculative structure. |
| 4 | `Script { wire_kas, sess_ids }` paired `Option<bool>`s → enum | **reject** | Every combination is legal by design — the evidence-drift fixture requires the two axes decoupled; no illegal state exists to make unrepresentable. Test-harness code. |
| 5 | Repeated `match (bound, evidence)` across the two detectors | **reject** | The shared half is already factored (`mismatch_reason`); merging detectors with distinct evidence sources would obscure, not simplify. |
