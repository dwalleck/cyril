# conductor-spike

Captures from the 2026-05-03 spike that established two things:

1. **`sacp-conductor` is a clean drop-in for `kiro-cli acp`** — passes all Kiro extension methods (`_kiro.dev/*`) through with no field corruption, no `Method not found` rejections, and microseconds of overhead.
2. **kiro-cli-chat 2.1.0 and 2.2.0 produce field-level identical ACP wire output** when hitting the same backend on the same day. ACP wire-format changes between April 2026 and May 2026 (notably `meteringUsage[]` and `turnDurationMs` on `_kiro.dev/metadata`) are AWS backend rollouts, not binary changes.

See memory entries for full context: `reference_sacp_conductor_spike.md`, `reference_kiro_wire_audit_methodology.md`, `reference_kiro_2_2_0_diff.md`.

## Layout

| File | What |
|---|---|
| `conductor-wrapper-2.2.0.sh` | Wrapper that points conductor at the system-installed kiro-cli (whatever version is on `$PATH`). Used for the 2.2.0 capture since system kiro-cli is currently 2.2.0. |
| `conductor-wrapper-2.1.0.sh` | Wrapper that points conductor at `docs/kiro-binaries-2.1.0/kiro-cli-chat acp` directly, bypassing the kiro-cli router so the 2.1.0 backend is exercised regardless of `$PATH`. |
| `diff_fields.py` | Structural field-path differ. Parses two conductor debug logs and prints field additions/removals per JSON-RPC method. |
| `logs/conductor-2.2.0.log` | Conductor's debug log of the 2.2.0 binary capture — every JSON-RPC line in both directions, with `C →`/`C ←`/`0 →`/`0 ←` direction markers. |
| `logs/conductor-2.1.0.log` | Conductor's debug log of the 2.1.0 binary capture. |
| `test_bridge-2.2.0.out` | Harness-side output (cyril's notifications, step results) from the 2.2.0 run. |
| `test_bridge-2.1.0.out` | Same for the 2.1.0 run. |

## Reproducing

```sh
# Install conductor (sandboxed prefix)
cargo install sacp-conductor --root ~/.local/cargo-spike

# Run against current system kiro-cli
cargo run --example test_bridge -- \
    --agent experiments/conductor-spike/conductor-wrapper-2.2.0.sh

# Run against archived 2.1.0 binary (requires docs/kiro-binaries-2.1.0/ snapshot)
cargo run --example test_bridge -- \
    --agent experiments/conductor-spike/conductor-wrapper-2.1.0.sh

# Diff two captures
python3 experiments/conductor-spike/diff_fields.py \
    /tmp/conductor-spike/logs-210/<latest>.log \
    /tmp/conductor-spike/logs/<latest>.log
```

(Wrapper scripts write to `/tmp/conductor-spike/logs*/` since `/tmp` is fine for runtime artifacts. The captures saved here are the post-run copies.)

## Why this matters for cyril

The conductor passthrough result unblocks the integration plan in `project_cyril_conductor_integration.md`: cyril's bridge can swap `kiro-cli acp` for `sacp-conductor agent "kiro-cli acp"` with zero protocol changes, opening the door to pluggable proxy stages (skill resolver, transcript recorder, auto-approval, etc.) without rewriting cyril as a proxy.
