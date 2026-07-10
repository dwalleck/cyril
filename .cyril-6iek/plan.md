# cyril-6iek — Budgeted plan

Design: `.cyril-6iek/design.md` (APPROVED 2026-07-09, all four pause decisions recorded).
Global gates per slice (repo convention + cyril-ykkc): `cargo nextest run` **and**
`cargo nextest run --features kas`, `cargo clippy --all-targets -- -D warnings` in both
lanes, `cargo fmt --check` (scoped to touched files if pre-existing repo drift exists —
memory: main has failed `fmt --check` before; never reformat unrelated code),
`cargo test --doc`. Gates use real exit codes, never `| tail`.

No slice introduces a loop beyond O(1) map lookups / O(len) string builds — loop budgets
are stated per slice anyway per the rule.

---

## Slice 1: v3 parses as Kas everywhere a selector is read

**Claim:** C10 — `"v3"`/`" V3 "` → `AgentEngine::Kas` via FromStr; TOML `engine = "v3"`
deserializes to `Kas`; serialization still emits `"kas"`; parse-error text names all three.
**Oracle:** kiro-cli's own flag vocabulary — the wrapper spawn already emits
`--agent-engine v3` (config.rs:50, verified live by the KAS wrapper smokes); cyril's
selector now accepts what kiro's flag accepts.
**Stress fixture:** `" V3 "` (case + whitespace — fails an impl that special-cases "v3"
outside the existing normalization); `serde_json::to_string(&Kas) == "\"kas\""` (fails a
rename-instead-of-alias impl); `"v3x"`/`""` still rejected (fails a starts_with impl).
**Loop budget:** none (match arm, O(1)).
**Wall budget:** n/a (parse-time).
**Files:** `crates/cyril-core/src/types/agent_engine.rs` (code + tests); one-line doc
updates in `crates/cyril-core/src/types/config.rs` (l.45 doc comment) and
`crates/cyril/src/main.rs` (l.31 help text) — doc-string-only, justified as part of this
slice since no other slice touches those files.

**Code (advisory):** add `"v3" => Ok(Self::Kas)` arm; `#[serde(alias = "v3")]` on `Kas`;
error text `expected \`v2\`, \`kas\`, or \`v3\``; flip the D7 assertion (approved) with a
comment citing the pause decision.

**Verification:**
- [ ] Unit tests pass (both feature lanes)
- [ ] Stress fixture produces expected outcome
- [ ] prove-it oracle unaffected (no wire change)
- [ ] Budgets hold

---

## Slice 2: pure `init_mismatch` + feature-aware remedy messages

**Claim:** C3–C6 fn-level + C9 — `init_mismatch(bound, init, kas_available)` returns
`Some(actionable reason)` exactly when the initialize evidence contradicts the bound
engine, with wording keyed to `kas_available`.
**Oracle:** fixtures are serde-built from the **committed trace shapes** (KAS: `_meta.kiro`
object with the live 5 keys; v2: no `_meta`), not from the detector's own vocabulary.
**Stress fixture:** `_meta` present *without* `kiro`; `_meta.kiro` present as a **non-object**
(`true`) — both must be `None` under V2 bound (fails a presence-only detector, the
plausible false-positive bug); KAS-shaped fixture carries the full 5-key object (fails an
over-strict struct deserializer).
**Loop budget:** none — one map lookup per call, O(1).
**Wall budget:** n/a (runs once per handshake).
**Files:** `crates/cyril-core/src/protocol/fingerprint.rs` (new),
`crates/cyril-core/src/protocol/mod.rs` (register module).

**Code (advisory):** `wire_shows_kas(&InitializeResponse) -> bool` (agent_capabilities.meta
→ get("kiro") → is_object); `init_mismatch` matches (bound, evidence); shared
`mismatch_reason(bound, evidence_clause, kas_available)` builder. All compiled
unconditionally — `AgentEngine::Kas` is an enum variant in every build, so the full matrix
unit-tests in the **default** lane too.

**Verification:**
- [ ] Unit tests pass (matrix: 2 bounds × 4 init shapes × 2 kas_available, distinct asserts)
- [ ] Stress fixture produces expected outcome
- [ ] prove-it oracle unaffected (pure fn, no wire change)
- [ ] Budgets hold

---

## Slice 3: pure `session_id_mismatch`

**Claim:** C7/C8 fn-level — `sess_`-prefixed id contradicts V2 bound; non-`sess_` id
contradicts Kas bound; same remedy wording rules.
**Oracle:** fixture ids copied verbatim from the committed traces (`sess_001d7a4c-…`,
`786acc7e-…`).
**Stress fixture:** `"xsess_abc"` must NOT be KAS evidence (fails a `contains` impl —
the prefix/substring bug); `"sess_"` exactly (boundary: prefix == whole string, IS KAS
evidence); `""` (empty id: no evidence of KAS ⇒ contradiction under Kas bound, none under V2).
**Loop budget:** none (`starts_with`, O(len) with len < 64).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/fingerprint.rs`.

**Verification:**
- [ ] Unit tests pass
- [ ] Stress fixture produces expected outcome
- [ ] prove-it oracle unaffected
- [ ] Budgets hold

---

## Slice 4: wire hook 1 — initialize verified in `run_loop` (+ `Engine::kind()`)

**Claim:** C3/C4 bridge-level — bound=V2 + KAS-shaped initialize ⇒ one
`BridgeDisconnected` naming KAS and the remedy, loop never enters command phase; v2-shaped
initialize proceeds exactly as today.
**Oracle:** the fake-agent harness (`with_harness`) — **every existing harness test** is
the no-false-positive fence (they all run v2-shaped initializes and must keep passing
untouched); the new test's KAS-shaped `InitializeResponse` is serde-built from the
committed KAS trace.
**Stress fixture:** the fake's KAS-shaped `_meta` carries `kiro` **plus sibling keys**
(logging etc., as live) — fails a rigid-deserialization impl; assert the disconnect
reason mentions `--features kas` in the default lane (C9 wired, not just fn-level).
**Loop budget:** none — one check per handshake.
**Wall budget:** n/a (adds one O(1) inspection to an existing await).
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (hook + Script knob
`init_meta_kiro: bool` + test `fingerprint_stops_v2_bound_on_kas_wire`),
`crates/cyril-core/src/protocol/engine.rs` (`fn kind(&self) -> AgentEngine` on the trait;
V2Engine→V2, KasEngine→Kas — run_loop holds `Rc<dyn Engine>`, so identity comes from the
trait rather than threading a new parameter).

**Code (advisory):** replace the `_init_response` discard: on `Some(reason)` →
`notify_or_closed(BridgeDisconnected)` + `return Ok(())` (mirrors the engine_for gate at
bridge.rs:447-457).

**Verification:**
- [ ] Unit tests pass — new test + entire existing harness suite untouched
- [ ] Stress fixture produces expected outcome
- [ ] prove-it oracle still agrees (slice 6 re-runs the live harness)
- [ ] Budgets hold

---

## Slice 5: wire hook 2 — session ids verified at NewSession/LoadSession

**Claim:** C7/C8 bridge-level — bound=V2 + `sess_` id from `session/new`/`session/load` ⇒
`BridgeDisconnected` (no `SessionCreated` ever emitted); bound=Kas + bare id ⇒ same (kas
lane).
**Oracle:** fake-agent harness; id shapes from the traces. The Kas-side test uses
`with_engine_harness(KasEngine)` with the fake's initialize KAS-shaped (so hook 1 passes)
and ids left bare — isolating hook 2.
**Stress fixture:** v2-shaped initialize + `sess_` session id (evidence *drift* between the
two handshake points — exactly the case hook 1 cannot catch); fake id `sess_` exact-prefix
boundary via the Slice 3 fn tests.
**Loop budget:** none — one check per session creation/load.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (both arms + Script knob
`kas_session_ids: bool` + tests `fingerprint_stops_on_sess_id_v2_bound`, kas-lane
`fingerprint_stops_kas_bound_on_uuid_id`; add a fake `load_session` impl if the trait
default doesn't already serve the LoadSession arm).

**Code (advisory):** in each arm's `Ok(response)` branch, check before building
`SessionCreated`; mismatch ⇒ BridgeDisconnected + `break` (post-handshake, so `break` to
the loop's normal shutdown path rather than `return`, keeping any cleanup uniform).

**Verification:**
- [ ] Unit tests pass (both lanes; kas-lane test compiles only under `--features kas`)
- [ ] Stress fixture produces expected outcome
- [ ] prove-it oracle still agrees
- [ ] Budgets hold

---

## Slice 6: C11 live verification — the F4 cascade is replaced by one actionable message

**Claim:** C11 — a default build meeting a KAS subprocess surfaces one actionable
`BridgeDisconnected` and stops; a default build meeting v2 is byte-for-byte unaffected.
**Oracle:** the committed **pre-change** F4 baseline (`test_bridge-default-vs-kas.out`) —
diff against the post-change run; plus a fresh v2 run compared against the committed
2.12.0 audit output (`experiments/conductor-spike/test_bridge-2.12.0.out` shape).
**Stress fixture:** the real thing — live `kiro-cli acp --agent-engine kas` (2.12.0) via
`.cyril-6iek/kas-wrapper.sh`; expected outputs written down now: (a) KAS run shows the
C3 message and **no** `PersistenceClassification` lines, no prompt attempt; (b) v2 run
still reaches `[SessionCreated]` + streams the prompt turn.
**Loop budget:** n/a (no code).
**Wall budget:** each harness run bounded by test_bridge's own drain windows (<3 min).
**Files:** `.cyril-6iek/test_bridge-default-vs-kas-POST.out`,
`.cyril-6iek/test_bridge-v2-POST.out`, `.cyril-6iek/build-audit.md` (artifacts only).

**Verification:**
- [ ] Both live runs match the written expectations
- [ ] prove-it oracle agrees with the binary (this IS that check)
- [ ] No regression in the v2 path
- [ ] Budgets hold (n/a)

---

## Plan self-review

1. **Loops:** none introduced anywhere; every check is O(1) lookup / O(len<64) prefix /
   O(msg-len) string build, once per handshake or session-creation. No always-on phases.
2. **Fixtures:** each targets a named bug class — alias-outside-normalization &
   rename-vs-alias (S1); presence-only `_meta` detector & rigid deserializer (S2, S4);
   `contains`-vs-`starts_with` & exact-prefix boundary & empty id (S3); cross-point
   evidence drift (S5); the live cascade itself (S6). No happy-path-only fixture.
3. **Doc-comment preconditions:** the fingerprint fns are total (no preconditions); the
   only contract is "compiled unconditionally — never move behind the kas feature," which
   is enforced by the default-lane unit tests exercising the Kas variants (a feature-gate
   regression fails the default `cargo nextest run`).
4. **Write targets:** BridgeDisconnected reasons = data on the notification channel (the
   App renders them); `tracing::error!` = diagnostic (stderr/cyril.log). test_bridge
   artifacts = files under `.cyril-6iek/`. No new stdout writes.
5. **Tracker references:** cyril-gua0 (respawn affordance, verified open) and cyril-ykkc
   (kas-lane gates — this plan *implements* the practice for its slices rather than
   deferring). No new deferrals introduced.

Claim coverage: C1/C2 passed pre-plan (wire facts; S6 re-verifies live) · C3/C4 → S2+S4 ·
C5/C6 → S2 (+S5 kas-lane wiring) · C7/C8 → S3+S5 · C9 → S2 (+S4 wired assert) · C10 → S1 ·
C11 → S6. All 11 covered.
