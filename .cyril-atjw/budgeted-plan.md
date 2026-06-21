# cyril-atjw (KAS-0) — Budgeted plan

**Design:** cyril-atjw "Design Notes" (D1–D9) + ADR-0004 (single-mediator loop, non-blocking forward) + ADR-0001 (Engine trait) + ADR-0002 (kas feature). **Approved.**
**Cheapest-falsifier:** `crates/cyril-core/examples/kas0_turnend_probe.rs` — models the proposed loop (two producers → one internal channel → `select!` + flag), oracle = current bridge invariants (bridge.rs:1441–1549). **PASSED 2026-06-21** (D2/D3/D4 hold). Caveat: validates the concurrency *shape*; Slice 2's FakeAgent harness is the *integration* oracle.

**Global acceptance:** strict v2 behavioral parity — ZERO user-visible change. **Every slice ends green on: full v2 test suite + `cargo clippy --all-targets -- -D warnings` + `cargo fmt --check`.** No slice introduces an always-on phase, so no wall budgets apply. The only new loop in the whole plan is Slice 7's fixture iteration.

Shape: 7 slices, ordered so each is independently green. Slices 1 and 2 are independent (engine vs. bridge plumbing); the rest layer on.

---

## Slice 1: `Engine` trait + `V2Engine`, wired through `convert` and `client_capabilities`

**Claim:** Engine core surface (D6 deferred to KAS-1 — see below). The Kiro-scoped `Engine` trait has TWO convert methods (`convert_session_update` → `Option`, `convert_ext_notification` → `Result<Option>` — the plan's "convert" was shorthand for both wire dialects) + `client_capabilities`; `V2Engine` implements it by **delegating to the existing `convert::session_update_to_notification` / `convert::kiro::to_ext_notification`** and returning the same empty `ClientCapabilities`. `KiroClient` calls `engine.convert_*`; `run_bridge` builds `Rc<dyn Engine> = V2Engine` and uses `engine.client_capabilities()` at the handshake.

> **D6 (capability `as_*` accessor + `AuthResponder` stub) moved to KAS-1 (cyril-evwh).** Checkpointed-build found a consumer-less stub is dead code under `-D warnings` (no `#[allow]` allowed). ADR-0001 amended; the accessor pattern lands with its first real consumer in KAS-1.

**Oracle:** every existing `convert`/`client` test passes **unchanged** (delegation is identity); plus a direct equality check `V2Engine.convert(frame) == <old direct convert>(frame)` over captured frames.

**Stress fixture:** a batch containing **both** a generic `session/update` (e.g. `agent_message_chunk`) **and** a `_kiro.dev/*` ext frame (e.g. `steering_queued`). Expected: V2Engine routes **both** to the identical `Notification` the old direct calls produced. *Designed to fail the plausible bug:* "V2Engine wires only the generic path and silently drops the ext (`to_ext_notification`) path."

**Loop budget:** no new loop. (`convert` is O(1) per frame, unchanged.)

**Files:** `crates/cyril-core/src/protocol/engine.rs` (new), `crates/cyril-core/src/protocol/client.rs` (call `engine.convert`), `crates/cyril-core/src/protocol/bridge.rs` (construct + share `Rc<dyn Engine>`, `client_capabilities` at :320). *Touches 3 files — justified:* a defined-but-unwired `pub(crate)` trait is dead code under `-D warnings`, so the seam must land atomically; net change is ~50 lines, almost all delegation.

**Verification:**
- [ ] Unit tests pass (incl. the equality check)
- [ ] Stress fixture: ext + generic both route identically
- [ ] Cheapest-falsifier still green (mechanism unaffected)
- [ ] clippy/fmt clean

---

## Slice 2: Single-mediator loop — notifications + prompt task feed one internal channel; flag replaces `is_finished()`

**Claim:** D2/D3/D4. `KiroClient` is constructed with the internal `inbound_tx` (not the App tx); the off-loop `prompt_task` sends its synthesized `TurnCompleted` there too. `run_loop` becomes a `select!` over `command_rx` + `inbound_rx`; it forwards every item to the App and **clears loop-local `turn_in_flight: Option<SessionId>` on observing `Notification::TurnCompleted`**. The busy-guard (bridge.rs:395) and cancel-target (:455) read `turn_in_flight`; the `JoinHandle` is kept only for `Shutdown` abort (:1233).

**Oracle:** the **existing FakeAgent harness** (bridge.rs:1441–1549) — exactly one `TurnCompleted` per turn, and a mid-turn command processed before it — must still pass against the restructured loop. This is the real-bridge version of the cheapest-falsifier.

**Stress fixture:** FakeAgent turn with a `CancelRequest` injected **mid-turn** (loop must process it; cancel must target the in-flight session via the flag, not the `active_session_id` a mid-turn `NewSession` could have retargeted) **and** a second `SendPrompt` injected mid-turn (must be rejected by the flag) **then** one after `TurnCompleted` (must be accepted). *Designed to fail:* "the flag clears too early / never," "cancel reads the wrong session," "loop starves notifications behind a command."

**Loop budget:** no new loop — the `select!` replaces the existing `while let` command loop; **O(1) per item** (one forward + one flag check), same asymptotic cost as today. Channels bounded (16) as today; back-pressure unchanged.

**Files:** `crates/cyril-core/src/protocol/bridge.rs` (loop + flag + KiroClient construction with `inbound_tx`).

**Verification:**
- [ ] FakeAgent harness tests pass (one `TurnCompleted`/turn, mid-turn command first)
- [ ] Stress fixture: mid-turn cancel + reject-concurrent + accept-after all hold
- [ ] Cheapest-falsifier still green
- [ ] clippy/fmt clean

---

## Slice 3: Request interposition — permission routed through the loop, forwarded but never awaited

**Claim:** D5 + ADR-0004 non-blocking invariant. `KiroClient::request_permission` sends its `PermissionRequest` (embedded `responder` oneshot intact) to the loop's internal request channel instead of `permission_tx`. The loop's new `select!` arm **forwards it to the App's `permission_tx` and returns immediately** — the App's reply travels back on the embedded oneshot, bypassing the loop. v2 mediation is identity (forward unchanged).

**Oracle:** a FakeAgent that issues a permission request mid-turn: the converted response must reach the agent **unchanged** vs. today, AND notifications must keep flowing while the request is outstanding (the loop did not block on it).

**Stress fixture:** FakeAgent issues a permission request, and **while it is outstanding** (App "thinking") the agent streams two notification chunks. Expected: both chunks are forwarded to the App *before* the permission is answered (proves the loop forwarded the request without awaiting its resolution), and the eventual response still round-trips via the oneshot. *Designed to fail:* "the loop awaits the response inside the `select!` arm → chunks queue behind the open dialog (freeze)."

**Doc-comment-as-contract:** the loop arm carries `// invariant: forward, never await resolution`. Classified **structural invariant, not a correctness precondition** — violating it produces a *hang* (liveness), not wrong output, so the enforcement is "no `.await` on the response in this arm" + the stress fixture above, not a runtime check. (ADR-0004 records it for KAS-5/cyril-7bdu.)

**Loop budget:** no new loop; the request arm is O(1) per request (one forward).

**Files:** `crates/cyril-core/src/protocol/bridge.rs` (request arm + KiroClient construction with the request sender). *Optionally* fold notifications+requests into one `BridgeInbound` enum channel (ADR-0004) — still one file.

**Verification:**
- [ ] FakeAgent permission round-trip: response unchanged vs. today
- [ ] Stress fixture: chunks forwarded while permission outstanding (no freeze)
- [ ] clippy/fmt clean

---

## Slice 4: `AgentEngine` enum + `run_bridge` gate (default V2, refuse Kas cleanly)

**Claim:** D7 (gate half). `AgentEngine { V2, Kas }` (default `V2`); `run_bridge` matches it to build `Rc<dyn Engine>`: `V2 ⇒ V2Engine`; `Kas ⇒` a clean error surfaced via the existing `BridgeDisconnected`/error notification path ("KAS engine is not available yet"), **never a panic**. No flag yet (Slice 5) — defaults to V2, so zero behavior change.

**Oracle:** spawning with `AgentEngine::V2` is byte-identical to today (all bridge tests pass); spawning with `AgentEngine::Kas` yields a `BridgeDisconnected`/error notification and the process does not panic or hang.

**Stress fixture:** drive `run_bridge` (FakeAgent harness) with `AgentEngine::Kas`. Expected: exactly one error notification, no panic, loop exits cleanly. *Designed to fail:* "Kas path `unwrap()`s / panics / hangs waiting for a process that was never spawned."

**Loop budget:** no new loop.

**Files:** `crates/cyril-core/src/types/agent_engine.rs` (new enum, `Default = V2`), `crates/cyril-core/src/protocol/bridge.rs` (gate).

**Verification:**
- [ ] V2 path: all bridge tests pass (parity)
- [ ] Stress fixture: Kas ⇒ one error notification, no panic
- [ ] clippy/fmt clean

---

## Slice 5: `--agent-engine` CLI flag + config field → `spawn_bridge`

**Claim:** D7 (selection half). A `--agent-engine <v2|kas>` clap flag and a `config.agent.engine` field (both default `v2`) resolve to `AgentEngine` and are threaded into `spawn_bridge`. Argv is **not** sniffed for the engine (CONTEXT.md: Engine is a typed axis).

**Oracle:** no flag ⇒ `AgentEngine::V2`; `--agent-engine kas` ⇒ `Kas` (then refused by Slice 4's gate); an unknown value ⇒ clap rejects at parse time.

**Stress fixture:** parse-level table — `[] ⇒ V2`, `["--agent-engine","kas"] ⇒ Kas`, `["--agent-engine","V2"]`/case + unknown ⇒ explicit accept/reject. *Designed to fail:* "default isn't v2," "case/whitespace silently maps to v2," "unknown value silently defaults instead of erroring."

**Doc-comment-as-contract:** none new (clap enforces the value set at the boundary — a runtime check by construction).

**Output stream rule:** the flag's help/usage text is clap diagnostic → stderr (clap default). No new stdout. ✔

**Loop budget:** no new loop.

**Files:** `crates/cyril/src/main.rs` (clap flag + resolve), `crates/cyril-core/src/types/config.rs` (field + default).

**Verification:**
- [ ] Parse table: default V2, explicit Kas, unknown rejected
- [ ] `cargo run` (no flag) behaves identically to today
- [ ] clippy/fmt clean

---

## Slice 6: `kas` cargo feature (empty) + `kas-feature` CI job

**Claim:** D8 / ADR-0002. `cyril-core` gains `[features] kas = []`; `cyril` gains `kas = ["cyril-core/kas"]`; `cyril-ui` unchanged. A dedicated CI job builds + clippies(`-D warnings`) + tests `-p cyril --features kas`. The feature is **empty** in KAS-0 (no `KasEngine` yet) — it only proves the wiring exists and can't bitrot.

**Oracle:** `cargo build/clippy/test -p cyril --features kas` is green AND produces a binary behaviorally identical to the default build (feature is empty). The default build is unchanged.

**Stress fixture:** this slice is **config, not logic** (per the skill, pure-schema/config slices don't get a behavioral fixture) — the "fixture" is the CI job itself going green on `--features kas`, plus a one-line assertion in CI that `--features kas` and default builds both pass the same v2 test subset (the empty feature changes nothing). *Plausible bug guarded:* "the feature accidentally pulls in or gates real code" → caught by the identical-behavior expectation.

**Loop budget:** no new loop.

**Files:** `crates/cyril-core/Cargo.toml`, `crates/cyril/Cargo.toml`, `.github/workflows/*.yml` (CI job). *3 files, all one-liners except the CI job stanza — justified as the irreducible feature-wiring set.*

**Verification:**
- [ ] `cargo test -p cyril --features kas` green
- [ ] Default build unchanged; clippy(`--features kas`)/fmt clean
- [ ] CI `kas-feature` job present and green

---

## Slice 7: Verification spike — captured KAS `session/update` fixtures + deser test

**Claim:** D9 (NON-gating). Real KAS `session/update` frames (`agent_message_chunk`, `agent_thought_chunk`, `tool_call`, `tool_call_update`, `available_commands_update`, `config_option_update`, `session_info_update`) are captured via the existing `experiments/conductor-spike/probe-kas-*` scripts (manual, live KAS — *not* via cyril), committed as fixtures; a Rust test deserializes each into `acp::SessionNotification` and asserts `Ok`.

**Oracle:** schema 0.11.2's own `serde` impl — `serde_json::from_value::<acp::SessionNotification>(frame)` is `Ok` for every captured variant. Independent of `convert`/`Engine`.

**Stress fixture:** include the **`session_info_update`** frame specifically (the newest, KAS-defining variant most likely to be absent from 0.11.2). Expected: `Ok`. *If any variant is `Err`*, the test records it as a **documented upgrade-trigger** (no `#[serde(other)]` on `SessionUpdate`) and the spike's findings note flags it for KAS-2a (cyril-j16p) — it does **not** block KAS-0.

**Loop budget:** one new loop — iterate fixtures and deserialize. **O(fixtures), fixtures ≈ 6.** ~6 deser ops, far under 10⁶. ✔

**Output stream rule:** the deser test asserts (no stdout); the manual probe writes captured frames to fixture files (data) and progress to stderr. ✔

**Files:** `crates/cyril-core/tests/fixtures/kas/*.json` (captured), `crates/cyril-core/tests/kas_acp_coverage.rs` (deser test).

**Verification:**
- [ ] Deser test passes for all captured variants (or `Err`s are documented upgrade-triggers, non-gating)
- [ ] Fixtures committed; `session_info_update` present
- [ ] clippy/fmt clean

---

## Plan Self-Review

**1. Every loop — complexity stated & within budget?**
- Slice 7 fixture iteration: `O(fixtures)`, fixtures ≈ 6 ⇒ ~6 deser ops ≪ 10⁶. ✔
- Slices 2/3 restructure the existing command loop (not a new loop): O(1) per item, unchanged from today. ✔
- No other slice adds a loop. ✔

**2. Every fixture — designed to fail which bug class?**
- S1: ext path silently dropped (mixed ext+generic batch). ✔
- S2: flag clears early/never; cancel targets wrong session; notification starvation (mid-turn cancel + concurrent prompt). ✔
- S3: loop awaits the response → freeze (chunks-during-open-permission). ✔
- S4: Kas path panics/hangs (drive run_bridge with Kas). ✔
- S5: default≠v2 / silent-default-on-unknown (parse table). ✔
- S6: config, not logic — guarded by identical-behavior CI expectation (stated). ✔
- S7: newest variant absent from schema (session_info_update included). ✔

**3. Every doc-comment precondition — classified + enforced?**
- S3 "forward, never await resolution": **structural invariant** (violation = hang, not wrong output) → enforced by no-`.await`-in-arm + the freeze stress fixture, not a runtime check. ✔
- S5 engine value set: enforced at the boundary by clap (runtime rejection of unknown values). ✔
- No other "callers must X" preconditions introduced. ✔

**4. Every write target — data or diagnostic?**
- S5 clap help → stderr (diagnostic). ✔
- S7 captured frames → fixture files (data); probe progress → stderr (diagnostic). ✔
- No new `println!` to stdout in library code; runtime logging stays on `tracing` (existing convention). ✔

**5. Every tracker reference — resolves to a covering issue?**
- KAS-2a late-`TurnCompleted` / orphaned-`JoinHandle` hazard (from S2's flag model) → **cyril-j16p** (KAS-2a; "CRITICAL REWORK" covers the prompt_task/is_finished/late-response rework) + ADR-0004 records the specific hazard. ✔
- KAS-5 slow request resolution spawns off-loop (S3 forward-ref) → **cyril-7bdu** (filed 2026-06-21 for this citation). ✔
- No uncited deferrals remain. ✔

**No gaps. Plan complete.**
