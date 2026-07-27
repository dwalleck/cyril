# cyril-nd4h — prove-it-prototype findings

## What I learned that I did not know before probing

**The ticket understates its own defect in three ways: three of four `UiConfig`
fields are ignored (not the two named), `stream_buffer_timeout_ms` has no
runtime component to configure at all because `StreamBuffer` is unreachable
dead code, and the fourth field `mouse_capture` hides behind a substring
collision that a naive grep reports as 17 live references when the true count
is zero.**

## Slice 1 — which fields have a production consumer?

- **Probe** (`probe.py`, 74 lines): the *compiler* as instrument. Rename one
  field at a time inside `config.rs` only (file stays self-consistent), run
  `cargo check --all-features --all-targets --message-format=json`, and treat
  any error with a primary span outside `config.rs` as proof of a consumer.
  Semantic — sees through `..Default::default()`, aliases, and `cfg` gates.
  `--all-features` is load-bearing: a consumer behind `#[cfg(feature = "kas")]`
  would otherwise read as absent (cf. cyril-ykkc).
- **Oracle**: word-boundary textual scan (`grep -rnw`) over `crates/`,
  excluding the definition site. Fails in completely different ways than a
  type checker does.

| Field | Probe (compiler) | Oracle (grep -w) | Verdict |
|---|---|---|---|
| `max_messages` | 2 errors @ `main.rs:72` | 8 refs, full chain | **honored** |
| `highlight_cache_size` | 0 | 0 | ignored |
| `stream_buffer_timeout_ms` | 0 | 0 | ignored |
| `mouse_capture` | 0 | 0 strict / **17 naive** | ignored |

**AGREE on all four.** `max_messages`'s live chain is `main.rs:72` →
`App::new` → `UiState::new` → eviction at `state.rs:1859`.

The `mouse_capture` row is why the oracle had to be independent: a naive
substring grep returns 17 hits — all of them `mouse_captured`,
`set_mouse_captured`, `toggle_mouse_capture`, which are *different
identifiers*. Naive-grep would have scored this field "honored."

## Slice 2 — what value does the runtime actually use?

- **Probe** (`probe2.py`): behavioral. Writes a temp integration test that runs
  the real `HashCache`, inserts 1000 entries, and measures the high-water mark
  of retained entries. Then removes the test (`git status` verified clean).
- **Oracle**: the source literals and the prose docs — textual, independent of
  execution.

```
NDPROBE cap=20  inserted=1000 peak_held=20  final=20
NDPROBE cap=256 inserted=1000 peak_held=256 final=232
```

| Field | Documented | Actual runtime | Gap |
|---|---|---|---|
| `highlight_cache_size` | 20 (`config.rs:33`, `codebase_info.md:55`) | **256**, measured; literal at `highlight.rs:22` *and* `widgets/markdown.rs:20` | **12.8×** |
| `stream_buffer_timeout_ms` | 150 (`config.rs:34`, `AGENTS.md:160`) | **no effective value** — `StreamBuffer` is never constructed | n/a |
| `mouse_capture` | true (`AGENTS.md:160`) | hardcoded `true` @ `app.rs:81` | coincides ⇒ silent |

**AGREE**: measured behavior (256) matches the source literal (256), so the
*documentation* is what is wrong. `final=232 < peak=256` is the half-eviction
in `cache.rs:28` (`drain(..order.len()/2)`), not noise.

## Findings beyond the ticket text

1. **`mouse_capture` is a third ineffective field.** The ticket names two. AC #1
   ("no documented UI config field is silently ignored") covers three. Setting
   `mouse_capture = false` in `config.toml` today does nothing.
2. **`StreamBuffer` is dead, not merely unconfigured.** Zero references outside
   `stream_buffer.rs` and its `pub mod` line in `lib.rs`. "Honor
   `stream_buffer_timeout_ms`" is not a wiring job — there is no live consumer
   to wire it to. Needs its own ticket (no existing one; see
   `related-issues.md`).
3. **`highlight_cache_size` cannot map 1:1 onto production.** There are *two*
   independent 256-entry caches (highlight + markdown). One scalar cannot
   express two capacities without inventing a policy.
4. **`codebase_info.md:55` calls it an "LRU".** `HashCache` is not an LRU: it is
   insertion-order with oldest-half bulk eviction (`cache.rs:26-36`). A third
   documentation defect, independent of the value being wrong.

## Substrate check

No cause-1 disagreement (nothing broken underneath). The probes agree with the
oracles on every slice; the divergence is between **code and its
documentation**, which is precisely the ticket's subject rather than an
obstacle to it. Safe to proceed to design.

## Gate

- [x] Probe written, runs against the real codebase (`probe.py`, `probe2.py`)
- [x] Oracle defined and produces output (word-boundary grep; source literals)
- [x] Probe and oracle agree on ≥1 non-trivial slice (both slices)
- [x] One-sentence learning recorded (top of this file)
