# cyril-nd4h — falsifiable design

Input: the probe (`.cyril-nd4h/findings.md`). The design may not contradict it.

## The decision this design makes

The ticket says "choose removal/deprecation **or** wire them through." The probe
says the answer is not uniform across the three ineffective fields, so this
design **splits** them:

| Field | Decision | Why |
|---|---|---|
| `mouse_capture` | **Honor** | 2-site wire-through; a real user need (mouse capture hijacks terminal text selection); no collision with other tickets |
| `highlight_cache_size` | **Remove** | Cannot map 1:1 — *two* independent caches, and `HashCache` eviction is oldest-half sawtooth, so a "size" knob would misrepresent behavior. Wiring it also collides with cyril-x5xi |
| `stream_buffer_timeout_ms` | **Remove** | Nothing to wire to: `StreamBuffer` is unreachable dead code. The component's fate is cyril-ell0 |

Precedent: cyril-85py deleted a zero-reader field outright. This design follows
it for the two dead knobs but departs for `mouse_capture`, because unlike
`steering_depth` (internal state) these are **user-facing serialized config**,
and `mouse_capture` is the one where a consumer is both cheap and wanted.

## Architecture

Single source of truth for the startup mouse mode:

```
config.ui ──(exhaustive destructure, no `..`)──> App::new
                                                    │ sets
                                                    ▼
                                          ui_state.mouse_captured
                                                    │ read back by
                                                    ▼
                              main.rs: if app.mouse_captured() { EnableMouseCapture }
```

`main.rs` **derives** the terminal action from `App`'s state instead of reading
`config.ui.mouse_capture` a second time. Two independent reads is exactly the
shape that produces the inverted-toggle bug the existing `app.rs:80` comment
warns about; deriving makes the desync unrepresentable rather than merely
tested-against.

`App::new(bridge, &config.ui, cwd)` replaces the `max_messages: usize` scalar
and destructures `UiConfig { max_messages, mouse_capture }` with **no `..`**, so
a future field cannot be added without the compiler demanding a consumption
decision at this seam. (Same structural idea cyril-x5xi asks for on cache
identity: "adding a relevant field cannot silently bypass".) Requires a narrow
`App::mouse_captured()` accessor — `ui_state` is private and stays private.

## Input shapes

Feature input = the user's `config.toml`, via `Config::load_from_path`.
Established by probe: that function **never fails** — missing, unreadable, and
malformed all return `Self::default()` with a `warn!`; and `Config` is **never
serialized back to disk** in production (the sole `toml::to_string` is inside
`#[cfg(test)]`).

| # | Shape | Covered by |
|---|---|---|
| S1 | File absent (`NotFound`) | C1 |
| S2 | File unreadable (permissions) | C10 |
| S3 | Malformed TOML | C10 |
| S4 | Empty file | C1 |
| S5 | `[ui]` present but empty | C1 |
| S6 | `mouse_capture = false` | C2 |
| S7 | `mouse_capture = true` | C1 |
| S8 | Legacy file naming the removed keys | C5 |
| S9 | Wrong-typed value (`mouse_capture = "yes"`) | C10 (whole-file parse error ⇒ defaults) |
| S10 | Unknown key never known to cyril | C5 |
| S11 | `max_messages = 0` | **out of scope** — pre-existing boundary semantics of an already-honored field; this ticket does not change `max_messages` |

`bool` is the only new value domain (`mouse_capture`), and both variants are
enumerated (S6, S7).

## Removed-invariant sweep (change is subtractive)

**Constraint being removed:** "at startup, `ui_state.mouse_captured` is
unconditionally `true`", which made it *trivially* agree with `main.rs`'s
unconditional `EnableMouseCapture`.

Facts that constraint guaranteed for free, and their post-change status:

| Invariant | Still holds? |
|---|---|
| I1 — UiState flag == terminal's real mouse mode at startup | **At risk.** Two sites must move together. Claim C3 makes it structural (single read); C2 tests it |
| I2 — the first Ctrl+M press always produces a visible change | **At risk**, downstream of I1. Claim C4 |
| I3 — exit path `DisableMouseCapture` (`main.rs:98`) is correct | **Safe.** Disabling capture that was never enabled is a no-op on the terminal; runs unconditionally either way |
| I4 — removed struct fields were accepted by the deserializer | **Safe, and verified.** No `deny_unknown_fields` anywhere; C5 ran and passed |
| I5 — a user's config file is never rewritten by cyril | **Safe.** No production serialization path, so removal cannot silently delete a user's keys from their file |

## Claims

1. With no config file, or `mouse_capture` absent, cyril starts with mouse capture **enabled** (today's behavior preserved).
2. With `mouse_capture = false`, `App`'s initial UI state reports mouse **not** captured.
3. The terminal's startup mouse mode is **derived from `App`'s state**, not from a second independent read of the config.
4. Ctrl+M toggles correctly from **either** starting state — the first press always changes the mode.
5. A `config.toml` still naming the removed keys loads successfully, with every surviving field taking its file-specified value.
6. Adding a field to `UiConfig` **cannot compile** unless a production consumption site handles it.
7. After the change, **every** `UiConfig` field has ≥1 production consumer.
8. Removing the two fields leaves runtime behavior unchanged — the highlight and markdown caches still hold **256** entries.
9. Docs name exactly the surviving fields with their true effective values, and no longer describe `HashCache` as an "LRU".
10. Malformed / unreadable config still falls back to defaults rather than failing startup (unchanged).

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|---|---|---|---|---|---|
| 5 | Legacy config parses | Config naming removed keys **plus `max_messages = 999`**; 999 ⇒ parsed-and-ignored, 500 ⇒ silently fell back | The TOML file's literal text, independent of the code path | 5m | **passed** | integration test `nd4h_legacy_config_compat` (promote `falsifier-c5.py`) |
| 2 | `false` is honored | Write `mouse_capture = false`, build `App`, assert `mouse_captured() == false` | The TOML literal | 5m | pending | unit test `app::tests::mouse_capture_false_honored` — **fails on today's code** (`set_mouse_captured(true)` hardcoded), so it is a true sentinel |
| 1 | Default stays on | Config with no `mouse_capture`; assert `mouse_captured() == true` | `UiConfig::default()` literal, read independently | 5m | pending | unit test `app::tests::mouse_capture_defaults_on` |
| 4 | Toggle correct from either start | Drive Ctrl+M from initial `false` (expect `true`) and from `true` (expect `false`) | Independently computed `!initial` | 10m | pending | unit test `app::tests::ctrl_m_toggles_from_either_start` |
| 10 | Malformed ⇒ defaults | Feed invalid TOML and a wrong-typed value; assert defaults, no panic | `UiConfig::default()` literals | 5m | pending | existing `config.rs` load tests, extended |
| 8 | Caches unchanged at 256 | Re-run `probe2.py` post-change; `peak_held` must still be 256 | Behavioral execution of real `HashCache`, independent of config code | 5m | pending | source-scan test asserts both statics still read `HashCache::new(256)` |
| 3 | Single read, not two | Scan `main.rs`: the `EnableMouseCapture` branch must not reference `config.ui.mouse_capture` | Source text (structural), independent of runtime | 5m | pending | source-scan test `nd4h_single_mouse_read` |
| 6 | New field can't slip in | Add a dummy field to `UiConfig`; `cargo check` must **fail** at the destructure | The compiler, independent of our tests | 10m | pending | source-scan asserts the destructure contains no `..` |
| 9 | Docs match reality | Grep `AGENTS.md` + `.agents/summary/` for removed names and for "LRU" | grep, independent of code | 5m | pending | source-scan test `nd4h_docs_match_config` |
| 7 | No ignored fields remain | Re-run `probe.py`; every field must report `consumers > 0` | The compiler (rename-mutation), independent of the test suite | 15m | pending | structural fence is C6's destructure; `probe.py` rerun is the one-shot audit |

**Non-vacuity** — the buggy implementation each fence catches:

- **C2**: today's `set_mouse_captured(true)`. Pre-fix fails, post-fix passes.
- **C1**: an impl that defaults the field to `false`, or inverts the bool.
- **C3**: an impl where `main.rs` does `if config.ui.mouse_capture { … }` while `app.rs` separately calls `set_mouse_captured(config.ui.mouse_capture)` — passes C1 **and** C2, fails only C3. This is the desync-prone shape, which is why C3 is not redundant with C1/C2.
- **C4**: startup flag `true` while the terminal never got `EnableMouseCapture` — first Ctrl+M appears dead.
- **C6**: a destructure written with `..`.
- **C8**: an impl that "helpfully" wires the removed default (20) into the caches before deleting the field, dropping 256 → 20.
- **C9**: a change that edits code but forgets `.agents/summary/codebase_info.md:55-57`.
- **C10**: switching `load_from_path` to `?`/panic on parse error.

**Source-scan fences carry a known hazard:** cyril-xi4a (closed, P1) was exactly
a source-scanning test breaking on CRLF checkouts and reddening Windows CI. Any
scanner added here must normalize line endings.

## Negative space — what this deliberately does NOT do

1. **Does not delete or resurrect `StreamBuffer`.** Removing the config knob and
   deciding the dead module's fate are separate calls — tracked at **cyril-ell0**
   (filed during this design; `discovered-from cyril-nd4h`).
2. **Does not make cache capacity configurable.** Settled rationale, not a
   deferral: there are two independent caches and eviction is oldest-half
   sawtooth, so a single "size" number would describe behavior that does not
   exist. Re-introducing the knob would need a policy design first.
3. **Does not restructure cache keys or identity** — that is **cyril-x5xi**
   (verified open, P2), which edits the same two `LazyLock` statics. This design
   touches neither, so the two can land in either order.
4. **Does not add a deprecation warning for removed keys.** serde ignores unknown
   keys silently and cyril never writes config back (I5), so there is no
   corruption or data-loss risk; a warn-on-unknown would also fire for ordinary
   typos and for every future key. Settled rationale.
5. **Does not change `max_messages`** semantics, including its `0` boundary (S11).
6. **Does not set the compat posture for cyril-1h0v** (verified open, P3, the
   same review's finding #17). That is a dead *public Rust API*, a different
   surface from serialized user config; this design records the precedent but
   does not bind it.

## Tracker references (all verified present)

cyril-x5xi (open) · cyril-1h0v (open) · cyril-85py (closed, precedent) ·
cyril-xi4a (closed, CRLF hazard) · cyril-ykkc (open, `--features kas` gate) ·
cyril-ghuu (parent review) — all verified via `rivets show` from this branch.

**cyril-ell0** (filed during this design) verifies from the **primary checkout
on `main`**, not from this branch: per the ship convention this branch
deliberately does not carry `.rivets/issues.jsonl` updates, so its tracker
snapshot is pinned at `d21fbfc`. Confirmed present on main:
`○ cyril-ell0: Dead stream_buffer module: StreamBuffer has zero production
consumers` (chore, P3, `discovered-from cyril-nd4h`).
