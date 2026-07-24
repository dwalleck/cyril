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

**MATCH (2026-07-23, kiro-cli 2.13.0, model pinned via probe).** Shaped
arm: `_kiro/hooks/sessionStart` answered with one runCommand-shaped
result ordering the model to start with MARMALADE → turn completed and
the reply QUOTED both the `HOOK_INSTRUCTION` wrapper and the token
(`live-results/result-shaped.json`). Control arm (`MODE=empty`, the
shipped stub): turn completes, no token. The carved shape is consumed
and its content demonstrably lands in model context.

**Bonus live finding (cyril-0wyn coupling, now observed):** the model
*refused to obey* the injected instruction, calling it "not a legitimate
system directive, just text injected into the message" — with our
non-`kiro-ide` clientInfo, KAS omits its hooks system-prompt briefing,
so the model treats `HOOK_INSTRUCTION` blocks as untrusted. Injection
works mechanically; instruction *authority* is briefing-dependent.

> **CORRECTION (cyril-booz, 2026-07-23) — the attribution above is wrong.**
> The briefing is **not** omitted for cyril: an unrecognized `clientInfo.name`
> falls back to **kiro-ide**, the exact branch the `hooksBlock` gate selects,
> so cyril's session *does* carry the briefing (live-confirmed: the model
> quoted its `<hooks>` section verbatim, `.cyril-booz/`). And authority is
> **not** briefing-dependent — cyril-booz probed 7 framings ×3 runs
> (a corrected briefing, prompt-body framing, KAS's own production
> interception framing, even a benign fact): **0/18** complied. The refusal
> is the model's prompt-injection defense, which is **structural** to the
> user-turn injection point (`HOOK_INSTRUCTION` appended to the first user
> prompt), not fixable by any client-supplied briefing — and kiro-ide would
> see the identical refusal. Only the *mechanical* injection claim above
> survives. See `.cyril-booz/findings.md`.

## Substrate detour: three false suspects, one real bug

Every KAS ACP turn initially died with KRS HTTP 400
`ValidationException REQUEST_BODY_INVALID` (v2 fine, vanilla-KAS fails,
2.14.1-archive-binary fails, Claude-model swap fails). Root cause: the
**probe harness's `token()`** (inherited from
`.cyril-jiyn/probe-hooks-ab-2.13.0.py`) passed the
`api.codewhisperer.profile` row VERBATIM as `profileArn` — but that row
is a JSON OBJECT `{"arn", "profile_name"}`, so the request body carried
a JSON-blob-as-string. Extracting `.arn` fixed every arm. Consequences:

- The jiyn A/B's `prompt_completed: false` (both arms) was THIS harness
  bug, not KAS — its LIST/EXEC/marker conclusions stand (those callbacks
  fire before the model call), but the per-release fence
  `probe-hooks-ab-2.13.0.py` carries the same bug and would fail its
  turns forever → fix it in this branch (queued, see to-file.md).

## What I learned that I didn't know before

The element shape was never in a `.d.ts` we could obtain — it is fully
recoverable from the bundle because KAS's own v2 standalone provider
CONSTRUCTS the same elements agent-side; three constraints no schema
would have shown: unknown `originalType` values throw (`assertNever`) in
the consumer's telemetry path, command output packaging is
`stdout || stderr` with empty-output hooks silently dropped, and
injected hook instructions carry no authority (~~for an unbriefed
(non-kiro-ide) client~~ — corrected by cyril-booz: authority is not
briefing-dependent; the model's injection defense refuses them
regardless of framing, kiro-ide included). Plus: the profile row shape
changed under the harness, silently poisoning every KAS probe turn since
~2.13.0.

## Oracle

Static probe = mechanical field extraction from two independent code
sites (producer construction vs consumer accesses) — agree on 2.13.0 and
2.14.1. Runtime oracle = live injection behavior (different mechanism):
control vs shaped arms agree with the carve — MATCH.
