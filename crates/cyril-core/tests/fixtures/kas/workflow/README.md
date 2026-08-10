# Workflow lifecycle fixtures (cyril-6beh)

Canonical fixture home for the `_kiro/workflow/*` converter and state tests.
Unit tests `include_str!` these files; `.cyril-6beh/compare-oracles.sh` runs
the independent Python/jq oracles against the **same** files, so there is one
copy of every byte both sides compare.

## Live captures (immutable evidence — never edit)

Copied from their research originals; the originals remain the citable
artifacts for docs and the rivets issue.

| file | origin | what it proves |
|---|---|---|
| `terminal-failed-2.16.2.jsonl` | `.cyril-6beh/` probe run 2026-08-09 | terminal `failed` run_complete + retry frames |
| `terminal-aborted-2.16.2.jsonl` | `.cyril-6beh/` probe run 2026-08-09 | terminal `aborted` + live recipe catalog (`modelId`/`effortLevel` spellings) |
| `kas-repeat-watch-2.16.0.jsonl` | `experiments/conductor-spike/` | repeat + watch + steps_queued, `iter-N` paths |
| `kas-custom-dag-2.16.0.jsonl` | `experiments/conductor-spike/` | parallel branches, `branchId`, double `node_start` |
| `kas-csig-2.16.0-neutral.jsonl` | `experiments/conductor-spike/` | 2.16.0 induced completionSignal |
| `kas-csig-2.16.2-neutral.jsonl` | `experiments/conductor-spike/` | 2.16.2 absent completionSignal on a completed node |
| `kas-csig-2.16.2-explicit.jsonl` | `experiments/conductor-spike/` | 2.16.2 model-elected send_message signal |

## Contract and oracle artifacts

| file | role |
|---|---|
| `oracle-manifest.json` | frozen field/enum/bound contract; embedded via `include_str!` and diffed byte-exact in `workflow_oracle_manifest_matches_binary` |
| `oracle-replay-events.jsonl` | synthetic lifecycle sequence exercising shapes no capture reached (`node_paused` among them) |
| `oracle-replay-expected.json` | output of `.cyril-6beh/oracle-replay.py` over the eight replay sources, in `REPLAY_SOURCES` order — regenerate with `.cyril-6beh/compare-oracles.sh replay` inputs whenever a source changes |
| `oracle-snapshot-expected.json` | output of `.cyril-6beh/oracle-snapshot.py` over the two terminal captures |
