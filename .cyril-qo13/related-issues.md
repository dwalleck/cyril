# cyril-qo13 — prior art (tracker search, 2026-07-02)

Searched `.rivets/issues.jsonl` for `user_input`, `optionId`, `permission`, `consent`.

- **cyril-qo13** (this issue) — the bug itself; already carries two scope-widening notes
  (consent `_meta` echo, engine-conditional trust shapes).
- **cyril-0o7e** (open) — KAS `session_info_update` kinds dropped, including
  `pending_interaction` — the KAS-native mirror of the same user_input questions this
  issue covers. Related but distinct surface; the permission request is the actionable path.
- **cyril-7bdu** (closed) — KAS-5 host I/O callback responders; established `convert/kas.rs`
  as the KAS seam this fix should follow.
- **cyril-atjw** (closed) — KAS-0 engine trait; engine-conditional behavior precedent.
- **cyril-0v42** (open) — KAS fs_write atomicity; unrelated mechanism, same trace.

No pre-existing issue duplicates the optionId bug. Proceeding.
