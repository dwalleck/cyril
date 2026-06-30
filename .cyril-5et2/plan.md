# KAS-2b (cyril-5et2) — budgeted plan

From the approved design (`.cyril-5et2/design.md`). 4 slices, order 1→2→3→4
(each later slice depends on earlier ones). Claim coverage: C1,C2,C3,C6 → Slice 3;
C4,C5 → Slice 2; C7 → Slice 4. Real fixture: `.cyril-5et2/context_usage_raw.json`
(promoted to `tests/fixtures/kas/` in Slice 3).

Pinning fact (verified): `UiState::apply_notification` (state.rs:295–695) is
**exhaustive** — adding a `Notification` variant forces its arm. `SessionController`
has `_ => false`; the App routes via `apply_notification` (no per-variant arm).

---

## Slice 1: ContextBucket + ContextBreakdown types (cyril-core)

**Claim:** (supports C1, C7) The breakdown is 5 `ContextBucket {tokens,percent}`;
the 3 aggregate-only buckets carry no `items` field — constraint #1 encoded in the
type (make-illegal-states-unrepresentable; per-file drill-in is cyril-1116).
**Oracle:** compile + construction round-trip test (pure types — NOT the prove-it
oracle, which is for the converter).
**Stress fixture:** N/A — pure types, no logic (rule 4 exemption).
**Loop budget:** none (no loops).
**Wall budget:** N/A (not always-on).
**Files:** `crates/cyril-core/src/types/session.rs` (add types, beside `ContextUsage`),
`crates/cyril-core/src/types/mod.rs` (re-export).

**Code (advisory):**
```rust
#[derive(Debug, Clone)]
pub struct ContextBucket { tokens: u64, percent: f64 }
impl ContextBucket {
    pub fn new(tokens: u64, percent: f64) -> Self { Self { tokens, percent } }
    pub fn tokens(&self) -> u64 { self.tokens }
    pub fn percent(&self) -> f64 { self.percent }
}
#[derive(Debug, Clone)]
pub struct ContextBreakdown {
    context_files: ContextBucket, session_files: ContextBucket,
    tools: ContextBucket, your_prompts: ContextBucket, kiro_responses: ContextBucket,
}
// ContextBreakdown::new(...) + per-bucket accessors. Accessor style matches
// ContextUsage (private fields + getters), not pub fields.
```
**Verification:**
- [ ] Unit test constructs a `ContextBreakdown`, reads back each bucket's tokens/percent
- [ ] `cargo build -p cyril-core` + `clippy -D warnings`

---

## Slice 2: ContextBreakdownUpdated notification + UiState retain-last + scalar feed

**Claim:** C4 (UiState retains the last breakdown; absence ≠ clear) + C5 (under KAS
the scalar `Context: N%` updates from context_usage frames).
**Oracle:** UiState private-field read in state.rs's own test module — independent of
the converter (the test constructs the `Notification` directly).
**Stress fixture (adversarial, expected outcomes written first):**
- *retain-last:* apply `ContextBreakdownUpdated{usage:5.0, breakdown:Some(B1)}` then
  `{usage:7.0, breakdown:None}` → expect `context_breakdown == Some(B1)` (retained),
  `context_usage == 7.0`. **Fails under** `self.context_breakdown = note.breakdown.clone()`
  (overwrite-with-None).
- *scalar:* apply `{usage:42.0, breakdown:Some(_)}` → expect `context_usage() == 42.0`.
  **Fails under** an arm that stores the breakdown but never sets the scalar.
**Loop budget:** none.
**Wall budget:** N/A.
**Files:** `crates/cyril-core/src/types/event.rs` (variant),
`crates/cyril-ui/src/state.rs` (field + apply arm; makes the exhaustive match compile).

**Code (advisory):**
```rust
// event.rs
ContextBreakdownUpdated { usage_percentage: f64, breakdown: Option<ContextBreakdown> },
// state.rs UiState field: context_breakdown: Option<ContextBreakdown>
Notification::ContextBreakdownUpdated { usage_percentage, breakdown } => {
    // reuse ContextUsage::new() clamp, same as the v2 metadata path (state.rs:415)
    self.context_usage = Some(ContextUsage::new(*usage_percentage).percentage());
    // retain-last: only overwrite when a breakdown is present (absence ≠ clear).
    if let Some(bd) = breakdown { self.context_breakdown = Some(bd.clone()); }
    true
}
```
**Doc-comment-as-contract:** "absence ≠ clear" is **load-bearing for correctness**
(overwriting with None flickers the bars) → enforced by the `if let Some` guard, NOT
`debug_assert!`. Comment says so.
**Verification:**
- [ ] retain-last + scalar unit tests pass
- [ ] existing cyril-ui tests pass (exhaustive match compiles)
- [ ] `clippy -D warnings`

---

## Slice 3: KasEngine context_usage converter arm

**Claim:** C1 (breakdown→5 buckets exact), C2 (usage from flat `_meta.kiro.usagePercentage`),
C3 (absent/malformed breakdown → `Some` with `breakdown:None`, carrying the scalar — not
dropped, not `Err`), C6 (turn_end/other kinds unperturbed).
**Oracle:** `jq` on the promoted fixture (prove-it oracle, `.cyril-5et2/oracle.sh`) for C1;
hand-set divergent values for C2; constructed frames for C3; existing `convert::kas::tests`
for C6.
**Stress fixtures (expected outcomes written first):**
- (a) **real** `tests/fixtures/kas/session_info_update_context_usage.json` →
  `ContextBreakdownUpdated` with the 5 buckets' exact tokens/percent. **Fails under**
  transposed tokens/percent or per-bucket reads of the nested wrapper. Oracle: jq.
- (b) frame with flat `usagePercentage=9.9`, nested `contextUsage.usagePercentage=1.1` →
  `usage_percentage == 9.9`. **Fails under** reading the nested wrapper.
- (c) breakdown-**absent** frame → `Some(ContextBreakdownUpdated{breakdown:None})`, scalar set.
  **Fails under** `breakdown.unwrap()` or returning `None` (which drops the % update).
- (d) `turn_completion` → `None`; `turn_end` → `TurnCompleted` (existing). **Fails under**
  a context_usage arm placed before / capturing the turn_end check.
**Loop budget:** `parse_breakdown` iterates the **5 fixed buckets** → O(5) = O(1), 0 syscalls.
Within budget (≪ 10^6).
**Wall budget:** N/A (per-notification, not always-on).
**Files:** `crates/cyril-core/src/protocol/convert/kas.rs` (the arm + `parse_breakdown` helper),
`crates/cyril-core/tests/fixtures/kas/session_info_update_context_usage.json` (data, promoted).

**Code (advisory):** in `session_info_to_notification`, the turn_end branch stays first;
**after** it, gate on `kind == "context_usage"`:
```rust
if kind == Some("context_usage") {
    let usage = kiro.get("usagePercentage").and_then(Value::as_f64);
    let breakdown = parse_breakdown(kiro.get("breakdown")); // Option<ContextBreakdown>
    // return Some EVEN when breakdown is None — the scalar must still update.
    return usage.map(|u| Notification::ContextBreakdownUpdated { usage_percentage: u, breakdown });
}
// parse_breakdown: read 5 named buckets; any bucket missing tokens/percent => None (treat
// the whole breakdown as absent). No unwrap.
```
**Doc-comment-as-contract:** "absent/malformed breakdown → None but the notification still
carries the scalar" is **load-bearing** (dropping it freezes the toolbar %) → the arm returns
`Some` whenever `usagePercentage` is present, independent of `parse_breakdown`. No `unwrap`
(CLAUDE.md). C6: the new arm is gated on `kind=="context_usage"` and sits after turn_end.
**Verification:**
- [ ] 4 fixtures (a–d) pass; jq oracle agrees with (a)
- [ ] existing `convert::kas::tests` green (C6)
- [ ] prove-it oracle (`oracle.sh`) still agrees with the promoted fixture
- [ ] `clippy -D warnings` (no unwrap)

---

## Slice 4: TuiState::context_breakdown + toolbar 5-label render

**Claim:** C7 (toolbar renders 5 labeled categories — Context Files / Session Files / Tools /
Prompts / Responses — with percents, aggregate-only, no drill-in).
**Oracle:** TestBackend buffer scan (independent of UiState internals), like the existing
`toolbar::status_bar_renders_context_usage` test.
**Stress fixture (adversarial):** a `ContextBreakdown` with **5 DISTINCT** percents —
ContextFiles 1.1, SessionFiles 2.2, Tools 3.3, Prompts 4.4, Responses 5.5 (distinct so a
label↔value transposition is caught; the real frame's three 0%s would hide it) → render →
expect all 5 labels present AND each paired with its own percent AND no per-item/drill-in
line. **Fails under** an omitted category, a transposed label↔value, or rendering items.
**Loop budget:** render iterates the **5 fixed labels** → O(5) = O(1) span builds per frame.
**Wall budget:** always-on (per-frame). +5 spans on the existing status-bar render ≈ a few
dozen ops/frame ≪ 10^6. Within budget.
**Files:** `crates/cyril-ui/src/traits.rs` (trait method `context_breakdown()` + mock impl),
`crates/cyril-ui/src/state.rs` (UiState impl), `crates/cyril-ui/src/widgets/toolbar.rs`
(render in `render_status_bar`). **3 files — JUSTIFIED:** a read-only trait method is an
atomic change across its declaration (traits.rs), its single production impl (state.rs), and
its consumer (toolbar.rs); splitting would create a fixture-less plumbing slice (the
anti-pattern the skill warns against). The only logic (render) carries the stress fixture.

**Code (advisory):**
```rust
// traits.rs: fn context_breakdown(&self) -> Option<&ContextBreakdown> { None } default? No —
//   add to trait + impl for UiState (self.context_breakdown.as_ref()) + the test mock.
// toolbar.rs render_status_bar: after the "Context: N%" span, if let Some(bd) = state.context_breakdown():
//   push one span per bucket: format!("CtxF {:.0}%  SesF {:.0}%  Tools {:.0}%  Prompts {:.0}%  Resp {:.0}%", ...)
//   (exact labels/format are a UI detail; the claim is "5 labels + their percents, no items").
```
**Output-stream rule:** renders to the TUI frame (UI **data**), not stdout; warnings (none
here) go to `tracing`. No pipe concern (TUI app).
**Verification:**
- [ ] render test: 5 distinct labels+percents present, no item lines
- [ ] existing toolbar tests pass
- [ ] `clippy -D warnings`

---

## Plan Self-Review (5 lists — all empty = no gaps)

1. **Every loop:** Slice 3 `parse_breakdown` O(5)=O(1); Slice 4 render O(5)=O(1). Both fixed
   by the wire's 5 buckets, ≪ budget. No unbounded loops. ✓ no gaps
2. **Every fixture:** S2 retain-last (overwrite-with-None bug) + scalar-not-set bug; S3 real
   frame (transpose/read-nested), divergent flat/nested (read-nested), breakdown-absent
   (drop/unwrap), turn_completion+turn_end (arm-ordering); S4 5-distinct-percents
   (omit/transpose/render-items). S1 pure types (exempt). All adversarial. ✓ no gaps
3. **Every doc-precondition:** S2 "absence ≠ clear" = load-bearing → `if let Some` guard (not
   debug_assert); S3 "absent breakdown still carries scalar" = load-bearing → returns `Some`
   on present `usagePercentage`, no unwrap. Both classified + enforced. ✓ no gaps
4. **Every write target:** all rendering → TUI frame (data→UI); warnings → tracing
   (diagnostic). No stdout/stderr misuse (TUI). ✓ no gaps
5. **Every tracker reference:** cyril-1116 (per-file drill-in, out of scope) — filed + verified.
   No other deferrals. ✓ no gaps

## Hard gate
- [x] Every slice has all mandatory fields
- [x] Every loop has a complexity statement (S3, S4 both O(1))
- [x] Every logic slice has an adversarial stress fixture (S1 exempt — pure types)
- [x] Claim coverage matches the design (C1,C2,C3,C6→S3; C4,C5→S2; C7→S4)
- [x] Every tracker reference resolves to an existing issue (cyril-1116)
