# cyril-nhzw — Falsifiable Design: marshal kiro-cli settings into the KAS `_meta.kiro.settings` handshake

**Status:** cheapest falsifier PASSED (see §Falsification). Ready for `budgeted-plan`.
**Prove-it:** CONFIRMED — `experiments/conductor-spike/probe-kas-settings-subagent-orchestration-2.10.0.py`. Live 2.10.0 A/B: sending `_meta.kiro.settings.subagentOrchestration={enabled:true}` made the agent invoke `orchestrate_subagent` (DAG `pipeline.stages[]`); sending no settings → `invoke_sub_agent`. KAS reads the handshake.

## Purpose

Today `KasEngine::client_capabilities()` advertises `fs`+`terminal` but sends **no `_meta.kiro.settings`**, so every KAS session runs with KAS's bare fallback flags. This feature marshals the user's kiro-cli settings (`~/.kiro/settings/cli.json`) into an `AgentSettings` object and attaches it as `clientCapabilities._meta.kiro.settings` at `initialize`, so KAS honors the same feature flags the v2 client would (`thinking`, `toolSearch`, `codeIntelligence`, `subagentOrchestration`, …). Behind the `kas` cargo feature; V2Engine is untouched.

The design **faithfully replicates v2's `zme()`** (extracted verbatim from `kiro-tui-2.8.1.js`) rather than inventing cyril's own posture — this respects the user's config *and* gives v2 parity, and makes the mapping a finite, testable table rather than a judgment call.

## Architecture / components

- **New module `crates/cyril-core/src/protocol/kas/settings.rs`** (`#[cfg(feature = "kas")]`):
  - `read_cli_settings() -> serde_json::Map<String, Value>` — read `~/.kiro/settings/cli.json`; **absent file → empty map** (Ok, not error); **malformed JSON → empty map + `warn!`** (distinguish missing from corrupt; never crash). Home resolved via the platform helper, not `$HOME` string-concat.
  - `marshal_agent_settings(&Map) -> serde_json::Value` — the **zme table** (below) → an `AgentSettings` JSON object. Pure function; the whole test surface.
  - `kiro_settings_meta() -> acp::Meta` — wrap as `{"kiro": {"settings": <AgentSettings>}}` (`acp::Meta = serde_json::Map<String, Value>`).
- **Wire into `KasEngine::client_capabilities()`** — set `ClientCapabilities.meta = Some(kiro_settings_meta())` (field is `pub meta: Option<Meta>`, `rename="_meta"`). Composes with the existing `.fs(...).terminal(true)`.

### The zme mapping table (verbatim from `kiro-tui-2.8.1.js`, the independent source of truth)

Boolean keys (`cli.json key → AgentSettings key`, value wrapped `{enabled: <bool>}`, non-boolean values **ignored**):

| cli.json key | AgentSettings key |
|---|---|
| `chat.enableThinking` | `thinking` |
| `chat.enableKnowledge` | `knowledge` |
| `chat.enableCodeIntelligence` | `codeIntelligence` |
| `chat.enableTodoList` | `todoList` |
| `chat.enableCheckpoint` | `checkpoint` |
| `chat.enableTangentMode` | `tangentMode` |
| `chat.disableAutoCompaction` | `disableAutoCompaction` |
| `chat.enableSubagent` | `_subagent` |
| `chat.enableDelegate` | `_delegate` |

Defaults `{enabled:true}` applied only when the key is absent after mapping: `codeIntelligence`, `knowledge`, `thinking`, `subagentOrchestration` (note: `subagentOrchestration` has **no** cli.json source key — it is always default-on unless a future kiro-cli adds one).

Nested objects:
- `toolSearch` — emitted iff `toolSearch.enabled` is a boolean → `{enabled}` plus `minPct`/`minTokens` **iff numeric**.
- `compaction` — emitted iff `compaction.excludeContextWindowPercent` **or** `compaction.excludeMessages` is numeric → `{enabled:true, excludePercent?, excludeMessages?}`.

`semanticReview`/`fta` are **not** sent by zme (they are KAS-side defaults: semanticReview ON, fta OFF); cyril does not synthesize them.

## Input shapes

Input is the flattened settings map `e: Map<String, Value>` (cli.json top-level; keys are literal dotted strings, e.g. `"chat.enableThinking"` — the file stores dotted keys at top level alongside a separate nested `chat` object which zme **ignores**).

1. **File absent** (no `~/.kiro/settings/cli.json`) → empty map → defaults-only AgentSettings. → Claim 7
2. **File malformed** (invalid JSON) → empty map + `warn!` → defaults-only. → Claim 8
3. **Mapped bool key = `true`** → `{enabled:true}`. → Claims 2, 9
4. **Mapped bool key = `false`** → `{enabled:false}` (must NOT be dropped or defaulted-on). → Claim 4b
5. **Mapped key present, non-boolean** (e.g. `"chat.enableThinking": "yes"` or `1`) → ignored, then default logic applies. → Claim 3
6. **Mapped key absent, is a default key** (codeIntelligence/knowledge/thinking/subagentOrchestration) → `{enabled:true}`. → Claim 4
7. **Mapped key absent, non-default key** (checkpoint/tangentMode/_subagent/…) → omitted. → Claim 4
8. **`toolSearch.enabled` present bool, minPct/minTokens present numeric** → full object. → Claim 5
9. **`toolSearch.enabled` present, minPct/minTokens absent or non-numeric** → `{enabled}` only. → Claim 5
10. **`toolSearch.enabled` absent** (but minPct present) → `toolSearch` omitted entirely. → Claim 5b
11. **compaction: neither / only-percent / only-messages / both numeric** → omitted / partial / partial / full. → Claim 6
12. **Live fixture** (the real cli.json) → integration of 3/6/8. → Claim 9

Out of scope (with reason):
- **Workspace-scoped cli.json overlay** — kiro-cli has a `--workspace` scope, but v1 reads global only; path unconfirmed, none on disk. Tracked: **cyril-sa39**.
- **`knowledge`/`compaction` extended sub-fields** (`includePatterns`, `maxFiles`, `chunkSize`, …) — zme's tail sets more `knowledge` fields; v1 emits `{enabled}` for knowledge and the two compaction excludes only. If a live turn shows KAS needs the extras, they are additive. Tracked: **cyril-sa39** is workspace-only; the extended-fields deferral is filed as its own note below (see Negative space) — **filed: cyril-sa39 covers workspace, extended-fields = TODO in the plan, not shipped silently**.

## Removed-invariant sweep (step 2b)

**Core move: subtractive.** The change removes the invariant *"cyril sends no settings, so KAS runs its bare-fallback posture."* Enabling flags makes previously-impossible things happen. Walk the chain:

- **`thinking:{enabled:true}`** → KAS may now emit `agent_thought_chunk` on a plain turn. "Can't happen" removed: *no thought chunks on KAS*. Reader: the KAS converter arm. KAS-2a added an `agent_thought_chunk` arm, so this is handled — but must be **verified**, because an unhandled/typed-unknown `session/update` variant **hard-fails at acp deserialization** (no `#[serde(other)]`). → Claim 11
- **`subagentOrchestration:{enabled:true}`** → `orchestrate_subagent` tool_calls with `_meta.kiro.pipeline.stages[]` now appear. "Can't happen" removed: *only `invoke_sub_agent` subagents*. Reader: the tool_call renderer / `SubagentTracker`. KAS-3 crew rendering (**cyril-fjfu**) is not built, so these render as **generic tool_calls** — not a crash, but unrendered crews. → Claim 11 (no-hang) + Negative space (rendering is fjfu's job).
- **`toolSearch:{enabled:true}`** → the tools context bucket shrinks (lazy MCP loading). "Can't happen" removed: *full tools bucket*. Reader: the context-usage bar (cyril-5et2). Additive/among-buckets; no hard-fail. Noted safe — 5et2 already renders the aggregate `tools` bucket regardless of size.
- **`codeIntelligence`/`knowledge`/`todoList:{enabled:true}`** → may surface extra notifications/tools. Noted safe: additive; any unknown typed `session/update` is caught by Claim 11's no-hang falsifier, and unknown `_kiro/*` ext frames ride the JSON-tolerant `debug!` drop path.

The single load-bearing risk the sweep surfaces: **some enabled flag causes KAS to emit a typed `session/update` variant cyril's acp 0.11.2 can't deserialize → hard-fail mid-turn.** Claim 11 is its falsifier.

## Claims

1. Under `kas`, `KasEngine::client_capabilities().meta` is `Some(m)` with `m["kiro"]["settings"]` an object; `V2Engine::client_capabilities().meta` is `None`.
2. Each of the 9 boolean cli.json keys present as a bool maps to its AgentSettings key as `{enabled:<bool>}`, using exactly the zme table (no renamed/dropped key).
3. A mapped key whose value is not a JSON boolean is ignored (no coercion), then default logic applies.
4. The four default keys are emitted `{enabled:true}` when absent; non-default mapped keys are omitted when absent.
4b. A mapped bool key present as `false` is emitted `{enabled:false}` (not dropped, not overridden by a default).
5. `toolSearch` is emitted iff `toolSearch.enabled` is a bool; `minPct`/`minTokens` are included iff numeric.
5b. When `toolSearch.enabled` is absent, `toolSearch` is omitted entirely even if `minPct`/`minTokens` are present.
6. `compaction` is emitted iff at least one of `excludeContextWindowPercent`/`excludeMessages` is numeric; each sub-field is included iff numeric.
7. An absent settings file yields the defaults-only AgentSettings (4 default keys), no error, no panic.
8. A malformed settings file yields the defaults-only AgentSettings plus a logged warning, no panic (missing ≠ corrupt).
9. For the live `~/.kiro/settings/cli.json`, the produced AgentSettings equals the zme-derived expected object.
10. cyril's produced `_meta.kiro.settings` sent to a live KAS turn is accepted and makes the agent invoke `orchestrate_subagent` (behavioral, via cyril's real wire).
11. A KAS turn run with cyril's real settings reaches `turn_end` and renders without hang or deserialization hard-fail (removed-invariant guard).

## Falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| — | Mapping table == zme's table | Diff design table vs `zme()` in `kiro-tui-2.8.1.js` | the tui.js bundle (independent of cyril) | 5m | **passed** | unit `settings::tests::table_matches_zme` (table constants) |
| 9 | Live cli.json → expected AgentSettings | Apply mapping to real cli.json; compare to zme-derived expected | `.cyril-nhzw/cheapest-falsifier.py` (hand-derived from zme source) | 5m | **passed** | unit `settings::tests::marshal_live_fixture` |
| 1 | KAS meta set, V2 meta None | Call both `client_capabilities()`; inspect `.meta` | struct field read | 10m | pending | unit `settings::tests::kas_sets_meta_v2_none` |
| 2 | 9 bool keys map per table | Fixture with all 9 = true; assert 9 keys `{enabled:true}` | test fixture | 15m | pending | unit `settings::tests::all_bool_keys_map` |
| 3 | Non-bool ignored | `{"chat.enableThinking":"yes"}` → thinking from default-on, not from `"yes"` | test fixture | 10m | pending | unit `settings::tests::nonbool_ignored` |
| 4 | Defaults on-absent; non-defaults omitted | Empty map → exactly the 4 default keys, nothing else | test fixture | 10m | pending | unit `settings::tests::defaults_only` |
| 4b | `false` preserved | `{"chat.enableThinking":false}` → `thinking:{enabled:false}` | test fixture | 10m | pending | unit `settings::tests::false_preserved` |
| 5 | toolSearch gated on enabled + numeric extras | fixtures: enabled+nums / enabled-only | test fixture | 10m | pending | unit `settings::tests::toolsearch_shapes` |
| 5b | toolSearch omitted w/o enabled | `{"toolSearch.minPct":5}` only → no `toolSearch` key | test fixture | 5m | pending | unit `settings::tests::toolsearch_needs_enabled` |
| 6 | compaction gated on numeric | fixtures: neither/one/both | test fixture | 10m | pending | unit `settings::tests::compaction_shapes` |
| 7 | Absent file → defaults, no panic | Point reader at nonexistent path | `tempdir` (no file) | 10m | pending | unit `settings::tests::absent_file_defaults` |
| 8 | Malformed file → defaults + warn | Write `"{ not json"`; assert defaults, no panic | `tempdir` + garbage file | 10m | pending | unit `settings::tests::malformed_file_defaults` |
| 10 | Real wire → orchestrate_subagent | Send cyril's marshaled settings to live KAS; observe subagent tool | live 2.10.0 v3 (probe harness) | 30m | pending (prove-it already showed the shape) | **Claim 9 unit test is the CI fence** (deterministic proxy: the prove-it established "this wire → orchestrate_subagent") |
| 11 | Turn with real settings reaches turn_end, no hard-fail | Live KAS turn with cyril's settings; assert turn_end + no deser panic | live 2.10.0 v3 gated smoke | 30m | pending | gated `#[ignore]` smoke `kas_settings_handshake_smoke` (manual — user-approved, mirrors `kas_fs_host_io_smoke`) |

Cheapest falsifier is **passed** (two entries: table-vs-zme and live-fixture, both 5m, both run). Claims 10/11's deterministic CI fence is Claim 9 (cyril's exact outbound wire is asserted; the prove-it already proved that wire's behavior), plus a manual gated smoke — this satisfies the "measurement-based claim needs a CI fence or documented manual approval" rule.

## Negative space (what nhzw deliberately does NOT do)

1. **Does not render KAS subagent crews.** Enabling `subagentOrchestration` surfaces `orchestrate_subagent`/`pipeline.stages[]` tool_calls; rendering them as crews is **KAS-3 / cyril-fjfu**, not this feature. nhzw ships them as generic tool_calls.
2. **Does not invent cyril-specific defaults.** It mirrors zme verbatim; no flag cyril "wishes were on." If the user disables a flag in cli.json, cyril sends it disabled.
3. **Does not read workspace-scoped settings** (global `cli.json` only). Tracked: **cyril-sa39**.
4. **Does not emit `semanticReview`/`fta`** or the extended `knowledge`/`compaction` sub-fields (`includePatterns`, `maxFiles`, `chunkSize`, …). Extended sub-fields are a plan-tracked TODO, not silently shipped; if a live turn needs them they are additive on top of this table.
5. **Does not make settings live-reconfigurable.** Settings are read once at `initialize`; changing cli.json mid-session has no effect until respawn (matches engine-bound-at-spawn, ADR-0001).

## Hard-gate checklist

- [x] Every production-reachable input shape (§Input shapes 1–12) is covered by a claim, or out-of-scope with a reason.
- [x] Subtractive change swept (§2b); the removed "bare-fallback posture" invariant's broken facts each have a claim (11) or a noted-safe reason.
- [x] Every claim has a falsifier in the table.
- [x] Every falsifier names an independent oracle (zme bundle / test fixtures / live KAS — none is "another part of this feature"; the mapping oracle is the tui.js source, not cyril's Rust).
- [x] Non-vacuity: e.g. Claim 3's fence fails under a `.as_bool().unwrap_or(true)`-style coercion; Claim 4b's fails under `if v { set }` (dropping false); Claim 5b's fails under emitting toolSearch from minPct alone; Claim 8's fails under `read_to_string(..).unwrap()`.
- [x] Per-claim distinct outputs (each is its own named unit test).
- [x] Measurement/behavioral claims (10, 11) have a CI fence (Claim 9 unit) + a documented manual gated smoke.
- [x] Deferrals cite verified tracker IDs: **cyril-sa39** (filed this session), **cyril-fjfu** (KAS-3, exists).
- [x] Cheapest falsifier run and passed.
- [x] Negative space ≥ 3 (has 5).
