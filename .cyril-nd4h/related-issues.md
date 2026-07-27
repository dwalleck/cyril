# cyril-nd4h — prior art (tracker sweep, step 0)

Searched all 140 issues in `.rivets/issues.jsonl` (note: `rivets list` caps at
50 and has no `--search`; the JSONL is the source of truth). Keywords:
`highlight_cache`, `stream_buffer`, `mouse_capture`, `UiConfig`, `config.toml`,
`max_messages`, `HashCache`, `StreamBuffer`, `dead code`, `unused`.

## The decisive precedent — cyril-85py (closed)

`SessionController.steering_depth`: same shape as this ticket — a field
maintained by production code with **zero non-test readers**, filed with the
same explicit fork ("either (a) delete it + its tests, or (b) wire it up if a
future consumer needs it. Decide when touched").

**The project chose (a), delete.** Close note: *"steering_depth deleted
(write-only, zero readers — grep-verified; stale state.rs comment rewritten)."*

This is the house answer to "remove or honor" absent a concrete consumer, and
it is the strongest single input to this ticket's design decision. Note the
asymmetry, though: `steering_depth` was **internal** state with no user-facing
surface, whereas the nd4h fields are **serialized in a user's `config.toml`
and documented in AGENTS.md** — deleting them is a compat event, which is
exactly why AC #2 exists here and did not exist there.

## Same-review sibling with the identical shape — cyril-1h0v (open, P3)

Review finding #17 to this ticket's #5, same `discovered-from cyril-ghuu`
parent, same verbiage: *"Decide compatibility/deprecation, then wire or remove
the API and stale documentation."* AC #1 there is *"Public API compatibility is
explicitly decided"*; AC #2 is *"Documentation matches production behavior."*

**Constraint:** whatever compat posture nd4h establishes (hard removal vs.
deprecation window vs. warn-on-ignored) becomes the precedent 1h0v inherits.
Decide it once, deliberately, and state it so 1h0v can cite it.

## Collision risk on the same cache sites — cyril-x5xi (open, P2)

Review finding #7: *"highlight and Markdown cache keys manually enumerate all
theme roles… Introduce dependency-appropriate structural cache identities."*

Those are the **same two `LazyLock<Mutex<HashCache>>` statics** that
`highlight_cache_size` would have to reach if this ticket picks "honor":
`highlight.rs:22` and `widgets/markdown.rs:20`. x5xi restructures the cache
*key/identity*; nd4h would change the cache *capacity*. Adjacent and separable,
but both land in the same few lines — sequencing matters. Also relevant:
cyril-c6la (#12) and cyril-zj0m (#13) touch markdown/diff caching.

## Batch context

`cyril-ghuu` spawned 16 tracked findings. The three P1s were #1 (leiq), #6
(q9dx), and 60×16 usability (a14l) — **all closed**. cyril-nd4h (#5) is the
last open P1 of the batch.

## No prior art found for

No issue other than nd4h itself mentions `highlight_cache_size`,
`stream_buffer_timeout_ms`, or `mouse_capture`. No existing ticket covers
`StreamBuffer` being unreachable from production. If the probe confirms that,
it is a **new finding** and needs its own ticket (dead component, not merely an
unwired config field).
