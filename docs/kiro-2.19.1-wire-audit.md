# Kiro CLI 2.19.1 wire audit

**Audited:** 2026-08-21 (release date 2026-08-21). Patch release; binary `BUILD_HASH=5ac111d2` (built 2026-08-20), archived at `~/.local/share/kiro-research/binaries/2.19.1/` (origin sha verified; installed binary identical). tui.js snapshotted (`kiro-tui-2.19.1.js`, +7 KB). **KAS 0.46.1 → 0.48.0** — two more minors riding a patch; active development continues post-freeze.

**Cyril verdict: SAFE.** v2 wire is quiet (stall/retry/compaction string sets identical; `api.*` key set identical). All action is KAS-side, and everything degrades correctly by construction. One new opt-in wire surface worth adopting (cyril-2mo0).

## 1. NEW wire method: `_kiro/tools/content_chunk` — live shell-output streaming (unannounced)

The ride-along find, and the first `_kiro/*` + `_session/*` vocabulary expansion since 2.16.0's workflow emitter (110 → 111):

```json
{"method": "_kiro/tools/content_chunk", "params": {
  "sessionId": "…", "toolCallId": "…",
  "content": {"type": "content", "content": {"type": "text", "text": "<incremental shell output>"}}}}
```

- Emitted for a **running `execute` tool** in place of content-bearing `tool_call_update`s; the terminal update still carries full output. Chunks are sanitized, **redacted**, and coalesced server-side (`StreamCoalescer`, flush interval + max-bytes cap).
- **Client-gated**: `initialize.clientCapabilities._meta.kiro.streamingShellContent: true` (new member of the `textSearch`/`findFiles` capability family; default off → zero cyril impact today).
- **No shipped client consumes it**: tui.js 2.19.1 has 0 hits for the method and the capability — the emitter shipped ahead of any consumer (the mirror image of `_kiro/diagnostics/changed`, where the TUI subscribes to a non-existent emitter — still 0 in 0.48.0). Cyril can be the first client with live shell streaming → **cyril-2mo0**.

### LIVE CAPTURE (0.48.0, capability advertised; `kas-content-chunk-2.19.1.jsonl`)

Prompted `for i in 1 2 3 4 5; do echo "tick $i at $(date +%T)"; sleep 1; done; echo "key: AKIA…"; echo done`:

```
+2.72s tool_call         status=pending  title='Run Command'
+2.86s tool_call_update  status=in_progress   (×2, no content — chunks replace content updates)
+2.90s content_chunk  text='tick 1 at 18:54:35\n'
+3.90s content_chunk  text='tick 2 at 18:54:36\n'
+4.91s content_chunk  text='tick 3 at 18:54:37\n'
+5.91s content_chunk  text='tick 4 at 18:54:38\n'
+6.92s content_chunk  text='tick 5 at 18:54:39\n'
+7.89s content_chunk  text='key: AKIA1234567890ABCDEF\ndone\n'
+7.89s tool_call_update  status=completed
       content[0].content.text = "{\"output\":\"tick 1 at 18:54:35\\n…\\ndone\\n\"}"   ← FULL output, JSON-encoded
```

Live-established facts:

- **True real-time streaming**: one chunk per output line, ~1.00 s cadence matching the `sleep 1` ticks; delivery latency after the line appeared is sub-100 ms (`date +%T` values in the text match wall clock). The coalescer's flush interval is ≤1 s.
- **`toolCallId` correlates directly** with the `tool_call` lifecycle id (`run_command_toolu_bdrk_…`); mid-run `tool_call_update`s carry no content while chunks flow.
- **Terminal-update dedup contract**: the `completed` update's content is the **full accumulated output, wrapped as a JSON object string** (`{"output":"…"}`) inside the text block — a renderer that appended chunks must *replace* (or drop) the terminal content, and note the shape difference: chunks are raw text, the terminal is JSON-in-text.
- **Redaction is value-based, not pattern-based**: the fake `AKIA1234567890ABCDEF` passed through verbatim. `createStreamingRedactor` masks only the *values* of four sensitive env vars (`__MDE_ENV_API_AUTHORIZATION_TOKEN`, `__MDE_ENVIRONMENT_API`, `AWS_CONTAINER_CREDENTIALS_FULL_URI`, `ACTIVITY_LOG_QUEUE_ARN` — cloud/MDE sandbox credentials) as `[REDACTED:<name>]`, with a hold-back window sized to the longest value so a secret split across chunk boundaries cannot leak. On a normal workstation those vars are unset and the redactor is inert.

## 2. [V3] "Always allow" suppression — new consent fields (LIVE-PROBED)

New `src/acp/permission-options.ts`:

```js
canPersist      = consent && consent.persistableConsent !== false
canAlwaysAllow  = canPersist && consent.askType !== "explicit"
options = [ Allow(allow_once),
            …canAlwaysAllow ? Always allow(allow_always) : dropped,
            Deny(reject_once),
            …canPersist ? Always deny(reject_always) : dropped ]
```

`consentIsPersistable` verifies the candidate permission rule **round-trips** (rule derived from the command must match the command; check failure → degraded → false). When false, `_meta.kiro.consent` gains two new fields: `persistableConsent: false` and `persistableConsentReason` ("Kiro could not parse this command, so a saved rule would never match it. …", with a cmd.exe variant recommending PowerShell). This is the KAS analogue of v2 2.18.1's tar `trustOptions` suppression — but explanation-bearing and parser-driven rather than dangerous-flag-listed.

Live A/B (0.48.0, autopilot→off so approvals fire; probes committed): benign `echo hello`, command substitution + `eval`, a **multi-line command with a literal newline**, and a **heredoc** ALL drew the full 4-option set with clean parses (`triggeringResource` correctly extracted: `ls`, `echo first`, `cat`). The suppression was **not trippable with ordinary bash** — the parser is robust; the reason text naming cmd.exe suggests the practical trigger is mostly Windows. Confirmed consent shape when persistable: `{capability, resource, askType, triggeringResource, workspaceRoot}` — `persistableConsent` simply absent.

**Cyril impact: none required.** The approval overlay renders the offered `options` list dynamically (`state.rs show_approval` → windowed list), so a missing Always degrades correctly. Optional nicety whenever approval UI is next touched: surface `persistableConsentReason` so users know why Always is absent.

## 3. Other 2.19.1 items (verified where cheap)

- **Announced retry fixes** ([V3] EPIPE retry; truncated non-file tool-call retry loop): internal; no new wire vocabulary (`retry-wait` 2=2, watchdog strings still 0 in KAS — the stall window on v3 remains open, TurnStalled chip stays load-bearing).
- **[V3] initialize latency** (no longer waits on experiment resolution): timing-only. Related live observation: the model configOption's registry-settlement race is still visible — one probe's `set_config_option` response included `model` in the rebuilt list, another didn't (same day). KAS-4 consumers must stay update-driven.
- **Security: `grep_search`/`file_search` honor `.kiroignore`**: wiring of existing ignore machinery (string counts unchanged) into the search tools; agent-internal, no wire change.
- **`capturedOutput` still broken**: the extractor + transcript-fallback region is functionally byte-identical to 0.46.1 (only a bundler identifier rename) — the 2.19.0 audit § 14 finding stands unchanged in 0.48.0.
- **Doc manifest: zero drift** — identical path/title sets; 2.19.1 actually embeds a slightly *older* generation than 2.19.0 (2026-08-17 vs 08-19; `features/session-management.md` shows the pre-revalidation entry again). No new baselines committed.
- tmux/terminal-attribute fixes, `/knowledge` autocomplete + `rm` alias: TUI-side.

## 3b. Token usage: still absent from the client wire — now precisely mapped (0.48.0)

Checked because both engines were unfrozen this cycle. **Still no per-turn token counts anywhere a client can see**, but the plumbing is fully mapped:

- **The standard ACP slots exist and kiro leaves both empty.** The vendored protocol schema in the KAS bundle defines (a) the `usage_update` session-update kind (present only as a zod validator — zero emitters; omp fills this slot) and (b) `session/prompt` response `usage {inputTokens, outputTokens, totalTokens, cachedReadTokens?, cachedWriteTokens?, thoughtTokens?}` — every live prompt response this audit was a bare `{stopReason}`.
- **The numbers reach the agent.** The backend stream's `metadataEvent.tokenUsage {uncachedInputTokens, outputTokens, cacheReadInputTokens, cacheWriteInputTokens}` is parsed per request; KAS routes it to AWS telemetry histograms (`reportHistogramMetrics({inputTokens, outputTokens, cacheRead/WriteInputTokens})`), OTel `GEN_AI_USAGE_*` span attributes, and internal retry-safety accounting (`emissions.tokenMetadata`) — never to any ACP emission.
- **What a client does get, unchanged:** credits (`session_info_update kind:turn_completion promptTurnSummaries`, v2 `meteringUsage`, `_kiro/account/getUsage`) and the KAS `context_usage` breakdown's per-bucket token counts — which measure prompt *composition* (e.g. `tools: {tokens: 5063}`), not consumption. Persisted transcripts (`usage_summary`) are credits-only too.

### Can a client observe the telemetry token data WITHOUT modifying KAS? Yes — but the counts are backend-gated (LIVE-PROBED)

`initializeAgentTelemetry` runs **unconditionally** at KiroAgent startup (no `telemetry.enabled`/content-collection gate), and with no host-supplied providers it builds an OTLP exporter to `resolveTelemetryEndpoint()`, which honors **`OTEL_EXPORTER_OTLP_ENDPOINT` first**. So a client can divert Kiro's own telemetry to a local collector with one env var — **no bundle modification** (`probe-kas-otel-tokens-2.19.1.py`, summary `otel-token-capture-summary-2.19.1.json`):

- **Diversion PROVEN**: KAS POSTed `/v1/metrics` (JSON, ~223 KB) + `/v1/traces` (~55 KB) to a `127.0.0.1` sink — **125 metric streams**, per-turn `QApi.{inputSize:54016, outputSize:360, attempts:1, toolCalls:0, timeToFirstToken:1499ms, duration:3701ms, events:43}`, plus `AgentExecution.timeToFirstInference`, `contextUsagePercentAtModelResponse`, model-settlement, steering, fs, retry metrics.
- **But token COUNTS were ABSENT.** The `inputTokens`/`outputTokens`/`cacheRead/WriteInputTokens` fields ride the QApi histogram *conditionally* (`…modelMetrics.inputTokens !== void 0 && {…}`); the histogram fired fully yet emitted no token-count datapoint — i.e. `metadataEvent.tokenUsage` was not populated by the backend for `model=auto` on this turn. Only `timeToFirstToken` (a latency) carries the word "token". This is the **wire = binary × backend** axis: token-count emission is a *backend* property, currently off for this account/model — so the earlier "the data exists, KAS just drops it from the client wire" framing was **too strong**. The client-facing plumbing to carry it is present (the histogram field, the standard ACP `usage` slot), but the numbers are not arriving from the backend to begin with. Modifying KAS would not surface counts the backend isn't sending.
- The LangSmith `GEN_AI_USAGE_*` span path is **dark** in the standalone host (needs `OTEL_ENABLED`/`LANGSMITH_TRACING_MODE` + langsmith `initializeOTEL()`, which the CLI doesn't wire) — 0 `gen_ai` attributes in the captured traces.
- **Model sweep — the gate is ACCOUNT/BACKEND-level, not model-selectable (LIVE-PROBED).** Re-ran the OTLP capture pinning five explicit models across every provider family via `session/set_config_option model=<id>` (`probe-kas-otel-model-2.19.1.py`): `claude-sonnet-4.5`, `claude-opus-5`, `gpt-5.6-sol`, `deepseek-3.2`, `qwen3-coder-next` (catalog is 19 models: claude-\*, gpt-5.6-\*, deepseek, glm, minimax, qwen). **Every run was validly measured** (`QApi.inputSize=53974` + `QApi.success` present, 128 metric streams) and **every run emitted zero token-count metrics** — Anthropic, OpenAI, DeepSeek, Qwen alike. So pinning a model does not surface counts; `metadataEvent.tokenUsage` is simply not populated by the backend for this Tethys/IdC account right now, across all providers.
- **Net for a token-usage feature**: three non-modification observation paths exist — (a) this OTLP redirect, (b) TLS interception of the CodeWhisperer/KRS stream to read raw `metadataEvent.tokenUsage`, (c) a fresh AWS prompt-log pull — but all three are capped by the same account/backend gate, which a model choice cannot lift. The blocker is upstream emission, not KAS concealment. Re-probe on a different account/tier, or when a backend rollout starts populating `tokenUsage` (the `wire = binary × backend` watch signal).

## 4. Artifacts

- Probes: `experiments/conductor-spike/probe-kas-perm-persist-2.19.1.py` (+ the second-arm variant is parameter tweaks of the same file).
- Captures (token-scrubbed): `experiments/conductor-spike/kas-perm-persist-2.19.1.jsonl`, `kas-perm-persist2-2.19.1.jsonl` — full permission frames incl. consent meta and 4-option lists; fixture material for approval tests.
- Issues: **cyril-2mo0** (streamingShellContent + content_chunk rendering).
- Archive: binaries + BUILD-INFO + SHA256SUMS at `~/.local/share/kiro-research/binaries/2.19.1/`; `tui-bundles/kiro-tui-2.19.1.js`.
