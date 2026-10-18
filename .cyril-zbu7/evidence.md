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
Settled by the live run (design C6, `receipts/live/timeline.txt`): the plain call renewed the token at 19:44:40Z with ~3 min left — exactly when KAS entered its 180 s buffer — so KAS never saw an in-buffer token.

## P4 — Which kiro-cli to run

Source: this repo. Free path: `discovery::installed_cli_version` probes `AgentCommand::new("kiro-cli")` on the inherited PATH (std's lookup resolves `kiro-cli.exe` on Windows). Wrapper path: `version::version_probe_command` derives the kiro-cli element from the agent command, through the WSL launcher when used (cyril-861q). The token source reuses exactly these two rules.

## P5 — How a kiro-cli WITHOUT the helper answers (amendment premise)

Receipt: `receipts/missing-subcommand-shapes.txt` (2026-10-18). No token touched: the 2.32.0 probe used a bogus sibling subcommand; the archived builds ran under `env -i` with private HOME/XDG dirs (no credentials reachable).

- kiro-cli 2.32.0, `chat _ get-kas-token-does-not-exist`: exit 2, empty stdout, stderr `error: unrecognized subcommand 'get-kas-token-does-not-exist'` + clap usage.
- kiro-cli-chat 2.5.1 (`chat _` exists, helper absent): exit 2, empty stdout, stderr `error: unrecognized subcommand 'get-kas-token'`.
- kiro-cli-chat 2.4.1 (no `chat _` at all): exit 2, empty stdout, stderr `error: unexpected argument 'get-kas-token' found`.
- kiro-cli-chat 2.6.0 / 2.6.1: the helper EXISTS with a legacy envelope `{"success":false,"error":…}` (exit 1) — not a missing helper; cyril treats it as a hard error (no `kind`). 2.7.1+: the current `{kind,data}` envelope. (KAS itself needs ≥ 2.7.1.)

Validated → "missing helper" = exit 2 ∧ empty stdout ∧ a stderr line exactly `error: unrecognized subcommand 'get-kas-token'` or `error: unexpected argument 'get-kas-token' found`.

Windows (relayed by the coordinator from VM resourcefs-win11, kiro-cli 2.27.0 MSI — `kiro-cli.exe` is the chat binary itself, `kiro-cli --version` → `kiro-cli-chat 2.27.0`): `kiro-cli chat _ get-kas-token-does-not-exist` → exit 2, stderr `error: unrecognized subcommand 'get-kas-token-does-not-exist'`; `kiro-cli chat _ get-kas-token` while logged OUT → exit 1, stdout `{"kind":"error","data":{"message":…}}`. Same detection shape as Linux; logged-out stays the error envelope (hard error, no fallback).

Evidence limit (review nit): the 2.4.1 / 2.5.1 refusal shapes were captured by running `kiro-cli-chat` directly, not through the `kiro-cli` launcher of those versions (2.32.0 was observed through the launcher). A launcher that reshaped exit code or stderr would fail closed (NonZeroExit, no fallback).

## P6 — The Kiro TUI's kiro-cli lookup (`ku`, tui.js 2.32.0) — PR #173 review repair

Source: `~/.local/share/kiro-research/tui-bundles/kiro-tui-2.32.0.js`, `ku(e=Man)` with `YM` (PATH search), `y1e` (executable check), `Man.homeDir`.

1. `KIRO_CHAT_CLI_BIN` (set by the Rust host that launched the TUI) — required; returned if executable (and its pinned identity matches `KIRO_CHAT_CLI_BIN_IDENTITY` when set).
2. `KIRO_CHAT_CLI_FALLBACK_BIN` if executable; if the identity pin was set and failed → error.
3. `~/.local/bin/<basename of KIRO_CHAT_CLI_BIN>`.
4. PATH search for `[basename]` or `[basename, "kiro-cli"]`: absolute entries only, entries at/under the cwd skipped, `.exe` appended on win32, unix requires X_OK.
5. `~/.local/bin/kiro-cli`, `/usr/local/bin/kiro-cli`, `/opt/homebrew/bin/kiro-cli` (literal; on win32 `y1e` is just "is a file").
6. Otherwise `KIRO_CHAT_CLI_BIN` as-is.

Home = `HOME || USERPROFILE || os.homedir()` (cyril's `kiro_agent_config::home_dir` = `HOME || USERPROFILE`).

cyril has no host-provided binary (steps 1–3 and 6 have no equivalent), so its free path takes step 4 (with `kiro-cli` as the name; relative entries skipped) and then step 5. PATH first is deliberate (the TUI's step 3 only puts `~/.local/bin` first because the Rust host handed it that binary): the gate, the responder and the free path's `kiro-cli --version` probe keep asking the same kiro-cli. Step 5 is not applied on Windows — its literal paths name the extension-less unix binary, and the Kiro MSI puts `kiro-cli.exe` on PATH. The cwd-subtree skip in step 4 is replicated lexically (the TUI also compares realpaths).

Review 3 correction (2026-10-18): the "same kiro-cli as the version probe" premise above was not held at 54f8762f — `discovery::installed_cli_version` still probed bare `kiro-cli` through the OS lookup (which also honours the relative/cwd PATH entries cyril skips). It now runs `invocation_version` on the token source's resolved kiro-cli (`KasTokenSource::kiro_cli`), so probe, gate and responder use one binary (red-first: `receipts/r3-red-shared-kiro-cli.log` — with kiro-cli only in `~/.local/bin` the probe failed and bundle selection took the newest 2.25.0 extraction instead of the matching 2.24.0). Executability now follows the TUI's `y1e` more closely: `access(X_OK)` for the current user on unix (nix `unistd::access`, the call Node's `fs.accessSync(X_OK)` makes); on Windows any file. The cwd-subtree skip is case-insensitive on Windows (the TUI lowercases).
