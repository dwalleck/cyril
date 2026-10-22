## Ordinary independent source review: **PASS**

**Findings:** none meeting the actionable, patch-introduced bug criteria.  
**Overall correctness:** **correct**  
**Confidence:** **0.94**

This verdict covers the actual assembled Tqr production delta at `d0cc60d7662ced69a4a32c904e9800bea6990a22` against `03838a501dad8be140acd78dc3fff4cae35fafef`, including affected consumers outside the diff. It does **not** establish runtime/native/platform acceptance or issue completion.

### Governing-scope correction

The current `gilfoyle/references/module-shape.md` and `checkpointed-build.md` make substantive ordinary independent review the default. Isolated reconstruction is conditional, not a universal gate. The already-performed reconstruction comparison is **supplemental evidence**, not grounds for requiring another reviewer, reconstruction, or formatting/conformance pass.

## Actual review coverage

The inspected production delta comprises **11 behavior/export files**:

| Area | Changed files | Ordinary-review conclusion |
|---|---|---|
| Engine capability ownership | `crates/cyril-core/src/protocol/engine.rs` | Advertisement and execution derive from the same installed adapter. V2 and no-KAS refusal paths remain truthful. |
| Callback ingestion and execution | `protocol/domain_mediator/host.rs`, `protocol/kas/callbacks.rs` | Required-field parsing, typed callback construction, global scope, bounded enqueue-before-success, error propagation and SDK response wiring are connected. |
| KAS error conversion | `protocol/convert/kas.rs` | Exact supported classification and original message survive conversion without fabricating a turn terminal or disconnect. |
| Domain vocabulary | `crates/cyril-core/src/types/event.rs` | New payloads contain domain data, not SDK request/responder types. Session scope remains in the envelope. |
| UI projection and exports | `crates/cyril-ui/src/mcp_notice.rs`, `src/lib.rs` | Terminal-control/bidi escaping preserves other printable characters without an intermediate escaped-string allocation. Public visibility is the supplemental interface question below. |
| Transcript and attention policy | `crates/cyril-ui/src/state.rs`, `src/subagent_ui.rs` | Global/main notices, canonical scoped errors, attributed main copies, chronology, follow restoration and child-focus exit follow the approved behavior. Ordinary system-message policy is unchanged. |
| Rendering | `crates/cyril-ui/src/widgets/chat.rs` | Both existing branches consistently use reserved content width for preparation, wrapping, line counting and painting; scrollbar chrome uses the separate gutter. |
| Exhaustive executable consumer | `crates/cyril/examples/test_bridge.rs` | Both new notifications have concrete safe-output branches rather than disappearing through an unmatched consumer. |

The sole `crates/cyril/src/app.rs` change registers a test module; it introduces **no production App responsibility**.

I also inspected the changed dedicated tests/probes and inline regressions. They were reviewed as source, **not executed**.

## Load-bearing interactions checked

- **Complete URL callback route:** existing SDK extension catch-all → host dispatch → generic lifecycle admission → callback execution → bounded domain queue → turn forwarding → App → UI notice. Relevant consumers are at `client.rs:273-287`, `host.rs:96-120,270-307`, `callbacks.rs:429-440`, `domain_mediator/mod.rs:499-500`, and UI `state.rs:1130-1137`.
- **Truthful acknowledgement:** `callbacks.rs:429-440` constructs success only after enqueue succeeds. A closed queue cannot become a success acknowledgement.
- **No feature-only execution:** adapter refusal occurs before parameter handling in KAS-enabled builds; the known method is also explicitly refused without KAS (`host.rs:53-62,96-104`).
- **MCP-error ownership:** converted errors retain the actual routed session, pass through nonterminal turn handling, and reach main, workflow, tracked/optimistic-child or pending handling. Canonical storage precedes the attributed main copy (`state.rs:2095-2098,2136-2141`; `subagent_ui.rs:154-160`).
- **No adoption duplication:** late workflow adoption moves existing history rather than replaying notifications (`state.rs:2150-2155`).
- **Protected parents:** App has no production delta; generic host lifecycle remains capability-agnostic; Engine grows declarations/capability derivation rather than an OAuth state machine.
- **Dependency direction:** wire parsing remains in core; UI owns presentation and focus/follow policy; rendering consumes state without introducing protocol commands or async work.

No silent-drop integration defect or additional production owner was found.

## Supplemental conformance result and candidates

The earlier **literal design comparison remains FAIL on one interface discrepancy**, but this must not be conflated with the ordinary correctness verdict or a universal isolation gate:

- **Private-versus-public formatter:** design `.cyril-tqr8/design.md:14` calls for a private UI formatter, while `cyril-ui/src/lib.rs:12` and `mcp_notice.rs:8-10` expose it publicly for `test_bridge.rs:44-47,633-637`.
- **Proposed disposition:** Main should adjudicate the narrow public-sharing choice versus restoration of the private boundary. It is not a proven behavioral defect. The substantive trade-off is sharing one escaping policy across crates versus keeping the approved private interface; a second escaping convention should not be introduced accidentally.
- No additional review pass is requested solely to reformat or restate that decision.

Other bounded observations remain **coverage/risk qualifications, not findings**:

1. The direct-dispatch backpressure test proves callback delivery ordering, not full SDK initialization-saturation progress.
2. The conversion test’s `Debug`-string assertion is representation-coupled, with its base-compatible purpose documented.
3. Current application logging filters SDK payload DEBUG/TRACE events through an INFO-default subscriber. The source does **not** establish payload-safe diagnostics under arbitrary permissive subscribers.
4. The iterator’s cached-panel comment describes a hypothetical use; its actual current consumer is the formatter. No second concrete adapter was inferred.

## Limits

No edits, Git mutations, builds, tests, linters, formatters, applications or authentication probes were performed. Prior qualification/review claims were not used to establish this verdict. LSP had no loaded target workspace, so empty reference results were discarded and consumer tracing used source/AST/text inspection.

**Ordinary source review is complete with no actionable findings. Native, runtime, CI, platform and integrated acceptance remain separate obligations.**