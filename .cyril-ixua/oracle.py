from itertools import product
from pathlib import Path
import re
import subprocess
import sys

NAMES = {
    "Canvas background": "canvas", "Chrome background": "chrome",
    "Code background": "code",
    "Selection background": "selection", "Primary text": "text",
    "Muted text": "muted", "Border": "border", "Primary accent": "accent",
    "Secondary accent": "accent_alt", "User message": "user",
    "Agent message": "agent", "System message": "system", "Info": "info",
    "Success": "success", "Warning": "warning", "Danger": "danger",
    "Diff addition": "diff_add", "Diff deletion": "diff_delete",
    "Diff context": "diff_context",
}

spec = Path(".cyril-ixua/spec.md").read_text(encoding="utf-8")
source = {
    NAMES[name]: tuple(bytes.fromhex(value))
    for name, value in re.findall(r"\| ([^|]+) \| `#([0-9a-f]{6})` \|", spec)
    if name in NAMES
}
expected_roles = [
    NAMES[name]
    for name, _ in re.findall(r"\| ([^|]+) \| `([^`]+)` \|", spec)
    if name in NAMES
]
if len(sys.argv) == 3 and sys.argv[1] == "--input":
    probe = Path(sys.argv[2]).read_text(encoding="utf-8")
elif len(sys.argv) == 2:
    probe = subprocess.run(
        [sys.argv[1]], check=True, text=True, capture_output=True
    ).stdout
else:
    sys.stderr.write("usage: oracle.py PROBE | oracle.py --input TSV\n")
    raise SystemExit(2)
lines = probe.splitlines()
header = lines[0].split("\t")
rows = [dict(zip(header, line.split("\t"), strict=True)) for line in lines[1:]]
levels = [0, 95, 135, 175, 215, 255]
xterm = [(16 + i, rgb) for i, rgb in enumerate(product(levels, repeat=3))]
xterm += [(232 + i, (8 + 10 * i,) * 3) for i in range(24)]
ansi16 = [
    (0,0,0),(128,0,0),(0,128,0),(128,128,0),(0,0,128),(128,0,128),
    (0,128,128),(192,192,192),(128,128,128),(255,0,0),(0,255,0),
    (255,255,0),(0,0,255),(255,0,255),(0,255,255),(255,255,255),
]
dist = lambda a, b: sum((x - y) ** 2 for x, y in zip(a, b))
role_errors = []
value_errors = []
ansi256_errors = []
ansi16_errors = []
ansi256_count = 0
ansi16_count = 0
actual_roles = [row["role"] for row in rows]
expected_row_roles = [
    role for role in expected_roles if "rgb" not in header or role != "canvas"
]
if actual_roles != expected_row_roles:
    role_errors.append((expected_row_roles, actual_roles))
for row in rows:
    role = row["role"]
    if "rgb" not in row:
        continue
    rgb = tuple(bytes.fromhex(row["rgb"]))
    if source.get(role) != rgb:
        value_errors.append((role, source.get(role), rgb))
    if "ansi256" in row:
        ansi256_count += 1
        expected_index = min(
            xterm, key=lambda item: (dist(rgb, item[1]), item[0])
        )[0]
        try:
            actual = int(row["ansi256"])
        except ValueError:
            ansi256_errors.append((role, "non-integer", row["ansi256"]))
        else:
            if actual != expected_index:
                ansi256_errors.append((role, expected_index, actual))
    if "ansi16" in row:
        ansi16_count += 1
        expected_index = min(
            range(16), key=lambda i: (dist(rgb, ansi16[i]), i)
        )
        try:
            actual = int(row["ansi16"])
        except ValueError:
            ansi16_errors.append((role, "non-integer", row["ansi16"]))
        else:
            if actual != expected_index:
                ansi16_errors.append((role, expected_index, actual))
checks = [("role-names", role_errors, len(rows), len(expected_row_roles))]
if "rgb" in header:
    checks.append(("role-values", value_errors, len(rows), len(source)))
if ansi256_count:
    checks.append(("ansi256", ansi256_errors, ansi256_count, len(source)))
if ansi16_count:
    checks.append(("ansi16", ansi16_errors, ansi16_count, len(source)))
for label, errors, count, total in checks:
    if errors:
        details = "\n".join(str(error) for error in errors)
        sys.stderr.write(f"DISAGREE {label}\n{details}\n")
        raise SystemExit(1)
    sys.stdout.write(f"AGREE {label} {count}/{total}\n")
