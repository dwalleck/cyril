# design.md — cyril-zbu7

## Route and inputs

- Route: Structural (`route.md`); behavior = cyril-zbu7 acceptance 1–6 (route.md T4); premises = `evidence.md` P1–P4.
- No unresolved design-changing premise. kiro-cli's pre-expiry refresh window (P3 "Open") is observed by C6, not designed around: the approved design fixes `--force-refresh` iff the request asks.

## Design

On every `_kiro/auth/getAccessToken` (both spawn modes) and in the Free-path spawn gate, cyril runs `<kiro-cli> chat _ get-kas-token [--force-refresh]` with a 30 s bound, takes the last non-empty stdout line as the envelope, and relays `{accessToken, expiresAt, profileArn, authMethod?, provider?}` to KAS unchanged. cyril reads no credential store and does no freshness judgment: kiro-cli refreshes an expired token under its own cross-process lock, and KAS validates the expiry it receives (P2).

### Amendment (2026-10-18): stored-token fallback for a kiro-cli without the helper

Requester decision relayed by the coordinator (quoted in `route.md` T4 item 1). When — and only when — the helper is refused as an unknown subcommand (evidence P5: exit 2, empty stdout, stderr line `error: unrecognized subcommand 'get-kas-token'` or `error: unexpected argument 'get-kas-token' found`), `KasTokenSource::fetch` reads the stored token through `kas/stored_token.rs` (the pre-zbu7 sqlite reader + its 180 s stale refusal, moved verbatim), records the missing helper per process (keyed by the kiro-cli invocation; the helper is not re-probed), reads `<kiro-cli> --version` once for the message, and the responder surfaces one `SystemNotify` warning per connection. A stale stored token on the fallback is `StoredStale` (-32000, "run `kiro-cli login`, or update kiro-cli"); an unservable store is `StoredUnservable`. Every other helper failure returns before the fallback is considered.

### Removed invariants and why it is still safe

| Removed | What it enforced | Disposition |
|---|---|---|
| sqlite store reader + row tiebreak as the PRIMARY source; `KasMissing::{NoHomeForStore, StoreUnservable}` | reading the right token row | Kiro-cli picks the active credential (P3). The reader survives unchanged (tests included) in `kas/stored_token.rs`, reachable only through the missing-helper fallback (C10); fenced by C1. |
| cyril's 180 s `is_stale` refusal on the helper path (responder + gate) | never forward a token KAS will reject | Kept on the fallback path. On the helper path KAS performs the same check itself (P2). A cyril refusal inside KAS's background-refresh window would raise a UI auth error on every model call for what KAS treats as a benign retry. kiro-cli refreshes once expired; C6 proves the hour boundary live. |
| `auth_failure_notification`'s blanket "run `kiro-cli login`" append | an actionable hint on every auth failure | Each `KasTokenError` variant now carries its own remediation, so a missing/wedged kiro-cli is not told to log in (C2). |
| its `MethodNotFound` exemption (dn91 C13) | a refusal is not an auth failure | Structural now: refusals stay parse-time, and the responder's typed error cannot express one. |

## Claims / Falsification

| ID | Claim and covered cases | Falsifier / verification | Regression coverage | Status |
|---|---|---|---|---|
| C1 | Responder and Free gate get the token from `kiro-cli chat _ get-kas-token`; the credential store is read only by the missing-helper fallback module; the SSO token file never. | `discovery::tests::free_gate_asks_kiro_cli_get_kas_token_not_the_store` (child env: fake kiro-cli on PATH, no data.sqlite3): RED on base (StoreUnservable), GREEN after, journal ends `chat _ get-kas-token`. Source fences `credential_store_token_rows_only_in_the_fallback` (needles appear in `kas/stored_token.rs` only), `sso_token_path_never_resurrected`. | same tests | PASS (Evidence) |
| C2 | Error envelope, non-zero exit (with or without an envelope), timeout, spawn failure, malformed output (not JSON / empty / wrong kind / wrong field types / missing or empty required fields), unbound source → distinct typed `KasTokenError`, each with its own remediation, never a token or default; failures reach the UI as `BridgeError("auth")` and KAS as JSON-RPC errors (-32000 for kiro-cli's error envelope, -32603 otherwise). Gate failures fail the spawn with the same diagnostic. | `auth::tests::failure_shapes_map_to_typed_errors` (15 rstest cases), `error_envelope_hint_appears_exactly_once`, `acp_codes_separate_auth_state_from_helper_failure`, `fetch_times_out_on_a_wedged_helper`, `fetch_reports_spawn_failure_and_unbound`; `callbacks::tests::auth_failure_notification_carries_the_typed_remediation`; `discovery::tests::env_wrappers_resolve_xdg_data_home_like_kiro_cli` (gate → `AuthUnavailable` naming the helper). | same | PASS (Evidence) |
| C3 | The relayed reply is exactly the envelope's five fields, values unchanged; optional fields only when present (null/empty = absent); unknown data keys are not relayed (debug-logged). Last non-empty line wins (noise and CRLF tolerated). | `success_envelope_relays_the_data_fields` (4 cases), `fetch_runs_the_helper_with_force_refresh_and_relays_data` (process seam), live C6 (KAS authenticates with the relayed token). | same | PASS (Evidence) |
| C4 | `--force-refresh` is appended iff the request params carry `forceRefresh: true` (absent / null / false / non-boolean → not forced, non-boolean warns); placed after the invocation prefix (plain or WSL launcher). | `force_refresh_only_on_explicit_true` (6 cases), `invocation_appends_force_refresh_iff_requested` (3 cases), process-seam argv in `fetch_runs_the_helper_with_force_refresh_and_relays_data`, gate journal (no flag) in C1's test. | same | PASS (Evidence) |
| C5 | v2 unchanged: V2 binds no token source; non-`kas` builds compile none of this; all existing v2 tests pass. | Every ci.yml lane, including `--no-default-features` and default-feature lanes. | existing suites | PASS (Evidence) |
| C6 | LIVE: a real cyril KAS (Free) session keeps answering turns across the access token's expiry, with no BridgeDisconnected; renewal is visible as a new `expiresAt` in cyril's `getAccessToken served` log. | hex1b-driven TUI session opened before expiry, turns before / inside KAS's 3-minute buffer / after expiry; cyril.log + screenshots in `receipts/live/`. | `tests/kas_freepath_smoke.rs` (ignored live smoke) unchanged | PASS (Evidence) |
| C7 | Spawn-mode resolution: Free asks the PATH `kiro-cli` (std lookup → `kiro-cli.exe` on Windows) — the same instance the gate asked; Wrapper asks the agent command's kiro-cli via the cyril-861q prefix (through the WSL launcher when used) under the spawn's environment. Version-probe behavior is unchanged by the prefix extraction. | `invocation_appends_force_refresh_iff_requested::wsl_forced`; all `version::tests` (probe argv + messages unchanged); Windows VM run of the platform-neutral process tests. Wrapper live auth is not exercisable on kiro-cli ≥ 2.29 (default `--auth-method cli` sends zero callbacks — cyril-ny8z). | same | PASS (Evidence) |
| C8 | Custodian: the access token never appears in `Debug`, logs, or any diagnostic (malformed-output errors describe shape only, never the line). | `kas_token_debug_is_redacted`; `failure_shapes_map_to_typed_errors` asserts the secret is absent from Display and Debug for every case. | same | PASS (Evidence) |
| C9 | cyril never runs two token helpers at once on one connection (OIDC refresh tokens are single-use; KAS single-flights per process but the guarantee is cyril's to keep). | `fetch_serializes_concurrent_requests`: two concurrent fetches through one source against a fake helper that fails if another instance holds its lock dir. | same | PASS (Evidence) |
| C10 | (amendment) ONLY clap's refusal of the helper itself triggers the stored-token fallback; it serves the stored token, warns once per connection naming the kiro-cli version, probes the helper once per process; an expired stored token on the fallback is a hard -32000 error; every other failure (error envelope, other non-zero exit incl. other clap errors / exit 1 / any stdout, timeout, spawn failure, malformed) never falls back. | `missing_helper_is_detected_precisely` (10 rstest cases), `missing_helper_falls_back_to_the_stored_token_and_warns_once` (process seam: served, warning names 2.5.1, second request silent, one helper call), `missing_helper_with_expired_store_is_a_hard_error`, `other_helper_failures_never_fall_back` (servable store present, logged-out envelope → ErrorEnvelope); `stored_token::tests::*` (the moved reader's fences) incl. `read_stored_token_serves_fresh_and_refuses_stale`. Named mutations: detection widened to any failing exit → 5 rstest cases red; detection disabled → 5 tests red; restored → green (`receipts/mutation-c10*.log`). | same | PASS (Evidence) |

| W | (review should-fix) Wiring: the wire `forceRefresh` parse, the `DomainConfig → DispatchCtx` hand-off, the per-mode source in `resolve_spawn_command`, and the fallback warning's path to the UI. | `domain_mediator::host::tests::get_access_token_round_trips_through_the_bound_kiro_cli` (wire `{forceRefresh:true}` then `{}` → helper argv with/without `--force-refresh`, relayed body), `fallback_warning_reaches_the_ui_once` (one `SystemNotify{Warning}` for two requests), `domain_mediator::tests::serial::kas_token_source_moves_into_the_dispatch_context`, `bridge::tests::wrapper_spawn_binds_the_agent_commands_kiro_cli_as_token_source` (unix; V2 → none). Mutations: parse forced to `false` + warning dropped → both host tests red (`receipts/mutation-wiring-red.log`), restored → green. | same | PASS (Evidence) |
| C7b | (PR #173 review, item 1) The free path finds kiro-cli like the Kiro TUI's `ku` (evidence P6): first executable `kiro-cli` in PATH order (absolute entries only, entries at/under the cwd skipped), then — PATH lacking it — `~/.local/bin`, `/usr/local/bin`, `/opt/homebrew/bin` (none on Windows), else the bare name; the spawn-failure remedy names where it looked (free) or the `--agent-command` kiro-cli (wrapper). Host fully faked in tests. | `free_kiro_cli_prefers_path_in_order_and_skips_relative_entries`, `free_kiro_cli_falls_back_to_the_tui_locations` (4 rstest cases; Windows expects the bare name), `free_kiro_cli_skips_path_entries_inside_the_cwd`, `free_kiro_cli_without_path_or_home_is_the_bare_name`; `fetch_reports_spawn_failure_and_unbound` (wrapper remedy), `callbacks::…typed_remediation`, `env_wrappers_resolve_xdg_data_home_like_kiro_cli` (free remedy). Red-first: the fallback cases ran red against a PATH-only stage of the resolver (`receipts/r1-red-kiro-cli-lookup.log`). | same | PASS (Evidence) |
| C9b | (PR #173 review, item 2) Serialization is process-wide per kiro-cli invocation (one async lock per rendered prefix), so two connections in one process — e.g. the main bridge and the workbench reviewer runtime — never run the helper for one kiro-cli concurrently; per-connection serialization is the special case. | `fetch_serializes_across_sources_sharing_an_invocation` — red on the per-connection-only code (`receipts/r2-red-process-wide-serialization.log`: second helper exit 9, lock dir held), green after; `fetch_serializes_concurrent_requests` unchanged. | same | PASS (Evidence) |

## Evidence

- Red-first (C1): `receipts/red-run.log` (base 85becf9f + test only: `StoreUnservable … data.sqlite3 is absent`) → `receipts/green-run.log`.
- Mutations: C9 `receipts/mutation-c9-{red,restored}.log`; C10 `receipts/mutation-c10{,b}-red.log`, `mutation-c10-restored.log`; W `receipts/mutation-wiring-red.log`.
- Gate (all ten ci.yml lanes, Linux): `receipts/gate-summary.txt` (25880f5a source; every lane `-OK`).
- Windows VM (C2/C3/C4/C9/C10 process seam with the `.cmd` fixture): `receipts/windows-vm.txt` — 247 kas/domain_mediator tests pass on MSVC at 60155e5e, incl. every `kas::auth` process test and the wire-level host tests. The `win-clippy*` FAILs there are pre-existing Windows-only warnings in files this change does not touch (CI runs clippy on ubuntu only).
- Live C6: `receipts/live/timeline.txt` — renewal served at 19:44:40Z (expiry 19:47:39 → 20:44:40, plain helper, no force), turns answered before, inside the buffer, and after the old expiry; no BridgeDisconnected.
- Missing-helper shapes (C10 premise): `receipts/missing-subcommand-shapes.txt`, evidence P5.
- Independent review: one read-only reviewer pass over `85becf9f..2cf8d853`; no blocking findings; should-fixes 1–4 and nits 6/11 fixed in the review-fix commit; nits 7–10/12 dispositioned under Non-goals.

## Placement

- **Owner `protocol/kas/auth.rs`** — `KasTokenSource` (invocation + environment + 30 s bound + per-connection serialization), `KasToken` (redacted), `KasTokenError`, envelope parsing, reply building, `force_refresh_requested`, `respond_get_access_token`. Forbidden: reading kiro-cli files, OIDC, logging the token.
- **Owner `protocol/kas/version.rs`** — `kiro_cli_invocation` (the cyril-861q prefix, extracted from `version_probe_command`) and `run_captured` (the bounded child lifecycle, extracted from `run_version_probe`); both callers share them. No behavior change for the version probe.
- **`protocol/bridge.rs`** constructs the per-mode source in `resolve_spawn_command`; **`domain_mediator::DomainConfig` → `kas::callbacks::DispatchCtx`** carry it (`Option`, `None` for V2); **`domain_mediator/host.rs`** parses `forceRefresh` into `HostCallback::GetAccessToken`.
- **`kas/discovery.rs`** gate calls `KasTokenSource::fetch(false)`; the spawn's source is the one the responder uses. It keeps `default_store_path` for the fallback.
- **Owner `protocol/kas/stored_token.rs`** (amendment) — the stored-token reader, private to the `kas` module, called only from `KasTokenSource::stored_fallback`.

## Non-goals and future work

- Relay spawn (`kiro-cli acp --auth-method cli`) — rejected by the requester 2026-10-18.
- A cyril-side force-refresh when kiro-cli's plain answer is inside KAS's 180 s buffer — outside the approved "iff requested" design; revisit only if C6 shows a failing window (would be a new decision).
- Wrapper-mode spawn gate — cyril-cxd2 (open).
- Review nits accepted as-is (permanent rationale):
  - The fallback resolves the store from cyril's own environment even for a Wrapper (WSL / replacement-env) kiro-cli. Unreachable in practice: KAS needs kiro-cli ≥ 2.7.1, the helper exists from 2.6.0, and ≥ 2.29 wrappers never forward the callback (cyril-ny8z). A wrong-host store fails closed (absent → `StoredUnservable`).
  - The missing-helper cache is keyed by the rendered invocation, not its environment — one PATH per process in practice.
  - C8 covers stdout (where tokens live). The capped stderr tail and kiro-cli's own error message are relayed verbatim; no observed kiro-cli stderr or error message carries a token (`receipts/missing-subcommand-shapes.txt`).
  - On Windows a timed-out helper's descendants are not group-killed (unix kills the process group); kiro-cli's own refresh lock covers a straggler.
- `--auth-method client` on the launcher path — cyril-ny8z (open).

## Approval

Amendment (C10): requester decision 2026-10-18 relayed by the coordinator, quoted in `route.md` T4 item 1.

DG — delegated by the requester's brief for this run: "The approved fix (user decision 2026-10-18 — this IS your design approval; do not substitute the relay spawn)". The issue's Design Notes record the same decision ("DECIDED 2026-10-18 (user, over the relay spawn originally proposed here)").

## Review 2 — PR #173 cross-model review (reviewed 62031311): approve-with-nits, no blockers

The requester chose to fix all four items before merge (relayed by the coordinator, 2026-10-18). The fixes are technical repairs under the approved design; no behavior the design approved changed.

| # | Finding | Disposition |
|---|---|---|
| 1 (Low) | Free path hard-required PATH `kiro-cli`; the Spawn remedy cited `--agent-command`, which Free ignores. | Fixed — C7b: TUI `ku` lookup (PATH, then `~/.local/bin`, `/usr/local/bin`, `/opt/homebrew/bin`; cwd-subtree and relative PATH entries skipped); per-source remedy (free vs `--agent-command`). |
| 2 (Low) | Renewal serialization was per-connection only. | Fixed — C9b: process-wide lock per kiro-cli invocation; red-first receipt. |
| 3 (Nit) | `Timeout` remedy named only the network. | Fixed — also names another kiro-cli holding its token-refresh lock (`refresh lock timed out (peer wedged?)`); asserted in `callbacks::…typed_remediation`. |
| 4 (Nit) | A forced refresh is silently unhonoured on the fallback path. | Fixed — debug log `getAccessToken forceRefresh not honoured …` in `stored_fallback`. |

## Review 3 — PR #173 delta review (reviewed 54f8762f): approve-with-one-fix

Requester approved merging once ready; fixes relayed by the coordinator (2026-10-18). Technical repairs under the approved design.

| # | Finding | Disposition |
|---|---|---|
| 1 (Medium) | PATH-first rationale ("same kiro-cli as the version probe") not held: bundle selection probed bare `kiro-cli` via OS lookup, so with kiro-cli only in a TUI fallback dir the probe failed (→ newest extraction) while auth used `~/.local/bin/kiro-cli`; OS lookup also honours the relative/cwd PATH entries cyril skips. | Fixed — `installed_cli_version(token_source)` runs `version::invocation_version` on `KasTokenSource::kiro_cli()` under its environment; `version::kiro_cli_version` (now unused) removed. Red-first `discovery::tests::version_probe_and_token_source_share_the_fallback_kiro_cli` (unix child-process: kiro-cli only in `~/.local/bin`; one journal records `--version` then `chat _ get-kas-token`; the exact 2.24.0 extraction wins) — `receipts/r3-red-shared-kiro-cli.log` red, green after. |
| 2 (Low) | Lock key = rendered spelling; Free `/usr/local/bin/kiro-cli` and Wrapper bare `kiro-cli` got separate locks. | Fixed — C9c: `lock_key` = resolved binary (absolute as-is, bare name looked up on the source's effective PATH, then canonicalized) + args; also keys the missing-helper cache. `lock_key_unifies_spellings_of_one_binary` (pure, faked host) and `fetch_serializes_across_spellings_of_one_binary` (unix, absolute vs bare `sh` on a replacement PATH). Mutation: key reverted to the spelling → the process test goes red (`receipts/r3-red-lock-key-spellings.log`), restored → green. |
| 3a (Nit) | `is_executable_file` tested any exec bit. | Fixed — `access(X_OK)` for the current user via nix (TUI `y1e` / Node `accessSync(X_OK)`); Windows: any file. `is_executable_file_follows_access_x_ok` (0o600 no, 0o700 yes, 0o601 only as root). |
| 3b (Nit) | `under_cwd` case-sensitive on Windows. | Fixed — case-insensitive on Windows (TUI `YM` lowercases); `under_cwd_case_handling_follows_the_platform`. |
| 3c (Nit) | Struct doc still said "the PATH `kiro-cli`". | Fixed — documents the free-path resolver, the shared probe, and the resolved-binary lock key. |

## Review 4 — PR #173 delta review (reviewed 0b80df62): request changes (macOS CI red)

Technical repairs under the approved design (relayed by the coordinator, 2026-10-18).

| # | Finding | Disposition |
|---|---|---|
| 1 (High) | macOS CI: `free_gate_asks_kiro_cli_get_kas_token_not_the_store` planted the bundle under `$XDG_DATA_HOME/kiro-cli`, but macOS discovery ignores XDG (`~/Library/Application Support/kiro-cli`) → `Server(...)`; `version_probe_and_token_source_share_the_fallback_kiro_cli` had the same shape. `fetch_serializes_across_spellings_of_one_binary` gave the bare source `PATH=<dir of canonical sh>` only, while the fixture called `dirname` (/usr/bin on macOS) → exit 9. | Fixed — both discovery child tests plant under `unix_resolver_data_dir(home, xdg)`, derived from the production resolvers (`apple_kiro_data_dir` on macOS/iOS, `kiro_data_dir` elsewhere). Every POSIX fixture in this PR uses `${0%/*}` instead of `dirname`; the spellings test's replacement PATH adds `/usr/bin:/bin` for mkdir/sleep/rmdir. Audit of every test this PR adds or changes for host traps: `env_wrappers_resolve_xdg_data_home_like_kiro_cli` now gates with an explicit absent kiro-cli instead of `on_path()` (which would probe the host's real /usr/local/bin, /opt/homebrew/bin); `serial::kas_token_source_moves_into_the_dispatch_context` no longer calls `on_path()`. |
| 2 (Low) | `lock_key` computed once in `new()`: a versioned-symlink install upgraded mid-session keys a pre- and a post-upgrade source on different Cellar paths → two helpers could race the single-use refresh token. | Fixed — C9d: `KasTokenSource::current_lock_key()` resolves + canonicalizes per `fetch`, just before taking the lock (also keys the missing-helper cache). Red-first `fetch_serializes_across_a_symlink_retarget` (unix; `bin/kiro-cli -> Cellar/v1` retargeted to `Cellar/v2` between the two sources' construction): `receipts/r4-red-symlink-retarget.log` red (exit 9, lock dir held), green after. |
| 3 (Nit) | `missing_helper` doc said "keyed by the rendered invocation prefix". | Fixed — keyed by `lock_key`, computed per fetch. |

### macOS paths verified by reading code (no macOS machine available)

- `discovery::kiro_data_dir_from_env` on `macos`/`ios` → `apple_kiro_data_dir(kiro_agent_config::home_dir())` = `$HOME/Library/Application Support/kiro-cli`; XDG_DATA_HOME and LOCALAPPDATA are not read. The two discovery child tests set HOME to their private dir, so `unix_resolver_data_dir` names exactly that path.
- `kiro_agent_config::home_dir()` = `HOME`, then `USERPROFILE` — the fake `~/.local/bin` in the shared-kiro-cli test is under the child's HOME.
- `resolve_free_kiro_cli` on macOS takes the unix branch (fallbacks `~/.local/bin`, `/usr/local/bin`, `/opt/homebrew/bin`). In both discovery child tests the fake is found BEFORE the real `/usr/local/bin` / `/opt/homebrew/bin` (Review 5: each child has a PRIVATE PATH — the fake-only `bin` in one, an empty dir in the other, where the PATH-first walk finds nothing and the `~/.local/bin` fallback is the first hit), so the runner's own Homebrew can never be reached.
- `/bin/sh`, `/bin/mkdir`, `/bin/sleep`, `/bin/rmdir` exist on macOS; `dirname` lives in `/usr/bin` there and is no longer used. `printf`, `[`, `echo` and `${0%/*}` are shell builtins or expansions.
- macOS tempdirs live under `/var/folders` (a symlink to `/private/var`); only the lock-key tests canonicalize, and they compare canonicalized paths on both sides.

### Gate scope

The local gate covers Linux (all ten ci.yml lanes) plus the Windows VM (`resourcefs-win11`: kas/domain_mediator tests, MSVC). macOS has no local machine: its evidence is the `Test (macos-latest)` CI leg on the pushed head.

## Review 5 — PR #173 delta review (reviewed c847d0fd): APPROVE, tests-only follow-up

| # | Finding | Disposition |
|---|---|---|
| 1 (Low, token safety) | `version_probe_and_token_source_share_the_fallback_kiro_cli` gave its child `PATH=/bin:/usr/bin`; PATH is walked before the `~/.local/bin` fallback, so a distro `/usr/bin/kiro-cli` would have run the REAL kiro-cli (`--version`, `chat _ get-kas-token`) against the developer's login. The "/bin for the fixture's sh" comments were a misconception: the fixture's `#!/bin/sh` is kernel-resolved, its commands are builtins, and `KIRO_AGENT_PATH` makes node lookup moot. | Fixed — that child gets a private empty PATH dir; `free_gate_asks_kiro_cli_get_kas_token_not_the_store` gets a PATH of only its fake's `bin`; comments corrected. Audit of every test in the PR with a child or replacement PATH: `env_wrappers…` (PATH = private root), `tests/spawn_isolation.rs` free-path launches (private fake dir first, so its fake is the first hit), `fetch_serializes_across_spellings_of_one_binary` (PATH resolves `sh`, never kiro-cli), the bridge Wrapper test and every `fake_kiro_cli` process test (absolute programs, no kiro-cli lookup). No other test lets a system dir precede a fixture kiro-cli. |
| 2 | design.md said "`~/.local/bin` first" for the shared-kiro-cli test. | Fixed — the free path is PATH-first (requester decision, evidence P6); the macOS note now states each child's private PATH. |
| 3a (Nit) | `env_wrappers…` dropped its `install kiro-cli` remedy assertion. | Fixed — the child now builds the real free-path source through `KasTokenSource::free_path` (new: the injected-host form of `on_path`, probe confined to the test's private root) and asserts the gate's diagnostic carries `FREE_PATH_REMEDY`. |
| 3b (Nit, optional) | The symlink-retarget test execs freshly written scripts. | Kept and documented in the test: its two Cellar versions must be distinct real files (symlinks to one committed fixture would canonicalize to one key and blind the test); ETXTBSY-safe under nextest's process-per-test model. |
