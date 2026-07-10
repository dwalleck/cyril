# cyril-6iek — checkpointed-build audit

Slices from `.cyril-6iek/plan.md`; every slice passed `cargo nextest run` (default +
`--features kas`), `cargo clippy --all-targets -- -D warnings` (both lanes),
`cargo fmt --check`, `cargo test --doc` before its commit.

## Slice results

| Slice | Commit | Claims | Result |
|---|---|---|---|
| 1 — v3 alias | `feat(core): accept v3 as an alias…` | C10 | pass; D7 assertion flipped per pause decision |
| 2′ — init fingerprint + bridge hook | `feat(core): fingerprint the wire's engine identity at initialize` | C3, C4, C9 | pass |
| 3′ — session-id layer + arms | `feat(core): session-id fingerprint layer…` | C7, C8 | pass |
| 6 — live C11 verification | this commit | C11 (+C1/C2 re-verified live) | pass, see below |

## Plan deviations (checkpointed-build latitude, noted per skill)

1. **Slices 2+4 and 3+5 merged.** A consumer-less `pub(crate)` fn is dead code under
   the workspace's `-D warnings`, so each pure fingerprint fn had to land with its
   bridge wiring. Claims/fixtures/oracles/budgets unchanged.
2. **`Engine::kind()`** instead of threading `AgentEngine` into `run_loop` — the loop
   holds `Rc<dyn Engine>`; identity now comes from the trait (one-line impls).
3. **Test-fake wire personality**: `Script.wire_kas: Option<bool>` (None = auto-match
   the bound engine) + `Script.sess_ids` id-shape override for the evidence-drift
   fixture. Without auto-match, every existing KasEngine parity test would have
   tripped the new gate; no existing test hardcoded `fake-{n}` ids (verified by grep).
4. **LoadSession checks pre-flight** (caller-supplied id, checked before the RPC);
   NewSession checks the agent-minted id post-response. Asymmetry is intentional and
   documented at both call sites.
5. **`never_loop` clippy catch** in the C3 test — rewritten to a single recv, which is
   stricter anyway (the disconnect must be the *first* notification).

## Slice 6 — C11 live verification (expected vs actual)

Expected (written in the plan before running):
(a) KAS run shows the C3 message, no `PersistenceClassification` cascade, no prompt
attempt; (b) v2 run still reaches `[SessionCreated]` and streams the prompt turn.

**KAS run** (`test_bridge-default-vs-kas-POST.out`, default build vs
`kiro-cli acp --agent-engine kas` 2.12.0): exactly one fingerprint ERROR + one
`BridgeDisconnected` naming the evidence (`_meta.kiro`), the detected engine, and the
default-build remedy (`--features kas` rebuild). Zero `PersistenceClassification`
lines (14 in the F4 baseline), zero sessions created, no prompt attempted, harness
exits at test [1]. **Matches (a).**

**v2 run** (`test_bridge-v2-POST.out`): first attempt timed out waiting for
`SessionCreated` — initialize had already passed the fingerprint (`ACP bridge
initialized`, no mismatch ERROR), so this was backend/MCP cold-start slowness
outrunning the harness window, not a detector false positive; the immediate rerun is
a full healthy pass: `SessionCreated` (bare-UUID id through the new check), both
`McpReady`, prompt streamed `Hello.`, `TurnCompleted`, `/stats`, pickers — the
committed output. **Matches (b).**

## Final integration check

- `cargo nextest run` (default): all pass — includes every regression fence
  (`fingerprint_stops_v2_bound_on_kas_wire`, `fingerprint_stops_on_sess_id_v2_bound`,
  `fingerprint_stops_on_sess_id_load_v2_bound`, fingerprint unit matrix, flipped D7
  parse tests) and the no-false-positive fence (every pre-existing harness test).
- `cargo nextest run --features kas`: all pass — adds `fingerprint_stops_kas_bound_on_uuid_id`
  and the KasEngine parity suite against the auto-matching fake.
- Wire oracle (prove-it): the two POST harness runs above re-confirm C1/C2 live on
  2.12.0 — `_meta.kiro` + `sess_` on KAS, neither on v2.
