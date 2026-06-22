# Raw request — KAS-1 (cyril-evwh)

## Requester's words (this session)
> "lets checkout main, pull, and start the gilfoyle tasks on the work that needs
> to be done" → (on being asked which milestone) chose **"KAS-1 (cyril-evwh) —
> Engine selection + _kiro/auth/getAccessToken responder (the entry gate)"**,
> over "KAS-2a" and "Engine-select only, first."

## Rivets task (cyril-evwh) verbatim scope
- Make the KAS spawn real: the AgentEngine gate from KAS-0 resolves the KAS
  engine to a live process; default stays v2. New KAS code behind the `kas`
  cargo feature from KAS-0.
  - Engine flag is version-dependent: `kas` (2.7.1) → `v3` (>=2.8.0). Resolve
    per installed version; don't hardcode.
- Implement a `_kiro/auth/getAccessToken` server→client responder: reply
  `{accessToken, expiresAt, profileArn}`. KAS validates `expiresAt > now + ~3min`
  and REQUIRES `profileArn` (backend 400s without it).
- Token sourcing: MIRROR kiro-cli's own auth (social / Builder ID / external
  IdP), proactive OIDC refresh before the ~3-min buffer via a lock-guarded
  coordinator. Cleanest: DELEGATE to kiro-cli's own auth.
- cyril becomes CUSTODIAN of a kiro credential — no logging, read-only, minimal
  lifetime.
- AUTH half of the initialize._meta.kiro (KiroClientMeta) handshake; SETTINGS
  half is cyril-nhzw (separate).

## The free-path tension (latest note, 2026-06-21)
"FREE PATH CONFIRMED; de-risks the entry gate (auth responder is no longer a
hard prerequisite for a first live KAS turn)."

Source: `docs/kiro-2.8.1-wire-audit.md` §"KAS runtime behavior" + ROADMAP KAS-1
"Free-path de-risking." Two spawn shapes:
- **Wrapper** `kiro-cli acp --agent-engine <v3|kas>` → injects
  `--auth=acp-callback` → `_kiro/auth/getAccessToken` MANDATORY.
- **Direct spawn** `node --experimental-wasm-modules …/acp-server.js` (no
  `--auth`) → tier-5 FileAuthProvider reads `~/.aws/sso/cache/kiro-auth-token.json`,
  self-refreshes → responder fires 0× → a full turn completes with ZERO
  credential code (if user ran `kiro-cli login`). Trade-off: cyril owns
  server-entry discovery (`KIRO_KAS_SERVER_PATH` else walk
  `node_modules/@kiro/agent/dist/server/acp-server.js`), node runtime
  (`KIRO_AGENT_PATH`), and `--experimental-wasm-modules`.

ROADMAP recommendation: "ship the free path first; the responder is for the
blessed wrapper lifecycle + Builder-ID/external-IdP users."

→ THE load-bearing spec decision: does KAS-1 build the auth responder (wrapper),
ship the free path (direct spawn, no responder), or both?
