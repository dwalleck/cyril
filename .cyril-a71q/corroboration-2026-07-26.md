# Source corroboration of the ordering premise — 2026-07-26

Runs the probe `next-steps.md` deferred ("needs the research archive"; that laptop lacked
`~/.local/share/kiro-research/`). Purpose: upgrade the ordering premise — *"`turn_end`
then response, both present"* — from a **two-turn live observation**
(`kas-live-session-trace-2.11.0.jsonl`) to source-confirmed producer/client behavior.

**Verdict: CORROBORATED on both sides, with one new nuance and one version caveat.**
Per CLAUDE.md the covenant is the contract; tui.js and `@kiro/agent` are implementation
evidence.

## 0. Covenant (the contract) — confirms role, silent on ordering

`docs/kiro-kas-acp-covenant.md` §4: `turn_end {stopReason}` ← **turn completion signal**;
`turn_completion {promptTurnSummaries, elapsedTime, status}` ← **metering** (§8). This
resolves why the capture shows `turn_completion` *before* `turn_end`: they are not two
lifecycle terminals, they are metering-then-completion. As `next-steps.md` predicted, the
covenant says nothing about cardinality or ordering vs. the `session/prompt` RPC response
— which is the gap the two sides below fill.

## 1. Client side — tui.js 2.11.0 (matches the capture version)

**Kiro's own client never consumes `turn_end`.** Zero occurrences in any casing (an
apparent 36 `turnEnd` hits are substring matches inside highlight.js's `returnEnd`). No
KAS-specific completion path exists (`engine:"v3"` / `engine:"kas"` = 0 occurrences).

It ends the turn on the **RPC response**:

```js
let i = await Promise.race([this.connection.prompt({prompt:e, sessionId:this.sessionId}), n]);
this.observeV2TurnCompletion(i?.stopReason, (performance.now()-t)/1000)
```

`n` rejects only on connection abort ("Agent connection closed unexpectedly"). `stopReason`
is read from the response. Its `session_info_update` handler branches on exactly one kind
— `turn_completion` — and uses it for context-usage + metering telemetry, never for busy.

⇒ **Directly refutes the voided choice-A** ("sole `turn_end` release authority"): the
reference client does the opposite. And it independently supports keeping the response as
a genuine release source (cyril-3zy4), because that is the *only* source Kiro itself uses.

## 2. Emitter side — `@kiro/agent` `dist/server/acp-server.js` (2.12.0 bundle)

**Q: is `turn_end` emitted unconditionally, before the prompt RPC resolves?** Yes, and by
construction rather than by accident.

- All three terminal dispatches funnel into one method, each gated by `isOwnedEvent`:
  `AgentExecutionSuccess` → `persistTurnCompletion("end_turn", …)`,
  `AgentExecutionFailed` → `persistTurnCompletion("error", …)`,
  `AgentExecutionAborted` → same path.
- `persistTurnCompletion` persists `session_pause` + `turn_end` atomically, calls
  `commitTurnTermination(persistId)`, then `broadcastTurnEnd(stopReason)`.
- `pendingTerminalStatus` is the ordering guarantee, in the emitter's own words: success
  and failure "keep `status` non-terminal until the turn's `turn_end` record is durable",
  and `commitTermination` is applied "**between the durable write and the wire broadcast**
  — so the flip to terminal lands after persistence (no phantom cancellation) and
  **before clients are told the turn ended** (no new-request race against a still-active
  turn)."

⇒ the execution cannot be terminal — hence the prompt RPC cannot resolve — until
`turn_end` is durable, and the broadcast follows. The observed 0–1 ms `turn_end`→response
gap is an architectural invariant, not a scheduling coincidence.

**Q: which paths skip it?** None found. Even the short-circuit that never starts an
execution emits it: `emitSyntheticAgentTurn` (spec/no-workspace refusal) brackets the
reply with persisted + broadcast `turn_start`/`turn_end` "so observers see the same turn
lifecycle as a normal turn." Crash/replay paths *synthesize* it — `makeSyntheticTurnEnd`
(`stopReason: "cancelled"`), and cold-load emits `turn_start` + `turn_end cancelled` for a
trailing orphan prompt. The emitter actively works to guarantee the bracket exists.

**Q: can one prompt emit two scoped `turn_end` frames?** Not on any path found. Each turn
brackets under one `executionId`; the adapter owns one at a time and `isOwnedEvent` drops
orphans. A superseding prompt calls `abortActiveExecutionsForSessionAndWait`, whose
`abortAndWait` "resolves only after the abort's `turn_end` is durable", explicitly so the
prior turn is terminated **before** the new turn persists its user message and takes the
adapter's owned id.

## 3. NEW — the wire `turn_end` is confirmed identity-free

The *persisted* `TurnEndPayloadSchema` carries `executionId` (optional). Every **wire**
emission drops it. All three sites emit `{kind, stopReason}` only:

```js
broadcastTurnEnd(stopReason) { await this.sessionInfoEmitter.send({ kind: "turn_end", stopReason }); }
case "turn_end": return [buildSessionInfoUpdate({ kind: "turn_end", stopReason: payload3.stopReason })];
await outbound.sessionUpdate(buildSessionInfoUpdate({ kind: "turn_end", stopReason: "cancelled" }));
```

⇒ **Source-confirms the design's central asymmetry.** cyril cannot id-match a wire
`turn_end` because the producer never puts an id on it — which is exactly why the design
stamps `Option<TurnId>` on the synthesized completion and uses a session-keyed one-entry
ledger for the unstampable wire arm. This was a design assumption; it is now evidence.

## 4. Annotations for design.md

- **B1** ("scripted traces cannot see unrepresented interleavings"): narrowed on the
  emitter side. `turn_end`-before-response is enforced by `pendingTerminalStatus` +
  `commitTermination` ordering, not by scheduling. Receipt-order independence (C2) is
  retained as defence-in-depth against *cyril-side* channel jitter, which remains
  unexercised by traces.
- **B10** ("two-turn capture cannot prove every KAS version honors order or at-most-one
  `turn_end`; duplicates carry no identity"): the *identity-free* half is now
  source-confirmed fact, not inference (§3). The *at-most-one* half is corroborated by
  construction for this bundle — single owned `executionId`, `isOwnedEvent` orphan drop,
  supersede-aborts-and-awaits — but remains version-scoped (see caveat). Duplicate
  `turn_end` stays the one named unsafety; absorb-first still bounds the damage.

## 5. Caveats

- **Version gap.** Emitter evidence is the **2.12.0** KAS bundle; the capture is 2.11.0 /
  KAS 0.8.0. Extracted bundles on this machine start at 2.12.0, so the emitter was not
  read at the capture's exact version. Client evidence *is* at 2.11.0.
- Scope is one bundle read, not a differential across versions. B10's "every KAS version"
  wording stands; this narrows it to "the shipped emitter, as of 2.12.0, cannot skip or
  duplicate it on any path found."
- Nothing here re-opens the spec. No legitimate response-only path, no
  response-before-`turn_end` emission, and no multi-`turn_end` path was found — the three
  contradiction triggers `next-steps.md` named. Per its own criterion: **proceed to the
  plan.**
