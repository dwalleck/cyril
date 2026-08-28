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
* **`_kiro/governance/state`, `_kiro/policy/*`, `_kiro/sandbox/status`,
  `_kiro/progressive_context/items_changed`** — in the 110-method census, never
  audited. Same "dark surface we never covered" class as powers.
* Watchdog **soft-warn on the non-streaming path** emits metrics/debug only, no
  wire signal. Only the streaming path notifies.

---

## 6. Workflow inter-step data passing — capture is PROMPT-SHAPE controlled, and now fails WRONG rather than empty

Re-verification of the `{{id.output}}` series on 0.54.3, with the artifacts file
channel measured **side by side in the same run** (same model, same session,
same hour) rather than separately.

Series to date:

| binary / KAS | `capturedOutput` | reading |
|---|---|---|
| 2.19.0 / 0.46.1 | `""` everywhere | `{{id.output}}` broken |
| 2.19.2 / 0.52.1 (+0.48.0 pin) | `"ALPHA"` | model/backend turn shape, not an engine fix |
| **2.20.1 / 0.54.3 (+0.52.1 pin)** | **depends on the step's prompt** | **see below** |

Probe: `probe-kas-workflow-channels-2.20.1.py`. Two steps; `s1` writes the token
to a file *and* is asked for it in text; `s2` receives **both** channels and
writes what it actually saw to `result.json`, which is read off disk afterwards:

* **CHANNEL A** — `{{s1.output}}` template capture
* **CHANNEL B** — `{{artifacts.value}}` path registry + a real file

Only `s1`'s prompt shape differs between the two styles:

* `restate` — "…then reply with exactly this single word and nothing else"
* `terse` — "…that is the entire task. Do not write any summary, explanation or
  closing message; signal completion and stop."

### Results

| style | bundle | runs | `capturedOutputs.s1` | what `s2` received as A | B |
|---|---|---|---|---|---|
| `restate` | 0.54.3 live | 4 | `"ALPHA"` | `"ALPHA"` ✅ | `"ALPHA"` ✅ |
| `restate` | 0.52.1 pinned | 1 | `"ALPHA"` | `"ALPHA"` ✅ | `"ALPHA"` ✅ |
| `terse` | 0.54.3 live | 4 | **`"Done."`** ×3, empty ×1 | **`"Done.\n"`** ❌ | `"ALPHA"` ✅ |

**Channel B was correct in 8/8 runs across both bundles and both styles.**

### The two findings that matter

**1. Capture correctness is controlled by prompt shape, not luck.** The
"roulette" framing is now too pessimistic *and* too optimistic. A step whose
prompt explicitly demands a trailing restatement captured correctly in every
observation; a step that does its work and ends its turn on the completion tool
never did. The extractor takes the last assistant message's text entries — so
whether the payload is there is a direct, controllable consequence of how the
step is told to finish. Identical behaviour on the 0.52.1 pin re-confirms this
is model/turn-shape, not engine.

**2. The failure mode has changed character, and is now far more dangerous.**
2.19.0 failed *empty* — visibly, loudly: the consuming step saw a hole, asked
via `send_message need_input`, and **paused the run**. On 0.54.3 the terse step
captured **`"Done."`** — the model's closing pleasantry — and handed that to
`s2` as the payload. `s2` accepted it, wrote it into `result.json`, and the run
finished **`status: "completed"`** with no error, no warning, and no pause.

That is **silent inter-stage data corruption**: a plausible-looking wrong value
propagating through the DAG under a green run status. An empty capture is a
detectable bug; `"Done."` is not distinguishable from a legitimate one-word
payload by any client-side check.

> A verdict heuristic of "capture is non-empty ⇒ capture worked" is unsafe —
> it scored the corrupted run as a pass during this audit before the check was
> tightened to compare against the expected value.

### Guidance (unchanged conclusion, much stronger reason)

Use **`artifacts` + files** for anything load-bearing — 8/8 correct, immune to
turn shape. If `{{id.output}}` is used at all, the producing step's prompt
**must** explicitly require a trailing restatement of exactly the payload, and
the consuming step should validate the value rather than trust it. Do not treat
a non-empty capture as evidence of a correct one.

---

## Artifacts

* Workflow channel probe: `experiments/conductor-spike/probe-kas-workflow-channels-2.20.1.py`
  (`WF_STYLE=restate|terse`, `KAS_PIN=<ver>` for the bundle control).
* Flag-table extractor (re-run each release):
  `experiments/conductor-spike/extract-kas-feature-flags.py`.
* Probes: `experiments/conductor-spike/probe-kas-stall-watchdog-2.20.1.py`
  (legs `soft` / `hard` / `control` / `envoff` / `envon` / `clientmeta`,
  thresholds overridable via `WD_WARN` / `WD_TIMEOUT`), `experiments/conductor-spike/probe-kas-powers-2.20.1.py`
  (`SEED_POWERS=1` copies the real powers tree into the throwaway HOME).
* Captures: `kas-watchdog-{soft,hard,control,envoff,envon,clientmeta}-2.20.1.jsonl` + `-verdict.json`,
  `kas-powers-2.20.1.jsonl` + `-verdict.json`.
* Bundle pin for controls: `KIRO_KAS_SERVER_PATH=<kas>/2.19.2-*/…/acp-server.js`
  (exclude the sibling `*.lock` directory when globbing).

---

## 5. Feature-config providers — `client` / `session` are NOT wired (cyril-34yq)

The provider precedence array reads
`["governance","env","client","session","experiment"]`, which suggested an ACP
client might set `stream_idle_watchdog` over the wire. **It cannot.**

Static: there is exactly **one** registry construction, and it passes **two**
providers:

```js
buildFeatureConfigRegistry(t){
  let r = new QNe(this.getExperimentConfigService()),  // source = "experiment"
      n = new bhe(process.env);                        // source = "env"
  return new ZNe({sessionId: t, providers: [n, r]});   // env + experiment ONLY
}
```

Only two provider classes exist (`bhe` `source="env"`, `QNe`
`source="experiment"`). Nothing declares `source` `client`, `session`, or
`governance` — the array is a **sort key over a forward-looking vocabulary**,
and three of its five sources have no implementation.

Live confirmation (`clientmeta` leg): five plausible shapes injected on **both**
`initialize._meta` and `session/new._meta` —
`kiro.settings.streamIdleWatchdog{enabled:false}`,
`kiro.settings.stream_idle_watchdog`, `kiro.settings.featureConfig.*`,
`kiro.featureConfig.*`, `kiro.features.*` — produced **4 notifies and the
`-32000` terminal**, identical to the no-meta baseline. The meta is **accepted
silently**: no error, no echo, no indication it was ignored (another
schema-accepted ≠ functional case).

### The env provider IS the lever — full flag map

The complete registry, cross-referenced from the three source objects in the
bundle: const → wire key, wire key → default, const → env var. **15 flags; 9 are
env-reachable.**

Regenerate with `experiments/conductor-spike/extract-kas-feature-flags.py`
(defaults to the newest installed bundle). It anchors on **string literals**,
never on the objects' variable names — those are minifier-assigned (`Fo`, `$O`,
`UAi` in 0.54.3) and will be reassigned in later builds.

| wire key | default | env var |
|---|---|---|
| `system_field_injection` | `false` | — none — |
| `system_prompt_migration` | `false` | — none — |
| `steering_supervisor` | `false` | `KIRO_FEATURE_STEERING_SUPERVISOR_ENABLED` |
| `cgs_delegation_v2` | `false` | `KIRO_FEATURE_CGS_DELEGATION_V2_ENABLED` |
| `session_title_llm` | `false` | `KIRO_FEATURE_SESSION_TITLE_LLM_ENABLED` |
| `user_agent_refactoring_enabled` | `false` | `KIRO_FEATURE_USER_AGENT_REFACTORING_ENABLED` |
| `kiroInfraSafetyMonitor` | `false` | `KIRO_FEATURE_KIRO_INFRA_SAFETY_MONITOR_ENABLED` |
| `fta_vibe` | `false` | `KIRO_FEATURE_FTA_VIBE_ENABLED` |
| `empty_response_retry` | **`true`** | — none — |
| `truncated_response_retry` | **`true`** | — none — |
| `stream_error_retry` | **`true`** | — none — |
| `stream_idle_watchdog` | **`true`** | `KIRO_FEATURE_STREAM_IDLE_WATCHDOG_ENABLED` |
| `auth_expiry_retry` | **`true`** | `KIRO_FEATURE_AUTH_EXPIRY_RETRY_ENABLED` |
| `memory_internal_enabled` | **`"disabled"`** (tri-state) | — none — |
| `memory_external_enabled` | `false` | `KIRO_FEATURE_MEMORY_EXTERNAL_ENABLED` |

**The 6 without an env var are experiment-only** — resolvable solely through the
backend `experiment` provider, so neither cyril nor spawn-time env can influence
them by any route: `system_field_injection`, `system_prompt_migration`, the
retry family (`empty_response_retry`, `truncated_response_retry`,
`stream_error_retry`), and `memory_internal_enabled`.

Notes that matter when using this table:

* **Value parsing is strict and fails open.** The `bhe` provider accepts exactly
  `"true"` or `"false"` (after `.trim()`). Anything else logs
  `featureConfig.env.unparsable` and **falls through to the default** — no error,
  so a typo silently yields default behaviour.
* **Do NOT derive the env var from the wire key — three different transforms are
  in play.** `cgs_delegation_v2` keeps everything; `kiroInfraSafetyMonitor` is
  camelCase→SCREAMING_SNAKE; `user_agent_refactoring_enabled` *drops* its
  trailing `_enabled` before `_ENABLED` is appended. `UAi` is the only
  authority, and a generated name no-ops silently down the `undefined → default`
  path.
* **`memory_internal_enabled` is tri-state (`disabled|insider|all`), which is
  probably why it has no env var** — the env provider is hardcoded boolean-only,
  so a tri-state flag cannot be expressed through it. A type constraint, not an
  oversight.
* **The defaults split by intent:** every `false` default is an unshipped
  feature awaiting rollout; every `true` default is a resilience mechanism (the
  retry family plus the watchdog) exposed so it can be turned *off*.

Separately, the watchdog's two **threshold** vars are not feature flags and are
parsed as integers, not booleans: `KIRO_STREAM_IDLE_WARN_MS` (default 60000) and
`KIRO_STREAM_IDLE_TIMEOUT_MS` (default 300000).

### Live: the env flag works on the main path but leaks a residual warning

Same thresholds (`warn=100ms`, `timeout=300ms`) throughout:

| leg | notifies | terminal |
|---|---|---|
| no flag (baseline) | 4 | `-32000` |
| `…WATCHDOG_ENABLED=true` | 4 | `-32000` |
| `…WATCHDOG_ENABLED=false` ×3 runs | **1, 0, 1** | `end_turn` every time |

So `false` reliably kills the hard timeout, the retry storm and the `-32000`
terminal — but an **intermittent single soft warning still escapes**.

Cause is visible in the accessor:

```js
streamIdleWatchdogThresholds(){
  return this.featureConfigRegistry?.get(Fo.STREAM_IDLE_WATCHDOG)
      ?? $O[Fo.STREAM_IDLE_WATCHDOG].default   // true
      ? Z7i() : {warnMs:0, timeoutMs:0}
}
```

A model instance on which `setFeatureConfig` was never called has
`featureConfigRegistry === undefined`, so `undefined ?? true` → **watchdog
enabled**, and it still picks up the `KIRO_STREAM_IDLE_*` thresholds. The flag
is therefore honoured per-model-instance, not globally; at least one secondary
call path is unregistered.

**Consequence for cyril: disabling the flag does NOT guarantee zero
`_kiro/system/notify` frames.** Cyril must handle the notification regardless of
how the watchdog is configured.
