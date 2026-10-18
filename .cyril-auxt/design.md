# design.md — cyril-auxt (+ cyril-gn07): KAS "Always allow" via the consent echo

## Route and inputs

- Route: `route.md` (Structural; behavior source = T4 contract there).
- KAS wire facts: `route.md` T1 (2.32.0 live captures; KAS read sites at `acp-server.js` bytes 11332104 / 11483680).
- Reference client: Kiro TUI 2.32.0 bundle, provenance below. One carried premise (relative in-workspace folder glob honoured by KAS) → claim C9, discharged live.

## TUI provenance (kiro-tui-2.32.0.js, sha256 `eba39f7b…432cd0`)

| Byte offset | Symbol | Behaviour mirrored |
|---|---|---|
| 11046275 | `handlePermissionRequest` | request `_meta.kiro.consent` → `consentContext`; reply `{outcome:{…,_meta}, _meta}` |
| 11091808 | `Pb` | shell capabilities = `shell` \| `exec` \| `shell:exec` |
| 11091872 | `Ub`, `Knt="*"`, `dLn` | gated = `triggeringResource ?? resource`; exact = gated unless `"*"`; pattern = `"<first> *"` for a shell capability when the command has >1 token and the first token is not `*`, contains no `=`, and is not `sudo`/`doas`/`env` |
| 11402861 | `_lt` | non-main-session grant formability (owner id, capability, scope; `workspace` needs a root; a non-whole grant needs a shell capability and `triggeringResource` absent or equal to `resource`) |
| 11404644 | `cw` | echo builder `{kiro:{consent:{capability?, scope, resource?, workspaceRoot?}}}`; scope = picked scope (allow_always default `session`); resource = picked resource, or `"*"` for whole tool |
| 11406318 | `C8n` | non-main allow_always that cannot form a grant is downgraded to the request's allow_once option, with no grant metadata |
| 11407073 | `vlt.respondToApproval` | `d.resolve({outcome:"selected", optionId, _meta: cw(…)})` |
| 13062342 / 13062415 | `Ov`, `c3`, `s3`, `Ol` | scope order `session → workspace → global`, cycled with `s`; labels `session` / `workspace` / `user`; header `Scope:  [session]  workspace   user ` |
| 13617367–13619976 | `m2t`, `A2t`, `tsr`, `f2t`, `rsr`, `b2t`, `vde`, `h2t`, `y2t`, `Cde`, `g2t` | folder offer, per-scope availability, path normalisation, validation messages, `"<dir>/**"` |
| 13620805 | `E2t` | folder row label `Allow a folder <phrase>…`, description `<prefill> · <scope>`; confirm → `{kasScope, kasResource:"<dir>/**"}` |
| 13622384 | `isr` | row order: exact (`Trust "<gated>"`, `exact match · <scope>`), pattern (`Trust "<cmd> *"`, `pattern · <scope>`), folder rows, whole (`Trust entire tool (<tool>)`, `<scope>`) |
| 13622796 | `v2t` | KAS allow_always with no `trustOptions` opens the scope page; each row's response meta |
| 262951 / 263225 | `UM`, `dl` | label truncation (>50 scalars → first 47 + `…`); "unprintable" = C0 except `\t\n`, C1, `U+00AD U+200B U+200E U+200F U+2028–202F U+2060–206F U+FEFF U+FFF9–FFFB` |

## Behaviour

**Scope page.** Picking an `allow_always` option on a request that carries KAS metadata (`_meta.kiro`, typed as `PermissionConsent`) and no V2 `trustOptions` opens `Always Allow — Choose Scope` instead of answering — the TUI's `De` (KAS engine, no trust tiers). It opens at the scope the previous scope page last showed (`k2t` carries `trustScope`), initially `session`. It shows `capability → resource`, the scope line, and rows built from the consent:

| Row | When | Echoed resource |
|---|---|---|
| `Trust "<gated>"` — `exact match · <scope>` | gated resource present and not `*` | gated resource |
| `Trust "<cmd> *"` — `pattern · <scope>` | shell capability, pattern differs from gated | `<cmd> *` |
| `Allow a folder <for this session \| in this workspace \| for your user>…` — `<prefill> · <scope>` | main session, capability `fs_write`, not `askType:"explicit"`, resource present, not `*`, no glob characters, not absolute when a workspace root is present, normalises without `..`, parent ≠ `.`; scope `workspace` only in a workspace, `user` only outside one, `session` always | `<folder>/**` |
| `Trust entire tool (<toolId>)` — `<scope>` | always | `*` |

`s` cycles the scope; the highlighted row is kept when it still exists (a folder row's label changes with scope, so it resets to the first row, as in the TUI). Esc returns to the option list.

**Folder entry.** Enter on the folder row opens a one-line editor prefilled with the parent folder. The status line is the TUI's live validation: valid → `Allows writes in <path> and every folder under it, <for this session only | in this workspace | for your user>.`; invalid → the TUI's reason, coloured as an error once Enter was pressed on it. Enter on a valid folder answers; Esc returns to the scope page.

**Response.** cyril answers the picked `allow_always` option id and the converter adds `_meta.kiro.consent{capability, scope, resource, workspaceRoot}` at the response's top level (where KAS reads it; T1). `capability` and `workspaceRoot` are copied verbatim from the request's own consent, as the TUI does. A non-main-session pick that cannot form a TUI child grant answers the request's allow_once option instead, without metadata (`C8n`).

**V2 path on KAS.** The domain mediator forwards no V2 `trustOptions` on a KAS-engine request, so the agent-JSON `allowedCommands` write (`App::persist_trust_grant`) can only run for v2. v2 requests carry no `_meta.kiro` and keep their trust-tier flow unchanged.

## Claims / Falsification

| ID | Claim and covered cases | Falsifier / verification | Regression coverage | Status |
|---|---|---|---|---|
| C1 | Echo shape: an Always-allow response on KAS carries top-level `_meta.kiro.consent{capability, scope, resource, workspaceRoot}`; capability/root copied from the request; omitted when the request lacks them. Cases: shell exact/user, fs_write folder/workspace, whole tool/session (`*`). | `convert/kas.rs` tests on `from_permission_response` with request params taken from `kas-lead-shellecho-2.32.0.jsonl` line 39 → JSON equals the audit's reply line 40 for the shell/user case. Red first: the variant does not exist / echo absent. | new convert tests | PASS — `kas.rs` echo tests (red: `receipts/slice1-red.txt`; green: `receipts/gate.txt`) |
| C2 | No echo when not eligible: v2 engine; picked id not an offered `allow_always`; Cancel/Selected/RejectWithReason unchanged (rejectionReason still works). | convert tests asserting `_meta` absent / unchanged; existing `rejection_feedback.rs` and `convert/mod.rs` permission tests stay green. | existing + new | PASS — `consent_echo_needs_kas_and_an_offered_allow_always`, `consent_echo_omits_what_the_request_did_not_name`, existing rejection tests (`receipts/gate.txt`) |
| C3 | Row derivation mirrors `isr`/`Ub`/`b2t`/`A2t` per capability: shell multi-token (exact+pattern+whole), shell single token (no pattern), `sudo`/`env`/`*`/`FOO=1` first token (no pattern), resource `*` (whole only), fs_write in-workspace relative file (folder for session/workspace, not user), fs_write without a root, absolute (folder for session/user, not workspace), fs_write at workspace top (no folder), `askType:explicit` (no folder), non-main session (no folder), glob/`..` resource (no folder). | `rstest` table in `always_allow.rs` against expected rows transcribed from the TUI rules above. | new | PASS — `scope_rows_mirror_the_tui` (23 cases; `receipts/gate.txt`) |
| C4 | Folder validation mirrors `vde`: empty, unprintable (incl. U+200B), `..`, `~`, `#`, glob, workspace must be relative and not `.`, outside must be absolute, must contain the target; valid → `<dir>/**` (root `/` → `/**`), backslashes and trailing slashes normalised. | `rstest` table with the TUI's exact messages. | new | PASS — `folder_validation_{refuses,accepts}_like_the_tui` (`receipts/gate.txt`) |
| C5 | Non-main rule mirrors `_lt`/`C8n`: whole tool → keep allow_always+echo; shell exact → keep; fs_write exact → downgrade to allow_once (no echo); workspace scope without a root → downgrade; no allow_once offered → keep allow_always+echo; main session never downgrades. | `rstest` table on the resolve function. | new | PASS — `always_allow_resolves_like_the_tui` (8 cases); sub-agent attribution is the recorded divergence under Non-goals |
| C6 | UI flow: allow_always on a KAS-consent request opens the scope page (not an answer); `s` cycles; Up/Down bounded; Enter on a row answers with the matching grant; Esc returns to options; folder row → editor prefilled; invalid Enter keeps the editor with the reason; valid Enter answers; Esc returns to scope page; paste lands in the folder editor. V2 trust-tier flow and plain allow_always without consent unchanged. | `UiState` tests driving `show_approval` → keys → asserting the response received on the oneshot. Red first: today allow_always answers immediately with no grant. | new + existing approval tests | PASS — `cyril-ui/src/always_allow_tests.rs` + App `kas_always_allow_keys_grant_a_folder_and_write_no_agent_json` (red: `receipts/slice2-red.txt`) |
| C7 | Render: scope page shows `capability → resource`, `Scope:  [session]  workspace   user `, rows with descriptions; folder page shows editor + validation line; no panic on tiny areas. | `TestBackend` render tests; existing approval render tests green. | new | PASS — render tests in `always_allow_tests.rs`; live screens `receipts/live-after-s1-*.txt` |
| C8 | V2 write path off on KAS: a KAS-engine permission request reaches the UI with no `trust_options`; a v2-engine one keeps them. | mediator/bridge contract test with a request carrying `trustOptions` under each engine. | new | PASS — `current_runtime_contract::always_allow` (sensitivity: `receipts/slice2-red.txt`) |
| C9 | End to end on KAS (real cyril TUI, private `HOME`, `SHELL=fish`): main binary → "Always allow" writes no `permissions.yaml` and a fresh session asks again; branch binary → shell `Trust "<cmd>"` at user scope writes `$HOME/.kiro/settings/permissions.yaml` and a fresh session runs the command without asking; fs_write folder grant at workspace scope writes `$HOME/.kiro/workspace-roots/<hash>/permissions.yaml` and a fresh session writes into that folder without asking (also discharges the relative-glob premise). | hex1b-driven live runs; receipts in `receipts/`. | live receipt | PASS — `receipts/live-summary.md` |
| C10 | Repository gates: every ci.yml lane green. | gate script, `<lane>-OK` per lane. | — | PASS — `receipts/gate.txt` |

Bug sensitivity (auxt is a bug): C1 and C6 are observed red on the pre-fix revision (no echo; allow_always answers immediately) and green after; C9's before arm is the live pre-fix observation.

## Placement

- **Owner `cyril-core::always_allow` (new module).** Pure domain: `GrantScope`, `GrantResource`, `AlwaysAllowGrant`, `ScopeChoice`, `FolderOffer`/`AllowedFolder`/`FolderError`, row derivation, folder validation, and the main/child resolution to a `PermissionResponse`. Forbidden: ACP types, JSON, UI concepts.
- **`review::consent::PermissionConsent`** gains the verbatim consent capability, `triggeringResource`, and `askType == "explicit"`, set in `convert/kas.rs::permission_consent` (the only `_meta.kiro` parser).
- **`types::PermissionResponse::AlwaysAllow { option_id, grant }`** — new variant; the existing `Selected` / `RejectWithReason` / `Cancel` keep their wire output.
- **`protocol/convert/kas.rs::attach_consent_echo`** builds `_meta.kiro.consent` from the request args and the typed grant (KAS wire owner; `mod.rs` only dispatches, `#[cfg(feature = "kas")]` like `attach_rejection_metadata`).
- **`protocol/domain_mediator`**: on KAS, `trust_options` = empty.
- **`cyril-ui`**: `ApprovalState.consent`; `ApprovalPhase::{SelectScope, EnterFolder}`; `UiState` transitions; `widgets/approval.rs` gains two render functions (no change to the existing option/trust/reason renderers, so cyril-q0kg / cyril-rcub rebase cleanly). The folder editor reuses `RejectionFeedback` (the approval's existing text editor) rather than a second editor.
- **`cyril` App**: routes `s` and the folder editor's keys/paste; no domain logic.

## Non-goals and future work

- **Invocation echo on allow_once / reject_once / reject_always.** The TUI also echoes `scope:"invocation"` on those (`cw`); for reject_always that turns KAS's default session-scoped deny into a one-shot deny. Outside this issue (grants), behaviour-changing for "Always deny" → not mirrored; raised to the requester as an open question in the PR / hand-off (no tracker issue filed: requester decision pending).
- **TUI-local queue auto-approval** (`k8n`/`D4`: after a whole-tool or child grant the TUI resolves already-queued approvals itself). A client-side convenience, not persistence; cyril still prompts for requests queued before the grant. Raised in the hand-off.
- `persistableConsent:false` (cyril-k8wc) — prerequisite, separate PR #172; not touched.
- **Sub-agent requests count as main-session requests (known divergence).** The TUI re-attributes a KAS permission request to its sub-agent subtask (`prepareKasPermissionRequest`, byte 10608706: `toolCallToSubtask.get(toolCallId)`, learned from `tool_call` frames tagged `_meta.kiro.agentSubtaskId`), so a sub-agent's approval gets no folder row and an exact `fs_write` grant is downgraded (`C8n`). KAS 0.66.43 does not put `agentSubtaskId` on the permission request itself (its `_meta.kiro` is `toolId, command, agentManagesTrust, disclosedContext, workflowWatch, workflowPlan, consent, consentRound, mcpTool`; `acp-server.js` ~byte 11482266), and cyril tracks no subtasks — that is cyril-ebqu (KAS crew rendering). Until then cyril applies the main-session rows to sub-agent approvals; the `_lt`/`C8n` rule is implemented and covers requests on other session ids (workflow steps). Raised to the requester; independent review should-fix 1.
- Whole-tool label uses `_meta.kiro.toolId` (the TUI uses its tool_use message name). Label text only; the echoed resource (`*`) is identical.

## Approval

DG — delegated by the requester through the orchestrator's hand-off, 2026-10-18, quoting the user's decisions: "1. Fix = send the consent echo on the permission response; drop the agent-JSON write path ON KAS (v2 behaviour unchanged). 2. **Scope/resource choices MIRROR Kiro's TUI 2.32.0 exactly, per capability** (shell commands, fs_write incl. the custom-folder `<dir>/**` glob, etc.) … Record the TUI provenance (function names / offsets) in your route/design artifacts." and "this is your design approval".
