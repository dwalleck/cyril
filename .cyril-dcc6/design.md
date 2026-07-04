# cyril-dcc6 design — KAS versioned-dir discovery + sqlite-backed callback auth

Falsifiable-design, 2026-07-04. Probe basis: [`probe-findings.md`](probe-findings.md)
(prove-it-prototype, all oracles agreed). Branch `feat/dcc6-kas-discovery-auth`.

## Purpose

Make cyril's KAS engine startable and authenticatable on kiro ≥2.10.0 without
manual workarounds: (A) discovery resolves the versioned self-extraction dirs
(`~/.local/share/kiro-cli/kas/<semver>-<sha64>/…`), and (B) auth is sourced
from the CLI sqlite store via `--auth=acp-callback` on both spawn modes,
retiring every dependency on the poisoned `~/.aws/sso/cache/kiro-auth-token.json`.

## What the probe established (design may not contradict)

- kiro-cli 2.11.0 spawns the **exact-version-matched** versioned dir with argv
  `--experimental-wasm-modules <entry> --transport=stdio --auth=acp-callback` (/proc oracle).
- A responder replying `{accessToken, expiresAt}` from sqlite
  `auth_kv['kirocli:odic:token']` + `profileArn` from
  `state['api.codewhisperer.profile']` completes live authenticated turns in
  BOTH topologies (direct spawn; kiro-cli wrapper forwarding the callback).
- An unanswered/empty callback reply kills the turn
  (`TokenInvalidError: Host refresh callback returned no access token`).
- Probe B succeeded with the dead SSO file PRESENT → `--auth=acp-callback`
  ignores the file (provider selection is by flag, tier 2 < tier 5).

## Components

1. **`discovery.rs`** — extend the pure `resolve()`: inject a directory-listing
   closure alongside `exists`; selection policy = exact CLI-version match >
   newest by semver tuple > legacy unversioned > `KasMissing::Server`.
   Versioned-dir name grammar: `^\d+\.\d+\.\d+-[0-9a-f]{64}$`; a dir without
   the inner `acp-server.js` is not a candidate. Same-version/different-sha
   duplicates: deterministic max by full dir name (byte order). Free-path argv
   gains `--auth=acp-callback`. CLI version comes from the existing
   `version.rs::kiro_cli_version("kiro-cli")` (PATH); failure ⇒ no exact
   preference, newest wins (warn-logged).
2. **`auth.rs`** — source swap in the responder: `read_token_file(SSO path)` →
   `read_sqlite_store(db path)` returning the same `AuthReply`. Everything
   else keeps: `is_stale` 3-min buffer + actionable stale error, redacted
   `AccessToken`, `build_response` three camelCase keys, load-bearing
   `profileArn`. sqlite key mapping: `access_token`→`accessToken`,
   `expires_at`→`expiresAt` (RFC3339-Z, 9 sub-second digits — parses with the
   existing strict `rfc3339_to_epoch`), profile row `.arn`→`profileArn`.
3. **Login precheck** — `resolve()`'s token-file existence check is replaced by
   a sqlite gate in the spawn path: `auth_kv` row present ⇒ logged in; row or
   DB absent ⇒ `KasMissing::NotLoggedIn` ("run `kiro-cli login`"). Row-absent
   is the real logout shape (logout deletes the row; the DB file survives).
4. **Dependency** — `rusqlite` (workspace dep, `default-features = false`,
   `features = ["bundled"]`), optional in cyril-core behind the existing `kas`
   feature. Reads run under `tokio::task::spawn_blocking` (rusqlite is sync;
   the bridge is a current_thread runtime that must not stall — same rationale
   as the existing `tokio::fs` comment). Open mode: `SQLITE_OPEN_READ_ONLY` +
   short busy timeout; kiro-cli writes the same DB concurrently.
5. **Dispatch** — unchanged: `client.rs:270` answers
   `kiro/auth/getAccessToken` unconditionally for KAS sessions, so free-path
   `--auth=acp-callback` is served by the same responder (probe-confirmed
   contract; the wrapper forwards, the direct spawn asks directly).

## Input shapes (step 2)

**Discovery — kas root contents** (each has a claim):
empty/missing root (C6) · versioned dirs incl. exact CLI match (C1) · versioned
dirs, none matching (C2) · malformed dir names — ignored (C4 grammar) · dir
without inner entry / partial extraction (C4) · legacy unversioned only (C5) ·
legacy AND versioned (versioned wins — C1/C2 imply; fence asserts) ·
same-version different-sha duplicates (deterministic — C1 fence variant).
**CLI version**: parseable (C1) · kiro-cli absent/unparseable (C3).
**Env**: `KIRO_KAS_SERVER_PATH` set (C7) · unset · blank (existing `nonempty`
tests, unchanged). **home**: `Some`/`None` (existing `NoHome` tests, unchanged).
**sqlite store**: DB present + both rows valid (C9) · DB absent (C10) ·
`auth_kv` row absent — logout shape (C10) · row present, malformed JSON (C10) ·
token fields empty/missing (C10) · profile row or `.arn` absent (C11) ·
`expires_at` stale (existing `is_stale` tests — policy unchanged, settled) ·
DB busy/locked (C12) · store mutated between callbacks — re-login (C13).
Out of scope: Windows-host store access (WSL-internal paths) — **cyril-lwpm**.

## Removed-invariant sweep (step 2b — this change is subtractive)

Removed constraint 1: *"free-path KAS authenticates with zero cyril
participation"* (file-auth). Chain: free path never needed the responder ⇒
nothing guaranteed the responder/dispatch works for `KasSpawn::Free`. Now every
free-path turn depends on it → **C14**.
Removed constraint 2: *"NotLoggedIn ⇔ SSO file absent"*. The file gate also
(wrongly) guaranteed KAS's file-auth had something to read. Replaced by the
sqlite gate → **C10, C14**; file presence/absence no longer observable anywhere.
Removed constraint 3: *"cyril never opens the CLI's sqlite DB"* — new failure
modes (locked DB, accidental create/write) → **C12**.
Still-safe: wrapper spawn command construction (untouched, `version.rs` tests);
staleness policy (unchanged code + tests).

## Claims and falsification

| # | Claim (one sentence) | Falsifier | Oracle | Cost | Status | Regression fence |
|---|---|---|---|---|---|---|
| C1 | With a versioned dir matching the installed CLI version, resolve picks it even when a newer dir exists. | Fixture root {2.10.0, 2.11.0}, CLI=2.10.0 → must pick 2.10.0; newest-always impl fails. | Hand-derived expectation + real-machine pick == kiro-cli's own /proc argv. | 5m | **passed** (`falsifier-selection-policy.py` C1 + C1-real) | unit `discovery::tests::picks_exact_version_match` |
| C2 | With no matching dir, resolve picks the highest by semver-tuple order. | Root {2.9.0, 2.10.0}, CLI=2.11.0 → 2.10.0; lexicographic-max impl picks 2.9.0 and fails. | Hand-derived (2.10.0 > 2.9.0 numerically). | 5m | **passed** (C2) | unit `picks_newest_semver_not_lex` |
| C3 | With the CLI version unavailable, resolve picks the newest versioned dir rather than erroring. | Root {2.10.0, 2.11.0}, no CLI → 2.11.0; err-on-no-cli impl fails. | Hand-derived. | 5m | **passed** (C3) | unit `no_cli_version_falls_back_newest` |
| C4 | A versioned dir lacking the inner acp-server.js is skipped. | Root {2.10.0 complete, 2.11.0 partial} → 2.10.0; name-glob-only impl picks 2.11.0 and fails. | Hand-derived. | 5m | **passed** (C4) | unit `partial_extraction_skipped` |
| C5 | With no versioned candidates, the legacy unversioned path is used when present. | Root with legacy only → legacy; versioned-only impl errors and fails. | Hand-derived. | 5m | **passed** (C5) | unit `legacy_fallback` |
| C6 | With no candidates anywhere, resolve returns `KasMissing::Server` naming the searched location. | Empty root → None/Server; wrong-variant or path-less message fails. | Hand-derived. | 5m | **passed** (C6) | unit `nothing_found_is_server_missing` |
| C7 | `KIRO_KAS_SERVER_PATH` bypasses the glob entirely. | Override set + versioned dirs present → override chosen; glob-anyway impl fails. | Existing override semantics (probe-era env contract). | 10m | pending | unit `override_beats_versioned` (+ existing override tests) |
| C8 | Free-path argv is exactly `[--experimental-wasm-modules, <entry>, --transport=stdio, --auth=acp-callback]`. | Compare resolve() argv to the /proc-captured flags; missing `--auth` (today's code) fails. | kiro-cli's own spawn flags (/proc, banked). | 10m | **passed** (oracle banked; today's code correctly FAILS it) | unit `argv_matches_kiro_cli_own_spawn` |
| C9 | The responder's reply fields come from the sqlite rows (token → accessToken/expiresAt, profile → profileArn). | Fixture DB with known rows → reply equals them; SSO-file-reading impl (no file present) fails. | Live: probe B backend acceptance (banked, 2×); fixture values hand-written. | 15m | passed-live / fence pending | unit `reply_from_sqlite_rows` |
| C10 | A missing DB or absent/malformed `auth_kv` row yields the actionable NotLoggedIn error, never a reply. | Fixture: DB w/o row (logout shape); empty-reply impl fails. | Logout-deletes-row fact (verified 2026-07-02, launch-contract memory). | 15m | pending | unit `logged_out_row_absent_errors` |
| C11 | An absent profile row/arn is an error, never a null/empty profileArn reply. | Fixture DB w/o state row; empty-arn-default impl fails. | Live fact: null arn 400s "profileArn is required" (2026-06-30, banked). | 15m | pending | unit `missing_profile_arn_errors` |
| C12 | The DB is opened read-only and a missing DB is never created. | Point store at absent path, call responder, assert Err AND no file created; default read-write open creates it and fails. | Filesystem state after call. | 15m | pending | unit `readonly_never_creates_db` |
| C13 | The store is re-read on every callback, so a mid-session re-login is served on the next callback. | Read → mutate fixture rows → read; cache-at-startup impl serves stale and fails. | Fixture mutation visible in second reply. | 15m | pending | unit `store_reread_per_callback` |
| C14 | Free-path spawns get their callback answered and the login gate keys on the sqlite row, not the SSO file. | (a) unit: resolve/gate Ok with SSO file absent + fixture row present (file-checking impl fails); (b) live smoke: free-path `--auth=acp-callback` turn completes (wrapper-only dispatch impl fails). | (a) fixture; (b) live backend (probe B banked, 2×). | 15m + live | probe-passed / fences pending | unit `gate_is_sqlite_not_file` + `kas_freepath_smoke` (#[ignore], updated) |

Per-claim distinctness: each fence is its own named test; the design-time
falsifier prints one labeled verdict per claim. Non-vacuity: every falsifier
above names the specific buggy implementation it kills; C8's fence fails
against TODAY's code (true regression sentinel for the bug class).

Cheapest falsifier run: `falsifier-selection-policy.py` — **7/7 PASS**
(C1–C6 synthetic + C1 real-machine /proc cross-check), 2026-07-04.

Cost distribution: only C14(b) needs a live backend; its evidence is already
banked twice from prove-it-prototype, and the deterministic halves fence in CI.
The `#[ignore]` smoke is the manual re-verification path (existing repo
convention for `kas_*_smoke`).

## Negative space (what this deliberately does not do)

1. **No token refresh** — cyril never reads or transmits `refresh_token`;
   stale token ⇒ actionable error; stale-policy evolution is **cyril-taba**
   (reframed 2026-07-04).
2. **No writes to any credential store** — sqlite is read-only; the SSO file
   is neither read, written, seeded, nor deleted.
3. **No silent multi-store fallback** — sqlite is the only source; falling
   back to the SSO file on error would silently resurrect the dead-identity
   bug this issue exists to kill.
4. **No bundle download/extraction** — missing bundle stays an actionable
   error naming `kiro-cli acp --agent-engine v3`.
5. **No spawn-mode policy change** — `KasSpawn::Free` remains the default;
   wrapper command construction is untouched.

## Deferrals (tracker-verified)

- Stale-token policy / re-login affordance → **cyril-taba** (note appended 2026-07-04).
- Windows-host access to the WSL-internal sqlite store/bundle → **cyril-lwpm** (filed 2026-07-04).
- Child-process reaping on the spawn path → **cyril-0pms**.
- Rendering of errored turns (TokenInvalidError currently looks like a silent end) → **cyril-l7tw**.
- Engine fingerprinting at handshake → **cyril-6iek**.
