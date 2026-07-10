# cyril-vgcm — prove-it-prototype findings

Probed 2026-07-09 on kiro-cli **2.12.0**, both engines, through cyril's real
spawn path (`kiro-cli acp [--agent-engine kas]`), idle + busy-turn lifecycles.
Probes: `probe-steer-clear-live-2.12.0.py` (idle lifecycle, both engines),
`probe-steer-clear-behavior-2.12.0.py` (3-turn busy behavior w/ marker
suppression + KAS auth responder mimicking `kas/auth.rs`).

## Headline: the issue's premise is stale

**F1 — v2 2.12.0 supports `_session/steer/clear`, and it is FUNCTIONAL.**
Idle: steer → `{queued:true}`, clear → `{cleared:true}`. Busy: a cleared steer's
marker never appears in output (control marker without clear DOES land, same
session). The issue's "KAS-only" call was built on static literal absence — but
the method string is runtime-assembled: it is absent from `strings` of EVERY
binary **including 2.12.0, which live-accepts it**. Static-absence reasoning is
invalid for this method; live probe was the only truth. Which v2 release added
acceptance is undetermined (untestable statically for the same reason) — the
runtime -32601 gate already in bridge.rs remains the correct guard for older
binaries.

**F2 — v2 steering echoes were renamed by a BACKEND rollout between 2026-06-17
and 2026-07-09; cyril drops them since.** (CORRECTED — the first draft dated
this "binary 2.9.0" from the strings rename table; see F8.) Live v2 2.12.0
emits (verbatim, `_kiro.dev/session/update` envelope):

- `AgentExecutionUserMessageQueued` `{messageId, content}` (was `steering_queued` `{message}`)
- `AgentExecutionSteeringInjected` `{messageId, content}` (was `steering_consumed` `{content}`)
- `AgentExecutionUserMessageCleared` `{messageIds}` (was `steering_cleared`)

`convert/kiro.rs` matches only the old literals → unknown-variant Err → dropped
(fence `steering_unknown_variant_errs` proves the drop dynamically). Net effect
on every kiro-cli ≥2.9.0: the optimistic steer chip increments and **never
reconciles** — queued/consumed echoes all silently vanish. This is a broader
root cause overlapping cyril-nvmh's phantom-chip paths.

**F3 — KAS steering echoes are also dropped, by a different gap.** KAS rides
`session_info_update` with `_meta.kiro.kind` = `steering_queued` /
`steering_injected` / `steering_cleared` (old-style names, `content` field,
note **injected, not consumed**). `convert/kas.rs` matches only
`turn_end`/`context_usage`; steering kinds fall to `None` (fence
`other_sub_kind_is_ignored` proves the drop shape). So under KAS, /steer chips
never reconcile either, and a wired /steer clear would never drain the UI.

**F4 — cleared-broadcast semantics differ per engine.** KAS broadcasts
`steering_cleared {messageIds}` on explicit clear AND routinely after
injection (healthy turn 2: queued → injected → cleared, same id, marker
landed, no clear ever sent). v2 emits `Cleared` ONLY on explicit clear.
Therefore "cleared" must be treated as **id-scoped queue-removal**, not as
"user cleared everything": `Notification::SteeringCleared` today carries no
ids and UiState zeroes ALL queued chips on it — correct for explicit clear,
wrong for KAS post-injection cleared (it would zero a second still-queued
steer's chip... actually it wouldn't: post-injection cleared carries only the
injected id, but the lossy no-payload notification can't know that). The
notification needs `message_ids`.

**F5 — bridge -32601 handling poisons working steer.** `ClearSteering`'s error
arm inserts into the SAME `steering_unsupported` set that pre-send-gates
`SteerSession`. On any binary where steer works but clear doesn't (2.7.0 –
pre-clear v2), one `/steer clear` → -32601 → session marked → all subsequent
steers silently skipped (debug-log only). Latent today (nothing constructs
ClearSteering); becomes live the moment this feature wires the trigger.

**F6 — `/steer clear` today steers the literal text "clear".**
`SteerCommand::execute` treats any non-empty arg as steer text (impl read +
dynamic fence `steer_command_parses_message_and_rejects_empty`). The new
subcommand carves "clear" out of the steer-text namespace — a user who wants
to literally steer the word "clear" loses that (assessed: acceptable; design
pause decision).

**F7 — response-shape contract (for state design).**

| | v2 2.12.0 | KAS 0.8.0 (2.11.0/2.12.0) |
|---|---|---|
| steer resp | `{queued:true}` (no id) | `{queued:true, messageId:"steer-<uuid>"}` |
| clear resp | `{cleared:true}` (no ids) | `{cleared:true, messageIds:[…]}` |
| clear on empty | `{cleared:true}` | `{cleared:true, messageIds:[]}`, **no broadcast** |
| clear unknown session | (not probed on 2.12.0) | -32603 w/ details (7/02 probe) |

**F8 — the rename is a backend rollout, not a binary change (wire = binary ×
backend, again).** Two same-axis captures pin it: (a) the committed K1b wire
log (`.k1b-steering/idle-steer-wire-capture.log`, 2026-06-17, binary 2.8.0,
idle steer) shows the OLD family live: `steering_queued {message}` — no
messageId. (b) Today (2026-07-09), the ARCHIVED 2.7.0 binary's idle steer
emits the NEW family (`AgentExecutionUserMessageQueued {messageId, content}`).
Same binary generation, different wire → backend axis moved. The strings
rename-table dating (2.8.1→present, 2.9.0→absent) was a red herring for the
wire — the binary literal families coexist in all versions. Consequences: the
old family was live THREE WEEKS AGO, so the converter must handle BOTH
families (staged/revertible rollouts); and `_session/steer/clear` is accepted
by v2 today back to at least the archived 2.7.0 binary (probed idle: 2.7.0,
2.8.1, 2.10.0, 2.11.0 all return `{cleared:true}` on the current backend), so
the "-32601 clear on working-steer session" case (F5) is a robustness path,
not a live population.

## Oracles and agreement

- **KAS bundle byte-identity**: sha256(acp-server.js) 2.11.0 == 2.12.0
  (`037e979…`) → the 7/02 recorded KAS contract must carry over; live probe
  reproduced it exactly (idle lifecycle, resp shapes, broadcast kinds). AGREE.
- **Binary strings dumps** (independent static mechanism, archived
  `~/.local/share/kiro-research/`): rename-table fragment 2.8.1→present,
  2.9.0→absent predicts the echo rename; live capture shows exactly the
  renamed variants. AGREE. (Strings also showed the clear literal absent
  everywhere — DISAGREED with live acceptance; resolved: runtime string
  assembly, F1. The disagreement was information: static reasoning invalid here.)
- **Existing dynamic fences** (`steering_unknown_variant_errs`,
  `other_sub_kind_is_ignored`, `steer_command_parses_*`; all 13 steer fences
  pass) prove cyril's drop/parse behavior for exactly the shapes captured live.
  AGREE.
- Marker-suppression control design (clear leg vs no-clear leg, same session)
  internally controls the behavioral claim. Both engines: suppressed-with-clear
  AND landed-without-clear.

## What I learned (that I didn't know before)

The feature isn't "adopt a KAS-only clear behind an engine gate" — clear works
on BOTH current engines, while cyril's entire steering echo pipeline (queued/
injected/cleared, both dialects) has been silently dead since kiro-cli 2.9.0;
wiring /steer clear without first re-basing the converters on the current wire
would ship a button whose effect the UI can never see.

## Scope implication for the design (input to falsifiable-design)

1. Converter re-base: kiro.rs handles BOTH literal families (2.7/2.8 old names
   + ≥2.9.0 AgentExecution*); kas.rs maps the three steering kinds.
2. `Notification::SteeringConsumed`→ injected-semantics; `SteeringCleared`
   gains `message_ids` (id-scoped drain).
3. `/steer clear` subcommand → `BridgeCommand::ClearSteering` (plumbing exists).
4. F5 fix: clear's -32601 must NOT poison the shared steering_unsupported set.
5. cyril-nvmh interaction: echo revival changes its calculus (consumed events
   exist again) — note on the issue, don't fix its safety net here.
