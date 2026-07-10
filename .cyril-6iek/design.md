# cyril-6iek — Falsifiable design: engine-identity fingerprinting at handshake

Status: **APPROVED 2026-07-09** (ship hard pause). Decisions: v3 alias in both
FromStr and serde (D7 reversal approved); mismatch policy = fail-stop
(`BridgeDisconnected`); session-id second layer (C7/C8) included; C1/C2's
long-term fence = per-release wire audit (explicitly accepted).
Basis: `.cyril-6iek/findings.md` (probe + oracle agree on every discriminator).

## Purpose

`AgentEngine` is bound at spawn purely from flag/config (`bridge.rs` `engine_for`);
nothing verifies the subprocess speaks the assumed dialect. The F4 baseline
(`test_bridge-default-vs-kas.out`) shows a default build meeting KAS: silent
`SessionCreated`, then a cascade of cryptic internals (`PersistenceClassification`,
`accessToken` null) that never mention the engine. This design adds **verification
without selection**: fingerprint the wire at the two handshake points, and on
contradiction fail loud with an actionable remedy — never silently rebind.

## Core rule

> The bound engine is a claim about the wire; the wire's own evidence
> (`initialize.agentCapabilities._meta.kiro`, `sess_` session-id prefix) must not
> contradict it. Contradiction = `BridgeDisconnected` with a remedy, before any
> turn can die cryptically.

Evidence table (probe-proven, see findings.md):

| Signal | v2 (2.4.1→2.12.0) | KAS (2.10.0→2.12.0) |
|---|---|---|
| `initialize.agentCapabilities._meta.kiro` object | never present (no `_meta` at all) | always present |
| session id from `session/new`/`session/load` | bare UUID | `sess_` prefix |

## Architecture

- **New pure module `protocol/fingerprint.rs`** — compiles **unconditionally**
  (never behind the `kas` feature; ADR-0002 gates KAS *code*, and this is
  engine-neutral wire inspection — the whole point is that a default build can
  diagnose a KAS wire). Two pure functions, unit-testable without a subprocess:
  - `init_mismatch(bound: AgentEngine, init: &acp::InitializeResponse, kas_available: bool) -> Option<String>`
  - `session_id_mismatch(bound: AgentEngine, session_id: &str, kas_available: bool) -> Option<String>`
  - KAS evidence = `agentCapabilities._meta` carries a `"kiro"` key whose value is
    a JSON **object** (key-set intentionally ignored — it already drifted 3→5 keys).
- **Hook 1 (`run_loop`, bridge.rs:613):** the currently-discarded `_init_response`
  is inspected; `Some(reason)` → `BridgeDisconnected { reason }` + clean return
  (identical mechanism to the existing `engine_for` gate at bridge.rs:447).
- **Hook 2 (NewSession + LoadSession arms):** the response's session id is checked;
  mismatch → same fail-stop path (replaces the `SessionCreated` notification).
- **`kas_available`** is `cfg!(feature = "kas")` at the call site; the pure fns take
  it as a parameter so both message variants are testable in one build.
- **v3 alias:** `AgentEngine::FromStr` accepts `v3` → `Kas` (kiro-cli's own
  vocabulary since 2.8.0; the wrapper spawn already emits `--agent-engine v3`);
  `#[serde(alias = "v3")]` on the variant so TOML `engine = "v3"` also parses;
  serialization still emits `"kas"`; parse-error text updated to name all three.
  ⚠ This deliberately reverses the D7 parse-table decision
  (`agent_engine.rs:65-68` asserts v3 is rejected) — **open decision for the pause**.

### Remedy messages (feature-aware)

| bound | wire evidence | kas feature | message core |
|---|---|---|---|
| V2 | KAS | off | agent speaks KAS (initialize advertised `_meta.kiro`); this build has no KAS support — rebuild with `--features kas` and run `--agent-engine kas`, or spawn a v2 agent |
| V2 | KAS | on | agent speaks KAS; restart cyril with `--agent-engine kas`, or spawn a v2 agent |
| Kas | v2 | on (by construction) | agent speaks v2 (no `_meta.kiro` at initialize); restart with `--agent-engine v2` or drop the flag |
| any | id-shape contradiction | — | same remedies, evidence clause names the session-id shape instead |

## Input shapes (step 2)

- `AgentEngine`: `V2`, `Kas` (Kas reachable only under the `kas` feature — `engine_for` refuses it earlier otherwise).
- Initialize `_meta`: **absent** · **present without `kiro` key** · **present with non-object `kiro`** · **present with `kiro` object**. Only the last is KAS evidence (strict-object guard against future generic `_meta` use).
- Session id string: `sess_`-prefixed · bare UUID · other/empty (treated as non-KAS-shaped; under Kas bound that is a contradiction, which is correct — a malformed id should be loud).
- `kas_available`: true/false (message wording only, never verdict).
- FromStr input: `v2` · `kas` · `v3` · case/whitespace variants of each · unknown · empty.
- Proxied wire (sacp-conductor in the spawn path): passes Kiro extensions through unchanged (conductor spike), so evidence survives proxying — same shapes as above, no special case.
- Out of scope: non-Kiro ACP agents (they present no `_meta.kiro` and no `sess_` ids, so under the default V2 binding they proceed untouched — the detector only ever *stops* on positive contradiction).

## Removed-invariant sweep (step 2b)

The change is **additive** (a new gate; no lock, guard, ordering, or uniqueness
property is removed). The one new behavior — `BridgeDisconnected` can now follow
`initialize`/`session/new` — rides an event the App already handles at arbitrary
times (cyril-l7tw made bridge death visible and safe mid-anything), so no consumer
assumption breaks. The D7 v3-rejection reversal widens `FromStr`'s accepted set;
its only consumer is CLI/config parsing, both of which want the wider set.

## Claims and falsification

| # | Claim | Falsifier | Oracle | Cost | Status | Regression fence |
|---|-------|-----------|--------|------|--------|------------------|
| C1 | KAS `initialize` always carries an `agentCapabilities._meta.kiro` object; v2 never carries `_meta` (all observed releases). | Live handshakes 2.12.0 both engines + free-path 2.10.0/2.11.0/2.11.1; sweep every committed capture (2.4.1→2.12.0). If any v2 capture shows `_meta` or any KAS lacks `kiro`, false. | Committed traces recorded by kiro's own recorder/reference client (independent of probe script). | 10m | **passed** | Wire claim — re-verified by the per-release audit checklist; code fences below assume it. |
| C2 | KAS session ids are `sess_`-prefixed; v2 ids are bare UUIDs (all observed releases). | Same sweep: any v2 `sess_` id or KAS bare id falsifies. | Same traces. | (with C1) | **passed** | Same as C1. |
| C3 | Bound=V2 + initialize with `_meta.kiro` object ⇒ `BridgeDisconnected` naming KAS + remedy; the command loop is never entered. | Bridge-level test: fake agent returns KAS-shaped initialize; assert BridgeDisconnected (reason contains "KAS"), assert NewSession gets no SessionCreated. Buggy impl that fails it: today's code (init response discarded). | Fake-agent fixture shaped from the committed KAS trace, not from the detector. | unit | pending | bridge test `fingerprint_stops_v2_bound_on_kas_wire` |
| C4 | Bound=V2 + initialize with no `_meta`, `_meta` without `kiro`, or non-object `kiro` ⇒ **no** disconnect (no false positive). | Unit: all three shapes through `init_mismatch` expect `None`; bridge test: v2-shaped fake proceeds to SessionCreated. Buggy impl that fails it: detector keying on `_meta` presence alone. | Historical sweep (C1) proves the production shape; fixtures cover the hypothetical shapes. | unit | pending | unit `no_false_positive_on_v2_and_generic_meta` |
| C5 | Bound=Kas + initialize lacking `_meta.kiro` ⇒ fail-stop naming v2 + remedy. | Unit (kas lane): `init_mismatch(Kas, v2-shaped, true)` is `Some`, message names v2. Buggy impl: inverted comparison (passes C3, fails C5 distinctly). | v2-trace-shaped fixture. | unit | pending | unit `kas_bound_on_v2_wire_stops` (kas feature lane) |
| C6 | Bound=Kas + `_meta.kiro` present ⇒ proceed. | Unit (kas lane): expect `None`. Buggy impl: unconditional `Some` (fails C6, not C5). | KAS-trace-shaped fixture. | unit | pending | unit `kas_bound_on_kas_wire_proceeds` (kas lane) |
| C7 | Bound=V2 + `session/new`/`session/load` id starting `sess_` ⇒ same fail-stop (second layer; catches evidence drift past initialize). | Unit on `session_id_mismatch` + bridge test: fake agent v2-shaped initialize but `sess_` id ⇒ BridgeDisconnected, no SessionCreated. Buggy impl: check wired into NewSession only — a LoadSession-path unit assert fails. | Fixture ids taken from the traces. | unit | pending | bridge test `fingerprint_stops_on_sess_id_v2_bound` |
| C8 | Bound=Kas + bare/other id ⇒ fail-stop. | Unit (kas lane): bare-UUID and empty ids ⇒ `Some`. Buggy impl: `contains` off-by-negation / missing arm ⇒ this assert fails. | Trace-derived fixtures. | unit | pending | unit `kas_bound_on_uuid_id_stops` (kas lane) |
| C9 | Disconnect reason is feature-aware: `kas_available=false` ⇒ names `--features kas` rebuild; `true` ⇒ names `--agent-engine` restart. | Unit: both bools through the message builder; assert each names its remedy and not the other's. Buggy impl: one static string ⇒ exactly one of the two asserts fails. | Message-content asserts vs this table. | unit | pending | unit `mismatch_reason_names_remedy_per_build` |
| C10 | `"v3"`/`" V3 "` parse to `Kas` (FromStr); TOML `engine = "v3"` deserializes to `Kas`; serialization still emits `"kas"`. | Unit: parse + serde round-trip asserts. Buggy impl: **today's code** (v3 rejected) fails it; alias-on-serialize bug fails the emit-"kas" assert. | kiro-cli's own flag vocabulary (`--agent-engine v3` is what the wrapper spawn already sends, live-verified in the KAS smokes). | unit | pending | updated `from_str_parses_known_and_rejects_unknown` + `config_roundtrips_lowercase` |
| C11 | A default build meeting a KAS subprocess surfaces one actionable BridgeDisconnected and stops — replacing the F4 cascade. | Re-run the F4 harness (`test_bridge` default build vs `kas-wrapper.sh`) post-build: expect the C3 message, expect **no** `PersistenceClassification` cascade. Buggy impl: any wiring gap between pure fns and run_loop. | F4 baseline output committed pre-change (`test_bridge-default-vs-kas.out`) — diffable before/after. | 5m live | pending | C3/C7 bridge tests are the deterministic CI form |

Cheapest falsifiers (C1, C2) ran **before** this doc was finalized and passed —
including the two shapes the issue text got wrong (key-set drift; `agentInfo` null).

## Negative space (what this deliberately does not do)

1. **No engine auto-switching or fallback.** Detection never rebinds the engine
   (ADR-0001 startup-only binding; spec-B6 fail-stop precedent). The remedy is a
   message; a respawn/reconnect affordance is tracked at **cyril-gua0**.
2. **No sniffing-based selection.** `AgentEngine` stays a typed, explicit startup
   selection (CONTEXT.md); the fingerprint only *verifies* it.
3. **No suppression switch.** No env var/config to skip the check: a false positive
   would mean the wire contract changed, which the per-release audit catches, and
   the disconnect reason names its exact evidence so diagnosis is immediate.
4. **No mid-session re-verification.** Two handshake points only (initialize,
   session creation/load) — not per-notification policing.
5. **No non-Kiro fingerprinting.** Other ACP agents present neither signal and are
   never stopped; vendor-neutral work stays in the Phase-1/4 seam.
6. **No `_meta.kiro` key-set inspection.** Presence-of-object only; key sets drift
   per release (proven 3→5) and belong to the release audit, not runtime checks.

## Open decisions for the hard pause

1. **v3 alias reverses D7** (`agent_engine.rs:65-68` deliberately rejects v3).
   Recommend: accept alias (kiro's own vocabulary; wrapper already emits v3), keep
   canonical spelling `kas` in serialization/docs.
2. **Fail-stop vs warn-and-continue** on fingerprint contradiction. Recommend
   fail-stop (`BridgeDisconnected`): every observed mismatch behavior is broken
   anyway (F4 cascade), and warn-only would let the cryptic cascade proceed.
3. **Session-id second layer (C7/C8)** — include, or initialize-only? Recommend
   include: it is nearly free, and it is the only guard that fires if a future
   release moves/renames the `_meta` advertisement while ids stay stable.
4. **Serde alias for config `engine = "v3"`** — include for CLI/config symmetry?
   Recommend yes (one attribute).
