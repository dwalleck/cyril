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
| C1 | Responder and Free gate get the token from `kiro-cli chat _ get-kas-token`; the credential store is read only by the missing-helper fallback module; the SSO token file never. | `discovery::tests::free_gate_asks_kiro_cli_get_kas_token_not_the_store` (child env: fake kiro-cli on PATH, no data.sqlite3): RED on base (StoreUnservable), GREEN after, journal ends `chat _ get-kas-token`. Source fences `credential_store_token_rows_only_in_the_fallback` (needles appear in `kas/stored_token.rs` only), `sso_token_path_never_resurrected`. | same tests | PENDING — slice 1 |
| C2 | Error envelope, non-zero exit (with or without an envelope), timeout, spawn failure, malformed output (not JSON / empty / wrong kind / wrong field types / missing or empty required fields), unbound source → distinct typed `KasTokenError`, each with its own remediation, never a token or default; failures reach the UI as `BridgeError("auth")` and KAS as JSON-RPC errors (-32000 for kiro-cli's error envelope, -32603 otherwise). Gate failures fail the spawn with the same diagnostic. | `auth::tests::failure_shapes_map_to_typed_errors` (15 rstest cases), `error_envelope_hint_appears_exactly_once`, `acp_codes_separate_auth_state_from_helper_failure`, `fetch_times_out_on_a_wedged_helper`, `fetch_reports_spawn_failure_and_unbound`; `callbacks::tests::auth_failure_notification_carries_the_typed_remediation`; `discovery::tests::env_wrappers_resolve_xdg_data_home_like_kiro_cli` (gate → `AuthUnavailable` naming the helper). | same | PENDING — slice 1 |
| C3 | The relayed reply is exactly the envelope's five fields, values unchanged; optional fields only when present (null/empty = absent); unknown data keys are not relayed (debug-logged). Last non-empty line wins (noise and CRLF tolerated). | `success_envelope_relays_the_data_fields` (4 cases), `fetch_runs_the_helper_with_force_refresh_and_relays_data` (process seam), live C6 (KAS authenticates with the relayed token). | same | PENDING — slice 1 |
| C4 | `--force-refresh` is appended iff the request params carry `forceRefresh: true` (absent / null / false / non-boolean → not forced, non-boolean warns); placed after the invocation prefix (plain or WSL launcher). | `force_refresh_only_on_explicit_true` (6 cases), `invocation_appends_force_refresh_iff_requested` (3 cases), process-seam argv in `fetch_runs_the_helper_with_force_refresh_and_relays_data`, gate journal (no flag) in C1's test. | same | PENDING — slice 1 |
| C5 | v2 unchanged: V2 binds no token source; non-`kas` builds compile none of this; all existing v2 tests pass. | Every ci.yml lane, including `--no-default-features` and default-feature lanes. | existing suites | PENDING — slice 1 |
| C6 | LIVE: a real cyril KAS (Free) session keeps answering turns across the access token's expiry, with no BridgeDisconnected; renewal is visible as a new `expiresAt` in cyril's `getAccessToken served` log. | hex1b-driven TUI session opened before expiry, turns before / inside KAS's 3-minute buffer / after expiry; cyril.log + screenshots in `receipts/live/`. | `tests/kas_freepath_smoke.rs` (ignored live smoke) unchanged | PENDING — slice 1 |
| C7 | Spawn-mode resolution: Free asks the PATH `kiro-cli` (std lookup → `kiro-cli.exe` on Windows) — the same instance the gate asked; Wrapper asks the agent command's kiro-cli via the cyril-861q prefix (through the WSL launcher when used) under the spawn's environment. Version-probe behavior is unchanged by the prefix extraction. | `invocation_appends_force_refresh_iff_requested::wsl_forced`; all `version::tests` (probe argv + messages unchanged); Windows VM run of the platform-neutral process tests. Wrapper live auth is not exercisable on kiro-cli ≥ 2.29 (default `--auth-method cli` sends zero callbacks — cyril-ny8z). | same | PENDING — slice 1 |
| C8 | Custodian: the access token never appears in `Debug`, logs, or any diagnostic (malformed-output errors describe shape only, never the line). | `kas_token_debug_is_redacted`; `failure_shapes_map_to_typed_errors` asserts the secret is absent from Display and Debug for every case. | same | PENDING — slice 1 |
| C9 | cyril never runs two token helpers at once on one connection (OIDC refresh tokens are single-use; KAS single-flights per process but the guarantee is cyril's to keep). | `fetch_serializes_concurrent_requests`: two concurrent fetches through one source against a fake helper that fails if another instance holds its lock dir. | same | PENDING — slice 1 |

| C10 | (amendment) ONLY clap's refusal of the helper itself triggers the stored-token fallback; it serves the stored token, warns once per connection naming the kiro-cli version, probes the helper once per process; an expired stored token on the fallback is a hard -32000 error; every other failure (error envelope, other non-zero exit incl. other clap errors / exit 1 / any stdout, timeout, spawn failure, malformed) never falls back. | `missing_helper_is_detected_precisely` (10 rstest cases), `missing_helper_falls_back_to_the_stored_token_and_warns_once` (process seam: served, warning names 2.5.1, second request silent, one helper call), `missing_helper_with_expired_store_is_a_hard_error`, `other_helper_failures_never_fall_back` (servable store present, logged-out envelope → ErrorEnvelope); `stored_token::tests::*` (the moved reader's fences) incl. `read_stored_token_serves_fresh_and_refuses_stale`. Named mutations: detection widened to any failing exit → 5 rstest cases red; detection disabled → 5 tests red; restored → green (`receipts/mutation-c10*.log`). | same | PENDING — slice 1 |

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
- `--auth-method client` on the launcher path — cyril-ny8z (open).

## Approval

Amendment (C10): requester decision 2026-10-18 relayed by the coordinator, quoted in `route.md` T4 item 1.

DG — delegated by the requester's brief for this run: "The approved fix (user decision 2026-10-18 — this IS your design approval; do not substitute the relay spawn)". The issue's Design Notes record the same decision ("DECIDED 2026-10-18 (user, over the relay spawn originally proposed here)").
