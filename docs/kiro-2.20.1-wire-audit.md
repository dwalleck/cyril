# kiro-cli 2.20.1 wire audit (delta from 2.19.2)

**Audited:** 2026-08-27 (2.20.0 released 08-26, 2.20.1 released 08-27). Scope is
the combined **2.19.2 → 2.20.1** delta: the `_kiro/*` method census is
**identical across both releases** (110 = 110, zero added, zero removed), so
there is no per-release attribution question at the protocol level and the two
releases were audited as one hop. Installed `kiro-cli 2.20.1`; **KAS 0.52.1 →
0.54.3**.

**Verdict: SAFE for cyril today** — nothing cyril currently sends or parses
broke. Two new client-visible behaviours (stall watchdog, powers push) are
additive and currently unrendered.

---

## 1. METHODOLOGY BREAK — the KAS bundle is now MINIFIED

`@kiro/agent/dist/server/acp-server.js`: **559,734 → 17,564 lines**, 23.33 MB →
11.35 MB, identifiers mangled (`vho`, `puu`, `Sho`), `.d.ts.map` sidecars
dropped. No code-splitting (same file list, one bundle); `node_modules` is
essentially unchanged (530 MB → 519 MB). This is minification, not removal.

What survives: **runtime string literals** — wire method names, env-var keys,
user-facing messages, config keys. What does not: local/internal symbol names
and **all comments**.

```
symbol                     0.52.1   0.54.3
extractCapturedOutput          5        0
RootConversationIdSchema      11        0
spawnMemoryExtraction          2        0
disableAutoCompaction         30        7
userMemoryOptIn                1        1
```

**Consequences for the audit playbook** (supersedes the "read the bundle as the
reference implementation" technique):

1. **`grep -c` is invalid across the boundary** — it counts *lines*, and the
   minified bundle puts everything on ~17k enormous lines. Use occurrence
   counts (`grep -o | wc -l`).
2. **String-count A/B is only valid for true runtime literals.** Worked example
   of the trap: `oversized` counts 3 → 0, which reads as a removed feature. All
   three 0.52.1 hits are inside **JSDoc comments**; minification strips them.
   Same for `single message` (1 → 0).
3. **Live probing is now primary evidence; static diff is only a hypothesis
   generator.** This inverts the prior playbook. Every claim below that matters
   is backed by a live capture plus a pinned-0.52.1 control.

---

## 2. Stream-idle watchdog — NEW in KAS 0.54.3, LIVE-PROVEN

2.19.0 gave the **v2** engine a stream-idle watchdog and explicitly did **not**
give KAS one, leaving the v3 stall window open (cyril-bh7g / cyril-14ou). **KAS
0.54.3 closes it.**

### Contract (recovered from surviving literals, then live-verified)

```js
var Wwc=6e4, Gwc=3e5,                       // warn 60s, hard timeout 300s
    jwc="KIRO_STREAM_IDLE_WARN_MS",
    Vwc="KIRO_STREAM_IDLE_TIMEOUT_MS";
function Z7i(e=process.env){ return {warnMs:Wj(e[jwc],Wwc), timeoutMs:Wj(e[Vwc],Gwc)} }
```

* Feature flag `STREAM_IDLE_WATCHDOG = "stream_idle_watchdog"`, **default true**,
  resolved through a `FeatureConfigRegistry` whose provider precedence is
  `["governance","env","client","session","experiment"]`.
* Timers are **per stream `next()` gap**, not per turn. A turn far longer than
  `timeoutMs` never trips it as long as each inter-chunk gap is under it.
* Both thresholds are **env-overridable**, which makes the watchdog
  deterministically testable — the 60s/300s defaults collapse into seconds.
  This is what finally made the bh7g class of failure reproducible on demand
  instead of requiring a rare backend stall window.
* Shipped alongside a retry family, all default-true: `empty_response_retry`,
  `truncated_response_retry`, `stream_error_retry`, `auth_expiry_retry`
  (the last matches 2.20.1's "expired credentials refresh without ending the turn").

### Client-visible behaviour (all three legs live, same hour, same backend)

Soft warn fires `onRecoverySignal(msg, "warning")`, whose handler is

```js
(msg, level) => this.connection.extNotification("_kiro/system/notify", {level, message: msg})
```

| threshold | what cyril receives |
|---|---|
| `warnMs` (60s default) | `_kiro/system/notify` `{"level":"warning","message":"The model response paused unexpectedly. Waiting for it to resume…"}` — once per **attempt** |
| `timeoutMs` (300s default) → retry | `_kiro/system/notify` `{"level":"warning","message":"The connection to the model was interrupted. Retrying…"}` |
| retries exhausted | **`session/prompt` returns a JSON-RPC error** (below) |

```json
{"code": -32000,
 "message": "The model response stalled and timed out. Please try again. (Request ID: …)",
 "data": {"errorType": "StreamIdleTimeoutError",
          "retryErrorType": "TRANSIENT",
          "requestId": "…"}}
```

**The turn now always terminates.** This is the direct fix for the cyril-bh7g
finding that a missing terminal meant the turn *never ended*. Note the code is
**`-32000`**, not v2's `-32603`.

`_kiro/system/notify` is **session-less** — the frame carries only
`{level, message}`, no `sessionId` and no `_meta` (verified on the raw capture).
Like `_kiro/workflow/*`, it cannot be attributed to a session, so with parallel
sessions (subagents, workflow steps) cyril cannot tell *which* stalled.

### Live results

| leg | bundle | warn / timeout | notifies | terminal |
|---|---|---|---|---|
| soft | 0.54.3 | 250ms / 600s | 1 × "paused unexpectedly" @+2.0s | `end_turn` 4.9s |
| hard | 0.54.3 | 150ms / 900ms | 2 × "paused unexpectedly" | `end_turn` 5.2s (recovered) |
| exhaust | 0.54.3 | 100ms / 300ms | 3 × paused + 1 × "Retrying…" | **`-32000 StreamIdleTimeoutError`** @4.1s |
| **control** | **0.52.1 pinned** | **100ms / 300ms** | **0** | **`end_turn` 5.0s** |

The control leg is the attribution proof: identical env, identical prompt, same
hour, same backend — the env vars are simply **inert** on 0.52.1 because the
code does not exist. Binary-attributed, not a backend rollout.

---

## 3. `_kiro/powers/*` — pre-existing wire surface, new TUI consumer

2.20.1 adds a `/powers` command. The wire surface is **not new**: `powers/list`,
`powers/refresh` and `powers/items_changed` are present in 0.52.1 *and* 0.54.3
(`handlePowersList` occurs twice in both). 2.19.2's audit never covered it —
this is a gap of ours, not a 2.20.x arrival.

A **power** is an installable MCP-server bundle at
`~/.kiro/powers/installed/<name>/mcp.json`, registry-tracked via
`~/.kiro/powers/installed.json` (directory presence alone is not enough — a
planted fixture without a registry entry is ignored), mentionable as `@powers`.

* **`_kiro/powers/list`** — takes **no params**; returns `{powers:[…], errors:[…]}`.
  It is **not advertised** in `agentCapabilities._meta.kiro.extensionMethods`
  (still 23 entries) yet dispatches fine.
* **`_kiro/powers/refresh`** — **declared but NOT implemented**. The literal is
  in the bundle (in the persistence-classification switch) but there is no
  handler: `-32603 Internal error, data.details "Unknown ext method:
  _kiro/powers/refresh"`. A trap for any client that assumes declared ⇒ callable.
  *(This is also a rare counter-example to the "KAS emits zero `-32601`, so you
  cannot feature-detect" note — it errors, just as `-32603`.)*
* **`_kiro/powers/items_changed`** — **session-scoped** (carries `sessionId`,
  unlike `system/notify`) and **pushed unprompted at session creation** with the
  full list, so a client never needs to call `list` to populate:

```json
{"sessionId":"sess_…","status":"success","powers":[…]}
```

Item schema (live, real installation):

```json
{"name":"aws-infrastructure-as-code",
 "description":"…","displayName":"Build AWS infrastructure with CDK and CloudFormation",
 "keywords":["aws","cdk","cloudformation","…"],
 "mcpServerNames":["awslabs.aws-iac-mcp-server"],
 "hasSteeringFiles":false,"isAgentPlugin":false,
 "_meta":{"kiro":{"resource":{"resourceType":"power","source":{"origin":"user"}}}}}
```

---

## 4. Residuals / not closed this pass

* **Oversized-request auto-compaction + retry** (2.20.0). The changelog entry
  carries **no `[V3]` tag**, so it is a **v2-engine** change; the KAS static
  signal was the JSDoc artefact debunked in §1. Needs a v2 probe with a genuine
  context overflow — not attempted.
* **`FeatureConfigRegistry` `client` / `session` providers.** The precedence
  list includes `client` and `session`, which suggests an ACP client may be able
  to set `stream_idle_watchdog` (and the retry flags) over the wire. Not probed;
  potentially a real cyril lever.
* **`_kiro/governance/state`, `_kiro/policy/*`, `_kiro/sandbox/status`,
  `_kiro/progressive_context/items_changed`** — in the 110-method census, never
  audited. Same "dark surface we never covered" class as powers.
* Watchdog **soft-warn on the non-streaming path** emits metrics/debug only, no
  wire signal. Only the streaming path notifies.

---

## Artifacts

* Probes: `experiments/conductor-spike/probe-kas-stall-watchdog-2.20.1.py`
  (legs `soft` / `hard` / `control`, thresholds overridable via `WD_WARN` /
  `WD_TIMEOUT`), `experiments/conductor-spike/probe-kas-powers-2.20.1.py`
  (`SEED_POWERS=1` copies the real powers tree into the throwaway HOME).
* Captures: `kas-watchdog-{soft,hard,control}-2.20.1.jsonl` + `-verdict.json`,
  `kas-powers-2.20.1.jsonl` + `-verdict.json`.
* Bundle pin for controls: `KIRO_KAS_SERVER_PATH=<kas>/2.19.2-*/…/acp-server.js`
  (exclude the sibling `*.lock` directory when globbing).
