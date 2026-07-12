#!/usr/bin/env python3
"""Check runtime speaker cells against independent chat-source bindings."""
from pathlib import Path
import csv
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
CHAT = ROOT / "crates/cyril-ui/src/widgets/chat.rs"
PROBE = Path(__file__).with_name("render-probe-output.tsv")
OUTPUT = Path(__file__).with_name("render-oracle-output.tsv")
ARMS = {"user": "UserText", "agent": "AgentText", "system": "System"}
MARKERS = {"user": "You:", "agent": "Kiro:", "system": "Q9DX-SYSTEM"}
FIXED = {"user": "LightBlue", "agent": "LightGreen", "system": "LightMagenta"}

source = CHAT.read_text(encoding="utf-8")
bindings = {}
for role, variant in ARMS.items():
    match = re.search(
        rf"ChatMessageKind::{variant}.*?(?=\n\s*ChatMessageKind::)", source, re.S
    )
    if match is None:
        raise SystemExit(f"missing renderer arm: {variant}")
    fields = re.findall(r"\.fg\(theme\.(\w+)\)", match.group())
    if fields != [role]:
        raise SystemExit(f"binding differs for {variant}: {fields!r}")
    bindings[role] = fields[0]

with PROBE.open(encoding="utf-8", newline="") as handle:
    rows = list(csv.DictReader(handle, delimiter="\t"))
if [row["role"] for row in rows] != list(ARMS):
    raise SystemExit(f"runtime roles differ: {rows!r}")

output = ["role\tsource_binding\tmarker\tx\ty\texpected\tobserved"]
for row in rows:
    role = row["role"]
    expected = FIXED[bindings[role]]
    if row["marker"] != MARKERS[role] or row["foreground"] != expected:
        raise SystemExit(f"DISAGREE {role}: {row!r}, expected={expected}")
    try:
        x, y = int(row["x"]), int(row["y"])
    except (KeyError, TypeError, ValueError) as error:
        raise SystemExit(f"DISAGREE {role}: invalid coordinates: {error}") from error
    if not (0 <= x < 80 and 0 <= y < 24):
        raise SystemExit(f"DISAGREE {role}: marker outside 80x24")
    output.append(
        f"{role}\t{bindings[role]}\t{row['marker']}\t{row['x']}\t{row['y']}"
        f"\t{expected}\t{row['foreground']}"
    )
recorded = OUTPUT.read_text(encoding="utf-8").splitlines()
if recorded != output:
    raise SystemExit("independent render-oracle output is stale")
sys.stdout.write(
    "AGREE visible=3/3 bindings=user,agent,system "
    "colors=LightBlue,LightGreen,LightMagenta\n"
)
