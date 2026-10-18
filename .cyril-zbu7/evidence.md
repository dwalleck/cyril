# evidence.md — cyril-zbu7 premises

All sources read 2026-10-18 on this host. No access token appears in this file.

## P1 — The Kiro TUI's getAccessToken handler (the parity target)

Source: `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.32.0.js` (minified; symbol names as of 2.32.0).

- `Rsr(e, t)`: `n = ["chat","_","get-kas-token"]; if (e?.forceRefresh) n.push("--force-refresh")`; runs `Oc(n, t)`; throws on `!ok`, on `kind === "error"`, and on any `kind !== "getKasToken"`; returns `{accessToken, expiresAt, profileArn, ...(authMethod ? {authMethod}) , ...(provider ? {provider})}`.
- `Oc(argv, spawn, timeout = 30000)`: resolves the binary (`ku()` — `KIRO_CHAT_CLI_BIN`, then `~/.local/bin`, PATH `kiro-cli`), spawns with stdin ignored, races a 30 s timeout (`kiro-cli … did not complete within 30000ms`), parses stdout with `_1e`.
- `_1e(stdout)`: last non-empty line, `JSON.parse`, must be an object with string `kind` and object `data`; otherwise `No output` / `Unexpected output` / `Malformed`.
- `w1e`: the expected exit is 1 for `kind:"error"`, 0 otherwise; a mismatch only warns.

Validated → `--force-refresh` iff `forceRefresh` truthy; envelope = last line; relay the five fields.
Learned (deliberate divergence, design C2): the TUI accepts a success envelope with a failing exit (warns); cyril fails closed.

## P2 — KAS's side of the callback

Source: `~/.local/share/kiro-cli/kas/2.32.0-f3c5…/node_modules/@kiro/agent/dist/server/acp-server.js`, class `CCe` (`AcpCallbackAuthProvider`).

- `doRefresh`: `Lh(conn, "_kiro/auth/getAccessToken", {})` — params are always `{}` (no `forceRefresh` in this KAS); rejects a reply without `accessToken`, with an unparseable `expiresAt`, or with `expiresAt <= now + 180000 ms`.
- `ensureToken`: cached and outside the 180 s buffer → cached; inside the buffer but unexpired → background refresh (failure is a KAS-side warn, cached token kept); expired or none → awaited refresh (failure fails the call).
- `startRefresh` single-flights refreshes per KAS process.

Learned: cyril must relay, not judge freshness — an error reply in KAS's buffer window would surface a UI auth failure for what KAS treats as a benign background retry.

## P3 — kiro-cli's helper

- Live, 2026-10-18T19:01:34Z, `kiro-cli 2.32.0`, plain `kiro-cli chat _ get-kas-token` (redacted harness `zbu7-kastoken.py`): exit 0, one stdout line, `kind:"getKasToken"`, data keys `{accessToken, expiresAt, profileArn, provider}` (`provider:"Enterprise"`, no `authMethod`), expiresAt `2026-10-18T19:47:39.083490355Z` (46 min left).
- `strings ~/.local/bin/kiro-cli-chat` (2.32.0): `get-kas-token` — "Resolve and refresh-if-expired the active OIDC access token for KAS"; args `--force-refresh` ("Force a network token refresh even when the cached token looks valid"), informational `--reason` / `--current-expires-at`; `chat_cli_v2::auth::refresh_coordinator` with `refresh.lock` ("acquired refresh lock", "refresh lock timed out (peer wedged?)") and `kirocli:auth:last-forced-refresh-at`; error text "You are not logged in. Please log in with `kiro-cli login`."
- Earlier live note on the issue (2026-10-18 18:48): `--force-refresh` renewed immediately (+60.0 min).

Validated → renewal stays inside kiro-cli under a cross-process lock; cyril never needs the refresh token.
Open (non-design-changing): kiro-cli's own pre-expiry refresh window for the plain call — observed by the live run (design C6).

## P4 — Which kiro-cli to run

Source: this repo. Free path: `discovery::installed_cli_version` probes `AgentCommand::new("kiro-cli")` on the inherited PATH (std's lookup resolves `kiro-cli.exe` on Windows). Wrapper path: `version::version_probe_command` derives the kiro-cli element from the agent command, through the WSL launcher when used (cyril-861q). The token source reuses exactly these two rules.

## P5 — How a kiro-cli WITHOUT the helper answers (amendment premise)

Receipt: `receipts/missing-subcommand-shapes.txt` (2026-10-18). No token touched: the 2.32.0 probe used a bogus sibling subcommand; the archived builds ran under `env -i` with private HOME/XDG dirs (no credentials reachable).

- kiro-cli 2.32.0, `chat _ get-kas-token-does-not-exist`: exit 2, empty stdout, stderr `error: unrecognized subcommand 'get-kas-token-does-not-exist'` + clap usage.
- kiro-cli-chat 2.5.1 (`chat _` exists, helper absent): exit 2, empty stdout, stderr `error: unrecognized subcommand 'get-kas-token'`.
- kiro-cli-chat 2.4.1 (no `chat _` at all): exit 2, empty stdout, stderr `error: unexpected argument 'get-kas-token' found`.
- kiro-cli-chat 2.6.0 / 2.6.1: the helper EXISTS with a legacy envelope `{"success":false,"error":…}` (exit 1) — not a missing helper; cyril treats it as a hard error (no `kind`). 2.7.1+: the current `{kind,data}` envelope. (KAS itself needs ≥ 2.7.1.)

Validated → "missing helper" = exit 2 ∧ empty stdout ∧ a stderr line exactly `error: unrecognized subcommand 'get-kas-token'` or `error: unexpected argument 'get-kas-token' found`.
