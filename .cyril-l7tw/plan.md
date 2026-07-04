# cyril-l7tw — budgeted plan

8 slices. Workspace gate after every slice: `cargo test -p cyril-core` (both
default and `--features kas`), `cargo clippy --all-targets -- -D warnings`,
`cargo fmt --check`. The prove-it oracle re-run (l7tw_death_probe transcript
vs tracing/stderr log) is slice 8's job; slices 1-7 rely on the in-process
fences plus the standing `l7tw_agent_drop_resolves_prompt_err_and_completes_io`
mechanism fence.

## Slice 1: prompt-task Err emits BridgeError before TurnCompleted

**Claim:** C1 (BridgeError{op="prompt", message passthrough} precedes the
synthesized TurnCompleted), C2 (still exactly one TurnCompleted(EndTurn)),
C12 (no BridgeError noise on a successful turn).
**Oracle:** channel transcript order as seen from the App side of the
harness (not bridge internals); C12 via zero-BridgeError scan of the existing
happy-path turn test.
**Stress fixture:** fake agent streams 2 chunks THEN fails the prompt
(script gains an error mode) — catches "error notification jumps the queue
ahead of already-streamed content" and "busy sticks because error replaced
completion". Expected: chunks, then BridgeError("prompt", <script's message>),
then exactly one TurnCompleted(EndTurn).
**Loop budget:** none added (two sequential sends on an existing path).
**Wall budget:** n/a (event-driven arm).
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (Err arm + tests;
Script extension in the tests mod).

Code (advisory): in the spawn_local Err arm, before the TurnCompleted send:
`let _ = turn_tx.send(Notification::BridgeError { operation: "prompt".into(),
message: e.to_string() }.into()).await;` — but per zero-`let _ =` discipline,
use the debug-log-on-err shape the TurnCompleted send already uses.

**Verification:**
- [ ] `death via script error: chunks → BridgeError → single TurnCompleted` fence green (fails pre-change)
- [ ] `harness_drives_one_turn` extended zero-BridgeError assert green
- [ ] full gate green (both feature sets)

## Slice 2: harness agent-kill lever + real-death fence

**Claim:** C1/C5 via the real death mechanism (clean EOF), not a scripted
error: killing the agent side mid-turn produces BridgeError → TurnCompleted.
**Oracle:** the standing mechanism fence (C-mech) proved drop ⇒ prompt Err +
io Ok(EOF); this slice's fence must agree with the S1 scripted-error fence's
transcript shape — two independent triggers, same observable contract.
**Stress fixture:** kill while the prompt is PARKED (gate never released) —
catches "Err arm only reached when the agent replies first". Expected:
BridgeError("prompt", …) then one TurnCompleted within 5s, no hang.
**Loop budget:** none (test-only slice).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (tests mod only:
`with_harness` exposes a kill handle — take the `a_task` JoinHandle + agent
conn cell into an `AgentKill` closure/struct passed to `body`; kill = clear
cell + abort a_task; empirically verify EOF reaches the client).

**Verification:**
- [ ] `death_mid_turn_emits_bridge_error_before_turn_completed` green
- [ ] `death_mid_turn_single_turn_completed` green
- [ ] existing harness tests untouched and green (lever is additive param — update call sites mechanically)
- [ ] full gate green

## Slice 3: io watcher + idle-death disconnect + loop exit

**Claim:** C3 (idle death ⇒ BridgeDisconnected with "agent connection
closed" + tail excerpt, then run_loop exits), C5 (detection on clean EOF).
**Oracle:** loop JoinHandle completion (tokio) + reason substring; the
spurious-disconnect counter-fixture uses the App-visible transcript.
**Stress fixture:** TWO fixtures. (a) idle kill ⇒ disconnect + loop exit
within 5s. (b) **normal Shutdown ⇒ ZERO BridgeDisconnected** — designed to
catch the plausible bug "watcher fires on ordinary teardown too" (Shutdown
breaks the loop first, so the watcher's send hits a dropped receiver — that
must stay silent).
**Loop budget:** one drain loop at death: `while let Ok(n) = inbound_rx.try_recv()`
— O(NOTIFICATION_CAPACITY)=O(256) once, at death only. Within budget.
**Wall budget:** watcher = one `.await` on the io JoinHandle; no polling.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (run_bridge watcher +
run_loop signature `io_done: oneshot::Receiver<String>` + select arm; tests
mod: harness builds the watcher over its `c_task`, kill lever now also ends
`c_task` so the arm fires).

Design detail (from approved design): arm guarded `, if conn_dead.is_none()`;
`Err(RecvError)` treated as death with a generic reason (watcher can only
vanish via LocalSet teardown after the loop returns — tolerate, warn). Reason
built via `tail_excerpt(&StderrTail) -> Option<String>` helper (last 5 lines;
None when empty) introduced here, used again in slice 5. In the harness there
is no real process, so the watcher reason is built with an empty tail.

**Verification:**
- [ ] `death_while_idle_emits_disconnected_and_exits` green (times out pre-change by construction — detached pump)
- [ ] `shutdown_emits_no_disconnect` green
- [ ] full gate green

## Slice 4: mid-turn defer — BridgeError → TurnCompleted → BridgeDisconnected

**Claim:** C4 (disconnect deferred until the killed turn's TurnCompleted is
observed; full order), C6 (post-exit `BridgeSender::send` errors — no silent
acceptance after death), C2 preserved (dedup undisturbed).
**Oracle:** ordered App-side transcript; sender `Result`; C13 = existing
KAS-2a idempotent-completion tests stay green untouched.
**Stress fixture:** (a) kill mid-parked-turn ⇒ exact sequence BridgeError,
TurnCompleted, BridgeDisconnected, then loop exit; sender.send afterwards
errs. (b) duplicate TurnCompleted injected after conn_dead is set (via
inbound_tx clone in the fixture) ⇒ **exactly one** BridgeDisconnected —
catches "dedup `continue` skips the deferred-disconnect hook" and
"double-disconnect on duplicate completion".
**Loop budget:** none added (flag check on existing arm).
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs`.

**Verification:**
- [ ] `death_mid_turn_disconnect_after_completion` green
- [ ] `dead_conn_prompt_errors_not_silent` (post-exit sender errs) green
- [ ] duplicate-completion fixture green; KAS-2a suite green both feature sets
- [ ] full gate green

## Slice 5: stderr tail appended to propagated bridge errors

**Claim:** C7-unit (handshake-failure reason carries the tail excerpt), C8
(spawn failure still yields BridgeDisconnected; empty tail ⇒ well-formed
reason, no dangling "stderr:" stub).
**Oracle:** C7's ultimate oracle is kiro-cli's own stderr text vs the
notification reason (live in slice 8); unit form asserts the append helper's
composition on a synthetic tail. C8's oracle: real `spawn_bridge` against a
bogus binary name — OS-level spawn failure, drained from the real channel.
**Stress fixture:** tail with >5 lines (excerpt must keep the LAST 5 —
catches first-N-instead-of-last-N), tail with empty-string lines, and the
empty tail (None ⇒ reason byte-identical to input — catches stub-append).
**Loop budget:** excerpt = O(50) ring-buffer lines, once, at failure only.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (run_bridge
`.map_err(append)` on run_loop's Err + helper; tests).

**Verification:**
- [ ] `handshake_failure_reason_includes_stderr_tail` (unit, synthetic tail) green
- [ ] `spawn_failure_disconnect_reason_wellformed` green (real spawn_bridge, bogus binary)
- [ ] full gate green

## Slice 6: undroppable fail-stop emission (bounded-timeout send)

**Claim:** C9 (BridgeDisconnected survives a full channel with a live App),
C10 (dropped receiver ⇒ prompt return, no hang).
**Oracle:** receiver-side drain content after the emission call (C9);
wall-clock watchdog around the call (C10). The C9 fence fails against
today's `try_send` by construction.
**Stress fixture:** channel filled to exactly NOTIFICATION_CAPACITY (256)
with junk notifications, consumer drains only AFTER emission starts —
catches both `try_send`-drops and "send succeeded but delivered before the
backlog" misordering assumptions (assert disconnect is LAST).
**Loop budget:** fixture fill loop O(256), test-only.
**Wall budget:** emission bounded at 5s by design (timeout).
**Files:** `crates/cyril-core/src/protocol/bridge.rs` (extract
`emit_failstop_disconnect(rt: &Runtime, tx: &Sender<RoutedNotification>,
reason: String)`; spawn_bridge calls it; tests).

Doc-comment contract: "call only after the LocalSet has returned" is a
sanity hint (single call site, same fn) — no runtime check;
the 5s bound IS the load-bearing enforcement against a wedged App.

**Verification:**
- [ ] `failstop_disconnect_survives_full_channel` green (fails pre-change)
- [ ] `failstop_disconnect_no_hang_on_dropped_receiver` green
- [ ] full gate green

## Slice 7: KAS auth-callback failure emits BridgeError("auth", …)

**Claim:** C11 (responder Err ⇒ BridgeError with the actionable message —
"run kiro-cli login" hint — AND the JSON-RPC error still returns to KAS),
C12-kas (no BridgeError on auth success).
**Oracle:** unit test drives `KiroClient` seam helper with a constructed
`acp::Error` (deterministic — no sqlite store involved; injectable store is
cyril-5db7, verified open); asserts both the notification on a real channel
receiver AND the returned Err. Buggy impl it catches: today's swallow-into-
JSON-RPC-only.
**Stress fixture:** auth error whose message already contains "login"
(no double-hint), and a NON-auth ext method failing (must NOT emit
BridgeError — scope check).
**Loop budget:** none.
**Wall budget:** n/a.
**Files:** `crates/cyril-core/src/protocol/client.rs` (ext_method wiring +
`notify_if_auth_failure(&self, method, &Result<…>)` helper, kas-gated;
tests colocated).

**Verification:**
- [ ] `auth_callback_err_emits_bridge_error` green under `--features kas`
- [ ] `non_auth_ext_err_emits_nothing` green
- [ ] full gate green BOTH feature sets (this slice compiles only under kas — per cyril-ykkc the kas run is mandatory here)

## Slice 8: live validation + probe re-run + audit trail

**Claim:** the assembled binary honors C1-C8 against real transports:
(a) `l7tw_death_probe` (replay agent, verbatim 2.11.0 frames) transcript now
shows BridgeError → TurnCompleted → BridgeDisconnected; (b) real logged-OUT
kiro-cli 2.11.0 handshake failure reason contains "not logged in" (C7-live).
**Oracle:** the prove-it oracle — tracing/stderr log + OS process table —
must agree with the enriched transcript (same comparison as probe runs 1-2).
**Stress fixture:** the probe's phase 2 (post-exit prompt) — expected to
now surface as an App-side send error rather than a silent TurnCompleted;
recorded in findings.md run 3.
**Loop budget:** n/a (validation only).
**Wall budget:** probe wall ≈ 40s (as run 2).
**Files:** `.cyril-l7tw/findings.md` (+ probe run 3 outputs); no prod code.
A logged-IN mid-turn kill vs real kiro-cli is optional extra credit gated on
the user running `kiro-cli login` (offered at merge pause; fences are the
CI-permanent form).

**Verification:**
- [ ] probe run 3 transcript shows the C4 sequence; oracle log agrees
- [ ] logged-out real-kiro run shows "not logged in" inside BridgeDisconnected reason
- [ ] findings.md updated with both transcripts

## Plan Self-Review

1. **Loops:** slice 3 drain O(256) once-at-death; slice 5 excerpt O(50)
   once-at-failure; slice 6 fixture fill O(256) test-only. All ≪ 10^6; no
   always-on loops added. No unbounded loops anywhere.
2. **Fixtures:** every slice names the bug class its fixture is designed to
   fail under (queue-jump ordering S1; parked-prompt hang S2; spurious
   disconnect on Shutdown S3; dedup-skips-hook + double-disconnect S4;
   first-N-vs-last-N + empty-tail stub S5; try_send drop + ordering S6;
   double-hint + non-auth scope S7; real-transport end-to-end S8). No
   happy-path-only slices.
3. **Doc-comment preconditions:** slice 6's "call after LocalSet returns" =
   sanity hint (single call site), 5s timeout is the load-bearing bound;
   slice 3's oneshot "sender sends before drop" = enforced at runtime by
   treating RecvError as generic death (no silent arm). No unenforced
   contracts introduced.
4. **Write targets:** prod writes are channel notifications (data path) and
   `tracing` (diagnostic, stderr-bound in the probe). Probe stdout =
   transcript data, stderr = oracle log — already established in the probe
   stage. No new println!.
5. **Tracker references:** cyril-gua0 (respawn — filed this cycle),
   cyril-1ixa, cyril-a71q, cyril-3zy4, cyril-5db7, cyril-ykkc — all verified
   present in rivets during design/probe stages. No uncited deferrals.

Claim coverage vs design: C-mech (done), C1(S1,S2), C2(S1,S4), C3(S3),
C4(S4), C5(S2,S3), C6(S4), C7(S5 unit, S8 live), C8(S5), C9(S6), C10(S6),
C11(S7), C12(S1,S7), C13(S4). Complete.
