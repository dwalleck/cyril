# cyril-tpfd — prove-it-prototype findings

## Q1 (the issue's blocker): what is the `AcpPrecomputedHookResult` element shape?

**CARVED AND CROSS-WITNESSED (static).** No covenant `.d.ts` ships in any
local KAS bundle (only `@kiro/agent`), but the compiled `acp-server.js`
contains both a producer and a consumer of the type, and they agree
exactly (probe: `probe-carve-shape.sh`, identical on 2.13.0 and 2.14.1):

```jsonc
{
  "id": "<hook id>",          // consumer reads hookId ?? id
  "name": "<hook name>",      // telemetry (emitHookInvoked)
  "hookId": "<hook id>",      // duplicate of id, both set by producer
  "originalType": "runCommand" | "askAgent",   // ANYTHING ELSE THROWS
  "content": "<text to inject>"
}
```

- **Producer** (v2 standalone provider `extractPrecomputedResults`): builds
  elements from SessionStart hook runs; command hooks use
  `stdout || stderr` (stdout OR stderr — **not combined**, unlike our
  executeHook convention) and are **skipped entirely when output is
  empty**; agent hooks use the `appendix` as content with
  `originalType: "askAgent"`.
- **Consumer** (`handlePrecomputedTrigger`): for each result, wraps
  `content` in a `<HOOK_INSTRUCTION>…</HOOK_INSTRUCTION>` block appended
  to the session's first user prompt (runCommand content additionally
  prefixed `[Session Start Hook Output]\n`), and emits telemetry via
  `wireTypeToActionKind(originalType)` — whose `default:` arm is
  `assertNever` → **an unknown `originalType` throws inside the agent**.
- Empty `results` → no-op (state returned unchanged), which is why the
  shipped `{results: []}` stub is wire-safe.

## Q2: does the request fire live, and with what params?

**LIVE-VERIFIED (already, from the jiyn A/B captures 2026-07-19):**
`.cyril-jiyn/ab-results-host/result.json` records
`_kiro/hooks/sessionStart {trigger: "sessionStart", sessionId}` arriving
under `{enabled: true}` on kiro-cli 2.13.0; the v2 arm drives zero host
callbacks (winner-take-all, consistent with the jiyn A/B).

## Q3 (live oracle): is a carved-shape reply actually consumed and injected?

**Probe written (`probe-sessionstart-live.py`): reply with one
runCommand-shaped result ordering the model to start its reply with
MARMALADE; success = sessionStart arrives AND the completed turn's text
contains the token.**

**STATUS: BLOCKED ON AUTH** — `kiro-cli` login expired
(`error: You are not logged in`, `live-results/spawn.stderr`). The live
run needs the user to `kiro-cli login` first. Static carve + live request
evidence stand; the response-consumption proof is pending this run.

## What I learned that I didn't know before

The element shape was never in a `.d.ts` we could obtain — it is fully
recoverable from the bundle because KAS's own v2 standalone provider
CONSTRUCTS the same elements agent-side; and two constraints no schema
would have shown: unknown `originalType` values throw (`assertNever`) in
the consumer's telemetry path, and command output packaging is
`stdout || stderr` with empty-output hooks silently dropped.

## Oracle

Static probe = mechanical field extraction from two independent code
sites (producer construction vs consumer accesses) — agree on 2.13.0 and
2.14.1. Runtime oracle = live injection behavior (Q3), pending auth.
