# route.md — cyril-zbu7: KAS access token from `kiro-cli chat _ get-kas-token`

- Change: rivets `cyril-zbu7` (P1, KAS default-engine flip blocker; blocks cyril-4c4d). Closes cyril-taba as a duplicate (issue design notes, 2026-10-18).
- Date: 2026-10-18
- Branch: `fix/cyril-zbu7-get-kas-token` (linked worktree)
- Behavior source: `rivets show cyril-zbu7` — Design Notes ("DECIDED 2026-10-18") and Acceptance Criteria 1–6; notes dated 2026-10-15 and 2026-10-18.

## Route tests

### T1 Empirical premise — NO (answered by existing evidence before design)

Every external fact the design leans on was settled from primary sources, recorded in `evidence.md`:

- **The TUI contract** (tui.js 2.32.0, `Rsr`/`Oc`/`_1e`): argv `chat _ get-kas-token` + `--force-refresh` iff `forceRefresh`; 30 s bound; last non-empty stdout line is the envelope; `{kind:"getKasToken",data:{accessToken,expiresAt,profileArn,authMethod?,provider?}}` / `{kind:"error",data:{message}}`; relay exactly those five fields.
- **KAS's request shape** (KAS bundle in kiro-cli 2.32.0, `AcpCallbackAuthProvider`): params are always `{}`; KAS single-flights its own refresh, calls when its cached token is inside a 180 s buffer, and rejects a returned token inside that buffer.
- **kiro-cli's helper is real and single-flight**: live plain call today returned a well-formed envelope; `kiro-cli-chat` strings name `get-kas-token` "Resolve and refresh-if-expired the active OIDC access token for KAS" and a `refresh_coordinator` with a cross-process `refresh.lock`.

One residual fact is NOT design-changing (the approved design fixes `--force-refresh` iff requested): the exact pre-expiry window in which kiro-cli's plain call refreshes. It is observed by the live acceptance run, not assumed (design C6).

### T2 Structural module shape — YES

- `protocol/kas/auth.rs` changes responsibility: from reading kiro-cli's sqlite credential store to invoking a kiro-cli subprocess and relaying its envelope (new owner of a process-invocation + envelope contract).
- Internal interfaces change: `HostCallback::GetAccessToken` gains `force_refresh`; `DispatchCtx` and `DomainConfig` gain the bound token source; `resolve_kas_command` takes it; `KasMissing` loses the store variants and gains `AuthUnavailable`.
- `protocol/kas/version.rs` shares its kiro-cli invocation prefix and bounded subprocess runner with the auth responder (seam extraction, no new dependency).

No crate/dependency-direction change, no public (`pub`) API change.

### T3 Production-scale risk — NO

One short-lived subprocess per `getAccessToken` request; KAS calls it about once per hour (plus a few times inside its 3-minute pre-expiry window). Bounded at 30 s and serialized per connection.

### T4 Explicit behavior — YES

Acceptance criteria 1–6 of cyril-zbu7 are the complete outcome contract:

1. KAS (Free and Wrapper): responder and spawn-time gate get the token from `kiro-cli chat _ get-kas-token` (+ `--force-refresh` iff requested), never from data.sqlite3 / a token file — **amended 2026-10-18 (requester, relayed by the coordinator):** "ONLY when the subcommand itself is missing/unrecognised … fall back to today's stored-token read AND surface one clear, user-visible warning (not per-request spam) that this kiro-cli lacks get-kas-token, naming the kiro-cli version if cheaply available … Every OTHER failure … stays a hard, typed, actionable error — no fallback. Cache the 'subcommand missing' result per process". The missing-subcommand shape was observed before design (`receipts/missing-subcommand-shapes.txt`, evidence P5). The issue's acceptance text is updated by the coordinator on this branch.
2. Error envelope, non-zero exit, timeout, spawn failure, malformed JSON → typed, actionable errors that reach the UI (no stale token, no defaults).
3. accessToken, expiresAt, profileArn, authMethod, provider pass through unchanged.
4. LIVE: a cyril KAS session keeps answering turns across an access-token renewal, no BridgeDisconnected.
5. v2 behavior unchanged.
6. Tests: envelope parsing (success/error/malformed), forceRefresh propagation, timeout/spawn failure; every ci.yml lane green.

Scope boundaries settled by the requester's brief: keep both spawn modes (no relay), never do OIDC, resolve kiro-cli consistently with existing rules (Free: PATH `kiro-cli` like the Free version probe; Wrapper: the agent command's kiro-cli, through the WSL launcher when used).

## Selected route: **Structural**

T2 yes, T4 explicit, no unresolved design-changing premise → `falsifiable-design` → `budgeted-plan` → `checkpointed-build`.

## Required artifacts

| Artifact | Owner | Note |
|---|---|---|
| `route.md` | change-workflow | this file |
| `evidence.md` | prove-it-prototype | premise sources (T1); no probe file needed |
| `design.md` | falsifiable-design | claims C1–C7, approval DG |
| `plan.md` | budgeted-plan | one atomic slice |
| `receipts/` | checkpointed-build | red/green runs, gate summary, live evidence |

`spec.md` is N/A — T4 explicit (issue acceptance + requester brief).

## Terminal criterion

Structural: design claims each have a PASS with evidence; all ten ci.yml lanes green locally (plus Windows-VM proof for the platform-neutral process tests); live acceptance (C6) recorded; independent review done; PR open, not merged.
