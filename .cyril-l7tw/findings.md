# cyril-l7tw — prove-it-prototype findings

## POST-FIX LIVE VALIDATION (slice 8, 2026-07-04)

**Run 3** (`probe-run3.{stdout,stderr}`, replay agent, verbatim 2.11.0
frames): mid-turn SIGKILL now yields the full story, in order and fast —
`BridgeError op=prompt msg=Internal error: "server shut down unexpectedly"`
→ `TurnCompleted(EndTurn)` → `BridgeDisconnected("agent connection closed
unexpectedly")`, all at t+0.34s. Oracle log agrees (`ACP IO task ended
(agent EOF)` + one `prompt failed`). The old phase 2 (silent prompt at a
dead loop) is structurally impossible: the probe's second SendPrompt fails
loudly with `bridge channel closed` (exit 1).

**Run 4** (`probe-run4.stdout`, REAL logged-out kiro-cli 2.11.0 — the
logged-out state is the C7 fixture): the handshake-failure disconnect now
carries the agent's own words:

```
BridgeDisconnected reason=protocol error: ACP initialization failed: … "server shut down unexpectedly"
agent stderr:
error: You are not logged in, please log in with kiro-cli login
```

Run 4 also caught a cosmetic defect the unit fence missed — re-wrapping a
Protocol error stacked a second "protocol error:" prefix — fixed in
`append_stderr_reason` (reuse the inner message) and fenced.

Original pre-fix findings below.

---

## Smallest question

When the agent process dies mid-turn, what does the bridge's notification
channel emit to the App? (Issue prediction: only `TurnCompleted(EndTurn)` —
no `BridgeError`, no `BridgeDisconnected`.)

## Probe

`crates/cyril/examples/l7tw_death_probe.rs` — drives the **real**
`spawn_bridge` pipeline, starts a streaming turn, SIGKILLs the agent child at
the first `AgentMessage` chunk, records the channel transcript for 20s, then
sends a second prompt at the dead connection and records 8s more.

Agent under the bridge: `.cyril-l7tw/kiro-replay-agent.py`, replaying
**verbatim frames from the committed kiro-cli 2.11.0 v2 live trace**
(`experiments/conductor-spike/v2-live-session-trace-2.11.0.jsonl`). Real
kiro-cli 2.11.0 was attempted first (run 1) but the machine is logged out
(`kiro-cli login` is interactive); run 1 still yielded a real data point — see
Learned #3. Live re-validation against logged-in kiro-cli is a design-stage
claim (user-gated, like dcc6 C14b).

## Oracle

A different mechanism than the channel: the bridge's **tracing/stderr
pipeline** (unaffected by the not-forwarded-to-channel bug) plus the **OS
process table** (`kill -9` succeeded; child dead → stdout EOF). If the bridge
detects a failure internally, it must appear as an `ERROR` line on stderr; the
channel transcript then shows independently whether that detection was
surfaced.

## Result: probe and oracle agree (run 2, `probe-run2.{stdout,stderr}`)

| Phase | Oracle (stderr) | Probe (channel) |
|---|---|---|
| Mid-turn SIGKILL | `ERROR prompt failed error=Internal error: "server shut down unexpectedly"` | `TurnCompleted stop_reason=EndTurn` at t+0.34s — **zero** BridgeError / BridgeDisconnected |
| 2nd prompt, dead conn | second identical `ERROR prompt failed` line | instant `TurnCompleted stop_reason=EndTurn` — **zero** error notifications; loop still accepting commands |

Two failures detected internally, zero surfaced. Invisibility confirmed on the
real bridge. Issue items (1) and (2)'s user-visible symptom reproduce exactly.

## What I learned (not obvious before the probe)

1. **A SIGKILL'd agent is a *clean* EOF, not an io error.** The oracle log has
   NO "ACP IO task failed" line: the io pump exits `Ok(())` on EOF and logs
   nothing at all. The only internal witness to mid-turn death is the acp
   crate clearing `pending_responses`, which resolves `conn.prompt()` to
   `Err("server shut down unexpectedly")`. A fix hung off io-task `Err` alone
   would miss the most common death mode; the io-pump *end* (Ok or Err, with
   a turn in flight or a dead conn left behind) is the signal, not io `Err`.
2. **The command loop happily outlives the connection** — phase 2's SendPrompt
   was accepted and silently failed. "Dead conn, live loop" is a real state
   the design must name.
3. **Handshake-phase death is already visible** (run 1, real logged-out
   kiro-cli 2.11.0): `run_bridge` returned `Err` and the fail-stop path
   delivered `BridgeDisconnected("ACP initialization failed: … server shut
   down unexpectedly")` to the channel. Item (4)'s residual risk is only the
   `try_send` drop when the 256-slot notification channel is full — not the
   happy-path emission.
4. `kill -0` succeeds on the zombie for a while post-SIGKILL (child not yet
   reaped) — `still_alive=true` in the KILL transcript line is the zombie, not
   a failed kill; death ground truth is the EOF the bridge saw 40ms later.
5. UI plumbing needs no work: `UiState` already renders `BridgeError`
   (state.rs:730) and `BridgeDisconnected` (state.rs:465). The entire fix is
   bridge-side emission.
