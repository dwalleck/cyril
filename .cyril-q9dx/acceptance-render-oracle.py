#!/usr/bin/env python3
"""Independent runtime oracle for every ANSI-16 identity consumer."""
from io import StringIO
from pathlib import Path
from typing import NoReturn
import csv
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CHAT_SOURCE = ROOT / "crates/cyril-ui/src/widgets/chat.rs"
EXPECTED = {
    "committed_user": ("user", "You:", "LightBlue"),
    "committed_agent": ("agent", "Kiro:", "LightGreen"),
    "system_unicode": ("system", "系统 status", "LightMagenta"),
    "main_streaming_agent": ("agent", "Kiro:", "LightGreen"),
    "subagent_streaming_agent": ("agent", "reviewer:", "LightGreen"),
}


def fail(message: str) -> NoReturn:
    sys.stderr.write(f"C4 FAIL {message}\n")
    raise SystemExit(1)


def single_binding(source: str, label: str) -> str:
    fields = re.findall(r"\.fg\(theme\.(\w+)\)", source)
    if len(fields) != 1:
        fail(f"{label} bindings={fields!r}")
    return fields[0]


source = CHAT_SOURCE.read_text(encoding="utf-8")
bindings = {}
for path, variant in [
    ("committed_user", "UserText"),
    ("committed_agent", "AgentText"),
    ("system_unicode", "System"),
]:
    arm = re.search(
        rf"ChatMessageKind::{variant}.*?(?=\n\s*ChatMessageKind::)", source, re.S
    )
    if arm is None:
        fail(f"missing renderer arm {variant}")
    bindings[path] = single_binding(arm.group(), path)

main = re.search(r"pub fn render\(.*?(?=\nfn render_subagent_drill_in)", source, re.S)
subagent = re.search(
    r"fn render_subagent_drill_in\(.*?(?=\nfn push_thought_lines)", source, re.S
)
if main is None or subagent is None:
    fail("main or subagent renderer block missing")
main_streaming = re.search(
    r"// Render streaming text(.*?)(?=// Render streaming thought)", main.group(), re.S
)
subagent_streaming = re.search(
    r"// Render streaming text(.*?)(?=let visible_height)", subagent.group(), re.S
)
if main_streaming is None or subagent_streaming is None:
    fail("main or subagent streaming block missing")
bindings["main_streaming_agent"] = single_binding(
    main_streaming.group(1), "main_streaming_agent"
)
bindings["subagent_streaming_agent"] = single_binding(
    subagent_streaming.group(1), "subagent_streaming_agent"
)

result = subprocess.run(
    [
        "cargo", "test", "-p", "cyril-ui",
        "widgets::chat::tests::ansi16_identity_consumers_use_speaker_roles",
        "--", "--exact", "--nocapture",
    ],
    cwd=ROOT,
    capture_output=True,
    text=True,
    check=False,
)
if result.returncode != 0:
    fail(f"runtime probe did not run:\n{result.stderr}")
block = re.search(
    r"BEGIN_ANSI16_IDENTITY_PROBE\s*(.*?)\s*END_ANSI16_IDENTITY_PROBE",
    result.stdout,
    re.S,
)
if block is None:
    fail("runtime probe markers missing")
rows = list(csv.DictReader(StringIO(block.group(1)), delimiter="\t"))
if [row["path"] for row in rows] != list(EXPECTED):
    fail(f"runtime paths={[row['path'] for row in rows]!r}")

total_cells = 0
for row in rows:
    path = row["path"]
    role, marker, foreground = EXPECTED[path]
    if bindings[path] != role:
        fail(f"path={path} source_binding={bindings[path]} expected={role}")
    if row["marker"] != marker or row["foreground"] != foreground:
        fail(f"path={path} runtime={row!r} expected={EXPECTED[path]!r}")
    try:
        cells = int(row["cells_visited"])
    except (KeyError, TypeError, ValueError) as error:
        fail(f"path={path} invalid cells_visited: {error}")
    if not 0 < cells <= 1_920:
        fail(f"path={path} cells_visited={cells}")
    total_cells += cells
    sys.stdout.write(
        f"C4 PASS path={path} role={role} marker={marker} "
        f"foreground={foreground} cells={cells}\n"
    )
if total_cells > 9_600:
    fail(f"total cells_visited={total_cells}")
sys.stdout.write(f"C4 PASS paths=5 cells_visited={total_cells} budget=9600\n")
