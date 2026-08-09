# Related issues

- `cyril-j1b3` (closed, PR #90): established request-time approval snapshots. Its `approval_snapshot_is_independent` fence currently treats displacement and responder closure as acceptable independence; this fix must preserve snapshot stability while reversing that responder-drop assertion.
- `cyril-qo13` (closed): established exact `PermissionOptionId` replies and the two-phase trust-selection flow. A queued request may advance only after a terminal response; entering or leaving trust selection must keep the same queue head.
- `cyril-kbgo` (open): proposes centralized lifecycle for all modal types. This ticket changes only approval storage and does not pre-empt that broader modal-policy refactor.
- `cyril-jxfu` (open, blocked by `cyril-6beh`): will add workflow run/node session attribution. This ticket can label raw non-main `SessionId` values now; run/node labels remain deferred until that registry exists.
