# Feature: KAS-1 — live KAS engine spawn + auth responder (the entry gate)

## What this is
Today cyril's `engine_for(Kas)` returns "not available yet" — selecting KAS
yields a clean BridgeDisconnected and nothing spawns. KAS-1 makes the KAS engine
spawn a live `@kiro/agent` process so a KAS session can `initialize` and run a
turn. It lands in two ordered parts: **(A) the free path** — spawn the bundled
`acp-server.js` directly over stdio with no `--auth`, so KAS uses its own tier-5
file-auth and no credential code is needed; then **(B) the wrapper + auth
responder** — spawn `kiro-cli acp --agent-engine <v3|kas>` and answer the
`_kiro/auth/getAccessToken` server→client request with a valid
`{accessToken, expiresAt, profileArn}`, by delegating to kiro-cli's own auth.
All KAS code is behind the `kas` cargo feature from KAS-0; default stays v2.

## Users

- **KAS operator (cyril end-user on the KAS engine):** runs cyril with
  `--agent-engine kas` (or `[agent] engine = "kas"`). Wants a KAS turn to run.
  Authenticated via `kiro-cli login` under one of three identity types (social
  GitHub / AWS Builder-ID / external-IdP). Sees: a KAS session that initializes
  and completes a turn, or — if a precondition is missing — a specific actionable
  error, never a silent hang or a silent downgrade to v2.
- **cyril maintainer (this project):** needs KAS-1 to fill the KAS-0 seam without
  touching the v2 path, behind the `kas` feature, with the auth responder
  testable (shape + expiry) and the credential handled as a custodian.
- **NOT a user:** the v2 operator — KAS-1 must not change v2 behavior at all.

## Behavior

### B1 — Free-path spawn (direct stdio, no --auth)
- **Given**: engine = Kas, `kas` feature built, KAS bundle present at
  `$KIRO_KAS_SERVER_PATH` (else the walked default
  `~/.local/share/kiro-cli/kas/node_modules/@kiro/agent/dist/server/acp-server.js`),
  a node runtime present (`$KIRO_AGENT_PATH` else `node` on PATH), and the
  tier-5 token file populated by a prior `kiro-cli login`.
- **When**: the bridge spawns the agent process.
- **Then**: it execs `<node> --experimental-wasm-modules <acp-server.js>
  --transport=stdio` (no `--auth` flag) and the ACP `initialize` handshake
  succeeds; `_kiro/auth/getAccessToken` fires **0 times** for the whole session.

### B2 — Free-path turn completes
- **Given**: a free-path session initialized per B1.
- **When**: the operator sends one prompt.
- **Then**: the turn reaches `stopReason: end_turn` with no
  `[TokenInvalidError]`. (Rendering the stream is KAS-2a, out of scope here — B2
  asserts only that the turn completes server-side via the existing pipeline.)

### B3 — Wrapper spawn (version-correct flag)
- **Given**: engine = Kas, the wrapper path selected, `kiro-cli` installed.
- **When**: the bridge resolves the engine flag from the installed kiro-cli
  version and spawns.
- **Then**: it execs `kiro-cli acp --agent-engine v3` on kiro-cli ≥ 2.8.0, or
  `--agent-engine kas` on 2.7.x; on < 2.7.1 it does NOT spawn and emits an
  actionable error ("KAS requires kiro-cli ≥ 2.7.1").

### B4 — Auth responder replies a valid token
- **Given**: a wrapper session (B3) where KAS was launched with
  `--auth=acp-callback`.
- **When**: KAS sends `_kiro/auth/getAccessToken` (a server→client request).
- **Then**: cyril replies `{accessToken, expiresAt, profileArn}` where
  `profileArn` is non-empty, `expiresAt > now + 3min`, sourced by delegating to
  kiro-cli's own auth for the operator's active identity type (social /
  Builder-ID / external-IdP) — cyril does NOT reimplement OIDC refresh. KAS
  accepts it (no `profileArn is required` 400, no expiry rejection) and the turn
  proceeds.

### B5 — Expired/invalid token is refused before reply
- **Given**: the delegated source yields a token whose `expiresAt ≤ now + 3min`
  (or no `profileArn`).
- **When**: the responder would reply.
- **Then**: it does NOT send a known-bad reply; it triggers/awaits a refresh via
  the delegated auth (so the eventual reply satisfies B4), or — if no valid token
  can be obtained — emits an actionable error rather than a silent bad reply.

### B6 — Precondition failure (free path) fails fast
- **Given**: engine = Kas, free path, and a missing precondition (bundle absent /
  node absent / not logged in).
- **When**: the bridge attempts to spawn.
- **Then**: it emits a `BridgeDisconnected` naming the *specific* missing
  precondition and its fix; it does NOT spawn, does NOT auto-recover, and does
  NOT fall back to v2.

### B7 — v2 untouched
- **Given**: engine = V2 (default) OR a build without `--features kas`.
- **When**: anything.
- **Then**: behavior is byte-identical to post-KAS-0; no KAS spawn-discovery,
  node lookup, or auth code runs.

## Success criteria

- **SC1 (free-path live turn):** On the KAS engine via the free path, a gated
  end-to-end smoke runs `initialize` + `session/new` + 1 prompt and observes
  `stopReason == end_turn` with `_kiro/auth/getAccessToken` fired 0×, measured by
  a `#[ignore]`d live test/example against the real `acp-server.js`. Pass = 1/1.
- **SC2 (responder live, ≥2 auth types):** On the wrapper path, the responder is
  exercised live and the turn completes for the operator's **2 available
  identities — GitHub-social + AWS-IdP** (≥ 2 of 3), measured by the gated smoke
  run once per identity. The 3rd store (AWS Builder-ID, pending probe
  confirmation of the AWS-IdP mapping) is **unit-tested only** — accepted gap,
  recorded not hidden.
- **SC3 (responder unit shape + expiry):** A unit test asserts the reply is
  exactly `{accessToken, expiresAt, profileArn}` with `profileArn` present, AND
  that an `expiresAt ≤ now + 3min` input is refused per B5. Pass = both.
- **SC4 (no credential in logs):** The access-token string never appears in
  `cyril.log` or captured tracing output, measured by a test that drives a
  responder reply and `grep`s the captured logs for the token value. Pass = 0
  occurrences.
- **SC5 (v2 parity):** The full v2 test suite + `--features kas` lane both pass;
  a `cargo run` v2 smoke is unchanged. Measured by CI (`ci-success`) +
  the KAS-0 FakeAgent parity tests staying green.

## Edge cases and decisions

| Edge | Decision | Rationale |
|---|---|---|
| KAS bundle not extracted (no `acp-server.js`) | B6 fail-fast: "run `kiro-cli acp --agent-engine v3` once to extract, or set `KIRO_KAS_SERVER_PATH`" | Decision #3; no side-effecting auto-extract |
| `node` not found | B6 fail-fast: "node not found — install node or set `KIRO_AGENT_PATH`" | direct spawn needs a node runtime |
| Not logged in / tier-5 token file absent | B6 fail-fast: "run `kiro-cli login`" | free path relies on the SSO-cache file |
| Free path for a GitHub-**social** user (token in keyring, not the AWS file) | **RESOLVED (probe 2026-06-22):** free path SERVES social — the social token is in `~/.aws/sso/cache/kiro-auth-token.json` with `profileArn`; live turn completed, `getAccessToken` 0× | probe-findings.md §"What I learned" #1 |
| kiro-cli < 2.7.1 | B3: refuse with "KAS requires kiro-cli ≥ 2.7.1" | no KAS engine before 2.7.1 |
| kiro-cli 2.7.x vs ≥2.8.0 flag (`kas` vs `v3`) | resolve from `kiro-cli --version`; never hardcode | probe-verified flag rename |
| Token within the 3-min pre-expiry buffer at reply time | B5: refresh via delegated auth before replying | the exact failure earlier probes hit |
| Builder-ID / external-IdP token store | covered (Decision #2 = all three); profileArn always emitted | rivets original scope |
| Concurrent `getAccessToken` requests | delegated auth's own single-flight serializes; cyril does not add a second refresh path | don't reimplement the refresh coordinator |
| Credential in a panic/Debug print | token type must not derive/emit Debug of its secret; redacted | SC4 custodian |

## Out of scope

This change does NOT include:
- **Rendering** KAS turn output (converter arms, turn-end/busy-clear) — that is
  KAS-2a (cyril-j16p).
- **`AgentSettings`** / the `_meta.kiro.settings` half of the handshake —
  cyril-nhzw.
- **fs / terminal host callbacks** (`_kiro/fs/*`, `_kiro/terminal/*`) — KAS-5.
- **governance / safety / mcp / metering** surfaces — KAS-2b/2c/2d, KAS-8.
- Any **v2** behavior change.
- A KAS-2d agent-config migration notice.

## Constraints

| Dimension | Limit | How measured |
|---|---|---|
| v2 parity | byte-identical v2 path; no KAS code runs without `--features kas` + engine=Kas | CI v2 lane + cfg gating review |
| Credential in logs | 0 occurrences of the token value | SC4 grep test |
| Token lifetime in cyril | not stored on any long-lived struct; fetched per request, dropped after reply | code review / type design |
| Token store access | read-only; cyril never writes kiro's token stores | code review |
| Refresh logic | cyril reimplements 0 OIDC refresh; delegates to kiro-cli auth | design review |
| Default engine | v2 unless explicitly Kas | existing KAS-0 default test |

## Decisions log

| # | Question | Decision | Why |
|---|---|---|---|
| 1 | Which spawn shape does KAS-1 deliver? | Both: free path (direct spawn) FIRST as the demo slice, then wrapper + auth responder | Free path reaches a live turn with zero credential code (ROADMAP "ship free path first"); wrapper+responder covers the blessed lifecycle + non-SSO users |
| 2 | How many auth types must the responder cover? | All three (social + Builder-ID + external-IdP); profileArn always emitted | Operator wants completeness; reinforces "delegate to kiro-cli auth" (don't reimplement 3-store resolution) |
| 3 | Free-path precondition-failure behavior? | Fail fast with the specific missing precondition + fix; no auto-recover, no v2 fallback | CLAUDE.md "errors are not default values" / "distinguish missing from corrupt" / no silent failure; honors the explicit engine choice |
| 4 | Live-test coverage given a single machine? | Operator has an AWS account → ≥2 of 3 identity types live-testable (AWS file-cache enables the free path too); 3rd is unit-only, gap recorded | Shrinks the unit-only verification gap on Decision #2; AWS login populates the tier-5 file the free path reads |
| 5 | "Delegate to kiro-cli's own auth" — exact mechanism? | **SOURCE RESOLVED (probe 2026-06-22):** read `~/.aws/sso/cache/kiro-auth-token.json` → `{accessToken, expiresAt, profileArn}` (KAS/kiro-cli self-refresh it in place; verified expiresAt advanced expired→fresh across a run). Residual: wrapper-mode refresh-on-stale trigger (KAS stops self-refreshing when `--auth=acp-callback`) → carried to falsifiable-design | probe-findings.md §"Decision #5" |

## Sign-off

The requester typed, verbatim:
> "1. unit test is fair for 3rd auth type. 2. I accept that risk. I have github
> and Aws IDp login. This will build the mechanism to get auth and start a KAS
> session 'turn'"

Operator's identity types: **GitHub (social)** + **AWS IdP** (IAM Identity
Center / external-IdP — exact store confirmed in the probe). → 2 of 3 live-
testable; the remaining store (AWS Builder-ID, pending probe confirmation of the
AWS-IdP mapping) is unit-tested only, per accepted gap. AWS-IdP login populates
the `~/.aws/sso/cache` file the free path's tier-5 provider reads, so the **free
path is live-testable via the AWS-IdP login**; whether it also serves the
GitHub-social (keyring) token stays the open B-table probe question.

Date: 2026-06-22 — SIGNED OFF.
