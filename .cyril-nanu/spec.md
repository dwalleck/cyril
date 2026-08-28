# Spec: Non-blocking usage-panel refresh

## Request (verbatim)
> MEASURED while building cyril-9kyk. UsageLog::snapshot() rebuilds the ENTIRE snapshot — overview, provider/model/folder/agent_type rollups, cost and charge totals, tools, context, recent, errors — and it runs on an always-on path. [...] SCOPE: stop recomputing everything per refresh. Options to weigh: debounce the refresh (coalesce bursts of context samples), cache the snapshot and invalidate on append only, split the panel's cheap fields from the expensive rollups so a context tick refreshes only what changed, or compute rollups off the UI path. Whatever is chosen must keep the panel's displayed numbers honest — a stale cache that silently lags a completed turn is worse than a slow refresh.

## What this is
_(pending — completed at step 9)_

## Roles
- **Cyril operator**: drives a Kiro v2 or KAS session and opens `/usage` to inspect live and aggregated usage, latency, context, tools, identity groupings, and account metering. Adopted verbatim from cyril-kryv's spec; this change alters when that operator's numbers update, not who reads them.

## Behavior
_(pending)_

## Success criteria
_(pending)_

## Out of scope
_(pending)_

## Related issues

- **cyril-9kyk** (closed): the p90/max latency work that measured this. Carries the cost history (~342 ms baseline → ~690–760 ms at 100k rows after p90/max) and the operator's 2026-08-28 "Raise budget + ticket the refresh" decision that created cyril-nanu. Its 700 ms `grouped-percentile` budget and 2 s `kiro_snapshot_100k` budget are the fences this change must not regress.
- **cyril-b163** (open, P3): `usage_turns` has no retention, so row count — and therefore `snapshot()` cost — grows for the life of the install. Bears directly: retention would shrink this cost but cannot fix the recompute-per-sample shape. Not a blocker; not in scope here.
- **cyril-kryv** (closed): shipped the `/usage` panel. Its spec already decided this exact shape for the *account* query — "opening `/usage` never waits for `_kiro/account/getUsage`; exactly one query is dispatched per open ... and the response refreshes the open panel", with "a failed refresh shows explicit status beside the last successful in-process value". That is the precedent D1 adopts for local rollups.
- **cyril-c6la** (open, P2): the same disease on the render path — every frame re-renders all historical messages before viewport clipping. Sibling, not dependency; its AC shape ("a production-shape benchmark exists; steady-state work is bounded; output remains correct") is the model for this spec's criteria.
- **cyril-gfkm**, **cyril-4h6i** (closed): built the usage observer that produces the `UsageWrite::Turn` / `UsageWrite::Context` writes driving the refresh. No staleness decision recorded in either.

## Decisions

| Question | Decision | Rationale | Implication |
|---|---|---|---|
| While the panel is open and a refresh is in flight, what must the operator see? | Numbers may lag a completed turn, provided the panel marks it on screen and converges within a bounded time. | Requester selected "Lag, but visibly marked", 2026-08-28. Consistent with the precedent cyril-kryv already shipped for the account query: render what is held, refresh asynchronously, show explicit status rather than a silent stale value. | The panel gains a visible in-flight marker. A refresh never blocks the event loop, and the displayed values are those of the last completed snapshot until a newer one lands. Rules out the silent-lag option the ticket itself warns against. |
| Is the panel's first `/usage` open in scope, or only the recurring refreshes? | Open goes asynchronous too. | Requester selected "Open goes async too", 2026-08-28. Open is the single worst stall (~700 ms at 100k rows, `app.rs:1838`) and it reuses the D1 marker rather than needing a second mechanism. | `/usage` renders its frame immediately in a computing state with no rows, and fills in when the first snapshot lands. The panel gains a no-data-yet state it does not have today. No call site of `snapshot()` remains on the event loop. |
| Is reducing `snapshot()`'s own cost in scope, or is moving it off the loop enough? | Off-loop is enough; `snapshot()` is unchanged. | Requester selected "Off-loop is enough", 2026-08-28. The ticket's complaint is the stall, and off-loop plus coalescing removes it without decomposing a query layer that cyril-9kyk has just fenced. | Convergence bound is one snapshot duration after the last trigger, not a smaller number. Caching, invalidate-on-append and cheap/expensive field splitting are out of scope. Cost still grows with row count; that axis is cyril-b163. |
| What happens to triggers that arrive while a background snapshot is already running? | Set a dirty flag; when the running snapshot lands, if the flag is set, exactly one more runs. | Requester selected "Mark dirty, run once more", 2026-08-28. Dropping mid-flight triggers can leave the panel showing pre-turn numbers indefinitely — the silent lag the ticket rules out; cancel-and-restart can starve under a sample stream arriving faster than one snapshot. | At most one extra recompute per burst. The panel always converges to the newest committed write once triggers stop. A burst of N context samples costs at most 2 snapshots, not N. |

## Approval
_(pending — step 10)_
