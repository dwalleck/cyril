# route.md — cyril-auxt (+ cyril-gn07): KAS "Always allow" persists through the consent echo

- Change: rivets `cyril-auxt` (P1 bug, KAS-default flip blocker for cyril-4c4d), folding in `cyril-gn07` (P3 task: the consent-echo mechanism).
- Date: 2026-10-18
- Branch: `fix/cyril-auxt-consent-echo`
- Request source: the orchestrator's hand-off of 2026-10-18 quoting the user's decisions, plus the 2026-10-18 notes on both issues (`rivets show cyril-auxt`, `rivets show cyril-gn07`) and `docs/kiro-2.32.0-wire-audit.md` §2a/§6.

## Route tests

### T1 Empirical premise — NO (one premise carried to the live acceptance)

Every fact the design depends on already has evidence:

- **KAS persists a grant only from the response echo.** LIVE in the 2.32.0 audit: `_meta.kiro.consent{capability, scope, resource, workspaceRoot}` on the `session/request_permission` RESPONSE; scope `user` → `$HOME/.kiro/settings/permissions.yaml`, `workspace` → `$HOME/.kiro/workspace-roots/<hash>/permissions.yaml`; a fresh `session/new` runs without asking (`experiments/conductor-spike/audit-2.32.0/live/kas-lead-shellecho-2.32.0.jsonl` lines 39–40 and 185–186; folder arms in `kas-lead-folder-2.32.0.jsonl`; no-echo control `kas-lead-folder-control-2.32.0.jsonl`).
- **KAS reads the echo from the response's top-level `_meta`.** KAS 0.66.43 `acp-server.js` byte 11483680: `S8e(C.outcome.optionId, v, C._meta, …)` where `C` is the response; byte 11332104: `w=r?.kiro?.consent, _=w?.resource??n?.triggeringResource??n?.resource; d={effect, scope:w?.scope||"session", resource:_, capability:<KAS-side>, workspaceRoot:w?.workspaceRoot??n?.workspaceRoot}`. The echoed `capability` is ignored (KAS uses its own request's).
- **What the reference client offers and echoes** — read from the 2.32.0 TUI bundle (`~/.local/share/kiro-research/tui-bundles/kiro-tui-2.32.0.js`, sha256 sidecar alongside); provenance table in `design.md` §TUI provenance.

Carried premise (does not change design, discharged by the live acceptance): the TUI's in-workspace folder grant echoes a RELATIVE glob (`src/**`, because its folder validator requires a workspace-relative folder), whereas the audit's live folder arm echoed an ABSOLUTE `<ws>/src/**`. Whether KAS honours the relative form in a fresh session is checked LIVE in cyril's acceptance run; mirroring the TUI is the approved decision either way, and a failure there is reported, not patched around.

### T2 Structural module shape — YES

- New public interface in `cyril-core`: a `PermissionResponse` variant carrying a typed Always-allow grant, a new domain module owning the TUI-mirrored choice/validation rules, and new fields on the parsed KAS consent (`PermissionConsent`).
- `protocol/convert/kas.rs` gains the echo builder (KAS wire owner); the generic converter dispatches to it.
- `cyril-ui`: two new approval phases (scope page, folder entry) on `ApprovalState`/`UiState`, a new `ApprovalState` field, and their rendering in `widgets/approval.rs`.
- `cyril` App: key routing for the new phases (thin), and the domain mediator stops forwarding V2 `trustOptions` on KAS (drops the agent-JSON write path on KAS).

No crate or dependency-direction change. New responsibility cluster across existing owners → Structural.

### T3 Production-scale risk — NO

One permission response at a time; string checks over a single path. No latency/volume/concurrency risk.

### T4 Explicit behavior — YES

Outcome contract (approved by the user, 2026-10-18, relayed verbatim by the orchestrator — quoted once in `design.md` §Approval):

1. On KAS, an "Always allow" pick sends `_meta.kiro.consent{capability, scope, resource, workspaceRoot}` on the permission response so KAS persists the grant (`user`/`workspace`) or widens it for the session (`session`); a fresh session then runs the granted command/write without asking.
2. The scope and resource choices mirror Kiro's TUI 2.32.0 exactly, per capability: scope cycles session → workspace → user; resource rows are exact (`Trust "<resource>"`), shell base-command pattern (`Trust "<cmd> *"`), the fs_write custom-folder row (`<dir>/**`, with the TUI's folder validation and per-scope availability), and whole tool (`*`); non-main-session picks follow the TUI's child-session rule.
3. The V2 agent-JSON write path (`toolsSettings.allowedCommands`) no longer runs on KAS; v2 behaviour is unchanged.
4. Without the echo cyril's behaviour today is "grant dies with the session" (cyril-auxt 2026-10-18 note, S4b).

## Selected route: **Structural**

T2 yes, T4 explicit, no unresolved premise that could change the design → `falsifiable-design`, then `budgeted-plan`, then `checkpointed-build`.

## Required artifacts

| Artifact | Owning stage |
|---|---|
| `route.md` (this file) | change-workflow |
| `design.md` | falsifiable-design |
| `plan.md` | budgeted-plan |
| `receipts/` (red-first, gate, live before/after) | checkpointed-build |

No `spec.md`: behavior explicit (T4). No `evidence.md`/probe: T1 premises answered by committed captures and the bundles; the one carried premise is part of the live acceptance.

## Terminal criterion

Structural: design claims all PASS with receipts; every ci.yml lane green locally; live before/after on the real cyril TUI over KAS shows the permissions.yaml writes under a private `HOME` and a fresh session running without a prompt; independent review done; PR opened against `main` (not merged).

## Completion record

Delivered: consent echo + TUI-mirrored scope page/folder editor; V2 trust tiers off on KAS (commits 17fb84de, 50dcc3b3, 0a302b9d). Checks: design C1–C10 PASS (`design.md`), gate all lanes OK at 0a302b9d (`receipts/gate.txt`), live before/after (`receipts/live-summary.md`), independent review with no blocking findings (`independent-review.md`). Open: sub-agent attribution divergence (needs cyril-ebqu) and the non-mirrored invocation echo on allow_once/reject_* — both raised to the requester. PR opened, not merged.
