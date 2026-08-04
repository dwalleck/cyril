# Related issues

Tracker scan on 2026-08-03, matching `permission`, `toolCallId`, `approval`, `stub`, and `preview`:

- `cyril-j1b3` — this issue; KAS request-side approval preview must join a stub permission `toolCall` to the tracked notification by `toolCallId`.
- `cyril-qo13` — related but distinct KAS user-input permission-response fidelity; its scope is response-side option selection, not request-side preview rendering.
- `cyril-p7kp` — release-audit watch item for unknown `PermissionOptionKind` values hard-failing ACP deserialization; adjacent protocol robustness, not preview joining.
- `cyril-sive` — follow-up to verify v2 `trustOption` response echo shape; response-side trust metadata, not request-side preview rendering.

No prior issue was found that duplicates the request-side `toolCallId` preview join.
