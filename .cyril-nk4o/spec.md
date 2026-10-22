# cyril-nk4o — MCP management behavior proposal

## Request (verbatim)

> nk4o full MCP subsystem scope as ticket, no silent exclusion of requirements.

The authoritative tracker scope is the full `cyril-nk4o` record, including status/governance panel, reset with OAuth, **optionally** prompt/resource reads, and an explicit elicitation decision. No title-only interpretation is used.

## Behavior and success criteria

1. On KAS, `/mcp` opens a live, keyboard-navigable MCP server panel. Rows distinguish connecting, connected, failed transport, auth-required, disabled and unrecognized status. Connected rows expose advertised tools/prompts/resources/resource templates. No snapshot yet differs visibly from a known empty snapshot; zero configured servers is a legitimate explicit empty state.
2. `_kiro/mcp/status` is an authoritative replacement snapshot, including removals. Preserve its session identity and resource provenance. Optional registry mode fields remain optional: an absent accessMode is not a fabricated non-registry decision. Show catalog entries, enabled state, access-mode filtering and unresolved registry names; do not label policy-filtered servers as merely disconnected.
3. `_kiro/mcp/governance_disabled` visibly explains API failure versus administrator disablement, including future/unknown reason safely. The panel keeps policy explanations visible when the active pool is empty. Do not attempt to bypass governance or synthesize a local policy override. Status pushes update an already-open panel but never open a modal unsolicited.
4. Reset reconnects the selected actual server through `_kiro/mcp/resetServer {serverName}`; authenticate sends `{serverName,startOAuth:true}`. Actions are explicit and session-bound; pending state is visible, duplicate in-flight activation is disabled, and failures remain visible. A returned `{success:true}` is only acknowledgement; authoritative status decides connected/auth-required/failure. The response wait permits KAS's roughly 60-second bounded reset behavior and leaves notifications/host callbacks/UI live.
5. Passive `failedAuthorization && authorizationUrl` is actionable and remains visible per server until updated status resolves/removes it. The actual URL callback in `tqr8` surfaces the active reset/lazy sign-in URL; current KAS status—not the dark completion notification—clears attempts and shows connection success. The panel provides a manual URL even if no callback is emitted in passive mode.
6. Proposed optional read scope: support `_kiro/mcp/getPrompt` and `/getResource` from the connected server's advertised entries. Prompt arguments are explicit user-entered strings keyed by advertised argument names; resource selection retains the advertised URI. Display returned prompt/messages or resource contents as **read-only external data**. Fetch does not send a chat prompt, execute a tool, write a file or silently grant permission. Unsupported/nontext content is visibly represented by its kind/MIME/URI metadata rather than dropped or executed. Missing entries, disconnects, governance refusal, malformed result and transport failure show distinct actionable errors, never empty success.
7. Proposed elicitation decision: install a bound-KAS-only responder that returns the valid `{action:"decline"}` resolution and visibly names that an MCP form/URL interaction was declined because Cyril does not offer interactive elicitation in this issue. Do not invent an elicitation capability flag; current KAS can call unconditionally. Do not return JSON null or claim no callback is possible. v2 refuses unsupported KAS callbacks. This is a deliberate product decision for approval, not an implicit omission.
8. Initial pre-response MCP snapshots and policy events survive `SessionCreated`'s reset boundary and are applied only to the response's matching session. Late responses from a previous session do not mutate the current panel. Core owns wire parsing and domain semantics; UI renders typed data and sends no bridge commands.

## Decisions requiring approval

| Decision | Recommended selection | Alternatives / consequence |
|---|---|---|
| D1 — optional reads | Include both prompt and resource reads, with read-only previews and explicit argument entry. | Exclude either only with explicit scope approval. No exclusion or future-work promise is currently recorded. |
| D2 — elicitation | Explicit, visible default-decline for both form and URL modes; no invented advertisement. | Supporting acceptance instead requires a complete schema-driven form and URL-consent flow, typed responder lifecycle, queue/priority/cancellation and exact reply semantics. Do not advertise/accept a partial handler. |
| D3 — panel surface | Follow existing `/powers` push-backed panel and `/hooks` KAS-only command registration patterns. `/mcp` remains the agent command on v2. Fetch outputs are previews, never implicit model turns. | A transcript-only listing would not fulfill the ticket's MCP panel requirement; it is not proposed. |

No automatic browser launch or durable secret-storage decision is bundled into this scope. `tqr8` uses current first-party manual URL surfacing and independent storage default-off.

## Prior art / factual corrections

- `cyril-j16p` full record is closed and provides the KAS lifecycle prerequisite.
- `cyril-tqr8` full record owns URL capability/callback and MCP display errors; `cyril-caar` owns startup budget/timeout diagnostics.
- `cyril-1aq0` is an open broad slash-command parity umbrella; it is not an authorization to defer this issue's required panel/reset/governance behavior.
- `cyril-qo13` closed record establishes exact chosen-option fidelity for permission questions, not a completed generic schema/form elicitation implementation.
- Current KAS 0.66.43 source and caveats are recorded once in `.cyril-tqr8/evidence.md`. In particular, no proven elicitation flag exists; handler installation can be unconditional, and current initialize's extensionMethods list is incomplete for reset/read calls.
- `docs/kiro-mcp-remote-servers.md:134-152`, `docs/kiro-enterprise-mcp-governance.md:87-119`, and all issue notes establish status and policy surfaces. Runtime personal-account captures do not prove enterprise enforcement behavior.

## Out of scope

No new server configuration editor, production organization controls, registry bypass, automatic retries, telemetry system, external account provisioning, durable credential backend or speculative `oauthCompleted` support. Broader governance/focus/metering belongs to verified `cyril-0o7e`; v2 governance coverage is separately tracked by `cyril-bmek`. This issue must not claim either issue complete.

## Approval

**DG APPROVED — 2026-10-19, Main relaying the user's “Delegate routine sign-offs” selection.** Delegation: “I select and record evidence-backed designs. I still ask before scope changes, relaxed acceptance, architecture changes outside the existing boundaries, or explicit risk acceptance.” Main explicitly approved D1 both optional read-only prompt/resource previews, D2 explicit visible valid decline for unsolicited elicitation without an invented flag, D3 the existing push-panel pattern, and all specified governance/session safety. This is delegated approval, not a verbatim requester endorsement of each detail. Any design choice exceeding this spec returns to Main before dependent implementation.
