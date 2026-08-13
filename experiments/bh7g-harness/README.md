# bh7g wedge-hunting harness

Reusable live-repro rig for the cyril-bh7g class of failures: a KAS turn whose
terminal (`turn_end` + prompt response) never reaches the consumer. Built
2026-08-11; captured the backend-stall mechanism on its first run (see
`../conductor-spike/kas-turn-stall-2.16.2.md`).

## Pieces

- `src/main.rs` — instrumented cyril-core consumer (KAS free path): streams a
  multi-round Wayfinder charting session, prints every notification as stamped
  JSON events, exposes a `CANCEL` stdin command (→ `BridgeCommand::CancelRequest`)
  so a stalled turn can be cancelled live, and emits cyril-core tracing
  (`RUST_LOG=cyril_core=debug` → TurnMediator dispositions) on stderr.
- `tap.py` + `node-shim.sh` — wire tap interposed via `KIRO_AGENT_PATH`:
  records every ACP frame both directions as `{ts, dir, msg}` JSONL (auth
  redacted), passes KAS stderr through, and reaps the node child 15 s after
  stdin EOF (acp-server.js does not exit on EOF — orphan prevention).
- `driver.py` — automation: spawns probe runs, auto-answers question rounds,
  auto-approves fixture tool permissions, detects a wedge (210 s stdout
  silence), injects `CANCEL` and watches 90 s (the cyril-14ou
  cancel-under-stall arm), writes `captures/run-N/{wire,stdout,stderr}.jsonl`
  + `verdict.json`. `BH7G_MAX_RUNS` sets the batch size. Pins the KAS bundle
  via `KIRO_KAS_SERVER_PATH` — update the pin when auditing a new version.
- `classify.py` — post-batch triage: signature A (execution succeeded
  internally; wire decides emitted-vs-lost) vs signature B (backend stall,
  execution never succeeded), plus cancel outcomes.

## Running

```sh
cargo build                      # in this directory
BH7G_MAX_RUNS=6 python3 driver.py
python3 classify.py
```

Needs: a servable kiro login token (the spawn gate fails in 0.3 s with the
reason if not — tokens hard-expire ~2 h idle), `rivets` on PATH, and the
Wayfinder/grilling/domain-modeling skill files (paths at the top of main.rs,
overridable via env).

## Known result baseline (2026-08-11, 2.16.2 bundle)

Afternoon window: near-every-run wedge (signature B captured; five earlier
Tauri-driven sessions showed an A-shaped KAS log signature, consumer logs
unrecoverable). Evening: ~27 turns, zero wedges, two full chart-to-map
completions. The stall is a backend condition window, not a per-turn rate.
