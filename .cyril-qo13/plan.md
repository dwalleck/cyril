# cyril-qo13 — Budgeted plan: exact-choice permission responses

**Design:** `.cyril-qo13/design.md` (approved; cheapest falsifier C2 passed).
**Strategy:** expand/contract — the workspace compiles and `cargo test` +
`cargo clippy -- -D warnings` pass after every slice. No slice leaves the old
kind-keyed path broken until the contract slice deletes it.

Global notes applying to all slices:

- **Output streams:** cyril is a TUI; all diagnostics go through `tracing` to
  `cyril.log` (diagnostic). No slice writes to stdout except test output. No
  data-stream writes exist in this change.
- **Wall budget:** no slice adds an always-on phase; all new code runs once per
  user-visible permission event (human-paced). Wall budgets are therefore N/A
  everywhere; loop budgets are stated per slice.
- **Oracle (shared default):** `.cyril-qo13/oracle.py` output +
  `probe-output.txt` (recorded pre-change behavior), per the prove-it artifacts.
- C8's server-side half is validated live once at the end of the build
  (user-approved `manual` fence, see design) — it is not a slice.

---

## Slice 1: `PermissionOptionId` newtype, no behavior change

**Claim:** supports C1 — option ids get a domain type so a label or session id
can't be passed where an option id belongs (design "Proposed change", house rule).
**Oracle:** the full existing test suite (387 tests) passes unchanged, and
`probe_qo13_replay_trace_permissions` prints byte-identical output to the
committed `probe-output.txt` — the newtype must be invisible on the wire.
**Stress fixture:** existing trace replay is the fixture — real KAS ids are
40+-char `toolu_bdrk_…-option-N` strings; the newtype must round-trip them
unmodified (bug class: a constructor that trims/normalizes, or a `Display`
impl that debug-quotes).
**Loop budget:** no new loops.
**Wall budget:** N/A (see global notes).
**Files:** `crates/cyril-core/src/types/event.rs` (define newtype; migrate
`PermissionOption.id`), `crates/cyril-core/src/protocol/convert/mod.rs`
(construct in `to_permission_options`), plus mechanical test-constructor
updates in `crates/cyril-ui/src/state.rs` and
`crates/cyril-ui/src/widgets/approval.rs`. 4 files — justified: this is a
compile-driven mechanical rename with zero logic; the 2-file rule targets
logic slices, and splitting this one would leave the workspace uncompilable
between halves.

**Code (advisory):**
```rust
// types/event.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionOptionId(String);
impl PermissionOptionId {
    pub fn new(id: impl Into<String>) -> Self { Self(id.into()) }
    pub fn as_str(&self) -> &str { &self.0 }
}
```

**Verification:**
- [ ] `cargo test` (workspace) — all green, count unchanged
- [ ] Probe replay output identical to committed `probe-output.txt`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] No new loops (budget trivially holds)

---

## Slice 2: Expand — `PermissionResponse::Selected` variant + converter arm

**Claim:** C2 — a `Selected { option_id, trust_option }` response converts to
the exact wire outcome: bare `selected` + optionId, `_meta.trustOption = label`
only when `trust_option` is `Some`, `Cancel` → `cancelled`.
**Oracle:** the existing fence `probe_qo13_reply_shape_matches_reference_bytes`
(reference-client bytes) stays green; new unit tests hand-compute expected JSON
from the trace's reply vocabulary, not from the converter.
**Stress fixture:** three adversarial cases with expected output written first:
(a) `Selected { id: "toolu_…-option-1", trust_option: None }` → serialized
JSON has **no `_meta` key at all** (bug class: emitting `"_meta": null`, which
the C2 byte-compare would catch on the wire); (b) `trust_option:
Some("Allow similar commands — ripgrep (rg …)")` — label with spaces, em-dash,
parens — must appear verbatim under `_meta.trustOption` (bug class: ASCII
assumption); (c) an `option_id` **not present** in the request's options must
emit a `tracing::warn!` and still convert (see contract rule below).
**Loop budget:** one new loop — the membership warn check iterates
`args.options`: O(options) with options ≤ 4 observed (KAS/v2), bounded by the
request; ~4 comparisons per response, once per human approval. Far under 10^6.
**Wall budget:** N/A.
**Files:** `crates/cyril-core/src/types/event.rs` (add variant alongside old
ones), `crates/cyril-core/src/protocol/convert/mod.rs` (new match arm + warn +
colocated unit tests).

**Doc-comment contract:** `Selected.option_id` docs say "must name an option
from the originating request." Classification: **load-bearing** — a bogus id
silently produces a wrong wire answer. Enforcement: runtime membership check in
`from_permission_response` that `tracing::warn!`s (survives release) before
sending; constructive enforcement is the UI only offering real options, so the
warn is the release-build tripwire, not the primary guard. `debug_assert!`
alone would compile out and make the contract fiction.

**Verification:**
- [ ] New unit tests pass (no-meta case, unicode-label case, warn case)
- [ ] `probe_qo13_reply_shape_matches_reference_bytes` still green
- [ ] Old kind-based arms untouched — full suite green
- [ ] Loop budget: membership check O(options ≤ 4) recorded above

---

## Slice 3: Upgrade the replay probe into the asserting C1/C3 fence

**Claim:** C1 — for every request in **both** 2.11.0 traces and every in-bounds
k, `Selected { options[k].id }` produces wire optionId `options[k].optionId`;
C3 — for the distinct-kind requests, that output is identical to today's
recorded behavior.
**Oracle:** `.cyril-qo13/oracle.py` (raw-text extraction, no cyril code) for
C1; committed `probe-output.txt` (pre-change recorded output, cross-validated
against reference replies) for C3.
**Stress fixture:** the traces themselves (11 real requests) plus one synthetic
single-option request (input shape S4, absent from traces; expected: pick k=0
→ that option's id) — bug classes: off-by-one in option iteration, replying
with the selection *index* instead of the id (the fixture's ids are non-numeric
so an index-as-id bug cannot accidentally pass), same-kind collision (all
user_input options share `allow_once`).
**Loop budget:** replay loop O(requests × options) = 12 × ≤4 ≈ 48 conversions,
test-only. Trivial.
**Wall budget:** N/A (test).
**Files:** `crates/cyril-core/src/protocol/convert/probe_qo13.rs` (replace the
printing replay with per-request asserts labeled `user_input` / `tool_approval`
so a C1 failure and a C3 failure are distinguishable — per-claim distinctness).

**Verification:**
- [ ] Fence asserts (not prints) and passes on both traces + synthetic fixture
- [ ] Assert messages name the request id and kind-class (C1 vs C3 localization)
- [ ] `python3 .cyril-qo13/oracle.py` output unchanged (oracle still agrees)
- [ ] Loop budget recorded above

---

## Slice 4: UI phase-1 — confirm sends the picked option's id; oob/empty → Cancel

**Claim:** C1 (UI half) — `approval_confirm` in `SelectOption` sends
`Selected { option_id: options[selected].id, trust_option: None }`; C6 — a
confirm with `selected >= options.len()` (including empty options) sends
`Cancel`, never a fabricated or clamped id. C5 — Esc/cancel path untouched.
**Oracle:** hand-written expected values in state tests (ids drawn from the
trace vocabulary, computed by eye from the fixture — independent of the
implementation); existing cancel tests stay green.
**Stress fixture:** (a) three options **all `allow_once`** with ids
`…-option-0/1/2`, pick k=2 (last) → expect `Selected("…-option-2")` — the
same-kind collision that IS the bug (today's code would emit `…-option-0`);
(b) `selected = 5` on 3 options → expect `Cancel` (bug class: clamping to
last); (c) empty options + confirm → expect `Cancel` (bug class: panic or
fabricated id).
**Loop budget:** no new loops (indexing only).
**Wall budget:** N/A.
**Files:** `crates/cyril-ui/src/state.rs` (SelectOption arm + colocated tests).
Phase-2 (`SelectTrust`) arm keeps constructing the old `AllowAlways` variant in
this slice — it still exists during expand.

**Verification:**
- [ ] New state tests pass (same-kind-pick-last, oob, empty)
- [ ] Existing approval/cancel tests green
- [ ] Full suite + clippy green

---

## Slice 5: Trust-phase provenance — carry the phase-1 pick into `SelectTrust`

**Claim:** C4 — confirming a trust tier replies with the allow_always option id
picked in phase 1 plus `_meta.trustOption = label`, even when allow_always is
not `options[0]`.
**Oracle:** hand-computed expectations from the trace/v2 option-id vocabulary
(`accept` / `always-accept`); the convert-level `_meta` assertion reuses slice
2's unit oracle.
**Stress fixture:** (a) options `[allow_once='accept',
allow_always='always-accept']` + 2 trust options; pick k=1 → phase 2 → confirm
tier 1 → expect `Selected { "always-accept", Some(<tier-1 label>) }` (bug
class: reading `options[approval.selected]` at trust-confirm time, where
`selected` re-indexes `trust_options` — would emit `accept` and fail); (b)
**staleness fixture:** pick allow_always → phase 2 → Esc (back) → pick
allow_once (k=0) → expect `Selected { "accept", None }`, not the stale carried
`always-accept` (bug class: carried id surviving the back-transition).
**Loop budget:** no new loops.
**Wall budget:** N/A.
**Files:** `crates/cyril-ui/src/traits.rs`
(`ApprovalPhase::SelectTrust { chosen_option_id: PermissionOptionId }`, drops
`Copy`), `crates/cyril-ui/src/state.rs` (transition captures the id; SelectTrust
confirm sends `Selected`; back-transition discards it),
`crates/cyril-ui/src/widgets/approval.rs` (`match state.phase` →
`match &state.phase` with `SelectTrust { .. }`, plus its two test
constructors). 3 files — justified: an enum gaining a payload is atomic across
definition, constructor, and matcher; ~15 lines of mechanical fallout.

**Doc-comment contract:** "`chosen_option_id` is the phase-1 allow_always
pick" — **constructively enforced**: the only construction site is the phase-1
transition; no runtime check needed (the type carries the proof). Sanity-hint
class; no `debug_assert!` required because there is no other constructor.

**Verification:**
- [ ] Provenance fixture passes (allow_always-not-first)
- [ ] Staleness fixture passes (back-then-repick)
- [ ] `approval_confirm` still returns the chosen `TrustOption` for persistence
      (existing App-side tests green)
- [ ] Full suite + clippy green (Copy-loss fallout resolved)

---

## Slice 6: Contract — delete the kind-keyed path

**Claim:** completes C1/C6 (design "Removed-invariant sweep") — the kind-based
variants, `From<PermissionOptionKind> for PermissionResponse`,
`find_option_id`, and its first-match/fabricated-id fallbacks no longer exist;
C5 — `Cancel → cancelled` conversion retained.
**Oracle:** the compiler (no remaining constructors), plus the slice-3 fence and
`probe-output.txt` equivalence for distinct-kind requests; `grep -rn
"PermissionResponse::Allow\|PermissionResponse::Reject\|find_option_id" crates/`
must return zero production hits.
**Stress fixture:** the bridge harness test at `bridge.rs:~2026` re-answers its
scripted permission with `Selected { <scripted option id> }` and the turn still
completes (`drain_to_turn == EndTurn`) — bug class: harness answering with an
id the scripted agent doesn't recognize, which would hang the turn and time out
the test (5s timeout already present).
**Loop budget:** no new loops (deletions only).
**Wall budget:** N/A.
**Files:** `crates/cyril-core/src/types/event.rs` (delete variants + `From`
impl), `crates/cyril-core/src/protocol/convert/mod.rs` (delete kind arms +
`find_option_id` + its two unit tests, superseded by the slice-3 fence),
`crates/cyril-core/src/protocol/bridge.rs` (harness constructor). 3 files —
justified: deletion-heavy contract step; the compiler drives every edit.

**Verification:**
- [ ] Grep proves no kind-variant constructors or `find_option_id` remain
- [ ] Full workspace suite green; `cargo clippy --all-targets -- -D warnings`
- [ ] Slice-3 fence green (C1/C3 hold with the old path gone)
- [ ] `probe_qo13_reply_shape_matches_reference_bytes` +
      `probe_qo13_unknown_option_kind_parse` green (C2/C7 fences)

---

## Post-slice build-phase validation (not a slice)

C8 server-side half, per the design's user-approved `manual` fence: one live
KAS session (`kiro-cli acp --agent-engine kas`, quick-spec flow), answer the
three clarifying questions with non-first picks, confirm the generated spec
reflects the choices — mirroring the trace's reference behavior. Recorded in
the build log, not CI.

## Claim coverage map

| Design claim | Slice(s) | Fence |
|---|---|---|
| C1 exact choice | 3 (convert), 4 (UI) | asserting replay + state tests |
| C2 encoding | 2 | existing byte-compare fence + unit tests |
| C3 distinct kinds unchanged | 3, 6 | replay `tool_approval` assert family |
| C4 trust provenance | 5 | provenance + staleness fixtures |
| C5 cancel unchanged | 4, 6 | existing cancel tests + Cancel arm retained |
| C6 no fabricated ids | 4, 6 | oob/empty fixtures + fallback deletion |
| C7 unknown kinds unreachable | none needed | sentinel already committed (passed) |
| C8 KAS acts on choice | post-slice validation | C1+C2 fences (cyril side) + manual live run |

## Plan Self-Review

1. **Loops:** slice 2 membership check O(options ≤ 4) per human-paced approval;
   slice 3 replay O(12 × ≤4 ≈ 48), test-only. All others introduce no loops.
   No always-on phases; nothing approaches 10^6 ops / 10^3 syscalls. No gaps.
2. **Fixtures:** every logic slice has an adversarial fixture with expected
   output written before implementation — same-kind pick-last (the bug itself),
   `_meta`-absence vs `_meta: null`, unicode trust label, foreign option id
   (warn), index-as-id (non-numeric ids), oob/empty→Cancel, trust-phase
   provenance with allow_always-not-first, stale-carry after back-transition,
   harness turn-completion. Slice 1 is mechanical; its fixture is wire-output
   invariance against the committed probe output. No gaps.
3. **Doc-comment preconditions:** `Selected.option_id` membership =
   load-bearing → release-surviving `tracing::warn!` in the converter (slice
   2); `chosen_option_id` provenance = constructively enforced by its single
   construction site (slice 5). No unenforced contracts. No gaps.
4. **Write targets:** all diagnostics via `tracing` to `cyril.log`; test output
   to test stdout only; no data-stream writes exist. No gaps.
5. **Tracker references:** cyril-gn07 (consent scope), cyril-sive (v2 echo
   shape), cyril-p7kp (unknown-kind watch), cyril-0o7e (pending_interaction) —
   all verified to exist this session with covering descriptions. The plan
   introduces no new deferrals. No gaps.
