# Evidence: cyril-41bs

## Premise checklist

| ID | Decisive architecture premise | Smallest question | Verdict |
|----|-------------------------------|-------------------|---------|
| P1 | A `Send + 'static` SDK protocol actor can hand events to Cyril's serial `Rc`/`RefCell` domain actor without moving domain state across threads, unsafe code, or blanket shared locks. | Can SDK 2.0.0 exchange a bounded typed message with an unchanged current-thread domain actor? | PASS |
| P2 | SDK channel inspection preserves parsed frame/message semantics and source order, but cannot prove lexical fidelity; a raw copy made before parsing demonstrates the required capture placement without claiming production ingestion. | What survives single, batch, malformed, extension, unknown-update, and extreme-number round trips, and what must remain pending for the production adapter? | PASS — necessity/placement proven; production ingress assigned to design C4 |
| P3 | Unknown-standard-update containment depends on an untyped handler registered before typed SDK handlers; slow SDK handlers block later dispatch, so production handlers must enqueue work and return. | Do reversing/removing unknown-first and holding a handler pending expose the expected ordering/blocking behavior? | PASS |
| P4 | Cyril must retain a process adapter around SDK components because `AcpAgent` does not own Cyril's cwd, stall-watchdog, or always-available bounded stderr-tail contracts. | Which process contracts does `AcpAgent` preserve, and which remain Cyril-owned? | PASS |
| P5 | Direct SDK 2.0.0 stable-wire-v1 must preserve real Kiro v2 and KAS behavior, not only compile against their callback catalog. | Do authenticated v2 and KAS sessions produce parity through the direct component? | PASS |
| P6 | Official conductor topology must preserve real Kiro/KAS behavior through zero-proxy, no-op-proxy, and transforming-proxy chains in addition to passing deterministic full-duplex fixtures. | Do authenticated v2 and KAS sessions preserve initialization, extensions, callbacks, cancellation, failure, and ordering through all three chains? | PASS |
| P7 | A bidirectional proxy can transform request and notification traffic without taking Engine conversion or host-callback ownership, but the capability must be proven against real Kiro traffic. | Does the transforming audit proxy annotate real traffic in both directions while terminal Kiro behavior remains unchanged? | PASS |
| P8 | Linear conductor inspection is ordered and synchronous; Cyril needs a separate bounded broadcaster for isolated observers, replay, and explicit pressure policy. | Does a slow observer delay forwarding, and does the SDK provide per-observer isolation/replay? | PASS |
| P9 | First-prompt memory injection and normalized source capture must remain in their current adapters; a wire tap sees the prepared prompt and lacks Cyril's project/turn/disposition identity. | Can a tap reconstruct the original prompt and normalized capture contract from identical wire traffic? | PASS |
| P10 | SDK 2.0.0 can run stable ACP wire v1 while draft wire v2 stays an explicit, production-disabled negotiation path; it does not semantically translate traffic after handshake. | Which connector is selected for v1, v2, fallback, and rejected v2 initialization? | PASS |

## Data

- SDK: `agent-client-protocol = 2.0.0`, schema `1.5.0`, conductor `2.0.0`; upstream commit `ce023279824149008659dd8f4b8b70266a7e8210`.
- Wire: stable ACP `ProtocolVersion::V1` unless E10 explicitly enables `unstable_protocol_v2`.
- Generated fixtures: exact SDK `TransportFrame` single/batch/malformed messages, bounded actor handoffs, helper process trees, deterministic full-duplex agent/client components, and slow-observer timing.
- Production-shaped references: `experiments/conductor-spike/v2-live-session-trace-2.11.0.jsonl`, `experiments/conductor-spike/kas-workflow-channels-live-2.20.1.jsonl`, the KAS covenant, and Cyril's existing source-capture contract tests.
- Live scope: `e5` and `e6_live` create temporary directories and send inert text prompts. They perform no repository mutation, credential logging, persistent state writes, or consequential action.

## Probe

- Package: `.cyril-41bs/probe.sdk2/`; pinned manifest and lockfile, one binary per experiment, no production-code changes.
- `cargo run --quiet --bin e1` — `Send + 'static` component to bounded channel to current-thread `Rc<RefCell<_>>`.
- `cargo run --quiet --bin e2` — single/batch/malformed semantic inspection, an explicit fixture-byte copy before parsing, unknown update, parseable numeric normalization, and rejected `1e400` semantic parsing.
- `cargo run --quiet --bin e3` — registration-order mutation plus a deterministic slow-handler/fast-notification barrier proving slow SDK handlers block dispatch.
- `cargo run --quiet --bin e4` — cwd, stderr, exit, process group, stall, cancellation, EOF, and shutdown-grace matrix.
- `cargo run --quiet --bin e5 -- matrix` — all 18 request callbacks and two hook notifications answered through the direct SDK client.
- `cargo run --quiet --bin e5 -- all` — authenticated direct v2/KAS parity path; completed on stable wire v1 after the user refreshed the Kiro token.
- `cargo run --quiet --bin e6` — zero/no-op/transform plus distinct/repeated ordered conductor fixtures, extension request/notification flow, response identity, hop-local cancellation, and terminal-component failure.
- `cargo run --quiet --bin e6_live -- all` — authenticated v2/KAS × zero/no-op/transform matrix; all six cells completed on stable wire v1.
- `cargo run --quiet --bin e8` — concurrent stream plus permission response through two inline observers, slow-observer delay, observer disconnect, and fresh-channel replay characterization.
- `cargo run --quiet --bin e9` — current memory/capture seam compared with the same prepared prompt and updates at a raw tap.
- `cargo run --quiet --bin e10` — v1/v2 route, fallback, rejected-v2 negotiation, and core-only import census.

## Oracle

- Independent scripts: `python3 oracles/e1.py` through `python3 oracles/e10.py`, plus authenticated `oracles/e6_live_parity.py`, from the probe package. Probe and oracle JSON carry design-localizing `claim_ids`.
- Different failure mechanisms: standalone `rustc` negative compile, Python JSON/`Decimal`, upstream SDK exact tests, direct OS subprocess checks, committed wire captures/covenant, concurrent Python queue/observer scenarios, workspace manifest/source census, and Cyril's existing behavioral contract tests.
- Official SDK tests exercised: handler priority/claim/fallthrough/dynamic-install ordering and out-of-order slow requests; conductor one/two/three-component initialization; client↔agent hop-local cancellation; v2 selection, v1 fallback, and no retry after v2 rejection.
- Current Cyril oracle exercised twelve core/App/runtime contracts covering original/prepared prompt placement, exactly once, memory unavailable/starting/ready, UTF-8/tool budgets, terminal dispositions, quiescence, shutdown order, and source identity.
- Live oracle: after the user refreshed the Kiro token, `oracles/e6_live_parity.py` ran direct v2/KAS and all six conductor cells, normalized protocol/end-turn/required-method contracts, and returned `direct_vs_conductor_equivalent: true`. No credential value was printed.

## Comparisons

| ID | Probe output | Independent oracle | Verdict |
|----|--------------|--------------------|---------|
| P1 | Protocol and domain ran on distinct threads; bounded handoff recorded `bounded-handoff`; no unsafe/shared domain lock. | Moving `Rc` through `thread::spawn` failed to compile while a bounded string handoff compiled and ran. | PASS |
| P2 | SDK inspection preserved batch shape/order, malformed evidence, and the unknown nested update. The probe's explicit pre-parse copy retained exact `1e400`; SDK semantic parsing rejected it; parseable `1.2300` normalized to `1.23`. This proves semantic behavior and why lexical capture must precede parsing, not that the future process adapter already ingests raw bytes. | Python raw JSON plus `Decimal` independently supplied the exact fixture and semantic expectations. Production ingress proof remains pending in design C4. | PASS — necessity/placement only |
| P3 | Unknown-first contained the update; typed-first and removed-handler variants silently dropped it without closing the connection. A deliberately pending slow handler prevented the independent fast notification until release. | Five upstream exact tests passed, including dynamic install and out-of-order slow requests; source audit confirmed dynamic guard removal. | PASS — handlers must enqueue and return |
| P4 | Child inherited parent cwd; nonzero exit carried bounded stderr; process-group drop killed child/grandchild; no idle watchdog; clean stderr had no public tail; EOF shutdown was bounded to one second. | Direct OS cwd/stderr/exit check plus SDK/Cyril source comparison confirmed the missing cwd, stall, and tail contracts. | PASS — retain Cyril adapter |
| P5 | The direct SDK client completed authenticated Kiro v2 and KAS sessions on protocol v1; both prompt responses arrived after their notification streams, and the 18-request/two-notification host-callback matrix remained complete. | Covenant plus committed captures supplied the independent catalog/order oracle; the current live sessions ended normally. A non-blocking environment warning reported one unrelated Datadog MCP server still needs its own authorization. | PASS |
| P6 | All six v2/KAS × zero/no-op/transform live conductor cells negotiated protocol v1, returned a session ID, and ended with `stop_reason: end_turn`. `e6_live_parity.py` compared each cell with its direct-engine contract and returned six `contract_matches_direct: true` results. Deterministic fixtures also preserved IDs, cancellation, failure, direction, and distinct/repeated order. | Five official conductor initialization/cancellation tests passed; independent Python envelope composition reproduced distinct/repeated order. | PASS |
| P7 | In the refreshed real v2 and KAS transforming chains, the proxy annotated outbound `session/prompt` and inbound vendor notifications (`_kiro.dev/metadata` or `_kiro/mcp/status`); both agents still ended normally. Engine conversion and host callback effects stayed in the terminal client/agent components. | Independent raw-envelope model restored outer response identity and confirmed no Engine/host owner in the proxy source. | PASS |
| P8 | Four frames streamed around a permission request at 5 ms intervals. Two inline observers saw source order, but the 50 ms observer delayed the second observer and forwarding; the permission response returned on the same linear path; observer disconnect terminated that bridge; a fresh channel had no replay. | Independent concurrent Python model reproduced stream/permission order, slow-observer coupling, disconnect abort, and absent replay; source audit confirmed unbounded channel and single-successor protocol. | PASS — separate bounded broadcaster required |
| P9 | Tap observed `[memory]\\nlesson\\nUSER`, not `USER`, and lacked project/source-turn/bridge-turn/disposition fields. | Twelve current core/App/runtime contracts passed across original/prepared prompt, exactly-once, unavailable/starting/ready memory, UTF-8/tool budgets, all terminal dispositions, quiescence/shutdown, and identity. | PASS — keep current seams |
| P10 | Default builders selected v1; explicit v2 selected only v2; negotiated fallback selected v1; v2 rejection did not retry; conversion was handshake-only. | Three official protocol-v2 tests passed; independent member-manifest/Rust-source census found ACP imports only in `cyril-core`. | PASS |

## Validated / learned

- The private SDK/component seam can be proxy-ready without moving Cyril's serial domain actor into `Send` state. A bounded typed handoff is sufficient.
- SDK inspection is a semantic tap, not an exact byte recorder. E2 proves capture must occur before parsing; the real process-ingress capture remains a pending production-migration fence, not claimed empirical output.
- Unknown-update containment is an ordering invariant. Register the untyped fence before typed handlers, and make every SDK handler enqueue bounded mediator work and return because a pending handler blocks later dispatch.
- `AcpAgent` contributes useful process-group and bounded-exit behavior, but replacing Cyril's process adapter would regress cwd selection, stall recovery, and diagnostic-tail access.
- The conductor is a linear full-duplex chain. Slow/disconnected inline observers affect protocol progress, permissions stay on the same linear path, and fresh attachments receive no replay; conductor inspection is not a multi-observer bus.
- Ordered proxy composition is not inferred from a single stage: deterministic fixtures cover a distinct no-op/transform chain and two independently labeled instances of the same transforming type.
- Memory lesson injection remains before transport; normalized `SourceObserver` capture remains after domain conversion. A raw tap may supplement, not replace, either.
- Stable wire v1 is the production default. Draft wire v2 requires an explicit feature and connector/router path.
- All ten empirical premises pass. The prove-it-prototype handoff criterion is satisfied; this evidence is the verified input to falsifiable design.

## Related issues

- Consulted: `cyril-1gfe` (parent SDK 2 migration), `cyril-ai1y` (unknown nested `SessionUpdate` containment), `cyril-p7kp` (SDK/schema forward-compatibility watch), `cyril-9akh` (notification/response ordering), `cyril-1ixa` (unbounded ACP notification buffering), `cyril-wkt8` (oversized/split frame watch), and `cyril-bh7g` (KAS terminal/liveness evidence). Bounded search: `rivets list -l acp --json -n 50` on 2026-08-29.
- Newly recorded gap: SDK `AcpAgentConfig` lacks cwd and SDK `AcpAgent` lacks Cyril's stall-watchdog/public diagnostic-tail contracts. This is a required adapter responsibility in the eventual design/decomposition, not a separate defect outside `cyril-41bs`.
- Filed: none. No empirical finding blocks the selected topology: Cyril's retained process adapter resolves the `AcpAgent` gaps, and verified follow-on issues already cover multi-client delivery/backpressure. Therefore no new upstream issue/PR or speculative production code is justified.
