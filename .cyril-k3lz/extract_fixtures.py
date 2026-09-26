"""Extract cyril-k3lz test fixtures verbatim from committed 2.24.0 live captures.

Run from the repo root:  python .cyril-k3lz/extract_fixtures.py

The fixtures are the provider's own wire payloads (no hand editing): the KAS
`session/set_config_option` result sequence from the config leg, and the v2
`_kiro.dev/metadata` + `reasoning` command frames from the args2 leg.
"""
import json
from pathlib import Path

SPIKE = Path("experiments/conductor-spike")
KAS_SRC = SPIKE / "kas-new-surface-noprompt-0668-2.24.0.jsonl"
V2_SRC = SPIKE / "v2-reasoning-args2-2.24.0-2.24.0.jsonl"
KAS_OUT = Path("crates/cyril-core/tests/fixtures/kas/thinking/set-config-sequence-0668.json")
V2_OUT = Path("crates/cyril-core/tests/fixtures/v2/thinking/reasoning-args2-2.24.0.json")

# (capture line, step) — the `result` frame of each step in the § 7.1 table.
KAS_STEPS = [
    (13, "session_new"),
    (17, "cfg_model"),
    (19, "cfg_effort_max"),
    (21, "cfg_thinking_off"),
    (23, "cfg_effort_max2"),
    (25, "cfg_thinking_bogus"),
    (27, "cfg_model_gpt"),
]
# (capture line, step) — metadata frames and the reasoning-command responses.
V2_LINES = [8, 24, 25, 26, 27, 28, 29, 30]


def lines(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f]


def main():
    kas = lines(KAS_SRC)
    kas_out = []
    for n, step in KAS_STEPS:
        frame = kas[n - 1]
        assert frame.get("_tag") == step, (n, frame.get("_tag"), step)
        kas_out.append({"step": step, "captureLine": n, "configOptions": frame["result"]["configOptions"]})
    KAS_OUT.parent.mkdir(parents=True, exist_ok=True)
    # One compact step per line: diffable, and ~half the size of indented JSON.
    body = ",\n".join(json.dumps(s, separators=(",", ":")) for s in kas_out)
    KAS_OUT.write_text(
        '{"source":%s,"steps":[\n%s\n]}\n' % (json.dumps(str(KAS_SRC).replace("\\", "/")), body),
        encoding="utf-8",
    )

    v2 = lines(V2_SRC)
    v2_out = []
    for n in V2_LINES:
        frame = dict(v2[n - 1])
        tag = frame.pop("_tag")
        v2_out.append({"captureLine": n, "tag": tag, "frame": frame})
    V2_OUT.parent.mkdir(parents=True, exist_ok=True)
    V2_OUT.write_text(
        json.dumps({"source": str(V2_SRC).replace("\\", "/"), "frames": v2_out}, indent=1) + "\n",
        encoding="utf-8",
    )
    print(KAS_OUT, KAS_OUT.stat().st_size, V2_OUT, V2_OUT.stat().st_size)


if __name__ == "__main__":
    main()
