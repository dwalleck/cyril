# Related issue prior art

Search scope: `.rivets/issues.jsonl` keyword matches for interruption, tool-call, rawInput, and merge_update; full records read for the most relevant matches.

- `cyril-a5wo` — current issue; live 2.16.2 interruption capture and merge/commit verification.
- `cyril-w0vy` — open P1; a v2 security-filter interruption can emit a marker without a prompt response and wedge the bridge. Related interruption family, but a different wire symptom and scope.
- `cyril-a71q` — closed PR #69; turn-completion identity and stale completion handling. Relevant ordering precedent, not the same tool-call state.
- `cyril-9akh` — open P3; notification/response ordering can commit turn completion before streamed text. Related ordering hazard, not a tool-call recovery implementation.
- `cyril-j1b3` — existing KAS permission-to-tool-call merge behavior and absent `rawInput` cases; relevant field-preservation precedent.

No existing issue covers the exact combination of a live 2.16.2 subagent mid-arguments capture plus duplicate/stuck UI recovery behavior.
