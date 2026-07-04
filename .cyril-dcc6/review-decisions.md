# PR-39 review-feedback decisions (gilfoyle/assessing-review-feedback)

Source: 5-agent `/pr-review-toolkit:review-pr` sweep (code-reviewer, pr-test-analyzer,
comment-analyzer, silent-failure-hunter, type-design-analyzer), 2026-07-04.
Every finding verified against the code and keyword-matched against rivets before
any change was applied. Fix commits reference the F-numbers below.

Suite state after fixes: 484 lib tests green under `--features kas` (+8 new fences),
387 default-build, `clippy -D warnings` clean both feature sets, `cargo fmt --check` clean.

## Decision log

| # | Finding (one line) | Reviewer(s) | Category | Verified? | Decision | Note |
|---|---|---|---|---|---|---|
| F1 | getAccessToken failures reply to KAS but log nothing locally, notify App of nothing | failure-hunter (Critical) | Bug | Yes — no tracing in `respond_get_access_token`; KAS collapses failed callbacks (l7tw) | **Modify** | warn! per error path added (the cheap, unowned half). App-notification half is **duplicate of cyril-l7tw item (3)** — deferred there (needs channel plumbing that belongs with l7tw's bridge rework); l7tw note updated |
| F2 | Stale-token branch of the composed responder has zero non-live coverage (hardwired `default_store_path`) | test-analyzer (Critical) | Bug (test gap) | Yes — no test called the composed fn; inverting `is_stale` passed 476/476 | **Accept** | Extracted `get_access_token_from(db, now)`; 3 fences: fresh serve / stale −32000 + actionable hint / store error −32603 |
| F3 | Spawn gate admits a known-expired token (`store_has_login` never calls `is_stale`) | failure-hunter, type-analyzer | Bug | Yes — read code; gate checked rows-parse only | **Modify (absorb)** | = the "early stale detection" half of **cyril-taba** (reframed scope); implemented in the gate with injected `now` (the auth fixture's expiry is already past — a wall-clock gate test would be a time bomb). UI re-login affordance stays taba's scope; taba note updated |
| F4 | Locked/corrupt store misdiagnosed as "not logged in" at the gate | failure-hunter, code-reviewer S1, type-analyzer | Bug | Yes — `bool` gate discarded the reader's distinguished diagnostics | **Modify** | `store_has_login() -> bool` replaced by `store_unservable_reason() -> Option<String>`; diagnostic threads into new `KasMissing::StoreUnservable { store, why }` |
| F5 | Known CLI version with no exact-match dir silently spawns newest-by-semver | failure-hunter | Bug | Yes — no log on that path (only the version-*unavailable* path warned) | **Accept** | warn! in `select_server` (invariant: exact-miss + known version ⇒ mismatch); covers both fresh-upgrade and partial-exact-dir scenarios |
| F6 | `NoHome` at the store gate tells users to set `KIRO_KAS_SERVER_PATH` they may already have set | type-analyzer, failure-hunter, code-reviewer S2 | Bug | Yes — reproduced by reading the override+no-home path | **Modify** | Distinct `KasMissing::NoHomeForStore` naming the credential store; fence asserts it does NOT mention the bundle override |
| F7a | Stale `.expect("override + token + node present")` ×2 (token precondition removed by this PR) | comment-analyzer | Comment | Yes — lines confirmed | **Accept** | Both now "override + node present" |
| F7b | Doc claims all `read_sqlite_store` callers spawn_blocking-wrap it; the spawn gate calls it directly | comment-analyzer | Comment | Yes — gate call is direct, on the bridge spawn path | **Accept** | Reworded: live-callback wraps; startup-only gate calls direct |
| F7c | "slice-7 grep fence forbids resurrection" implies a standing guard; none exists in CI/tests | comment-analyzer | Comment | Yes — checked .github/workflows + tests | **Modify** | Made the claim true instead of softening it: executable `sso_token_path_never_resurrected` test scans `crates/**/*.rs` (needle via `concat!` so the test can't self-match) |
| F7d | `KasSpawn` type-level doc still says Free = zero-credential file-auth, contradicting its own variant doc | type-analyzer, comment-analyzer | Comment | Yes | **Accept** | Type doc now matches dcc6 semantics (both modes acp-callback) |
| F7e | "argv byte-identical to kiro-cli's own spawn" overstated — argv[0] node path is cyril's own | comment-analyzer | Comment | Yes — C8 test wording was already precise | **Accept** | Module doc now says non-path flags byte-equal |
| F7f | "refresh token never **read**" imprecise — the row containing it is SELECTed whole | comment-analyzer | Comment | Yes | **Accept** | Now "never extracted or transmitted" (the invariant the code enforces) |
| F8 | Internal tracker id "cyril-taba" shipped inside a user-facing runtime error string | comment-analyzer | Style | Yes — auth.rs error literal | **Accept** | Dropped from the wire string; doc comment keeps the pointer |
| F9 | Discovery candidates modeled as `(String, bool)` tuples, version parsed inside the policy | type-analyzer | Design | Yes — real, but zero-defect refactor of heavily-fenced code | **Reject (defer)** | Tracked at **cyril-5db7** (type-shape cleanup) |
| F10 | `KasMissing` not thiserror; `read_sqlite_store` errors are `String`; tests substring-match error text | type-analyzer, code-reviewer S4 | Design | Yes — pattern predates this PR (KAS-1) | **Reject (defer)** | Tracked at **cyril-5db7** |
| F11 | `(u32,u32,u32)` semver tuple escaped version.rs without a newtype | type-analyzer | Design | Yes | **Reject (defer)** | Tracked at **cyril-5db7** |
| F12 | `entry.ok()?` / `into_string().ok()?` silently drop kas-root entries (could be the exact-match dir) | failure-hunter, code-reviewer S3, type-analyzer | Bug (minor) | Yes | **Accept** | debug! per dropped entry, matching the file's own log standard |
| F13 | Unreadable kas root logged at debug only; the Server reason's "self-extract" remedy is wrong for EACCES | failure-hunter | Bug (minor) | Yes | **Accept** | Promoted to warn! carrying the real cause; the (static) Server reason unchanged — the log line disambiguates |
| F14 | `unwrap_or_default()` guards an invariant that always holds; if broken would silently yield empty entries | failure-hunter | Style | Yes — else-branch home provably `Some` | **Accept** | Restructured as a `match` binding home structurally; also completes the half-explained skip-condition comment (comment-analyzer #7) |
| F15 | Unmatched ext requests get success-shaped `Ok(null)` with zero log — a KAS method rename fails invisibly | failure-hunter | Bug (minor) | Yes — pre-existing shape, stakes raised by this PR | **Accept** | debug! names the method in both cfg variants; protocol-default null behavior unchanged |
| F16 | Hardcoded `~/.local/share/kiro-cli` may diverge from kiro-cli if it honors `XDG_DATA_HOME` | failure-hunter, code-reviewer S5 | Bug claim | **Partially** — the string exists in the 2.11.0 binary (3 hits, dirs-crate pattern) but the kas/store paths weren't probed | **Reject (defer)** | Needs a live extraction/login probe; tracked at **cyril-tpwn**. F19b's shared const makes the eventual fix one line |
| F17a | Corrupt sqlite *file* and schemaless db untested (JSON-in-valid-db was the only corruption tested) | test-analyzer | Test gap | Yes | **Accept** | Garbage-bytes + schemaless fences; both assert no fake-logout diagnosis |
| F17b | Token-row `field()` guards (missing/empty access_token, expires_at) never exercised | test-analyzer | Test gap | Yes | **Accept** | Fence added for both fields |
| F17c | Login-gate wiring in `resolve_kas_command` (ordering, applies-with-override) only proven live | test-analyzer | Test gap | Yes — fn reads real env/HOME | **Reject (defer)** | Needs injectability refactor; tracked at **cyril-5db7** item (5). Both halves are unit-fenced; composition stays live-smoke-covered |
| F17d | Pre-release dir grammar + lexically-greater-partial-duplicate untested | test-analyzer (nice-to-have) | Test gap | Yes | **Accept** | One line each in existing fences |
| F18 | `KasMissing::Server` reason hardcodes `--agent-engine v3`; the flag is `kas` on 2.7.1 | comment-analyzer | Style | Yes — `flag_for_version` exists for exactly this | **Reject** | Wrong-sized: affects 2.7.1 only (also: 2.7.1 self-extracts at install, so the remedy is rarely needed there), and the message's second remedy (`KIRO_KAS_SERVER_PATH`) is version-independent. Threading a version into a static reason isn't worth the plumbing |
| F19a | JSON-RPC codes −32603/−32000 repeated as magic numbers ×4 | type-analyzer | Style | Yes | **Accept** | Named consts (fell out of the F1/F2 refactor) |
| F19b | `.local/share/kiro-cli` spelled independently in two constants | type-analyzer | Style | Yes | **Accept** | `KIRO_DATA_DIR_REL` single spelling + drift-fence test |
| F19c | Two inline comments verbatim repeat the doc comment above them | comment-analyzer | Polish | Yes | **Accept** | Removed |
| F19d | `Clone` derive on `AccessToken` unused — needless secret copy surface | type-analyzer | Polish | Yes — grep: zero uses | **Accept** | Removed |
| F19e | Live-smoke sentinel appears in the prompt; a prompt-echoing error turn could still pass | test-analyzer (wouldn't block) | Polish | Plausible but contrived | **Reject** | The failure requires KAS to echo the exact literal inside an *error* turn's AgentMessage; smokes are manual `#[ignore]` diagnostics, and the accumulate-then-assert hardening already defeats the observed (l7tw) failure mode |

## Cross-reviewer convergences that drove the fix shape

- Three agents independently hit the `store_has_login`/gate region (F3/F4/F6) — one
  refactor (`store_unservable_reason` + `StoreUnservable`/`NoHomeForStore`) closes all three.
- Both Criticals (F1/F2) shared a root cause — the responder was an unparameterized
  free function — so one extraction bought observability, testability, and the
  error-code fence together.

## Deferral ledger (hard-gate requirement)

| Deferred | Tracker |
|---|---|
| App-notification for callback failures (F1 half) | cyril-l7tw (pre-existing; note added) |
| UI re-login affordance on stale (F3 remainder) | cyril-taba (pre-existing; note added — early-detection half absorbed here) |
| thiserror/StoreError/ExtractionDir/KiroVersion/gate-injectability (F9/F10/F11/F17c) | cyril-5db7 (filed) |
| XDG_DATA_HOME probe (F16) | cyril-tpwn (filed) |
