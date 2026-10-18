# plan.md — cyril-zbu7

One atomic slice: the token-source swap changes `HostCallback::GetAccessToken`, `DispatchCtx`, `DomainConfig`, `resolve_kas_command` and `KasMissing` together with every caller, so it cannot land green in parts.

## Slice 1: answer KAS auth through `kiro-cli chat _ get-kas-token`

**Outcome:** design C1–C9 — responder and Free gate ask kiro-cli (`--force-refresh` iff requested), relay the five envelope fields unchanged, map every failure to a typed actionable error that reaches the UI, never read kiro-cli's credential store, never run two helpers at once; v2 untouched.

**Changes:**
- `crates/cyril-core/src/protocol/kas/auth.rs` — rewritten (design Placement): sqlite reader, row tiebreak, RFC3339/stale logic removed; `KasTokenSource`, `KasToken`, `KasTokenError`, envelope parsing, `force_refresh_requested`, `respond_get_access_token(source, force_refresh)`.
- `crates/cyril-core/src/protocol/kas/version.rs` — extract `kiro_cli_invocation` (from `version_probe_command`) and `run_captured` / `Captured` / `CaptureFailure` (from `run_version_probe`); `render` becomes `pub(crate)`. Version-probe argv and messages unchanged.
- `crates/cyril-core/src/protocol/kas/discovery.rs` — gate = `token_source.fetch(false)`; `KasMissing::AuthUnavailable` replaces `NoHomeForStore` / `StoreUnservable`; `STORE_FILE_NAME`, `store_path`, `default_store_path` removed; store assertions dropped from the data-dir tests; env-wrapper child test asserts the gate's spawn-failure diagnostic; red-first test `free_gate_asks_kiro_cli_get_kas_token_not_the_store` (+ fixture `tests/fixtures/fake-kiro-cli.sh`).
- `crates/cyril-core/src/protocol/kas/callbacks.rs` — `GetAccessToken { force_refresh, reply }`; `DispatchCtx.kas_token`; dispatch maps `(acp::Error, KasTokenError)`; `auth_failure_notification(&KasTokenError)`.
- `crates/cyril-core/src/protocol/domain_mediator/{mod.rs,host.rs}` — `DomainConfig.kas_token` moved into `DispatchCtx`; `forceRefresh` parsed at the extension dispatch.
- `crates/cyril-core/src/protocol/bridge.rs` — `resolve_spawn_command` returns the per-mode token source (Free: `on_path`, shared with the gate; Wrapper: `kiro_cli_invocation(agent_command)` + spawn environment; V2: none).
- Test `DomainConfig` / `DispatchCtx` constructors (`domain_mediator/tests/serial.rs`, `bridge/tests/harness.rs`, `sdk_runtime/tests/process.rs`, `domain_mediator/host.rs`): `kas_token: None`.
- Stale docs: `engine.rs` `AuthAdapter`, `types/kas_spawn.rs`, `cyril-core/Cargo.toml` feature comment, the three KAS live-smoke headers, ROADMAP Open Tension #7 update note.

**Verification:**
- Red-first: `free_gate_asks_kiro_cli_get_kas_token_not_the_store` on base `85becf9f` → FAIL (`StoreUnservable … data.sqlite3 is absent`), `receipts/red-run.log`; after the fix → PASS, `receipts/green-run.log`.
- C9 sensitivity: lock removed → `fetch_serializes_concurrent_requests` FAIL (helper exit 9, "File exists"), restored → PASS (`receipts/mutation-c9-{red,restored}.log`).
- Design C1–C5, C7–C9 tests (design table) → PASS.
- Gate: all ten ci.yml lanes via `.cyril-zbu7/gate.sh` → every lane `-OK` (`receipts/gate/summary.txt`).
- Windows: the platform-neutral process tests (`kas::auth`) + kas/no-default clippy and tests on the Windows VM (`ssh resourcefs-win11`) → PASS (`receipts/windows-vm.txt`).
- Live C6: cyril KAS Free session across the 2026-10-18T19:47:39Z expiry → all turns answered, a new `expiresAt` served, no BridgeDisconnected (`receipts/live/`).
- Independent review of the diff → no blocking finding.

## Slice 2 (amendment): stored-token fallback when kiro-cli lacks the helper

**Outcome:** design C10, C1 (amended fence).
**Changes:** `kas/stored_token.rs` (new, the pre-zbu7 reader + tests moved verbatim, plus `read_stored_token`); `kas/auth.rs` (`HelperMissing` detection in `interpret`, `TokenOrigin`, per-process missing-helper cache, `stored_fallback`, `StoredUnservable` / `StoredStale`, one-time warning returned by `respond_get_access_token`); `kas/callbacks.rs` (warning → `SystemNotify{Warning}`); `kas/discovery.rs` (`default_store_path` / `store_path` restored for the fallback, R2 assertion restored); `kas/version.rs` (`invocation_version`); `kas/mod.rs`.
**Verification:**
- C10 tests (design table) → PASS; mutations `receipts/mutation-c10-red.log` (widened), `mutation-c10b-red.log` (disabled) → red; `mutation-c10-restored.log` → green.
- Slice-1 checks unchanged and re-run in the full gate.
**Dependencies:** slice 1.
