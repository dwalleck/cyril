# KAS-5b (cyril-ufie) — prove-it-prototype

**Goal:** before designing KAS-5b (terminal host-callback responders), confirm I
actually understand the system cyril extends — the typed `acp::Client` terminal
contract (acp 0.10.2 / schema 0.11.2), the genuine KAS wire, and the
single-threaded bridge runtime — and catch any divergence between what cyril
*would emit* (typed responses) and what KAS *provably accepts*.

**Probe:** `.cyril-ufie/probe.rs` (run from a scratchpad cargo crate, offline,
pinned to cyril's exact deps: `agent-client-protocol =0.10.2`, `tokio =1.50.0`).
Standalone, uses no cyril abstractions. On a **single-threaded** `current_thread`
runtime (mirrors the bridge) it: (1) deserializes the genuine captured request
wire into the typed acp terminal requests; (2) runs a command that **exits 42**
via `tokio::process`; (3) round-trips a terminalId registry incl. unknown-id miss;
(4) builds the typed acp replies and serializes them — the exact wire cyril emits
if its `acp::Client` overrides return the typed responses.

**Why exit 42 (not the captured `echo done-42`, exit 0):** an exit-0 command can't
distinguish a correctly-read exit status from a dropped one. The non-zero code is
the discriminator that makes the wait-reply *shape* observable.

## Oracle

Two independent oracles, each a different mechanism from the probe:

- **Oracle A (execution):** the same command run in a plain POSIX shell
  (`sh -c 'printf out-42; exit 42'`, bash/coreutils — not `tokio::process`),
  capturing stdout + `$?`. Result `stdout=out-42 exit_code=42` — **matches** the
  probe's `EXEC stdout="out-42" exit_code=Some(42)` exactly.
- **Oracle B (wire shape):** the reply JSON the **KAS-5a probe**
  (`experiments/conductor-spike/probe-kas-fs-terminal-host-2.10.0.py:147-161`)
  sent and KAS **accepted in a clean turn** — genuine ground truth from a
  different implementation (hand-rolled Python), unaffected by the acp Rust crate.

### Agreement (gate satisfied)

| method | probe (typed acp 0.10.2) | Oracle B (KAS-accepted) | verdict |
|---|---|---|---|
| `terminal/create` | `{"terminalId":"term-1"}` | `{"terminalId":tid}` | ✅ agree |
| `terminal/output` | `{"output":"out-42","truncated":false,"exitStatus":{"exitCode":42,"signal":null}}` | `{"output":…,"truncated":false,"exitStatus":{"exitCode":N}}` | ✅ agree (probe adds `signal:null`, a harmless superset) |
| `terminal/release` | `{}` | `{}` | ✅ agree |
| exec exit code | `42` (tokio::process) | `42` (Oracle A, bash) | ✅ agree |

Plus: every captured **request** fixture deserializes cleanly into the typed acp
requests — **no camelCase/field drift** (unlike the historical `fs_write` divergence).

### Disagreement — `terminal/wait_for_exit` reply wrapping

| | shape |
|---|---|
| probe (typed `WaitForTerminalExitResponse`, `#[serde(flatten)]`) | `{"exitCode":42,"signal":null}` **flat** |
| Oracle B (KAS-5a probe, accepted for exit-0) | `{"exitStatus":{"exitCode":N,"signal":null}}` **nested** |

**Resolution (cause #2 — inherited model was wrong, NOT a broken substrate → proceed, model corrected):**
The acp 0.10.2 schema flattens `WaitForTerminalExitResponse.exit_status`, so the
**official ACP wire is flat** `{exitCode, signal}`. The covenant
(`docs/kiro-kas-acp-covenant.md` §5) classifies the terminal lifecycle as
**bare-ACP** = spec-conformant; KAS (a TS ACP agent) deserializes cyril's reply
into the same shared-schema type. The KAS-5a note's nested shape was a hand-coded
reply **tolerated** only because its command exited 0 (extra `exitStatus` field
ignored, missing top-level `exitCode` defaulted to undefined ≈ 0). **cyril must
return the typed (flat) response and must NOT copy the KAS-5a probe's nested
shape** — doing so would silently zero out every non-zero exit code (a
silent-failure bug per CLAUDE.md).

**Residual falsifier — CLOSED LIVE 2026-06-30 (checkpointed-build):** one KAS
2.10.0 (`--agent-engine v3`) turn, agent runs `true` (really exits 0) while the host
injects an unpredictable `exitCode=42` into cyril's exact wire (flat
`WaitForTerminalExitResponse` + nested `TerminalOutputResponse`). The agent reported
`EXIT_CODE=42` — since `true` exits 0, 42 could only reach it by KAS correctly parsing
cyril's flat shape. **Flat is honored end-to-end; the design's central finding is
confirmed live.** (`experiments/conductor-spike/probe-kas5b-c2l-flat-wait-2.10.0.py`;
the first attempt with `exit 7` was confounded — the agent can *predict* 7, so it
reported 7 by reasoning regardless of the reply. An unpredictable injected value was
required.)

## What I learned (one sentence)

The `terminal/wait_for_exit` reply is **flat** `{exitCode, signal}` per the acp
spec (`#[serde(flatten)]`) and the covenant's bare-ACP classification — **not**
the nested `{exitStatus:{…}}` the inherited KAS-5a note recorded — and naively
trusting that note would have silently zeroed out non-zero exit codes.

## Other facts confirmed against the real system (de-risk the design)

- `acp::Client` declares all 5 terminal methods + default `method_not_found`; KAS-5b
  overrides them in `impl acp::Client for KiroClient` (client.rs:41), `#[cfg(feature="kas")]`,
  exactly where the code already earmarks them (client.rs:217-220).
- **Single-threaded bridge runtime confirmed:** `new_current_thread()` + `LocalSet`
  (bridge.rs:138/144); each inbound request runs as its own `spawn_local` task
  (rpc.rs:272-274) — concurrent but on ONE OS thread. A synchronous `std::process`
  wait pins the thread and starves all other terminals + the turn loop. `wait_for_exit`
  MUST await `tokio::process` (verified by construction in the probe).
- **Terminal is stateful** (create→id→output/wait/release/kill) — unlike the
  stateless fs free-functions. KAS-5b needs a live-terminal registry keyed by a
  unique `terminalId`; unknown-id ops must error, not panic (probe step 3).
- Cap advertisement: add `terminal: true` to `KasEngine::client_capabilities()`
  (engine.rs); `_kiro/terminal/shell_type` is an **ext request** routed in
  `handle_ext_request` (client.rs), not a typed method.

## Artifacts
- `.cyril-ufie/probe.rs` — the probe (committed)
- `.cyril-ufie/related-issues.md` — prior-art search
- Reuses KAS-5a captures: `.cyril-7bdu/host_callbacks_2.10.0.json`, `.cyril-7bdu/fixtures/`
