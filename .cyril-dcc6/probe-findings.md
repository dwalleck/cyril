# cyril-dcc6 — prove-it-prototype findings (2026-07-03/04)

All runs against kiro-cli 2.11.0, @kiro/agent 0.8.0, live backend, one IDC login
window (token expiry 03:35:26Z; every run finished ≥10 min before it).

## Q-A (discovery): which acp-server.js should cyril spawn?

**Probe A** (`probe-a-versioned-discovery.py`): the proposed Rust logic in
Python — glob `~/.local/share/kiro-cli/kas/<semver>-<sha64>/`, prefer the dir
whose version == `kiro-cli --version`, else newest, else legacy unversioned
path. Chose the `2.11.0-05e9…` entry via **exact version match** (with 2.10.0
also present).

**Oracle A** — kiro-cli's own resolution, a mechanism we don't control: force
`kiro-cli acp --agent-engine v3` to spawn its KAS and read the child's argv
from `/proc/<pid>/cmdline` (`oracle-kiro-cli-v3.py`). Spawned entry:
`kas/2.11.0-05e9…/node_modules/@kiro/agent/dist/server/acp-server.js`.

**Agreement: byte-identical**, both runs of the oracle.

## Q-B (auth): does `--auth=acp-callback` + a sqlite-backed responder yield an authenticated turn?

**Probe B** (`probe-b-acp-callback-auth.py`): direct-spawn the discovered
bundle with `--transport=stdio --auth=acp-callback`; answer
`_kiro/auth/getAccessToken` with `{accessToken, expiresAt}` from
`data.sqlite3 auth_kv['kirocli:odic:token']` + `profileArn` from
`state['api.codewhisperer.profile']`; run a no-tools prompt.
Result, twice (T-13min and T-11min before token expiry): callback fired
exactly **once**, `initialize`/`session/new` ok, turn `end_turn`, agent echoed
`KAS_AUTH_OK`.

**Oracle B** — two independent mechanisms:
1. The **backend itself** is ground truth for auth (server-side check none of
   our code influences): it accepted the relayed credential on every run.
2. The **product's own plumbing**: `kiro-cli acp --agent-engine v3` completing
   the same prompt end-to-end (`KAS_AUTH_OK`), with the auth flowing
   kiro-cli → (forwarded) → our client responder → same sqlite reply.

**Agreement:** both topologies (cyril-shaped direct spawn; kiro-cli wrapper)
authenticate from the same sqlite credential and complete the turn.

## The disagreement that mattered (oracle bug → contract discovery)

The first oracle run FAILED its turn: `TokenInvalidError: Host refresh
callback returned no access token`, while probe B succeeded in the same
minutes. Investigation (skill cause #3 — broken oracle): **kiro-cli in
wrapper mode FORWARDS `_kiro/auth/getAccessToken` to its outer ACP client**,
and the oracle script's catch-all `{}` reply to unknown requests WAS the
"no access token". Fixing the oracle to answer the forwarded callback from
sqlite made the turn pass.

Consequences:
- Live confirmation of the wrapper-mode contract cyril-evwh built against: a
  client that can't answer `getAccessToken` kills every wrapper-mode turn.
- This is the user-facing failure shape when cyril's **current** responder
  relays the dead SSO-file token (or nothing): instant errored turn, rendered
  by cyril as a silent `TurnCompleted` (see cyril-l7tw).
- A dead hypothesis, on the record: token expiry did NOT explain the failure
  (timeline pinned — all runs pre-expiry). Behavior at genuine expiry is
  still unverified → that's cyril-taba's question (responder policy on stale
  sqlite token: relay-as-is + fail loud vs pre-emptive refusal).

## What I learned (didn't know before probing)

kiro-cli wrapper mode forwards `_kiro/auth/getAccessToken` to its ACP client
— so cyril's sqlite-backed responder is the single fix for BOTH spawn modes
(direct `--auth=acp-callback` and wrapper), and the versioned-dir pick is
exact-version-match, both captured live from the product's own spawn.

## Design inputs for falsifiable-design

1. `discovery.rs`: extend `resolve()` — versioned-dir glob + exact-version
   preference + newest fallback + legacy fallback; add `--auth=acp-callback`
   to the free-path argv (today it spawns file-auth mode).
2. `auth.rs`: responder sources `accessToken`/`expiresAt` from the sqlite
   `auth_kv` row and `profileArn` from the `state` row — NOT the SSO file.
   Relay as-is; no refresh (never touch the CLI's refresh token).
3. Login precheck: sqlite row present = logged in; row absent = `NotLoggedIn`
   ("run kiro-cli login"). The SSO-file existence check is wrong in both
   directions and goes away.
4. Version-match policy needs a tiebreak decision when no dir matches the CLI
   version (newest? refuse? warn+newest) — design question, not a probe fact.
5. Same-seam bundles: cyril-0pms (reap children), cyril-taba (likely subsumed).
