# prove-it-prototype findings — KAS-1 (cyril-evwh)

Date: 2026-06-22. Machine: kiro-cli **2.8.1**, bundle @kiro/agent 0.3.257,
node v26.1.0, active identity **GitHub/social** (`provider: Github`,
`authMethod: social`).

## Probe (reused, not rebuilt)
`experiments/conductor-spike/probe-kas-direct-spawn-2.8.1.py` — spawns
`node --experimental-wasm-modules <acp-server.js> --transport=stdio` with **no
`--auth`**, runs `initialize` + `session/new` + one tiny turn ("Reply with
exactly: ok"), and counts inbound `_kiro/auth/getAccessToken` requests. Ran live
against the operator's real auth. Custodian-safe: logs counts/text, never the
token value.

## Oracle (independent mechanism)
The server's **own stderr** auth-provider declaration —
`logs/probe-kas-direct-spawn-2.8.1.log.stderr`. Computed a different way than the
probe's request-counting: the server prints which provider `selectAuthProvider`
chose, independent of whether the client counts callbacks.

## Agreement (the non-trivial slice)
| | Mechanism | Result |
|---|---|---|
| Probe | count inbound `_kiro/auth/getAccessToken` | **0** |
| Oracle | server stderr provider declaration | `[INFO] Auth: default token file` |

Both independently say: the free-path direct spawn authenticated from the **file
provider**, with **zero host callbacks** (`agent->client request methods: {}`).
Turn completed `stopReason: end_turn`. **AGREE.** prove-it-prototype gate met.

## What I learned (not obvious before the probe)
1. **The free path serves a GitHub-social user**, not just AWS-SSO logins — the
   social token lives in `~/.aws/sso/cache/kiro-auth-token.json` WITH a
   `profileArn`. → Resolves the spec's open B-table edge ("free path may only
   serve AWS-SSO-backed logins"): on this machine it serves social.
2. **KAS refreshes the token file in place.** `expiresAt` advanced
   `2026-06-21T20:36Z` (expired ~6h) → `2026-06-22T03:13Z` (fresh) across one
   run. The file is a self-maintaining shared store already holding exactly
   `{accessToken, expiresAt, profileArn}` (+ `refreshToken`, `provider`,
   `authMethod`). → The "delegate to kiro-cli auth" source IS this file; the
   free path needs zero token code because KAS keeps it fresh.
3. A plain turn already emits `session_info_update` **×10** + `agent_message_chunk`
   + `available_commands_update` ×2 + `config_option_update` — the KAS-2a
   converter/turn-end surface (out of scope here; evidence the pipeline flows).

## Decision #5 (delegate mechanism) — status after probe
- **Source resolved:** read `~/.aws/sso/cache/kiro-auth-token.json` →
  `{accessToken, expiresAt, profileArn}`. Right now it yields a **spec-B4-valid
  reply** (profileArn present; expiresAt `03:13Z` > now `02:14Z` + 3min).
- **Residual design risk (carry to falsifiable-design):** in the **wrapper**
  mode (`--auth=acp-callback`) KAS delegates refresh to the host and does NOT
  self-refresh the file. So a cyril file-read responder could return a STALE
  token if the file isn't being kept fresh by something else. Open design
  question: what cheap kiro-cli affordance refreshes the file on demand
  (`kiro-cli whoami`/`profile`/`user`? a lib call?) — bounded by needing a stale
  file to test (KAS just freshened it; ~1h to re-stale). NOT a blocker for Part
  A (free path), which is KAS-1's first shippable slice.

## kiro-cli auth surface (for Part B design)
Public subcommands: `login`, `logout`, `whoami`, `profile`, `user`. No direct
"emit token" subcommand observed; the file is the token interface.

## Implication for scope (input to falsifiable-design, NOT a re-decision)
The free path covers the common case (incl. social) with zero credential code
and self-refresh. Part B (wrapper + custodian responder) shrinks to "read the
file; refresh-on-stale via a kiro-cli affordance" and is primarily for any
identity whose login does NOT populate the file — to be confirmed per identity.
