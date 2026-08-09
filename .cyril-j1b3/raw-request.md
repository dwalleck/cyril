# Raw request

From Rivets issue `cyril-j1b3`:

> KAS permission requests carry a stub toolCall (no rawInput) — approval preview must join the tracked tool_call by toolCallId

From the 2026-07-02 KAS modes probe (`docs/kiro-kas-modes-2.10.0.md`, wire delta #3, dumps in `experiments/conductor-spike/kas-modes-dumps/`): on KAS 0.3.299 a `session/request_permission`'s `toolCall` is a STUB — `{status, title, toolCallId}` only, no `rawInput`, `kind`, or `locations`. The full payload (for Write File: `rawInput {path, text}` with complete file content, kind `edit`) is only on the earlier `tool_call` notification with the same `toolCallId`.

Action: verify whether Cyril's approval overlay renders preview content from the request payload or from the tracked tool call in `UiState`'s tool-call index. If it reads the request payload, any content/diff preview is empty under KAS — wire the join by `toolCallId` (the `tool_call` notification always precedes the permission request in the captures). If the overlay already joins via the tracked index, close this with a note plus a regression test against the modes-probe dump (`bug-fix.json` has four Write File permission/tool_call pairs to replay).

This is not the same issue as `cyril-qo13` (response-side option fidelity); this is request-side preview rendering.

Acceptance criteria:

- Under KAS, the approval overlay for an fs write shows the file path and content (or diff) sourced via `toolCallId` join.
- A convert/UI test replays a permission-request plus tool-call pair from `kas-modes-dumps/bug-fix.json` and asserts the preview content is present.
