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

## Approval
_(pending — step 10)_
