# Usage observer — P1 (omp-wire) design note

Design-first artifact for **cyril-gfkm** (epic cyril-4h6i). Gates the record schema on the *live* `omp acp` wire before any code, per the issue's requirement and the repo's schema-vs-runtime discipline. Probe + fixture: `experiments/conductor-spike/probe-omp-usage-update.py`, `omp-usage-update-2turn.jsonl` (omp 17.3.5, 2026-08-22).

## The wire, confirmed (2-turn live capture)

Per-turn token usage and the session cost/context gauge ride **two different surfaces** — a distinction the earlier omp-JSONL read did not reveal:

### 1. Per-turn tokens — on the `session/prompt` RESPONSE, not a notification

```
T1 response.usage: {"inputTokens":19428, "outputTokens":18, "totalTokens":19446}
T2 response.usage: {"inputTokens":259,  "outputTokens":5,  "totalTokens":19464, "cachedReadTokens":19200}
```

This is exactly the standard ACP `Usage` type (`unstable_session_usage`), which **cyril already enables** in the workspace `Cargo.toml` (`agent-client-protocol` features `["unstable_session_model", "unstable_session_usage"]`):

```rust
pub struct Usage { total_tokens: u64, input_tokens: u64, output_tokens: u64,
                   cached_read_tokens: Option<u64>, cached_write_tokens: Option<u64> }
```

Semantics (derived from T2): `input_tokens` is **uncached** input; `total_tokens = input_tokens + cached_read_tokens + output_tokens`. `cached_read_tokens` is present only on a cache hit (absent T1, 19200 T2). `cached_write_tokens` was never populated by omp (its JSONL also always showed 0) — model `Option`, tolerate `None`. This maps 1:1 onto the omp-dashboard tiles: Uncached Input / Cache Read / Output Tokens / Conversation Total, and Cache Rate = `cached_read / total`.

**cyril currently DROPS this.** `bridge.rs` (~L1545) reads only `response.stop_reason` to build `Notification::TurnCompleted { stop_reason }`; `response.usage` is discarded. First code change is here.

### 2. Session gauge + cost — on the `usage_update` notification (cumulative)

```
{"sessionUpdate":"usage_update", "size":272000, "used":19428, "cost":{"amount":0.0038916,"currency":"USD"}}
```

- `size` = context-window limit; `used` = current context tokens (grew 19428→19459 across turns).
- `cost` is **CUMULATIVE session total** (0.0039072 → 0.0043490 across T1→T2), NOT per-turn. Per-turn cost = **delta** the cumulative between turn boundaries. No cache split, no per-turn tokens here.

### 3. Model / provider — from the `model` configOption

`session/new` `configOptions` includes a `model` select with `currentValue: "openai-codex/gpt-5.6-luna"`, options like `"deepseek/deepseek-v4-flash"` — format **`provider/model`**. Attribution = read `model` currentValue, track changes via `config_option_update`. Same configOptions machinery as KAS-4; no new wire surface.

## Record schema (engine-agnostic)

```
TurnUsage {              // per completed turn
  tokens: { total, input_uncached, output, cached_read?, cached_write? },  // ACP Usage
  cost: Option<Money{ amount, currency }>,   // per-turn, delta of cumulative (None if agent sends no cost)
  model: Option<String>, provider: Option<String>,  // from model configOption at turn time
  // cyril-native, no engine dependency:
  duration_ms, ttft_ms,          // client stopwatch: prompt-sent → response / first agent_message_chunk
  tool_calls: Vec<ToolUse>,      // from tool_call notifications already tracked
  stop_reason, session_id, folder, ts,
}
SessionGauge { context_used, context_size, cost_cumulative }  // latest usage_update
```

The layer above `TurnUsage` (log + aggregation) must have **zero engine-specific branches** — that seam is the whole point of P2 (kiro fills the same record from credits + JSONL, per cyril-kryv).

## Implementation slices (P1)

1. **Capture** — domain `TurnUsage`/`Usage` newtype in `cyril-core` (convert boundary: no `acp::` past `convert/`); in `bridge.rs`, read `response.usage` on the Ok arm; attach to `TurnCompleted` (add a `usage: Option<TurnUsage>` field). Cost delta + `usage_update` (`SessionGauge`) parsed in `convert/`. Fence: a completed turn carries usage from a captured `PromptResponse`.
2. **Log** — append-only per-turn `UsageRecord` keyed by session/folder/model/provider, holding tokens + per-turn cost (cost = gauge-delta) + duration/TTFT (client stopwatch) + tool calls. Model attribution from the tracked `model` configOption.
3. **Aggregate** — engine-agnostic rollups: overview (requests, tokens, cache rate, cost, TTFT), by-model, by-provider, by-tool (credits/tokens attributed across the turn's tool calls), recent, errors, by-folder.
4. **View** — a TUI usage panel and/or a small local viewer (decide during slice 4; the OTLP-redirect dependency is already dropped — cyril's wire + stopwatch cover it).

## Gaps vs omp's dashboard (wire-only)

- `cached_write_tokens` and "premium requests" never populated by omp on the wire — render 0/absent, not a sentinel.
- Cache-write and some per-tool char totals live only in omp's JSONL; wire gives per-tool *calls* + attributed tokens/cost. JSONL backfill (P1 slice-2 optional) closes it for historical sessions.
