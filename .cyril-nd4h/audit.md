# cyril-nd4h — final audit (slice 8)

Both prove-it-prototype oracles re-run against the assembled tree.

## C7 — every `UiConfig` field has a production consumer

`probe.py` (compiler rename-mutation, field list derived from the struct):

```
UiConfig fields discovered: ['max_messages', 'mouse_capture']

max_messages     consumers=2
      crates/cyril/src/app.rs:74  struct `UiConfig` does not have a field named `max_messages`
      crates/cyril/src/app.rs:73  pattern does not mention field `max_messages_PROBEX`
mouse_capture    consumers=2
      crates/cyril/src/app.rs:75  struct `UiConfig` does not have a field named `mouse_capture`
      crates/cyril/src/app.rs:73  pattern does not mention field `mouse_capture_PROBEX`

PROBE VERDICT honored=['max_messages', 'mouse_capture']
PROBE VERDICT ignored=[]
AUDIT PASS
```

The second error on each field — `pattern does not mention field X_PROBEX` — is
**C6's exhaustive destructure firing**. The structural fence demonstrates itself
during the audit: renaming a field breaks the destructure precisely because it
carries no `..`.

### Two corrections the audit forced

1. **The probe's field list was hardcoded** and would have reported the two
   deleted fields as permanently "ignored" while never probing a new one — the
   same blind spot this ticket exists to close. Now derived from
   `pub struct UiConfig` in the source.

2. **The first re-run was a false pass.** With `--all-targets`, the only
   "consumers" reported were in `tests/nd4h_legacy_config_compat.rs`, never
   `app.rs`. Two compounding causes: a test that merely reads a field would
   count as a consumer, and cyril-core's own test target fails to compile first,
   so cargo never reaches the downstream `cyril` crate where the real consumers
   live. Dropping `--all-targets` scopes the check to production targets and
   answers the claim actually being made. `--all-features` is kept — a consumer
   behind `#[cfg(feature = "kas")]` or `"voice"` is still a production consumer
   (cyril-ykkc).

## C8 — the live caches are untouched

`probe2.py` (behavioral: runs the real `HashCache`, measures the high-water
mark):

```
NDPROBE cap=20  inserted=1000 peak_held=20  final=20
NDPROBE cap=256 inserted=1000 peak_held=256 final=232
```

Unchanged from the pre-implementation baseline in `findings.md`. The removed
knob's documented default (20) was not adopted anywhere; both statics still
construct at 256. `final=232 < peak=256` remains the oldest-half eviction in
`cache.rs:28`, not drift.

## Regression fences, all green

| Claim | Fence | Status |
|---|---|---|
| C1 | `app::tests::mouse_capture_absent_defaults_to_enabled` | pass |
| C2 | `app::tests::mouse_capture_false_is_honored` | pass (**verified RED pre-fix**) |
| C3 | `nd4h_source_fences::main_does_not_read_mouse_capture_directly` | pass |
| C4 | `state::tests::mouse_capture_toggles_from_either_starting_state` | pass |
| C5 | `nd4h_legacy_config_compat` (2 tests) | pass (**verified RED under `deny_unknown_fields`**) |
| C6 | `nd4h_source_fences::app_new_destructures_ui_config_exhaustively` | pass |
| C7 | `probe.py` audit above + `default_ui_config_schema_is_exactly_two_fields` | pass |
| C8 | `nd4h_source_fences::highlight_and_markdown_caches_still_hold_256` | pass |
| C9 | `nd4h_source_fences::docs_{do_not_advertise_removed_config_keys,do_not_call_the_hash_cache_an_lru}` | pass |
| C10 | `config::tests::wrong_typed_mouse_capture_falls_back_to_whole_file_defaults` | pass |

Non-vacuity was proven by mutation for the three load-bearing fences (C2, C4,
C5) — each was shown to go red under the specific bug it guards against, and
the source file restored byte-exact afterwards.

## Full gate

`cargo nextest run --all-features` — 1238 passed, 5 skipped ·
`cargo clippy --all-features --all-targets -- -D warnings` — clean ·
`cargo fmt --check` — clean.
