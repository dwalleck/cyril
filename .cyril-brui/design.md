# Design: cyril-brui — KAS discovery honours `XDG_DATA_HOME`

## Route and inputs

- Route: **Empirical** (`route.md`, T1 = yes). `spec.md`: N/A — behavior fully explicit; the complete given/when/then set is `route.md` T4 (reproduced as claims C1–C6 below).
- Empirical premises: `evidence.md` P1 (absolute XDG wins, `$HOME/.local/share` untouched) and P2 (empty/relative XDG → `$HOME/.local/share`), both `PASS` — dynamic strace probe vs. static `dirs`/`dirs-sys` source + binary string table.
- Requester decisions (relayed verbatim by the orchestrator; see Approval): resolve `$XDG_DATA_HOME/kiro-cli` when set, non-empty and absolute, else `$HOME/.local/share/kiro-cli`, for BOTH the KAS root and the credential store; invalid (empty or relative) XDG falls back to HOME with a logged warning naming the value; keep the F19b drift pin between root and store at the new resolution level; unit tests for both branches and the invalid fallback for both paths; kas feature gate legs mandatory.

## Input shapes

Inputs of the new resolver `kiro_data_dir(home: Option<&Path>, xdg_data_home: Option<&OsStr>)`:

| Shape | Status |
|---|---|
| `xdg = None`, `home = Some(abs)` | C2 |
| `xdg = None`, `home = None` | C2 (returns `None` → `KasMissing::NoHome` / `NoHomeForStore`, unchanged) |
| `xdg = Some(absolute, non-empty)`, `home = Some(..)` | C1 |
| `xdg = Some(absolute)`, `home = None` | C1 (XDG alone suffices) |
| `xdg = Some("")` (set, empty) | C3 |
| `xdg = Some("   ")` (whitespace — not absolute) | C3 |
| `xdg = Some("relative/data")` | C3 |
| `xdg = Some(invalid)`, `home = None` | C3 (warns, then `None`) |
| `xdg` absolute with spaces / Unicode (`/tmp/ü data`) | C1 (`Path::is_absolute` is byte-agnostic; `PathBuf::join` preserves) |
| Windows drive-relative (`C:foo`) / UNC | N/A — reason: `XDG_DATA_HOME` is only consulted by kiro-cli's Linux `dirs` build (`lin.rs`); on Windows cyril's KAS spawn is the WSL/native launcher path and the store-location gap is tracked separately as `cyril-lwpm` (intended future work, verified ID) |

Decision cells:

| Cell | Status |
|---|---|
| Predicate "XDG valid" = `Path::new(v).is_absolute()` (empty and relative both fail it) — every production condition arrives as: unset (`None`), valid absolute, invalid (empty / whitespace / relative) | C1, C2, C3 |
| Warning on invalid: exactly one `WARN` per resolution, field `value` = the offending `OsStr` (Debug), message names `XDG_DATA_HOME` | C3 |
| Consumers of the data dir: KAS root (`<data dir>/kas`) and store (`<data dir>/data.sqlite3`) derive from ONE resolution | C4, C5, C6 |
| `KasMissing` taxonomy unchanged: `NoHome` when neither valid XDG nor home (root side), `NoHomeForStore` (store side) | C2 (existing fences `resolve_no_home_no_override_errors`, `missing_reasons_are_actionable` retained) |

## Placement

- Capability: "resolve kiro-cli's data dir with kiro-cli's own semantics". **Owner:** `cyril-core/src/protocol/kas/discovery.rs` — it already owns both consumers (`KAS_ROOT_REL`, `default_store_path`). **New seam:** none — a private pure fn `kiro_data_dir(home, xdg)` plus a private env-reading wrapper `kiro_data_dir_from_env()`, mirroring the existing `kiro_agent_config::resolve_in_dirs` convention ("injectable inputs so tests don't mutate HOME — env mutation is `unsafe` in Rust 2024, forbidden by the workspace"). `pub(crate) default_store_path()` keeps its signature. **Forbidden:** no second env read of `XDG_DATA_HOME` anywhere else in the crate (grep fence: only `discovery.rs` reads it in `cyril-core`); no dependency on `cyril-memory::paths` (different crate, different semantics — its resolver returns `Err` on relative HOME; discovery keeps the `Option` contract); no change to `kiro_agent_config::home_dir()`.
- Constants: `XDG_DATA_HOME_DEFAULT_REL = ".local/share"`, `KIRO_DATA_DIR_NAME = "kiro-cli"`, `KAS_ROOT_REL = "kas"` (now data-dir-relative), `STORE_FILE_NAME = "data.sqlite3"`; `KIRO_DATA_DIR_REL` is replaced by the resolver (its only reader was `default_store_path` and the F19b pin, both re-homed).

## Module shape

N/A — `route.md` T2: ownership, interfaces, and dependency direction unchanged; no length-review trigger (no repository length gate; +≈40 production lines inside the existing responsibility).

## Claims

- C1 — `kiro_data_dir(home, Some(v))` with `v` absolute and non-empty returns `v/kiro-cli` for any `home`, including `None`.
- C2 — `kiro_data_dir(Some(h), None)` returns `h/.local/share/kiro-cli`; `kiro_data_dir(None, None)` returns `None` (today's behavior preserved, so `NoHome`/`NoHomeForStore` still fire).
- C3 — `kiro_data_dir(home, Some(v))` with `v` empty, whitespace-only, or relative returns the C2 result for `home` AND emits exactly one `WARN` event whose `value` field carries `v` and whose message names `XDG_DATA_HOME`.
- C4 — `resolve(Some(data_dir), …)` searches `data_dir/kas` (versioned dirs, then legacy) and a missing bundle reports `KasMissing::Server(data_dir/kas/<SERVER_IN_ROOT_REL>)`; `resolve_kas_command` lists `kiro_data_dir_from_env()/kas`.
- C5 — `store_path(&data_dir)` (the pure core of `default_store_path`, which wraps `kiro_data_dir_from_env()`) returns `data_dir/data.sqlite3`; `kas_root(&data_dir)` returns `data_dir/kas`.
- C6 — (F19b at the new level) on every env branch `kas_root(&kiro_data_dir(h,x))` and `store_path(&kiro_data_dir(h,x))` have the same parent, which equals `kiro_data_dir(h,x)`; in production `resolve_kas_command` derives root, `resolve` input and store from ONE `kiro_data_dir_from_env()` value and `default_store_path` wraps the same resolver (receipt: grep `kiro_data_dir_from_env()` = definition + exactly two call sites).

## Falsification

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C1 | XDG absolute wins | `xdg=/xdg`, `home ∈ {Some(/home/u), None}`; `/tmp/ü data` | Call the resolver with each input; expect `/xdg/kiro-cli` (resp. `/tmp/ü data/kiro-cli`); any path under `/home/u` or `None` falsifies. Other cause ruled out: `home=None` leg proves the result cannot come from the home branch. | `evidence.md` P1 — strace of the real launcher opening `<xdg>/kiro-cli/data.sqlite3` with zero `$HOME/.local/share` traffic (dynamic, external) | `discovery.rs kiro_data_dir`: delete the `if let Some(value) = xdg_data_home` branch (always take the home fallback) → red: `assert_eq!(…, "/xdg/kiro-cli")` prints `Some("/home/u/.local/share/kiro-cli")` / `None` | `protocol::kas::discovery::tests::kiro_data_dir_prefers_absolute_xdg_data_home` | seconds | PENDING — checkpointed-build, Slice 1 gate |
| C2 | default preserved | `xdg=None`, `home ∈ {Some, None}` | Expect `/home/u/.local/share/kiro-cli` and `None`; any other path falsifies. | Baseline fence at `f9bc81d8`: `kas_root_shares_kiro_data_dir` pinned `.local/share/kiro-cli` as the prefix (run in the falsifier log) + `dirs` source `h.join(".local/share")` | `discovery.rs`: change `XDG_DATA_HOME_DEFAULT_REL` to `".local/state"` → red on the expected-path assert | `…::tests::kiro_data_dir_defaults_to_home_local_share_without_xdg` | seconds | PASS (baseline) → re-discharged in Slice 1 |
| C3 | invalid XDG → home + one WARN naming it | `xdg ∈ {"", "   ", "relative/data"}` × `home ∈ {Some(/home/u), None}` | Under `capture_json_subscriber`, call the resolver; expect the C2 path and exactly one `WARN` whose `fields.value` Debug-contains `v` and whose `message` contains `XDG_DATA_HOME`. Zero events, two events, or a path under the relative value falsifies. Positive control for the absence half: the C1 leg emits zero WARNs under the same capture. | `evidence.md` P2 (strace: relative/empty → `<home>/.local/share/kiro-cli` created) for the path; for the warning, the requester's decision (behavioral requirement) | (a) `discovery.rs kiro_data_dir`: delete the `tracing::warn!` → red: WARN count 0 ≠ 1; (b) replace `xdg.is_absolute()` with `true` → red: path is `relative/data/kiro-cli` | `…::tests::invalid_xdg_data_home_falls_back_to_home_with_warning` | seconds | PENDING — checkpointed-build, Slice 1 gate |
| C4 | KAS root under the data dir | `data_dir=/xdg/kiro-cli`, empty listing, no override, node on PATH | `resolve(Some(/xdg/kiro-cli), None, None, PATH, &[], None, exists={node})` → `Err(Server("/xdg/kiro-cli/kas/node_modules/@kiro/agent/dist/server/acp-server.js"))`; `kas_root_from(Some(/home/u), Some(/xdg)) == /xdg/kiro-cli/kas`. A path under `/home/u`, or `NoHome`, falsifies. | On-disk layout of the real extraction root `~/.local/share/kiro-cli/kas/<semver>-<sha>/…` (`ls`, evidence P3 note) — the `kas` segment and `SERVER_IN_ROOT_REL` are what the resolver must reproduce under the new prefix | `discovery.rs resolve`: `let root = data_dir…join(XDG_DATA_HOME_DEFAULT_REL).join(KAS_ROOT_REL)` (re-derive from home semantics) → red: `Server(/xdg/kiro-cli/.local/share/kas/…)` ≠ expected | `…::tests::kas_root_and_store_follow_xdg_data_home` (+ retained `nothing_found_names_search_root`, `versioned_beats_legacy`, `override_beats_versioned` updated to the data-dir parameter) | seconds | PENDING — checkpointed-build, Slice 1 gate |
| C5 | store under the data dir | `data_dir ∈ {kiro_data_dir(/home/u, None), kiro_data_dir(/home/u, /xdg)}` | `store_path(&data_dir)` → `/home/u/.local/share/kiro-cli/data.sqlite3` and `/xdg/kiro-cli/data.sqlite3`; anything else falsifies. | `evidence.md` P1/P2: the launcher's `openat` target file name `data.sqlite3` directly under the resolved dir; `auth.rs` store reader (`get_access_token_from`) is the consumer | `discovery.rs store_path`: `data_dir.join(KIRO_DATA_DIR_NAME).join(STORE_FILE_NAME)` (stray extra segment) → red: `…/kiro-cli/kiro-cli/data.sqlite3` ≠ expected | same fence as C4 | seconds | PENDING — checkpointed-build, Slice 1 gate |
| C6 | root and store share the data dir on every branch | `(home, xdg) ∈ {(Some,None), (Some,Some abs), (None,Some abs)}` | `kas_root(&d).parent() == store_path(&d).parent() == Some(d)` with `d = kiro_data_dir(h,x)` for each; any mismatch falsifies. | Structural reading of the two production call sites (`resolve_kas_command` uses one `data_dir` value for root listing, `resolve` and the store gate; `default_store_path` wraps the same resolver) — grep `kiro_data_dir_from_env()` = definition + exactly two callers (checkpoint receipt) | same as C5's mutation (stray segment in `store_path`) → red on every branch: `store.parent()` is `d/kiro-cli`, not `d` | `…::tests::kas_root_shares_kiro_data_dir` (rewritten at the resolution level, name kept) | seconds | PENDING — checkpointed-build, Slice 1 gate |

Wrapper residual — **superseded by review round 1 (F1/F2, `review-decisions.md`)**. The original paragraph claimed the env wrappers `kiro_data_dir_from_env()` / `default_store_path()` "cannot be unit-fenced without env mutation"; that was false — the repository already re-enters the test binary with a private environment (`tests/spawn_isolation.rs`), and the reviewer's mutations R1 (wrong env var) and R2 (store gate re-derived from HOME) survived every fence above. Added claim:

- C7 — under a private `HOME`/`XDG_DATA_HOME`, `default_store_path()` returns `<data dir>/data.sqlite3` and `resolve_kas_command()` fails the login gate with `StoreUnservable { store: <data dir>/data.sqlite3 }` after finding the bundle under `<data dir>/kas`, where `<data dir>` is `<xdg>/kiro-cli` on the targets `dirs` routes to `lin.rs` (Linux, Android, the BSDs, illumos, Redox) and `<home>/.local/share/kiro-cli` on macOS/iOS/Windows, which never consult the variable (`dirs-6.0.0/src/lib.rs`; re-review N1 — `wasm32` is routed separately by `dirs` but is not a cyril target).

| # | Claim | Input shape | Falsifier | Oracle | Named mutation | Regression fence | Cost | Status |
|---|---|---|---|---|---|---|---|---|
| C7 | wrappers feed the resolver the real env, platform-gated like `dirs` | child process with `HOME=<tmpA>`, `XDG_DATA_HOME=<tmpB>`, `KIRO_AGENT_PATH=<fake node>`, `PATH` without kiro-cli, fake `2.24.0-<sha>` extraction under the host-expected data dir, no store | expect `default_store_path() == <data dir>/data.sqlite3` and `Err(StoreUnservable{store: <data dir>/data.sqlite3})`; a HOME-derived store on Linux, a `Server(..)` error (bundle looked up under the wrong prefix), or an XDG-derived path on macOS/Windows falsifies | `dirs-6.0.0/src/{lin,mac,win}.rs` (only `lin.rs` reads `XDG_DATA_HOME`) + `evidence.md` P1 | R1 `var_os("XDG_DATA_HOME")`→`"XDG_DATA_DIR"`; R2 store gate → `home_dir().join(".local/share/kiro-cli/data.sqlite3")`; F2-sym: the macOS/Windows `None` arm applied on every target | `…::tests::env_wrappers_resolve_xdg_data_home_like_kiro_cli` (Linux leg + macOS/Windows leg with the flipped expectation, CI-verified) | seconds | see `review-decisions.md` R-1 receipts |

What the issue's live acceptance run (`test_bridge --agent-engine kas` under `HOME=<tmp> XDG_DATA_HOME=~/.local/share`, still an **obligation not run**) now uniquely proves: that a *real* kiro-cli extraction layout and a *real* login store under the XDG dir are accepted end to end — the bridge spawns node on the resolved bundle and `_kiro/auth/getAccessToken` is served from that store — not the env plumbing, which C7 covers.

## Non-goals and future work

- Permanent non-goal: Windows / WSL-aware store location — the data dir on a WSL-located agent is inside the distro; that is `cyril-lwpm` (verified: `rivets show cyril-lwpm`, open), intended future work, not this change.
- Permanent non-goal: validating `HOME` itself (relative HOME) — `home_dir()` semantics are shared with agent-config resolution and untouched here.
- Permanent non-goal: type-shape cleanup of `KasMissing`/discovery (`cyril-5db7`, verified open) — not widened here.
- Permanent non-goal: changing `KasMissing::NoHome` / `NoHomeForStore` reason strings — they remain literally true (a missing home is the necessary condition on both branches); only their doc comments are updated to mention the XDG path.
- Related, already covered: `cyril-tpwn` asked for the probe of kiro-cli's `XDG_DATA_HOME` handling that `evidence.md` now records.

## Falsifier run log

- Cheapest falsifier: C2's baseline oracle — `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --features kas discovery` at `f9bc81d8` (before any edit): 24 passed incl. `kas_root_shares_kiro_data_dir` (pins `.local/share/kiro-cli` as the shared prefix). Log: `/tmp/brui-baseline.log`. **PASS** — the default branch the design preserves is what the repository ships today.
- Empirical premises P1/P2: `PASS` in `evidence.md` (probe vs. oracle agree on all three legs).

## Approval

Requester decisions, relayed verbatim by the orchestrator on 2026-09-27 as "the user's approved scope; treat them as the approved spec/design decisions the contract's approval semantics require":

> "In crates/cyril-core/src/protocol/kas/discovery.rs resolve the kiro data dir as `$XDG_DATA_HOME/kiro-cli` when XDG_DATA_HOME is set, non-empty and absolute, else `$HOME/.local/share/kiro-cli` — mirroring kiro-cli — for BOTH the KAS root (`KAS_ROOT_REL`, about lines 76/193/314) and the credential store path (`KIRO_DATA_DIR_REL`, about line 343). An invalid XDG_DATA_HOME (empty or relative) falls back to HOME with a logged warning naming the value (never silently). Keep the existing drift-pin test relationship between the two constants intact (update the test to the new resolution rather than breaking the pin)."
> "Fence: unit tests for both branches (XDG set / unset) and the invalid-XDG fallback, for both the KAS root and the store path. This code is behind the `kas` feature: the kas legs of the gate below are mandatory."
> "LIVE-CHECK CONSTRAINT (hard): … do NOT run any kiro-cli command that needs or refreshes auth … If the live acceptance check cannot be performed under this constraint, state that explicitly in route.md and the PR as an obligation not run, with the reason. Never mark it N/A and never claim it passed."

Risk acceptances approved: none requested. The wrapper residual above is not an `N/A — approved risk` row; it is carried as the live obligation not run, exactly as the constraint instructs.
