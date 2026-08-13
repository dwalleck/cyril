# KAS turn stall — live capture, kiro-cli 2.16.2 (cyril-bh7g)

**Date:** 2026-08-11 · **Setup:** cyril-core KAS free path (`node acp-server.js --auth=acp-callback`,
bundle `2.16.2-7148833c…`), wire tap interposed via `KIRO_AGENT_PATH` node shim, automated
multi-round Wayfinder charting driver. Capture: [`kas-turn-stall-wire-2.16.2.jsonl`](kas-turn-stall-wire-2.16.2.jsonl)
(`{ts, dir, msg}` JSONL, auth redacted; includes KAS stderr as `agent-stderr` rows).
Verdict: [`kas-turn-stall-verdict-2.16.2.json`](kas-turn-stall-verdict-2.16.2.json).

## What was captured

One session, four turns. Turns 1–3 delivered the canonical terminal triplet
(`turn_completion` → `turn_end` → `session/prompt` response, ≤2 ms apart; cyril's
TurnMediator logged forward + companion-absorb for each). **Turn 4 wedged**:

```
23:07:15.928  client→agent  session/prompt id=5           (round-4 answer)
23:07:15.930  agent→client  si:user_message_id_assigned
23:07:15.932  agent→client  si:turn_start
23:07:18.913  agent→client  si:focus_update
23:07:18.926  agent-stderr  [KRS] GenerateAssistantResponseCommand done totalEvents=30   (leg 1: tool call)
23:07:18.9xx  agent→client  tool_call_update (update_session_information: "…Charting the Wayfinder
                            map and child tickets in rivets now.") + 2× si:context_usage
              — then ZERO frames, ever. No text, no terminals, no response for id=5.
```

KAS-side ground truth (`~/.kiro/logs/20260811T230556191/`, session `sess_5c475c66…`):
execution `d5d7da33` **began 23:07:15.931 and never succeeded**. The second model leg
(the long map-creation call) stalled ~16 minutes; the assistant text (len 1335) persisted
at **23:23:42**, after the client was gone. `ActivityLogPublisher flush failed ECONNRESET`
at 23:14:33 corroborates network trouble. No KAS-side timeout exists on the converse stream.

## Localization (cyril-bh7g)

- **Absent on the wire.** Conversion, bridge mediation, and transport are exonerated for
  this instance: channels are lossless `.send().await`, `turn_end` conversion tolerates
  missing stopReason, mediator traces show nothing arrived to drop.
- The terminal was not *lost* — it was **never due**: the turn was legitimately still
  in flight on a stalled backend stream, invisible to the client (no chunks, no
  context_usage heartbeat, nothing for 7+ min).
- Phenomenology note: the "complete streamed question round" the consumer last sees is
  the *previous* turn's output; the wedged turn is the follow-on charting turn that
  streams only a brief tool blip before stalling.

## Second signature (not yet localized)

The Aug 11 13:07–14:46 Tauri-consumer runs wedged with a *different* KAS-side signature:
`Execution succeeded` + `turn_end{end_turn}` persisted within seconds for the wedged round
(5 sessions, wedge on round 1,1,2,3,5), yet no follow-up prompt ever arrived. No tap
existed for those runs, so emitted-but-lost vs consumer-side stall is undecided. (The
14:27+ runs are auth casualties — hard-expired login token, execution never began —
not this bug.)

## Operational fallout

`acp-server.js` does **not** exit when its stdin closes: every hard-killed consumer run
leaves an orphaned node process (~30 accumulated on this machine during this research).
Bridge teardown must actively kill the child; SIGKILL of the consumer orphans the agent.

## Implications for cyril

Do **not** synthesize a terminal on silence — the stalled turn completed 16 minutes later,
so a synthesized `TurnCompleted` would race a genuine late one (exactly the cyril-a71q /
cyril-pnwb hazard). The right surface is turn-liveness: bridge-level "no agent activity
for N min" signal + a cancel affordance, leaving terminal authority with the engine.
