# cyril-8tq6 — related issues (prove-it step 0)

Tracker searched 2026-08-01 (`rivets list` filtered wsl/path/windows/fs/host-io).

| id | state | relation |
|---|---|---|
| cyril-7bdu | closed | **Parent** (discovered-from). KAS-5a host-io responders; its `to_native_checked` fix made WSL-internal paths fail *honestly* (-32603 NotFound) instead of -32602 "must be absolute" — but they still don't resolve. Probe capture `.cyril-7bdu/host_callbacks_2.10.0.json` is this issue's production-shape data. |
| cyril-lwpm | open P4 | Sibling: KAS auth on a Windows host — sqlite token store lives *inside* WSL. The `\\wsl$` translation built here is plausibly the substrate for a WSL-aware store path later. Do not scope-creep into it. |
| cyril-ihj1 | open P4 | Same module (`kas/host_io.rs`), orthogonal concern (bounded read). No interaction with path translation. |
| cyril-0v42 | closed | `write_atomic` (temp + fsync + rename) now runs on whatever `to_native_checked` returns — after this fix that can be a `\\wsl$\...` UNC directory. `tempfile_in` + `persist` on a UNC dir uses plain Win32 APIs and the temp lives in the target's own dir (same P9 filesystem), so no EXDEV; real-Windows proof is deferred with the rest of the on-host AC. |
| cyril-xi4a | closed | Evidence that **Windows CI exists and runs the test suite** — Windows-gated unit tests added here will actually execute in CI. |
| cyril-6bol | in_progress | Adjacent Windows-host KAS work (terminal shell_type hardcodes "bash"). **Another session may be active on it** → keep `.rivets/issues.jsonl` out of this branch; file discoveries from the primary checkout. |

No existing issue covers the `\\wsl$`/`\\wsl.localhost` translation itself — cyril-8tq6 is not a duplicate.
