# Capture request: a real v2 `model_fallback` frame (cyril-ci3j live gate)

Requested 2026-10-08. Engine: the **v2 (Rust) engine**, `kiro-cli acp`, version **2.28.0**
(installed `~/.local/bin/kiro-cli-chat` is byte-identical to the archived 2.28.0;
sha256 prefix `ae5e172a905bd007`).

## Why

kiro-cli 2.27.1 turned v2 model fallback on by default. When the turn's model is over
capacity, or refuses the turn, the engine moves the rest of the turn to another model and
tells the ACP client with a `kiro.dev/session/update` notification:
`sessionUpdate: "model_fallback"`. cyril drops this notification today, so its model
badge goes stale while the turn, and its billing, runs on a different model.

**cyril-ci3j** fixes that. Its acceptance has a **live gate**: the converter must be
exercised against a **real captured frame**. So far we only have the shape that Kiro's own
TUI validates, not one the engine actually sent:

```json
{"sessionId": "…", "update": {"sessionUpdate": "model_fallback",
  "from": "<model id>", "to": "<model id>" | null,
  "cause": "refused" | "unavailable", "promoted": true | false /* optional */}}
```

Attempts so far:
- **Bogus model id:** `session/set_model` with a made-up id fails the next prompt with
  `-32603 "model … not available"` and **no** frame. That's the `InvalidModelId` path,
  which does not fall back.
- **Agent fault injection:** an agent-run proxy that injected backend errors was blocked
  by Claude Code safety checks. So the capture is handed to you.

## Reference files

| what | path |
|---|---|
| Ticket: design decisions, live gate, static findings (read its notes) | `rivets show cyril-ci3j`; epic `rivets show cyril-pouu` |
| Audit synthesis (§3, the v2 engine) | `docs/kiro-2.28.0-wire-audit.md` |
| Per-lane evidence (search "fallback") | `experiments/conductor-spike/audit-2.28.0/{host,tui,live}-findings.md` |
| Raw-ACP v2 driver to reuse: PATH-pinned launcher, `/proc` child-exe proof, `HOME` isolation, frame recorder with token scrub | `experiments/conductor-spike/probe-v2-leads-2.28.0.py` (mode `main` already does `commands/options` → `commands/execute fallback` → prompt) and `probe-v2-live-sweep-2.28.0.py` |
| A real 2.28.0 v2 capture, showing the frame layout to match (`{ts, dir, tag, msg}`) and the live `fallback{}` option block (tag `opts_model`) | `experiments/conductor-spike/v2-live-sweep-2.28.0.jsonl` |
| ACP-level logging proxy (client ↔ engine), for capturing a natural occurrence under a TUI | `experiments/kiro-proxy-rs/` (`run-with-proxy.sh`, `KIRO_PROXY_LOG`) |
| Kiro-native ACP frame recording in Kiro's own TUI | env `KIRO_ACP_RECORD_PATH=/path/trace.jsonl` (records `{ts, dir, msg}`) |
| Field-path differ, if you capture both a normal and a fallback turn | `experiments/conductor-spike/sweep-new-fields.py` (caveat: walks only 5 array elements, cyril-66tl) |
| Probe hygiene rules | `CLAUDE.md` § Reverse Engineering; memory `feedback_isolate_kiro_probes_with_home.md`, `feedback_kiro_token_renewal_single_flight.md` |

## What the binary says about triggering it (static, 2.28.0 `kiro-cli-chat`)

- **Chat call:** `GenerateAssistantResponse` on the CodeWhisperer streaming client.
  Endpoints include `q.us-east-1.amazonaws.com` and `runtime.{us-east-1,eu-central-1}.kiro.dev`.
  Endpoint override setting: `api.codewhisperer.service`.
- **Proxy and CA env the binary reads** (unproven, so verify that traffic really flows):
  `HTTPS_PROXY`/`https_proxy`, `HTTP_PROXY`, `ALL_PROXY`/`all_proxy`, `NO_PROXY`,
  `SSL_CERT_FILE`, `SSL_CERT_DIR`. The TLS stack is reqwest + aws-smithy-http-client +
  rustls-platform-verifier.
- **Errors that should lead to fallback** (the "unavailable" path, classified as
  `ModelOverloadedError`):
  - `ThrottlingException` with reason `INSUFFICIENT_MODEL_CAPACITY`, most likely HTTP 429;
  - `ServiceUnavailableException`, HTTP 503.

  The engine first retries, emitting `_kiro.dev/session/update` retry notices ("The model is
  unavailable, retrying in s (attempt …"), and only then decides whether to move.
- **Errors that won't:** `INVALID_MODEL_ID` / `InvalidModelId`, and `MONTHLY_REQUEST_COUNT`
  (monthly limit) are separate kinds.
- **The "refused" path:** `RefusalInfo{explanation, recommended_model}` on the response
  stream. The model list carries `refusal_fallback_models`, which is probably where the
  options' `vendorDefault` comes from.
- **Promotion seems to be refusal-only:**
  - "a refused turn was answered elsewhere, so that model becomes the conversation's primary"
  - "the promotion was withheld: the turn no longer runs on the model the walk named"

  So expect `promoted:true` only with `cause:"refused"`.
- **Engine decision log strings:**
  - "moving the rest of this turn to another model"
  - "the turn stays on the model that failed" (cause `no_target`)
  - "the failing model is unknown, so the turn stays where it is"
  - "the turn stays where it is, since the handle is fixed or an explicit pick moved it off the model that failed"
- **User-facing texts when no move happens:**
  - "No fallback model is configured for it."
  - "It has no other model to fall back to."
  - "Every fallback model has already been tried."
  - "This request has already changed models as often as it can."
  - "Its fallback models are not available right now: …"
  - "The model you've selected is temporarily unavailable."

**Fallback targets as served on 2026-10-08** (`commands/options {command:"model"}`).
Every model is `eligible:true`. Only these have a vendor default:

| model | `vendorDefault` |
|---|---|
| `claude-opus-5.5` | `["claude-opus-4.8"]` |
| `claude-sonnet-5.5` | `["claude-sonnet-5"]` |
| `claude-opus-5` | `["claude-opus-4.8"]` |

Every other model, including `auto`, `claude-sonnet-5` and `claude-haiku-4.5`, has
`vendorDefault: []`. Set an explicit target with an awaited JSON-RPC **request**
`kiro.dev/commands/execute {command:"fallback", args:{targetModelId, fallbackModelId}}`.
The reply is "Fallback for X set to Y." and the setting is session-scoped. Omitting
`fallbackModelId` clears it.

## Arms

Run each arm in a fresh session if you can. The ACP order is: `initialize` → `session/new`
→ `session/set_model` (primary) → optional `commands/execute fallback` →
`session/prompt` with a short harmless prompt. Leave other models' requests untouched.

| arm | primary model | fallback setup | backend condition for the primary only | expected |
|---|---|---|---|---|
| **A** (required) | `claude-sonnet-5` | `fallbackModelId: claude-haiku-4.5` | capacity error (429 `INSUFFICIENT_MODEL_CAPACITY`) on every attempt | `model_fallback {from: sonnet-5, to: haiku-4.5, cause: unavailable, promoted: false?}`; the turn completes on haiku |
| **B** (required) | `claude-haiku-4.5` | none (vendorDefault empty) | same | `to: null`, or no frame plus a "no fallback configured" error; record which |
| **C** | `claude-opus-5.5` | none (relies on vendorDefault `claude-opus-4.8`) | same | does vendorDefault get used without configuration? |
| **D** | `claude-sonnet-5` | `claude-haiku-4.5` | 503 `ServiceUnavailableException` instead of 429 | same classification as A? |
| **E** (optional) | `claude-sonnet-5` | `claude-haiku-4.5` | a refusal on the response stream (`RefusalInfo`), not an error | `cause: refused`, `promoted: true`? |
| **F** (follow-up after A and E) | — | — | none | `commands/options {command:"model"}` plus one more prompt: did the session's current model change? |

Arm **E** must be produced by the backend condition, never by writing harmful prompts.

## Hygiene (each run)

- `HOME=<fresh temp dir>` and `XDG_DATA_HOME=$HOME_REAL/.local/share`. KAS deletes old
  log dirs under `$HOME`; this is v2, but keep the habit.
- Pass proxy and CA settings **only** in the probe process's environment. Never change
  system or user trust stores.
- Run one engine process at a time. No `kiro-cli login`/`logout`. Don't run anything else
  that renews the token in parallel: the refresh token is single-use, and racing it logs
  you out.
- Redact `Authorization` and bearer tokens at capture time. Afterwards, delete the CA
  private key and the proxy state.

## What I need back

Drop these into the repo, uncommitted is fine:

1. **`experiments/conductor-spike/v2-model-fallback-2.28.0.jsonl`.** Every ACP frame,
   both directions, for every arm. One JSON object per line:
   `{ts, dir: "client->agent"|"agent->client", arm, msg}`. Include `initialize`,
   `session/new`, `set_model`, `commands/execute` and its reply, every `session/update`
   and `_kiro.dev/*` notification (retry notices, `metadata` with `meteringUsage`), and
   the `session/prompt` **response** with its `stopReason`. Tokens redacted.
2. **`crates/cyril-core/tests/fixtures/v2/model-fallback-live-2.28.0.json`.** Just the
   real `model_fallback` frame(s), verbatim, redacted, as a JSON array. Label each element
   with its arm, e.g. `{"arm": "A", "frame": {…}}`.
3. **`experiments/conductor-spike/v2-model-fallback-2.28.0.md`.** Short results:
   - per arm: did a `model_fallback` frame arrive? Paste it verbatim. Note every key and
     type, especially any key **not** in `{from, to, cause, promoted}`, whether `promoted`
     was present, and whether `to` was ever `null`;
   - the order of frames around the fallback: retry notices → `model_fallback` → agent
     text → `metadata` → prompt response;
   - which model `metadata.meteringUsage` and the credits name for the moved turn;
   - the `stopReason`;
   - after arms A and E: the `currentModelId` / options `[active]` marker and whether the
     next turn ran on the new model;
   - any user-facing error text (arm B);
   - the exact backend response that triggered it (HTTP status, exception type, body
     shape, with credentials stripped) and how many attempts the engine made before
     moving;
   - proof of which binary ran: child exe path plus sha256 prefix;
   - negative results matter too. If an arm produced no frame, say what it did produce.

When those land, I'll replace or supplement ci3j's synthetic fixture with the real frames,
adjust the ci3j design if the shape differs, and do the end-to-end cyril run that closes
the gate.
