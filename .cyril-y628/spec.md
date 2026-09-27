# Spec: Optional feedback when rejecting a tool call

Status: Approved by the requester on 2026-09-26.
Trigger: route.md T4 identifies observable approval UX decisions absent from the ticket.

## Request (verbatim)
> claim and implement cyril-y628

## What this is
Cyril's interactive operator can explain a one-time tool rejection in the approval overlay without changing the existing immediate plain-rejection action. The affordance is KAS-only; Cyril returns nonempty feedback as response-level `_meta.kiro.rejectionReason`, which retained live evidence proves KAS 0.66.8 forwards to the model.

## Roles
- **Interactive operator**: decides whether an agent may run the displayed tool and may explain a rejection.
- **Requesting agent**: receives exactly one decision for its permission request and may consume the optional feedback extension.
- **Requester**: approves the observable behavior and architecture for this change.

## Behavior

### Optional entry
- **Given**: a KAS permission overlay has an agent-provided reject_once option selected.
- **When**: the interactive operator presses `r`, advertised as “Reject with reason”.
- **Then**: the same approval opens an empty multiline feedback editor; no decision is sent yet. Enter in the original option list still rejects immediately without feedback.

### Engine and option scope
- **Given**: a v2/non-KAS permission request, or a selected option other than reject_once.
- **When**: the overlay renders or the interactive operator presses `r`.
- **Then**: no feedback action is advertised or entered. Existing allow, trust-selection, always-reject, and cancellation behavior remains unchanged.

### Editing and submission
- **Given**: the feedback editor is active.
- **When**: the interactive operator types, edits with standard textarea keys, pastes, presses Ctrl+J, or presses Enter.
- **Then**: typing/paste affect only the feedback draft; Ctrl+J inserts a newline; Enter submits exactly once. Explicitly modifier-reported Shift+Enter also inserts a newline, but legacy terminals may send an indistinguishable Enter and are not promised that binding. A non-whitespace draft is sent without trimming its text, using the exact original reject_once option ID and top-level `_meta.kiro.rejectionReason`. An empty or whitespace-only draft sends a plain rejection with no rejection metadata. Pasted line breaks remain line breaks; terminal control characters other than line breaks/tabs are not inserted as control sequences.

### Input bound
- **Given**: a feedback draft and an insertion would exceed 4,096 Unicode scalar values.
- **When**: the interactive operator types or pastes that insertion.
- **Then**: Cyril rejects that entire insertion, preserves the prior draft, and displays an input-limit notice. The overlay displays the limit; deletion/editing remains available. No partial paste is silently submitted.

### Back and draft lifetime
- **Given**: the feedback editor is active.
- **When**: the interactive operator presses Esc.
- **Then**: Cyril discards the draft and returns to the same selected approval option without sending a decision or cancelling the agent turn. Re-entering starts empty. Esc from the original option list retains existing cancellation behavior.

### Queue, identity, and isolation
- **Given**: multiple permission requests arrive, potentially from different sessions, while feedback is being edited.
- **When**: the interactive operator edits or submits the active approval.
- **Then**: later requests remain queued; each decision answers only its originating request using the exact offered option ID. No feedback reaches a later request, the normal chat draft, a trust choice, or an unrelated session. Existing originating-session attribution and tool preview remain available. Existing buffered-key protection applies after terminal decisions.

### Modal and terminal behavior
- **Given**: the feedback editor is active in a supported terminal size, including a cramped viewport.
- **When**: Cyril renders, receives pasted text, or receives a voice transcript.
- **Then**: the editor and cursor stay within the approval overlay using wrapping/scrolling; the normal chat input is not edited. Paste is routed into this editor only, not into other modal overlays. Voice input remains blocked under modal overlays. No local chat/system echo of the feedback is added.

### Wire compatibility
- **Given**: a KAS reject_once decision with nonempty feedback, or any decision without eligible feedback.
- **When**: Cyril serializes the permission response.
- **Then**: only the eligible response contains top-level `_meta.kiro.rejectionReason`; the selected outcome retains the exact offered ID. No reason is emitted for reject_always, allow, cancellation, a foreign option ID, or a non-KAS request. Existing v2 `outcome._meta.trustOption` remains unchanged. Older KAS may ignore feedback; this feature does not promise consumption by unsupported agents.

## Success criteria

- **Wire:** exact top-level metadata and selected ID match the committed KAS response contract; conversion and SDK round-trip checks verify the eligible case and exclusion cases.
- **Interaction:** a real Cyril TUI session against a controlled ACP agent exercises plain reject, `r` entry, multiline paste/edit, Enter submission, and Esc-back; captured agent responses prove text and decision identity.
- **Isolation:** approval state/App checks verify queued requests, normal chat draft, trust choices, voice guards, and buffered-key boundaries remain independent.
- **Bound:** exact 4,096-scalar input is accepted and over-limit insertion leaves the draft unchanged with visible notice; Unicode-boundary and oversized-paste checks prove this.
- **Display:** actual terminal captures cover ordinary and cramped geometry, visible entry guidance, editor text/cursor, and no input leakage; render checks cover clipping/scrolling behavior.
- **Repository:** applicable default and KAS tests and clippy gates pass; mutation checks demonstrate that required fences detect the named defects before restoration.

## Out of scope

No feedback affordance for v2/other agents or reject_always; no third-party delivery guarantee; no version-discovery feature; no local feedback history/echo, voice dictation into the editor, auto-approval policy, consent persistence, trust-model changes, or stall-clock repair. Consent research remains cyril-gn07; stall-clock work remains cyril-levc. These are unchanged independent responsibilities, not relaxed acceptance obligations for this feature.

## Related issues

Tracker discovered through docs/agents/issue-tracker.md. Searched native `rivets list --limit 100000 --json` across all statuses for title keywords reject, feedback, approval, permission on 2026-09-26; read matching feature-area records.

- cyril-y628: this feature; exact response metadata and paired KAS 0.66.8/0.66.0 evidence.
- cyril-qo13 (closed): exact option-ID fidelity, cancelled outcome, trust-phase chosen-ID provenance.
- cyril-z4eo (closed): FIFO permission queue and exact session attribution.
- cyril-j1b3 (closed): KAS stub-request preview enrichment and snapshot independence.
- cyril-5lwp (closed): workflow permission attribution is session-based; auto-approval is separate.
- cyril-levc (open): existing approval-wait/stall-clock issue; no liveness change here.
- cyril-p7kp (open): unknown permission-option kinds are a release-watch concern; existing reject_once only here.
- cyril-gn07 (open): always-allow consent-scope echo is separate research.

Other keyword matches concern prompt replay, initialization, workflow parse diagnostics, and file completion rather than this permission feature.

## Decisions

Rows marked proposed record their origin in the final proposal; the Approval below adopts every row.

| Question | Decision | Rationale | Implication |
|---|---|---|---|
| How is feedback entered? | Separate optional action | Requester selected “Separate optional action” on 2026-09-26 | Enter on Reject remains immediate plain rejection. |
| Which engines expose feedback? | KAS only | Requester selected “KAS only” on 2026-09-26 | v2/other agents keep existing UI; older KAS may ignore feedback. |
| What editor behavior? | Multiline with paste; Enter submits, Ctrl+J adds a line, explicitly reported Shift+Enter also works; Esc returns without a response, blank submits plain rejection | Initial “Multiline with paste” selection, amended by requester “Use Ctrl+J reliably” on 2026-09-26 after verifying legacy CR/LF decoding | Advertise Ctrl+J; do not enable or promise enhanced keyboard protocols. |
| Which shortcut? | Proposed: `r` when reject_once is selected | Visible opt-in action leaves existing keys unchanged | Hint and input routing share the eligibility rule. |
| Max scale? | Proposed: 4,096 Unicode scalar values, reject oversized insertions atomically with notice | Bounded human explanation without silently truncating a paste | Test exact bound, overflow, Unicode, editing after refusal. |
| Whitespace and control characters? | Proposed: omit whitespace-only reason; otherwise preserve text; exclude terminal control characters except line breaks/tabs | Avoid claiming special empty-feedback server behavior or allowing control-sequence injection | No trimming meaningful explanations; multiline paste retained. |
| Back-navigation draft lifetime? | Proposed: Esc discards draft and returns to same option; re-entry is empty | Avoid stale explanation reuse after changing decision | No answer and no turn cancellation on editor Esc. |
| Paste and voice behind overlays? | Proposed: accept paste only into active feedback editor; keep voice and other modal paste guards | Existing modal isolation must remain | Normal chat draft is unchanged. |
| Local echo and scope boundary? | Proposed: no new local echo/history, trust-policy work, stall fix, or version probe | Ticket requests rejection feedback, not new transcript/policy machinery | Out-of-scope list above applies. |
| Empty set? | No reject_once option means no feedback action | Agent owns offered choices; cyril-qo13 | Preserve existing empty-option/cancel handling. |
| Null/missing field? | Absent feedback means absent metadata; absent eligibility means no action | Optional extension and exact-choice fidelity | Plain responses unchanged. |
| Concurrent writes? | One editor per active FIFO request; no displacement | cyril-z4eo | Preserve responders and request-local draft. |
| Permission denied/unauthenticated? | No new permission/auth system; never approve as fallback | Existing decision semantics; cyril-qo13 | Reject stays reject; transport failure does not execute a tool. |
| Partial failure? | Existing responder-send failure path remains visible through existing error handling; no retry of a human decision | This feature changes response content, not transport ownership | Failed send does not answer the next queued request. |
| Retries/idempotency? | One answer per request; a new request starts with no prior feedback | Existing oneshot lifetime; cyril-z4eo | No automatic decision replay. |
| Soft-deleted records? | N/A — no persistence or deletion model | Ephemeral approval interaction | No new storage. |
| Multi-tenancy boundaries? | Bind feedback to originating session and request | cyril-z4eo and cyril-5lwp | No cross-session draft/response reuse. |
| Time-zone/DST? | N/A — no clock-dependent feature behavior | Text editing and immediate response only | No date/time contract. |
| Replication lag? | N/A — no replicated state | Local editor plus existing ACP channel | No consistency layer. |
| Cache invalidation? | Draft cleared on exit/resolution; tool preview remains existing snapshot | cyril-j1b3 | No cached feedback across requests. |
| Wire option identity? | Preserve exact offered ID; no synthetic ID | cyril-qo13 | Optional action resolves the original reject_once option. |
| Consent/autopolicy changes? | None | cyril-gn07 and cyril-5lwp | Existing responsibility ownership retained. |

## Approval

Requester approval (verbatim): "Approve specification"
Date: 2026-09-26
Scope: all behavior and Decisions rows, including the proposed rows presented for this sign-off. The proposal wording above records their origin; these rows are now approved.

Amendment approval (verbatim): "Use Ctrl+J reliably"
Date: 2026-09-26
Scope: replace unconditional Shift+Enter promise with Ctrl+J, retain explicitly reported Shift+Enter, verify raw LF through actual TUI; all other approved behavior is unchanged.
