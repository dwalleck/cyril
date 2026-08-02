# cyril-h8zb probe findings (2026-08-01)

Three questions, each probe+oracle agreeing. Probes runnable from repo root.

## Q1 — wire contract: what does the v2 engine emit for refusals?

- **Probe** (`probe-wire-refusal.py`): carve every `model_refusal` site from the sha-verified
  carved `kiro-tui-2.15.0.js` (the metadata **consumer**, TypeScript).
- **Oracle** (`oracle-wire-refusal.sh`): `strings`/`nm` on `kiro-cli-chat` 2.15.0 + 2.16.0 (the
  metadata **producer**, Rust). Contamination guard: the binary embeds tui.js, so the oracle
  filters to short strings + mangled symbols, which minified JS cannot produce.
- **Agreement**: `refusal {category, explanation, recommendedModel}` (camelCase) rides the
  `_kiro.dev/metadata` event alongside `effort`; metadata `stopReason` literal `"CONTENT_FILTERED"`.
  Rust side: `agent::agent_loop::types::RefusalInfo`, `RefusalCategory`/`RefusalDetails`
  (codewhisperer streaming client), standalone `recommendedModel` + `CONTENT_FILTERED` literals,
  telemetry fields `refusal_categor…/refusal_explanat…/refusal_recommen…`, embedded doc
  `model-refusal-alerts`. **Shape identical in 2.15.0 and 2.16.0** (and matches the 2.12.1
  audit carve) — three releases of stability.
- Still no live refusal capture (cannot force a backend refusal); keys remain provisional per
  the schema-vs-runtime discipline, now cross-confirmed by three independent shipped artifacts
  (TS consumer, Rust producer, embedded docs corpus).

### The 2.15.0 metadata handler, verbatim (site 0)

```js
let{refusal:r,stopReason:o}=e;
if(r||o==="CONTENT_FILTERED")
  le.debug("[acp] model refusal",{stopReason:o,refusal:r}),
  this.broadcastStreamEvent({type:"model_refusal",stopReason:o,
    category:r?.category,explanation:r?.explanation,recommendedModel:r?.recommendedModel})
```

### What the issue text did NOT say (new information)

1. **The alert condition is an OR**: a frame with `stopReason:"CONTENT_FILTERED"` and NO
   `refusal` object still alerts. AC2 ("absent refusal = no behavior change") must read
   "absent refusal AND benign stopReason".
2. **Kiro dedupes**: the consumer guards with a boolean (`if(g)break;g=!0`) — only the FIRST
   `model_refusal` event renders. Repeated refusal frames within a turn must not spam chat.
3. **Kiro's fallback text** when `explanation` is absent:
   "The selected model couldn't process this request. Try a different model with /model, rewind
   with /rewind, or start a new session with /chat new." — rendered as a `role:"system"` chat
   message. Cyril's equivalents: `/model`, `/new` (no `/rewind` in cyril).
4. **KAS has a separate refusal path** (site 1): the KAS adapter normalizes `i?.refusal` from a
   session update and emits `model_refusal` with HARDCODED `stopReason:"CONTENT_FILTERED"`.
   Out of scope here (v2-only issue); candidate follow-up — see `to-file.md`.

## Q2 — cyril today: is refusal dropped end-to-end?

- **Probe** (runtime): `cargo test -p cyril-core to_ext_notification_metadata_refusal` — the
  cyril-1gim fence proves the parser recognizes-and-ignores `refusal` + `stopReason` (no
  unknown-key log, no parse failure). PASSES.
- **Oracle** (static, independent mechanism): `grep -c refusal crates/cyril-core/src/types/event.rs`
  → 0 (no Notification field); `grep -rc refusal crates/cyril-ui/src` → no consumers.
- **Agreement**: dropped end-to-end. The only user-visible refusal signal today is the toolbar
  "Refused" chip from ACP `stop_reason: refusal` (`convert/mod.rs:92` → `toolbar.rs:189`).

## Q3 — ordering: can a refusal alert commit inside its turn?

- **Probe** (`probe-ordering.py`): `v2-live-session-trace-2.11.0.jsonl` (KIRO_ACP_RECORD_PATH
  recorder) — every one of 14 turns has its metadata frame IMMEDIATELY before the prompt
  response (…79→80, 98→99, …384→385).
- **Oracle**: two more captures from DIFFERENT recorders/versions/days
  (`trace-2.4.1-multi-subagent.jsonl` proxy capture, 7 turns; `trace-2.4.1-tui-recorder.jsonl`,
  1 turn) — same adjacency.
- **Agreement**: per-turn metadata precedes TurnCompleted. Commit-at-metadata-arrival renders
  inside the turn; do NOT anchor on TurnCompleted (see cyril-9akh ordering race).

## Stale-comment AC (issue AC4)

`types/session.rs:573-575` still claims "The bridge currently hardcodes `StopReason::EndTurn`";
`bridge.rs:1138` extracts the real value via `to_stop_reason(response.stop_reason)`. Confirmed
stale; delete in passing (the issue cited old line numbers 497-499 — code moved).

## One-sentence "what I learned"

Kiro alerts on `refusal || stopReason==="CONTENT_FILTERED"` (OR, not AND), dedupes to the first
event, and the refusal-bearing metadata frame lands immediately before the prompt response —
none of which the issue text stated.
