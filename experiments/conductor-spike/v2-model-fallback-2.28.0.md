# v2 `model_fallback` — live capture, kiro-cli 2.28.0 (cyril-ci3j live gate)

Captured 2026-10-08/09 by fault injection (see §8). **A real frame was obtained** — and it
carries a key the ticket's synthetic shape does not have (`source`).

Artifacts (uncommitted):

| what | path |
|---|---|
| every ACP frame, both directions, per arm | `experiments/conductor-spike/v2-model-fallback-2.28.0.jsonl` (217 lines) |
| the real frames, verbatim | `crates/cyril-core/tests/fixtures/v2/model-fallback-live-2.28.0.json` |
| this report | `experiments/conductor-spike/v2-model-fallback-2.28.0.md` |

Engine proof (from `/proc/<child>/exe` + sha256, per arm, recorded in each arm capture):

| exe | sha256 |
|---|---|
| `/home/dwalleck/.local/bin/kiro-cli` | `a629287174f294b52ec011300e08487afff36c6e00ff746b63ac85fceba9ba91` |
| `/home/dwalleck/.local/bin/kiro-cli-chat` (**the engine**) | `ae5e172a905bd0078e9d2faded43837d05bd684615572af9917702fc349bbd4d` |

`kiro-cli` is the launcher; `kiro-cli-chat` is the v2 (Rust) engine that owns the ACP
session and the CodeWhisperer calls. Engine version reported in `initialize`: `2.28.0`.
Spawn: `kiro-cli acp`, `HOME=<fresh tmp>`, real `XDG_DATA_HOME`, `XDG_RUNTIME_DIR=<tmp>`,
`KIRO_LOG_LEVEL=debug`. One engine process at a time.

---

## 1. The frame (arm A, verbatim)

```json
{"sessionId": "391bfbd2-a0b0-4886-b56d-a5e942b0c26c", "update": {"sessionUpdate": "model_fallback", "from": "claude-sonnet-5", "to": "claude-haiku-4.5", "cause": "unavailable", "source": "user", "promoted": false}}
```

Keys and types (as observed):

| key | type | observed values | notes |
|---|---|---|---|
| `sessionUpdate` | string | `"model_fallback"` | |
| `from` | string | `claude-sonnet-5`, `claude-haiku-4.5`, `claude-opus-5.5` | always present |
| `to` | string \| null | `claude-haiku-4.5`, `null` | **`null` is real** — arms B/C |
| `cause` | string | `"unavailable"` | never observed `"refused"` (§7) |
| `source` | string \| null | `"user"`, `null` | **NOT in the ticket's `{from,to,cause,promoted}` set** |
| `promoted` | bool | `false` | **present** in every observed frame; never absent, never `true` |

`source` is non-null exactly when a move target existed (`to` non-null) and the target came
from explicit configuration (`kiro.dev/commands/execute {command:"fallback"}`). It is `null`
when `to` is null. Engine-side internal type (debug log, arm A):

```
Internal(ModelFallback { cause: Unavailable, from: Some("claude-sonnet-5"),
                         to: Some("claude-haiku-4.5"), source: Some(User), promoted: false })
```

i.e. the engine struct is exactly `{cause, from, to, source, promoted}` — `source` is a
first-class wire field, not metadata noise. Fixture file has arms A/B/C.

## 2. Exact backend condition that produced it (arm A)

Injected at the **streaming chat call** (the only call the engine will move a turn for):

| item | value |
|---|---|
| endpoint | `POST https://runtime.us-east-1.kiro.dev/` (`x-amz-target: AmazonCodeWhispererStreamingService.GenerateAssistantResponse`) |
| status | `429` |
| header | `x-amzn-errortype: ThrottlingException` |
| body (credentials stripped; no auth material was present) | `{"message": "The model is temporarily unavailable.", "__type": "ThrottlingException", "reason": "INSUFFICIENT_MODEL_CAPACITY", "reason_code": "INSUFFICIENT_MODEL_CAPACITY"}` |
| scope | only requests whose `model_id` == the arm's primary; the fallback model's requests were forwarded untouched and returned 200 |

Engine-side classification evidence (debug log, arm A):

```
chat_cli_v2::api_client::retry_classifier: 37: QCliRetryClassifier: Monthly limit error detected: false
... attempt #1 failed with RetryIndicated(RetryableError { kind: TransientError, retry_after: None })
agent::agent::agent_loop: 282: ... res=Some(Ok(RetryWarning(RetryWarningEvent { attempt: 2, max_attempts: 3,
    delay_secs: 3.0, message: "claude-sonnet-5 is unavailable, retrying in 3s (attempt 2/3)" })))
agent::agent: 3865: moving the rest of this turn to another model from=claude-sonnet-5 to=claude-haiku-4.5 cause=Unavailable source=User
```

Attempts before the move: **3 engine attempts, each with up to 3 smithy attempts = 9
`GenerateAssistantResponse` requests on the failing model** (verified in the proxy request
log), then the move. Eight `retry_warning` notifications were emitted first:

```
{"sessionUpdate": "retry_warning", "attempt": 2, "maxAttempts": 3, "delaySecs": 3.0,
 "message": "claude-sonnet-5 is unavailable, retrying in 3s (attempt 2/3)",
 "capacity": true, "modelId": "claude-sonnet-5"}                      ← capacity retry (2×: 3s, 5s)

{"sessionUpdate": "retry_warning", "attempt": 2, "maxAttempts": 3, "delaySecs": 9.0,
 "message": "Retrying in 9s (attempt 2/3)"}                           ← smithy transport retry (6×: ~10s)
```

Note the `capacity: true` + `modelId` pair on the *capacity* notices — that pair is the
engine's own "this is a per-model capacity failure" marker, and it is what precedes a move.

## 3. Frame order around the fallback (arm A)

```
session/prompt (client→agent)
  _kiro.dev/session/update  retry_warning ×8        (capacity ×2 with capacity:true, transport ×6)
  _kiro.dev/session/update  model_fallback          ← the frame
  session/update            agent_message_chunk     ("OK")
  _kiro.dev/metadata        {contextUsagePercentage}
  _kiro.dev/metadata        {meteringUsage, turnDurationMs, …}
session/prompt RESPONSE     {"stopReason": "end_turn"}   (id 7)
```

The moved turn **completes normally** (`end_turn`) and is billed on the model that answered.
Metering carried by `metadata` for the fallback turn (arm A):

```json
{"contextUsagePercentage": 6.1875,
 "meteringUsage": [{"value": 0.011475607827529023, "unit": "credit", "unitPlural": "credits"}],
 "turnDurationMs": 115544,
 "reasoning": {"support": "toggleable", "effortLevels": ["low","medium","high","xhigh","max"]}}
```

Credits are named `credit`/`credits`; `metadata.reasoning` reflects the **original** model's
capabilities (sonnet-5), not haiku's — the engine does not re-announce the new model's
capability block mid-turn.

## 4. Per-arm results

| arm | primary → configured fallback | injected condition | `model_fallback` frame | outcome |
|---|---|---|---|---|
| **A** (required) | `claude-sonnet-5` → `claude-haiku-4.5` | 429 `ThrottlingException`/`INSUFFICIENT_MODEL_CAPACITY` | **yes** — `from=claude-sonnet-5, to=claude-haiku-4.5, cause=unavailable, source=user, promoted=false` | turn completed on haiku (`end_turn`), billed on haiku |
| **B** (required) | `claude-haiku-4.5` → none (`vendorDefault: []`) | same | **yes** — `from=claude-haiku-4.5, to=null, cause=unavailable, source=null, promoted=false` | turn **failed**: `_kiro.dev/error/rate_limit` + prompt error (§6) |
| **C** | `claude-opus-5.5` → none (`vendorDefault: ["claude-opus-4.8"]`) | same | **yes** — `from=claude-opus-5.5, to=null, …` | same as B ⇒ **vendorDefault is NOT used automatically** |
| **D** | `claude-sonnet-5` → `claude-haiku-4.5` | 503 `__type: ServiceUnavailableException` | **no** | retried, then failed (§7) |
| **E** (optional) | `claude-sonnet-5` → `claude-haiku-4.5` | forged refusal on the response stream | **no** — not produced (§7) | — |
| **F** (after A) | — | none | n/a | session model **unchanged**; next turn ran on the original model (§5) |

Arm B is the answer to "what does `to:null` actually look like": the frame is still emitted
(`source: null`, `promoted: false`) and *then* the turn dies with a rate-limit notification
and a `-32603`. Arm C proves `vendorDefault` is a **hint the client offers** (options
`fallback.vendorDefault`), not an engine-side automatic target: with no explicit
`commands/execute fallback`, opus-5.5 did **not** move to claude-opus-4.8.

## 5. Post-fallback model state (arm F)

Arm A, `commands/options {command:"model"}` after the turn:

```json
{"value": "claude-sonnet-5",
 "fallback": {"configured": "claude-haiku-4.5", "vendorDefault": [], "eligible": true},
 "description": "Claude Sonnet 5 model with 1M context window [active]"}
```

`currentModelId` / the `[active]` marker stay on **`claude-sonnet-5`** — the failing model.
Wire proof of where the next turn ran: the second `session/prompt` produced
`GenerateAssistantResponse` requests with `model_id: "claude-sonnet-5"` (200 OK, forwarded
untouched). So a `promoted:false` `unavailable` move is **turn-scoped**: the session primary
does not change, and the badge (`[active]`) remains on the model that failed.

Arm E could not be produced, so `promoted:true` was never observed live (§7). Static
evidence (engine strings + the Kiro TUI validator) still says promotion is the refusal-only
case.

## 6. User-facing error text (arm B)

Two distinct surfaces:

```json
_kiro.dev/error/rate_limit  {"sessionId": "3d66e7a8-…", "message": "The model you've selected is temporarily unavailable. Please use '/model' to select a different model and try again."}
```

```json
session/prompt RESPONSE error: {"code": -32603, "message": "Internal error",
  "data": "Encountered an error in the response stream: The model you've selected is temporarily unavailable. Please use '/model' to select a different model and try again."}
```

A final `_kiro.dev/metadata` accompanies the failure with `{"turnDurationMs": 101455,
"reasoning": {"support": "unavailable", "effortLevels": []}}` — note `support:"unavailable"`
and an empty effort list, a usable "this model is unusable" signal.

`_kiro.dev/error/rate_limit` **is already converted by cyril** (`convert/kiro.rs:1074`), so
arm B's notice is not a new gap. `retry_warning` and `model_fallback` are both dropped today
(no arm in the same match).

## 7. Negative results

1. **503 / `ServiceUnavailableException` does NOT fall back** (arm D). The engine retried the
   chat call and failed with no `model_fallback` frame. Its notices are a *different class*:

   ```
   {"sessionUpdate": "retry_warning", "attempt": 2, "maxAttempts": 3, "delaySecs": 3.0,
    "message": "Retrying in 3s after a server error (attempt 2/3)", "modelId": "claude-sonnet-5"}
   ```
   — note *no* `capacity` flag, and different text. Engine log: `TransientRetryExecuted {
   class: ServerError, … }`, final usage record `end_reason: Error, request_attempts: Some(9),
   metering_usage: []`. Prompt error: `-32603 … "Encountered an error in the response stream:
   Encountered unexpectedly high load when processing the request, please try again."`
   **This contradicts the static inference in `docs/kiro-2.28.0-wire-audit.md`/the capture
   request** that `ServiceUnavailableException` classifies as `ModelOverloadedError` and
   therefore moves the turn. Live: only the 429/`ThrottlingException` capacity path moves.
2. **`vendorDefault` is not used automatically** (arm C, §4).
3. **Refusal arm (E) not produced.** Four forge attempts were made by serving a hand-built,
   CRC-valid `application/vnd.amazon.eventstream` response for the primary model (the engine
   accepted it — assistant text and metering were delivered, turn completed `end_turn`):
   `stopReason` ∈ {`REFUSAL`, `END_TURN`} × refusal fields in camelCase vs snake_case. The
   engine's own response recorder (`KIRO_RECORD_API_RESPONSES_PATH`) shows the metadata event
   decoded with `refusal_category/refusal_explanation/refusal_recommended_model: null` in all
   four variants — i.e. the refusal fields ride in a form not guessed here (different event
   type or nested object), and **no `cause:"refused"` frame was emitted**. The refusal wire
   shape remains unknown; do not block ci3j on `cause:"refused"` / `promoted:true`.
4. **Probe pitfall worth recording:** `AmazonCodeWhispererService.SendTelemetryEvent`
   (`q.us-east-1.amazonaws.com`) also carries the turn's `model_id`. A model-keyed injection
   policy hits it too and adds ~6 spurious transport-retry notices. Inject only on
   `x-amz-target` ending `GenerateAssistantResponse`.
5. A fifth, minimal-trigger arm (429 with `__type: ThrottlingException` and **no** `reason`/
   `reason_code`) was abandoned: the harness run hung inside `session/new` (engine logged a
   clean `ListAvailableModels` deserialize and then idled; no chat request was ever issued).
   It produced no frames and is not included. The trigger shape in §2 is the one that
   demonstrably worked; whether `reason`/`reason_code` are strictly required is unverified.

## 8. Method (reproducible)

- `HTTPS_PROXY`-style **local TLS-terminating CONNECT proxy** (own CA, leaf per host) with
  `SSL_CERT_FILE=<bundle: system CAs + our CA>` in the probe process env only — the engine's
  rustls stack honored it, so no system/user trust store was touched. All non-target traffic
  was forwarded verbatim to the real hosts (including auth/refresh); the CA private key and
  leaf keys are deleted after the run.
- The endpoint-override route was tried first and **did not work**: setting
  `api.codewhisperer.service = {"endpoint":"http://127.0.0.1:8899","region":"us-east-1"}`
  (valid per `kiro-cli settings list`, and the v1 crate's override format) produced **zero**
  requests at the local server across a full `initialize`+`session/new`; the v2 engine kept
  using `management.kiro.dev`/`runtime.kiro.dev` directly. Interception had to be at the
  transport layer.
- Hygiene: `HOME=<fresh temp>`, real `XDG_DATA_HOME`, one engine process at a time, no
  `login`/`logout`, token fields redacted at capture time, CA material deleted afterwards.
- Harness (scratch, not committed): `/tmp/fbcap/fbproxy.py` (proxy + policy), `probe-arm.py`
  (raw ACP arm driver), `run-arms.sh`. Policy: inject only when
  `x-amz-target` ends `GenerateAssistantResponse` **and** the body's `model_id` equals the
  arm's primary.

## 9. What this changes for cyril-ci3j

1. **`source` must be handled.** It is present on the wire (`"user"` | `null`). The ticket's
   type `{from, to, cause, promoted}` and its "unknown key" stance is fine, but a test that
   round-trips the real frame must not be surprised by it, and the log-on-unknown-field
   behaviour should not fire for a known field. Recommended: `source: Option<String>` in
   `Notification::ModelFallback` (or explicitly ignored with a comment).
2. **`to: null` is real** (arms B/C) and is immediately followed by `_kiro.dev/error/rate_limit`
   plus a `-32603` prompt error — the ticket's "error-styled notice" for `to == null` matches
   observed behaviour.
3. **`promoted` was present and `false` in every live frame**; `promoted` absent and
   `promoted: true` remain unobserved. Keep `absent ⇒ false` (matches the TUI validator), and
   keep the handling generic.
4. **Correct the audit's trigger description**: `unavailable` = HTTP 429 +
   `ThrottlingException` (reason `INSUFFICIENT_MODEL_CAPACITY`); `ServiceUnavailableException`
   /503 is a service-level retry class that fails the turn without a move.
5. Related gap (not ci3j): cyril also drops `retry_warning` — the frames that tell the user
   "claude-sonnet-5 is unavailable, retrying in 3s (attempt 2/3)" arrive 8× before the
   fallback. Worth its own ticket if the retry banner should be surfaced.
