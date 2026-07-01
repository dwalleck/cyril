# cyril-nhzw — Budgeted Plan: marshal kiro-cli settings into `_meta.kiro.settings`

Design: `.cyril-nhzw/falsifiable-design.md` (approved; cheapest falsifier passed).
All slices land behind `#[cfg(feature = "kas")]`. No production-scale loops exist anywhere in this feature — every loop is over the fixed 9-entry zme table (`O(1)`), so the loop-budget sections state that once and move on.

Slice order: reader → marshal(bool+defaults) → marshal(nested) → integration fixture → meta-wrapper+wiring → gated live smoke.

> **Implementation reslice (checkpointed-build, reality-forced):** `read_cli_settings`/`marshal_agent_settings` are `pub(crate)` and would be **dead code (`-D warnings` fails)** until the wiring slice. So the original slices 1+2+5 merge into a **walking-skeleton slice A** (reader + full bool/defaults marshal + `_meta` wrapper + `KasEngine` wiring + V2 parity) — every commit stays live with zero `#[allow]`. Remaining: **B** = nested `toolSearch`/`compaction` (orig slice 3), **C** = integration fixture (orig slice 4), **D** = gated smoke (orig slice 6). Claim coverage is unchanged; only the commit granularity changed.

---

## Slice 1: `read_cli_settings()` — read the global cli.json, tolerate absent/corrupt

**Claim:** 7 (absent file → defaults, no panic) + 8 (malformed → defaults + warn, no panic).
**Oracle:** `tempdir` with (a) no file and (b) a garbage file; assert the returned map is empty in both, and that the *marshaled* result (via Slice 2) is defaults-only. Independent of the parser — the test constructs the filesystem state directly.
**Stress fixture:**
- `"{ not valid json"` written to the path → must return an empty `Map` + `warn!`, NOT panic and NOT propagate an error that aborts initialize. (Bug class: `serde_json::from_str(..).unwrap()` or `?`-propagation that kills the handshake.)
- Path does not exist → empty `Map`, no `warn!` (missing ≠ corrupt; only corrupt warns). (Bug class: collapsing NotFound and parse-error into the same path, or erroring on absent.)
**Loop budget:** none. One `fs::read_to_string` + one `serde_json::from_str`. Parse is `O(bytes)`, file ≈ <1 KB / ~30 keys. Well under budget.
**Files:** `crates/cyril-core/src/protocol/kas/settings.rs` (new); `crates/cyril-core/src/protocol/kas/mod.rs` (add `pub(crate) mod settings;`).

**Doc-comment-as-contract:** the "absent/corrupt → empty map" behavior is **load-bearing for correctness** (returning wrong data would send wrong flags), so it is real runtime code returning an empty `Map`, not a `debug_assert!`. The reader takes the path as a parameter so tests inject a tempdir path (no `$HOME` dependence in tests).

**Output stream:** the malformed-file message is a **diagnostic** → `tracing::warn!` (to cyril.log via the subscriber), never stdout. No `println!`.

**Code (advisory):**
```rust
/// Read the global kiro-cli settings map. Absent file → empty map (Ok).
/// Corrupt file → empty map + warn (missing ≠ corrupt; never abort the handshake).
fn read_settings_at(path: &std::path::Path) -> serde_json::Map<String, serde_json::Value> {
    match std::fs::read_to_string(path) {
        Ok(s) => match serde_json::from_str::<serde_json::Value>(&s) {
            Ok(serde_json::Value::Object(m)) => m,
            Ok(_) => { tracing::warn!(path=%path.display(), "cli.json not a JSON object; ignoring"); Default::default() }
            Err(e) => { tracing::warn!(path=%path.display(), error=%e, "cli.json parse failed; using KAS defaults"); Default::default() }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => { tracing::warn!(path=%path.display(), error=%e, "cli.json read failed; using KAS defaults"); Default::default() }
    }
}
/// `~/.kiro/settings/cli.json` via the home dir (not $HOME string-concat).
pub(crate) fn read_cli_settings() -> serde_json::Map<String, serde_json::Value> {
    match dirs_home() { Some(h) => read_settings_at(&h.join(".kiro/settings/cli.json")), None => Default::default() }
}
```
(`dirs_home()` = whatever home-resolution the crate already uses; check `platform/path.rs` / existing home usage before adding a `dirs` dep.)

**Verification:**
- [ ] Unit tests `absent_file_defaults`, `malformed_file_defaults` pass
- [ ] Stress fixtures produce empty map (no panic, no init abort)
- [ ] prove-it oracle unaffected (reader not yet wired)
- [ ] No loops; trivially in budget

---

## Slice 2: `marshal_agent_settings()` — 9 bool keys + 4 default-on keys

**Claim:** 2 (bool keys map per zme table) + 3 (non-bool ignored) + 4 (defaults on-absent, non-defaults omitted) + 4b (`false` preserved).
**Oracle:** `.cyril-nhzw/cheapest-falsifier.py`'s `marshal()` (independent Python impl of the same zme table) applied to each fixture; Rust output must match. The table itself is the tui.js bundle (external source).
**Stress fixture:**
- `{"chat.enableThinking": false}` → `thinking:{enabled:false}` and NOT overwritten to `true` by the default pass. (Bug class: `if v { insert }` drops `false`, then default-on re-adds it as `true` — silent inversion.)
- `{"chat.enableCheckpoint": "yes"}` → `checkpoint` **absent** (non-bool ignored; checkpoint is not a default key). (Bug class: `as_bool().unwrap_or(true)` coercion inventing an enabled flag.)
- `{}` (empty) → exactly `{codeIntelligence, knowledge, thinking, subagentOrchestration}` all `{enabled:true}`, nothing else. (Bug class: default pass leaking non-default keys, or missing a default.)
**Loop budget:** two loops, both over fixed tables: `O(9)` over `BOOL_MAP`, `O(4)` over `DEFAULTS`. Constant regardless of input. In budget.
**Files:** `crates/cyril-core/src/protocol/kas/settings.rs`.

**Doc-comment-as-contract:** none load-bearing beyond behavior already tested; `marshal_agent_settings` is pure and total (any `Map` in → a valid object out).

**Code (advisory):**
```rust
const BOOL_MAP: &[(&str, &str)] = &[
    ("chat.enableThinking", "thinking"), ("chat.enableKnowledge", "knowledge"),
    ("chat.enableCodeIntelligence", "codeIntelligence"), ("chat.enableTodoList", "todoList"),
    ("chat.enableCheckpoint", "checkpoint"), ("chat.enableTangentMode", "tangentMode"),
    ("chat.disableAutoCompaction", "disableAutoCompaction"),
    ("chat.enableSubagent", "_subagent"), ("chat.enableDelegate", "_delegate"),
];
const DEFAULTS_ON: &[&str] = &["codeIntelligence", "knowledge", "thinking", "subagentOrchestration"];

pub(crate) fn marshal_agent_settings(e: &serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    use serde_json::json;
    let mut n = serde_json::Map::new();
    for (src, dst) in BOOL_MAP {
        if let Some(serde_json::Value::Bool(b)) = e.get(*src) {   // only real bools
            n.insert((*dst).into(), json!({ "enabled": b }));
        }
    }
    for k in DEFAULTS_ON {
        n.entry(*k).or_insert_with(|| json!({ "enabled": true }));
    }
    // toolSearch / compaction inserted in Slice 3
    serde_json::Value::Object(n)
}
```

**Verification:**
- [ ] Unit tests `all_bool_keys_map`, `nonbool_ignored`, `defaults_only`, `false_preserved` pass
- [ ] Stress fixtures (false-preserve, coercion-omit, empty-defaults) produce expected
- [ ] Python oracle agrees on each fixture
- [ ] Loops `O(9)`+`O(4)`, in budget

---

## Slice 3: `marshal_agent_settings()` — `toolSearch` + `compaction` nested objects

**Claim:** 5 (toolSearch gated on `enabled` bool; `minPct`/`minTokens` iff numeric) + 5b (no `toolSearch` without `enabled`) + 6 (compaction gated on numeric).
**Oracle:** the Python `marshal()` nested-object branch on the same fixtures.
**Stress fixture:**
- `{"toolSearch.minPct": 5}` (no `enabled`) → `toolSearch` **absent** entirely. (Bug class: building `toolSearch` from a partial field.)
- `{"toolSearch.enabled": true, "toolSearch.minPct": true}` → `minPct` **omitted** (JSON `true` is not a number). (Bug class: `as_i64()`/`as_u64()` accepting bool, or `is_number()` misuse — serde_json `Value::Bool` is not `is_number()`, so guard with `.as_u64()`/`.as_f64()` and confirm it excludes bool.)
- `{"compaction.excludeMessages": 3}` (no percent) → `compaction:{enabled:true, excludeMessages:3}`, no `excludePercent`. (Bug class: requiring both, or inventing the missing one.)
**Loop budget:** none (fixed key lookups). In budget.
**Files:** `crates/cyril-core/src/protocol/kas/settings.rs` (extends Slice 2's fn).

**Code (advisory):**
```rust
    // inside marshal_agent_settings, before returning:
    if let Some(serde_json::Value::Bool(en)) = e.get("toolSearch.enabled") {
        let mut ts = serde_json::Map::from_iter([("enabled".into(), json!(en))]);
        if let Some(v) = e.get("toolSearch.minPct").and_then(num) { ts.insert("minPct".into(), v); }
        if let Some(v) = e.get("toolSearch.minTokens").and_then(num) { ts.insert("minTokens".into(), v); }
        n.insert("toolSearch".into(), serde_json::Value::Object(ts));
    }
    let pct = e.get("compaction.excludeContextWindowPercent").and_then(num);
    let msg = e.get("compaction.excludeMessages").and_then(num);
    if pct.is_some() || msg.is_some() {
        let mut c = serde_json::Map::from_iter([("enabled".into(), json!(true))]);
        if let Some(v) = pct { c.insert("excludePercent".into(), v); }
        if let Some(v) = msg { c.insert("excludeMessages".into(), v); }
        n.insert("compaction".into(), serde_json::Value::Object(c));
    }
// helper: numeric-but-not-bool. serde_json Value::Bool is NOT is_number(), but be explicit.
fn num(v: &serde_json::Value) -> Option<serde_json::Value> { v.is_number().then(|| v.clone()) }
```

**Verification:**
- [ ] Unit tests `toolsearch_shapes`, `toolsearch_needs_enabled`, `compaction_shapes` pass
- [ ] Stress fixtures (partial-toolsearch-omitted, bool-not-number, one-sided-compaction) produce expected
- [ ] Python oracle agrees
- [ ] No loops

---

## Slice 4: live-fixture integration test (checked-in cli.json → expected AgentSettings)

**Claim:** 9 (real cli.json → zme-derived expected). This is the **deterministic CI fence** for behavioral Claims 10/11.
**Oracle:** the expected AgentSettings hand-derived from zme in `.cyril-nhzw/cheapest-falsifier.py` (EXPECTED), independent of the Rust marshaler.
**Stress fixture:** a **checked-in** representative cli.json (a copy of the observed one at `tests/fixtures/kiro_cli_settings.json` — hermetic, NOT the user's live file) → assert `marshal_agent_settings` equals the expected object. (Bug class: a key-name typo in `BOOL_MAP` that passes narrow unit fixtures but breaks on a realistic file; the mixed present/absent/nested shape exercises the whole table at once.)
**Loop budget:** none.
**Files:** `crates/cyril-core/src/protocol/kas/settings.rs` (test) + `crates/cyril-core/tests/fixtures/kiro_cli_settings.json` (new fixture).

**Verification:**
- [ ] Unit test `marshal_live_fixture` passes against the checked-in fixture
- [ ] Expected object matches `.cyril-nhzw/cheapest-falsifier.py` EXPECTED (cross-check)
- [ ] No loops

---

## Slice 5: `kiro_settings_meta()` + wire into `KasEngine::client_capabilities()` + parity

**Claim:** 1 (KAS sets `_meta.kiro.settings`; V2Engine sets no meta).
**Oracle:** call both engines' `client_capabilities()` and read the `.meta` field — struct inspection, independent of the marshaler.
**Stress fixture:** assert `V2Engine.client_capabilities().meta.is_none()` — designed to fail the **parity-break** bug where the KAS meta body is copy-pasted into V2 (same fixture shape as the KAS-5b `kas_advertises_fs_and_terminal_v2_empty` test). And assert `KasEngine...meta` is `Some` with `["kiro"]["settings"]` an object.
**Loop budget:** none.
**Files:** `crates/cyril-core/src/protocol/kas/settings.rs` (add `kiro_settings_meta`) + `crates/cyril-core/src/protocol/engine.rs` (wire into `KasEngine::client_capabilities`).

**Doc-comment-as-contract:** none.

**Code (advisory):**
```rust
// settings.rs
pub(crate) fn kiro_settings_meta() -> acp::Meta {
    let settings = marshal_agent_settings(&read_cli_settings());
    serde_json::Map::from_iter([("kiro".into(), serde_json::json!({ "settings": settings }))])
}
// engine.rs, in KasEngine::client_capabilities():
acp::ClientCapabilities::new()
    .fs(acp::FileSystemCapabilities::default().read_text_file(true).write_text_file(true))
    .terminal(true)
    .meta(Some(crate::protocol::kas::settings::kiro_settings_meta()))
// ^ if no `.meta()` builder exists on ClientCapabilities, build the struct and set
//   `.meta = Some(...)` directly (field is `pub meta: Option<Meta>`). Verify at impl time.
```

**Verification:**
- [ ] Unit test `kas_sets_meta_v2_none` passes
- [ ] Stress fixture (V2 meta None) holds
- [ ] `cargo test -p cyril-core --features kas` + default both green; `clippy -D warnings` both feature sets
- [ ] prove-it oracle: cyril now sends the settings; re-run the probe harness path is Slice 6
- [ ] No loops

---

## Slice 6: gated live smoke — real KAS turn honors cyril's settings, no hard-fail

**Claim:** 10 (real wire → `orchestrate_subagent`) + 11 (turn with real settings reaches `turn_end`, no deserialization hard-fail — the removed-invariant guard).
**Oracle:** independent of cyril's marshaler — the subagent tool the live agent invokes (`orchestrate_subagent` with `pipeline.stages[]`) captured from `tool_call` frames, exactly as the prove-it probe did. `turn_end` arrival is the no-hang oracle.
**Stress fixture:** a delegation prompt (neutral, does not name the tool) on a live `--agent-engine v3` session driven through cyril's real bridge with the `kas` feature; assert (a) the turn reaches `TurnCompleted`/turn_end within the deadline and the bridge does not `BridgeDisconnected`, (b) an `orchestrate_subagent`/DAG subagent surfaces. (Bug class: an enabled flag makes KAS emit a typed `session/update` variant acp 0.11.2 can't deserialize → mid-turn hard-fail; the turn would never reach turn_end → this fails.)
**Loop budget:** the notification drain loop is bounded by a wall deadline (≤300s), not input size. Not an always-on phase. In budget.
**Files:** `crates/cyril-core/tests/kas_settings_handshake_smoke.rs` (new, `#[ignore]` gated, mirrors `kas_fs_host_io_smoke.rs`).

**Doc-comment-as-contract:** the test is manual-gated (`#[ignore]`, needs a fresh `kiro-cli login` + the KAS bundle + node), documented in the module header exactly like `kas_fs_host_io_smoke`. This is the **manual** regression fence for Claims 10/11 (user-approved per the design's Falsification table); the deterministic CI fence is Slice 4.

**Verification:**
- [ ] `cargo test -p cyril-core --features kas --test kas_settings_handshake_smoke -- --ignored --nocapture` reaches turn_end and observes `orchestrate_subagent`
- [ ] No `BridgeDisconnected` / deserialization panic during the turn
- [ ] Loop bounded by wall deadline, not input

---

## Plan Self-Review

**1. Every loop:**
- Slice 2: `O(9)` over `BOOL_MAP` + `O(4)` over `DEFAULTS_ON` — constant, in budget.
- Slice 6: notification drain bounded by ≤300s wall deadline (not input size) — in budget.
- Slices 1, 3, 4, 5: no loops.
→ no gaps.

**2. Every fixture (bug class it fails under):**
- S1: `unwrap()`-on-parse / abort-on-corrupt; missing-vs-corrupt collapse.
- S2: `false` dropped then default-on inverts it; non-bool coerced to enabled.
- S3: toolSearch built from partial field; JSON bool accepted as number; one-sided compaction.
- S4: key-name typo in the table that narrow fixtures miss (realistic mixed file).
- S5: parity-break (KAS meta leaks into V2).
- S6: an enabled flag hard-fails acp deserialization mid-turn (no turn_end).
→ each is adversarial, not happy-path. No gaps.

**3. Every doc-comment precondition:**
- S1 "absent/corrupt → empty map": load-bearing-correctness → real runtime fallback (empty map), not `debug_assert!`. Enforced in code.
- No other preconditions. No gaps.

**4. Every write target:**
- S1 malformed-file message → `tracing::warn!` = diagnostic (cyril.log), correct.
- No stdout writes; no `println!`. Marshaler returns a value; caller (engine) puts it on the wire. No gaps.

**5. Every tracker reference:**
- `cyril-sa39` (workspace overlay) — filed this session; covers the deferred work. Verified.
- `cyril-fjfu` (KAS-3 crew rendering) — exists; covers the "render orchestrate_subagent crews" deferral. Verified.
→ no gaps.

## Hard gate
- [x] Every slice has all mandatory fields
- [x] Every loop has a complexity statement
- [x] Every slice has a stress fixture
- [x] Plan claim coverage matches design claims (1,2,3,4,4b,5,5b,6,7,8,9,10,11 across slices 1–6)
- [x] Every tracker reference resolves to an existing covering issue (cyril-sa39, cyril-fjfu)
