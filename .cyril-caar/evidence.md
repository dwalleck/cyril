# cyril-caar — startup budget and inventory evidence

## Current draft publication — 2026-10-22

**Native acceptance BLOCKED by authentication; draft only.** The current proof packet owns qualification, attempt attribution, historical failures and the latest native-auth blocker. The raw premise investigation below is not current native or integrated acceptance evidence.

The [current proof index](current-main-proof/index.json) owns source/binary identities, the sealed local qualification, independent-review applicability and pending gates. See [local qualification](current-main-proof/qualification.json), [review applicability](current-main-proof/review-applicability.json), [native attempts and blocker](current-main-proof/native-consumer.json), and the [draft PR body](pr-body.md). No native/CI/final-conformance/merge-ready or issue-closure claim follows from this publication. Logical delivery remains Tqr #193 → NK #196 → Caar; the draft targets main for existing CI and explicitly depends on #196.

## Retained approved contract and canonical history

Imported from the owning canonical Caar record. Approved requirements remain governing; historical source pins, patch recipes and check statuses below remain attributed to their named earlier frames, not to the current qualified source. Archival patches/private receipts referenced below are not implicitly shipped or authorized for reapplication. Current status is the proof index above.

Investigation date: 2026-10-19. Inspected repository base `8e5c325e`, before production edits. Full issue read. Investigation ran a raw v2 protocol probe, not a Cyril smoke/build/test/lint/formatter gate. Subsequent implementation follows the approved stack/design; no implementation verification is claimed here.

## Premises

| ID | Question and dependent design | Result / limit |
|---|---|---|
| P1a | Is `_kiro.dev/mcp/startup_status` a request, and does its pending value contain server names? | **PASS, observed on kiro-cli 2.32.0.** Request requires `sessionId`. Actual result: `{allStarted:false,failed:[],pending:["probe-silent"],determinable:true}`. This corrects old research's notification/three-variant-enum inference. `failed` is an additional observed field absent from older struct-string summaries; nonempty failed element shape is not established by this run. |
| P1b | Can an authoritative effective inventory be obtained while new-session is genuinely stalled, early enough to name every configured-but-silent server at its client timeout? | **Timing guarantee unproven; explicit absent-inventory behavior APPROVED in spec D1.** Correct-ID status request was sent while new was pending, but its response arrived only alongside new-session completion. Neither this observation nor retained captures guarantees inventory availability for a new that never completes. Do not claim the interface fundamentally cannot do it or assume that it can. The approved implementation names known facts and explicitly reports no complete agent inventory when absent. |
| P2 | Does the current Cyril path drop early MCP frames solely because no session route has been registered? | The historical issue premise is not established on current SDK2 source. Extension handlers already enqueue while new-session await runs off-loop; existing v2 MCP conversions are global and can forward before an active session ID. Need consumer regression to prove retention across `SessionCreated`, and new KAS typed snapshots must not regress this. |

## Existing source facts

- `crates/cyril-core/src/protocol/domain_mediator/commands/mod.rs:17-44`: session/new and load share 30s; ordinary commands use 10s; prompt has no response timeout. `await_response` uses tokio timeout around an already-sent request.
- `.../commands/session.rs:132-177`: new-session response wait runs on a spawned command task; failure becomes a fatal disconnect. `:245-263` publishes usage start, `SessionCreated`, then config, because created is a per-session reset boundary. Preserve the ordering and fail-stop cleanup.
- `.../domain_mediator/inbound.rs:148-215`: extension ingress is independent of the new response and conversion currently scopes selected identity-bearing types but otherwise forwards globally. `:25-81` routes through existing turn mediation; do not bypass it with a second event loop.
- `.../convert/kiro.rs:862-949,1104-1157`: command advertisements currently discard `mcpServers`; existing ready/failure/OAuth events are converted. Conversion also discards the sessionId present on the new live v2 ready frame; startup attribution should inspect/retain true wire identity where supplied rather than invent it.
- `new_session_request` sends the SDK-default empty `mcpServers` list, not an effective declaration inventory. `kiro_agent_config.rs:1-18,107-143` owns trust persistence and `<agent>.json` path lookup, not the Kiro MCP resolver.
- `docs/kiro-mcp-remote-servers.md:43-85`: engine resolution combines global/workspace/profile/power/plugin/client/registry sources and policy filtering. A filesystem scan of two JSON files is not an equivalent complete inventory and must not be smuggled into caar.
- `experiments/conductor-spike/v2-live-session-trace-2.11.0.jsonl:8-9`: a known v2 command advertisement includes a still-loading server, but arrives after new-session response in that trace. It is useful when received, not a proved pre-timeout guarantee.
- KAS source/trace: `experiments/conductor-spike/mcp-remote-2.31.1/kas-static.md:429-443` and `experiments/conductor-spike/audit-2.31.1/live/kas-lead-mcp-2.31.1.jsonl:11-24` establish pre-response connecting/connected status. `nk4o` supplies typed status ownership. Current source-established OAuth/reset details live once in `.cyril-tqr8/evidence.md`.

## Probe and exact observations

Instrument: `probe-startup.py`. It creates two synthetic stdio servers (`probe-ready`, `probe-silent`, whose initialize waits 120s), isolated HOME/KIRO_HOME/workspace/cache/config/temp roots and a string-only mock backend. It never sends session/prompt, launches a browser or invokes a model. It terminates its owned process group; no authentication store is copied.

Initial clean-data arm:

```
python3 .cyril-caar/probe-startup.py --observe-seconds 40
```

Exited 1 before initialize completed, no new-session response. Its first receipt deliberately retained stderr byte count only, so no cause is inferred from that run. A changed-input mock arm:

```
python3 .cyril-caar/probe-startup.py --observe-seconds 40 --mock-backend
```

also exited 1 before initialize; stderr: `error: You are not logged in, please log in with kiro-cli login`. Receipt: `/home/dwalleck/.claude/tmp/cyril-caar-startup-v7p8kluk/receipt.json`. String mock backend alone does not bypass the startup login gate.

Safe authenticated arm (read-only existing native auth, no account/settings writes):

```
python3 .cyril-caar/probe-startup.py --observe-seconds 40 --mock-backend --native-auth-readonly
```

The command uses bubblewrap `--ro-bind / /` and makes only the freshly minted probe root writable, with a private PID namespace/die-with-parent. HOME/KIRO_HOME/cwd/config/cache/temp stay isolated; only XDG_DATA_HOME points at the existing native data root, read-only by mount enforcement. Provider credential variables and ambient hooks/settings are not forwarded. This booted real installed kiro-cli 2.32.0 v2. No prompt, model usage, copied credentials, browser or production settings mutation.

Owning raw receipt: `/home/dwalleck/.claude/tmp/cyril-caar-startup-3a3yzq4x/receipt.json`. Sanitized durable wire projection: `startup-status-2.32.0.json` (contains exact relevant envelopes, plus an explicitly labeled session-response projection).

| Monotonic seconds from process spawn | Observation |
|---|---|
| 0.921 | Initialize had completed; send status request with `{}` then session/new. |
| 0.921 | Status request rejected with parse error: missing field `sessionId`. |
| 2.923 | Send status request with empty sessionId while new pending. |
| 4.391 | Receive ready frame carrying true sessionId and serverName `probe-ready`. |
| 4.991 | Send status request using that true sessionId, still before new response. |
| 9.370 | Ready frame is repeated, new-session responds, empty-ID request returns unknown-session, and true-ID request returns named pending `probe-silent`. |

Observed process exit `-15` was **probe-owned teardown after collecting the requested result**, not a Cyril session-start failure. Stderr was empty. This is a factual protocol observation, not a consumer latency benchmark, a <=90s acceptance result, or a universal producer-timing proof.

## Validated / learned

- Current v2 exposes a real startup-status **query** with named pending servers. Existing docs and binary shape summaries were insufficient to infer its contract, and the current result has four observed fields.
- Current v2 ready notifications can expose a session ID before session/new responds. Do not preserve a historical assumption that all v2 MCP frames are sessionless.
- A status request sent before completion did not yield an early inventory in this controlled two-server scenario. Do not upgrade named `pending`, advertised/loading names or a post-success query into a complete startup set. KAS's correctly scoped current pool snapshot is authoritative when received.
- Missing complete inventory must be explicit, never fabricated names or `silent:none`. The requester's sufficient absent-data behavior is now approved and recorded once in `spec.md` D1; this does not make P1b's timing guarantee observed.
- No new probe of external OAuth semantics is needed: current KAS source already answers those facts. `.cyril-tqr8/fixture.py` supplies controlled OAuth integration after implementation.

## Related issues / handoff

`cyril-caar`, `cyril-nk4o`, `cyril-tqr8` full records read; caar has no prerequisite. `cyril-0dv4` is closed and fences required mcpServers request presence, not inventory completeness. No tracker mutation/closure.

Implement the full approved deadline/early-event/known-inventory/unknown-inventory design, then require parent-owned red/green and real consumer proof. No absent-inventory acceptance gate remains, but P1b must never be replaced with an alleged observed PASS from the successful query after session creation.
