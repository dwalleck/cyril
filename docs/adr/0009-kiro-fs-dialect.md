# Take Kiro's `_kiro/fs/*` host-callback dialect, including the delete grant; port semantics rather than invent them

Status: accepted (2026-08-01, cyril-kf2g) — extends [ADR-0003](0003-defer-proxy-stack-for-host-callbacks.md)

## Context

ADR-0003 made host callbacks cyril's near-term interception path, on the
grounds that KAS delegates file I/O to the host and cyril is therefore "the
natural audit/gate/transform point". It did not say *which dialect* of those
callbacks cyril answers, because at the time only one was known.

There are two, and **the client selects between them per operation**.
`resolveCapabilities()` reads `clientCapabilities.fs._meta.kiro.{readFile,
writeFile, stat, readDirectory, delete}` — nested under `fs`, *not* under
top-level `_meta.kiro`. Advertising a flag swaps that one operation's adapter;
declining it leaves KAS on bare ACP, or on its own in-process `NodeFileSystem`
where cyril never sees the operation at all. An earlier probe advertised
`_meta.kiro.kiroFsReadFile` — the *resolved* capability name at top level — and
moved nothing, which is why an earlier sweep recorded the trigger as unknown.

Declining the dialect is not neutral. Three of the five operations —
`stat`, `read_directory`, `delete` — have **no bare-ACP equivalent**. Under the
ACP dialect they do not arrive as callbacks; they execute inside the agent
process, where ADR-0003's audit point does not exist. And `_kiro/fs/read_file`
carries pagination (`line`, `limit`; live-observed `{line: 0, limit: 2001}`)
that `fs/read_text_file` has no field for, so a client on the ACP dialect must
return whole files.

## Decision

- **Advertise all five flags**, including `delete`. The three operations with
  no ACP equivalent are precisely the ones ADR-0003 exists to observe; leaving
  them inside the agent forfeits the interception this project's differentiator
  rests on.
- **`delete` is a deliberate grant, not a blanket opt-in by omission.** It
  hands the agent a host-executed delete path. It is granted because a client
  that advertised four of five would make the agent's delete succeed or fail
  based on a flag the user cannot see — parity chosen over comfort.
- **Semantics are ported from the reference implementation, not designed.**
  For every method KAS ships an in-process implementation serving the same port
  when the client declines the flag. That is the contract: a client that answers
  differently makes the agent behave differently depending on an invisible flag.
  Expected values in the tests were produced by **running** the carved JS
  (`@kiro/agent` @ KAS 0.27.8), not by reading it.
- **Audit, not gate** — see Consequences; this narrows ADR-0003 and is the most
  important thing in this document.

## Considered options

- **Stay on bare ACP** — rejected: forfeits `stat`/`read_directory`/`delete`
  entirely (they never reach cyril) and forces whole-file reads.
- **Advertise four flags, decline `delete`** — rejected: the agent still
  deletes, via its in-process `NodeFileSystem`; cyril simply stops seeing it.
  Declining buys no safety, only blindness, and makes agent behavior depend on
  a capability flag with no user-visible signal.
- **Advertise the dialect but "improve" the semantics** (confine paths, refuse
  recursive delete, normalize readdir order) — rejected as a *default*: it
  makes cyril's answers differ from the fallback the agent was written against.
  Divergences are allowed only where deliberate and documented; today there is
  exactly one (`read_directory` sorts, for reproducible captures).

## Consequences

- **This narrows ADR-0003.** That ADR listed "org write/exec policy" among the
  concerns host callbacks would deliver. These responders deliver the *audit*
  half only: every mutation logs at `info!` with session and path, and nothing
  here refuses anything. The central write/exec gate seam remains deferred to
  its first real consumer (cyril-g9vt). ADR-0003's claim should be read as
  "host callbacks are where the gate will go", not "the gate is implemented".
- **Permission posture is unchanged by the switch** (live-verified 2026-08-01,
  `probe-kas-fs-write-permission-2.16.0.py`, `kas-fs-write-2.16.0.jsonl`). KAS
  raises `session/request_permission` at the **tool-approval** layer, before the
  host callback — not per callback. Measured: 2/2 writes and 1/1 delete each
  preceded by an approval (`"Replace in File"`, `"Write File"`, `"Delete
  File"`). So advertising `writeFile` moves no write off a gated path. An
  earlier reading of the carved source concluded that no permission precedes
  `_kiro/fs/delete`; that was **wrong on the wire** and is corrected here.
- **What is unbounded is scope, not gating.** `to_native_checked` requires only
  an absolute path. An approved delete is unconfined and recurses into
  directories, and the approval names one path. Confinement, if wanted, belongs
  with the gate seam (cyril-g9vt), not scattered per responder.
- **Two traps that would have shipped silently**, both fenced:
  `_kiro/fs/read_file`'s `line` is **0-based** and its slice rejoins with `\n`,
  unlike ACP's 1-based newline-preserving helper — the only live-observed value
  is `line: 0`, where both readings agree, so every paginated follow-up would
  have been off by one. The two helpers stay separate with a test asserting they
  disagree. And advertising `writeFile` re-routes **range** writes
  (`KiroRangeWrite` over `LocalSpliceRangeWrite`); ignoring the range would turn
  every partial edit into a full-file overwrite. Offsets are **UTF-16** code
  units — Rust-`char` indexing would misplace every cut after an astral
  character.
- **Per-release fence.** Re-carve `spliceRange` and `NodeFileSystem` each wire
  audit and re-run the oracle tables; a semantics drift upstream silently makes
  cyril's answers wrong rather than failing. The first live `_meta.kiro.range`
  (`{"start":{"line":2,"character":0},"end":{"line":2,"character":7}}`) is now
  an oracle over `splice_range`.
- **Known deviation:** `read_directory` sorts entries; the reference returns raw
  readdir order. Kept for reproducible captures, documented inline.
