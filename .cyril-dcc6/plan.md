# cyril-dcc6 plan — 7 slices, expand/contract

Budgeted-plan, 2026-07-04. Design: [`design.md`](design.md) (approved; cheapest
falsifier 7/7). Invariant: workspace green (`cargo test` + `clippy -- -D warnings`,
both default and `--features kas`) after EVERY slice. Ordering constraint: the
responder goes sqlite-backed (slices 3–4) BEFORE the free-path argv flip
(slice 5) — the argv flip is what makes the responder load-bearing for free path.

---

## Slice 1: Pure versioned-dir selection policy

**Claim:** C1–C4 — exact CLI-version match > newest-by-semver; grammar
`<semver>-<sha64>`; dirs without the inner `acp-server.js` are not candidates.
**Oracle:** hand-derived expectations from `falsifier-selection-policy.py`
(passed 7/7) + real-machine /proc capture (banked).
**Stress fixture:** root `{2.9.0-<shaA>, 2.10.0-<shaB>}` with CLI=2.11.0 →
must pick 2.10.0 (lexicographic max picks 2.9.0 — the ordering bug); malformed
names (63-char sha, non-hex sha, no dash, `v2.10.0-…`) ignored; a 2.11.0 dir
whose inner entry is absent loses to a complete 2.10.0; empty root → None;
same-version different-sha duplicates → deterministic max by dir name.
**Loop budget:** O(entries in kas root) per spawn; production ≈ ≤10 dirs
(one per installed kiro version), spawn-time only — trivially under budget.
**Files:** `crates/cyril-core/src/protocol/kas/discovery.rs`

**Code (advisory):** new pure fn
`select_server(entries: &[(String, bool)], cli_version: Option<(u32,u32,u32)>) -> Option<Selected>`
taking (dir-name, inner-entry-exists) pairs; reuse `version::parse_semver` for
ordering (export pub(crate) if needed — counts as the 2nd file if so).

**Verification:**
- [ ] Unit tests pass (fences: `picks_exact_version_match`,
      `picks_newest_semver_not_lex`, `no_cli_version_falls_back_newest`,
      `partial_extraction_skipped`, malformed-name + duplicate-sha cases)
- [ ] Stress fixture produces expected outcome
- [ ] `falsifier-selection-policy.py` still 7/7 (policy parity)
- [ ] Budgets hold

## Slice 2: Wire selection into resolve() with fallbacks and real CLI version

**Claim:** C5–C7 (+C3 wiring) — legacy fallback; `KasMissing::Server` names the
searched root; `KIRO_KAS_SERVER_PATH` bypasses the glob; `resolve_kas_command`
feeds `kiro_cli_version("kiro-cli")` (failure ⇒ None ⇒ newest, warn-logged).
**Oracle:** same as slice 1 + existing override-semantics tests keep passing.
**Stress fixture:** root with BOTH legacy and versioned (versioned must win —
the legacy-checked-first bug); override set while versioned dirs exist
(override must win — the glob-anyway bug); kiro-cli absent from PATH (newest,
no error); `Server` error message contains the kas root path (actionability).
**Loop budget:** unchanged from slice 1; `kiro-cli --version` = 1 subprocess
per spawn (spawn-time only).
**Files:** `discovery.rs`, `version.rs` (visibility of `kiro_cli_version`).

**Verification:**
- [ ] Unit tests pass (incl. updated existing resolve tests — token-file check
      still present in this slice; only the server-path logic changes)
- [ ] Stress fixture produces expected outcome
- [ ] Oracle parity holds
- [ ] Budgets hold

## Slice 3: rusqlite dependency plumbing (kas-gated)

**Claim:** rusqlite (`default-features = false`, `features = ["bundled"]`)
compiles ONLY under `--features kas`; default build's dep tree is unchanged.
**Oracle:** `cargo tree -p cyril-core` (default vs `--features kas`) — an
independent tool, not our code.
**Stress fixture:** `cargo tree -p cyril-core | grep -c rusqlite` = 0 on
default, ≥1 with kas; `cargo check --all-targets` green both ways (the
optional-dep-leaks-into-default bug; the workspace-lints-not-inherited bug).
**Loop budget:** none (no code).
**Files:** `Cargo.toml` (workspace), `crates/cyril-core/Cargo.toml`.

**Verification:**
- [ ] Both feature builds green, clippy clean
- [ ] Stress fixture (dep-tree grep) as expected
- [ ] No source changes → oracle trivially unchanged
- [ ] Budgets hold (n/a)

## Slice 4: `read_sqlite_store()` — the AuthReply source from sqlite

**Claim:** C9, C11, C12 — reply fields come from `auth_kv['kirocli:odic:token']`
(`access_token`→accessToken, `expires_at`→expiresAt) + `state
['api.codewhisperer.profile'].arn`→profileArn; absent arn is an Err; DB opened
`SQLITE_OPEN_READ_ONLY` with short busy timeout; a missing DB is never created.
**Oracle:** live backend acceptance (probe B, banked 2×); fixture-DB rows
hand-written and cross-checkable with the `sqlite3` CLI.
**Stress fixture:** fixture DB with EXTRA unrelated rows in both tables (the
first-row-instead-of-keyed-row bug); token JSON in **snake_case** (the
camelCase-assumption bug — the retired file used camelCase); `expires_at` with
9 sub-second digits + `Z` (must parse via existing `rfc3339_to_epoch`); state
row present but `arn` key absent; DB path pointing at a nonexistent file →
Err AND no file created afterward (the read-write-open-creates bug).
**Loop budget:** 2 point queries per call; calls ≈ 1/session + ~1/55min
refresh — under budget, not always-on.
**Files:** `crates/cyril-core/src/protocol/kas/auth.rs`.
**Doc contract:** "`db` must be kiro-cli's `data.sqlite3`" is load-bearing →
enforced by runtime Errs on missing/malformed rows (never a default reply).

**Verification:**
- [ ] Unit fences: `reply_from_sqlite_rows`, `missing_profile_arn_errors`,
      `readonly_never_creates_db`, snake-case + extra-rows cases
- [ ] Stress fixture produces expected outcome
- [ ] Oracle parity (existing file-path tests untouched this slice)
- [ ] Budgets hold

## Slice 5: Responder swap — sqlite source live, per-call re-read

**Claim:** C10, C13 — `respond_get_access_token` reads the sqlite store via
`spawn_blocking`; DB/row absent → actionable "run `kiro-cli login`" error
(never a partial reply); the store is re-read on every callback so a
mid-session re-login is served without restart.
**Oracle:** probe B live turns (banked); logout-deletes-row fact
(launch-contract memory, verified 2026-07-02).
**Stress fixture:** row DELETED between two calls (logout mid-session → second
call errs actionably, not a cached reply — the cache-at-startup bug); row
REPLACED between calls (re-login → second reply carries the NEW token);
stale `expires_at` → existing stale error (policy unchanged, existing tests).
**Loop budget:** unchanged from slice 4; `spawn_blocking` = 1 blocking-pool
hop per callback (not always-on).
**Files:** `auth.rs` (+ `discovery.rs` for `default_store_path()` next to
`default_token_path()`).
**Doc contract:** the module doc's "custodian: read-only, one reply, never
logged" stays load-bearing → redacted `AccessToken` type reused; no new
logging of reply contents (diagnostics via `tracing` only).

**Verification:**
- [ ] Unit fences: `logged_out_row_absent_errors`, `store_reread_per_callback`
- [ ] Stress fixture produces expected outcome
- [ ] Oracle parity
- [ ] Budgets hold

## Slice 6: Free-path argv flip + sqlite login gate

**Claim:** C8, C14a — free-path argv becomes exactly
`[--experimental-wasm-modules, <entry>, --transport=stdio, --auth=acp-callback]`;
the login gate keys on sqlite-row presence (SSO file existence is no longer
consulted anywhere in resolve).
**Oracle:** kiro-cli's own spawn flags (/proc, banked); byte-compare the argv.
**Stress fixture:** SSO file ABSENT + sqlite row present → `Ok` (kills the
file-gate bug — this is the removed-invariant fence); SSO file PRESENT (dead
identity) + row absent → `NotLoggedIn` (file presence must not fake login);
full-argv byte equality vs the banked /proc list (kills the missing-flag bug —
this fence FAILS against pre-slice code by construction).
**Loop budget:** gate = 1 point query at spawn time.
**Files:** `discovery.rs` (+ `auth.rs` if the row-presence helper lives there).
**Doc contract:** `KasMissing::NotLoggedIn` payload becomes the DB path; its
`reason()` keeps "run `kiro-cli login`" (load-bearing actionability — asserted
in the existing `missing_reasons_are_actionable` test, updated).

**Verification:**
- [ ] Unit fences: `argv_matches_kiro_cli_own_spawn`, `gate_is_sqlite_not_file`
- [ ] Stress fixture produces expected outcome
- [ ] Oracle parity (argv now byte-equal to /proc capture)
- [ ] Budgets hold

## Slice 7: Contract — delete the SSO-file path; align smokes and docs

**Claim:** completes C14 and enforces negative-space #3 — no code path reads
`kiro-auth-token.json`; `kas_freepath_smoke`/`kas_wrapper_smoke` describe and
exercise callback auth; module docs match reality.
**Oracle:** `grep -rn 'kiro-auth-token' crates/` = 0 hits in src (shell, not
our code); both feature builds green.
**Stress fixture:** the grep itself (the silent-fallback-resurrection bug — any
surviving reference is a failure); `cargo test --features kas` compiles the
updated `#[ignore]` smokes (the stale-smoke-rot bug).
**Loop budget:** none (deletion).
**Files:** `auth.rs` + `discovery.rs` (delete `read_token_file`,
`default_token_path`, file tests); then `tests/kas_freepath_smoke.rs` +
`tests/kas_wrapper_smoke.rs` (same slice only if the diff stays mechanical;
otherwise split the smoke edits into a 7b).

**Verification:**
- [ ] Unit tests pass, both feature sets, clippy clean
- [ ] grep fixture: zero `kiro-auth-token` references in `crates/`
- [ ] Oracle parity (`falsifier-selection-policy.py` still 7/7)
- [ ] Budgets hold (n/a)

---

## Post-slice: one-time live validation (user-gated)

Mirror of qo13's C8 manual fence: run `kas_freepath_smoke` + `kas_wrapper_smoke`
(`--ignored`) against live KAS 2.11.0 inside a fresh token window — validates
C14b end-to-end with cyril's real binary rather than the probe. Requires the
user logged in; the deterministic fences above are the CI-permanent form
(design § Falsification, C14 row).

## Plan Self-Review

1. **Loops:** slice 1 dir-scan O(≤10)/spawn; slice 2 +1 subprocess/spawn;
   slices 4–6 ≤2 point queries per callback/spawn. No always-on loops; all
   under 10^6 ops / 10^3 syscalls. No unstated loops.
2. **Fixtures:** every slice names its bug class — ordering (semver-vs-lex),
   grammar (malformed sha), staleness of listing (partial extraction),
   precedence (legacy-first, glob-over-override), keyed-vs-first-row, casing
   (snake_case vs camelCase), create-on-open, cache-vs-reread, file-gate
   inversion (both directions), missing-flag argv, fallback resurrection
   (grep). No happy-path-only fixtures.
3. **Doc preconditions:** slice 4 (db shape → runtime Errs, load-bearing);
   slice 5 (custodian/no-logging → type-enforced redaction); slice 6
   (actionable NotLoggedIn → tested `reason()`). No unenforced contracts.
4. **Write targets:** no stdout writes anywhere; all diagnostics via `tracing`
   (cyril.log). No data-stream outputs in this feature.
5. **Tracker refs:** cyril-taba (stale policy — reframed note 2026-07-04),
   cyril-lwpm (Windows store access — filed 2026-07-04), cyril-0pms (reaping),
   cyril-l7tw (error rendering), cyril-6iek (fingerprinting) — all verified
   present this session. No uncited deferrals.

Claim coverage: C1–C4 (S1), C5–C7 (S2), dep-gate (S3), C9/C11/C12 (S4),
C10/C13 (S5), C8/C14a (S6), C14 completion + negative-space #3 (S7). All 14
design claims covered; nothing extra.
