#!/usr/bin/env python3
"""cyril-lki9 C21 module-shape fence (design.md § Module shape; plan.md every slice).

Checks the approved module ledger against the working tree and its merge-base
with the repository's default branch (discovered via `origin/HEAD`, never
hard-coded):

  R1  KAS wire literals ("turn_start", "notify-", "notify-wf-", "agentInitiated",
      "agentInitiatedReason", "notificationSeverity") appear in PRODUCTION string
      literals only in crates/cyril-core/src/protocol/convert/kas.rs.
  R2  Header / notice text markers ("⚙", "noted mid-turn", "agent follow-up",
      "agent-initiated ·", "workflow step ·") appear in production string literals
      only in crates/cyril-ui/src/turn_labels.rs (vacuous until that module exists).
  R3  Protected parents: production-line delta vs merge-base
      crates/cyril/src/app.rs <= +60, crates/cyril-ui/src/state.rs <= +70.
  R4  crates/cyril-core/src/protocol/convert/mod.rs production text is unchanged.
  R5  No `acp::` / `agent_client_protocol` in crates/cyril-ui/src production code.

"Production" = the file with every `#[cfg(test)]`-gated item removed (a brace-
matched block, or a single `mod x;` declaration), and files under `tests/`,
`examples/`, and `test_support.rs` excluded entirely. Comments are stripped
before literal scans. Exit 0 and `C21 PASS` when every rule holds; otherwise one
`C21 FAIL <path>[:<line>] <rule>: <detail>` line per violation and exit 1.
"""
import re, subprocess, sys
from pathlib import Path

ROOT = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
DEFAULT = subprocess.check_output(["git", "symbolic-ref", "--short", "refs/remotes/origin/HEAD"], cwd=ROOT, text=True).strip()
BASE = subprocess.check_output(["git", "merge-base", "HEAD", DEFAULT], cwd=ROOT, text=True).strip()

KAS_LITERALS = ["turn_start", "notify-", "agentInitiated", "notificationSeverity"]
KAS_OWNER = "crates/cyril-core/src/protocol/convert/kas.rs"
LABEL_MARKERS = ["⚙", "noted mid-turn", "agent follow-up", "agent-initiated ·", "workflow step ·"]
LABEL_OWNER = "crates/cyril-ui/src/turn_labels.rs"
PROTECTED = {"crates/cyril/src/app.rs": 60, "crates/cyril-ui/src/state.rs": 70}
FROZEN = "crates/cyril-core/src/protocol/convert/mod.rs"

def strip_cfg_test(text):
    """Return (production_text, kept_line_numbers) with #[cfg(test)] items removed."""
    lines = text.split("\n")
    keep = [True] * len(lines)
    i = 0
    while i < len(lines):
        if lines[i].strip().startswith("#[cfg(test)]"):
            j = i + 1
            while j < len(lines) and lines[j].strip().startswith("#["):
                j += 1  # stacked attributes
            if j < len(lines) and re.match(r"\s*(pub(\(crate\))?\s+)?mod\s+\w+\s*;", lines[j]):
                for k in range(i, j + 1): keep[k] = False
                i = j + 1; continue
            depth, started, k = 0, False, j
            while k < len(lines):
                code = lines[k].split("//", 1)[0]
                for ch in code:
                    if ch == "{": depth += 1; started = True
                    elif ch == "}": depth -= 1
                keep[k] = False
                if started and depth == 0: break
                if not started and code.rstrip().endswith(";"): break
                k += 1
            for m in range(i, j): keep[m] = False
            i = k + 1; continue
        i += 1
    kept = [(n + 1, l) for n, l in enumerate(lines) if keep[n]]
    return kept

def production_lines(text):
    return [l for _, l in strip_cfg_test(text)]

def string_literals(line):
    code = re.sub(r'//.*$', '', line) if '"' not in line.split("//", 1)[0] else line
    code = code.split("//", 1)[0] if code.count('"') % 2 == 0 and "//" in code and code.index("//") > code.rfind('"') else code
    return re.findall(r'"((?:[^"\\]|\\.)*)"', code)

def is_production_file(rel):
    return (rel.endswith(".rs") and "/src/" in rel and "/tests/" not in rel
            and "/examples/" not in rel and not rel.endswith("test_support.rs"))

def base_text(rel):
    try:
        return subprocess.check_output(["git", "show", f"{BASE}:{rel}"], cwd=ROOT, text=True, stderr=subprocess.DEVNULL)
    except subprocess.CalledProcessError:
        return ""

failures = []
rs_files = [p.relative_to(ROOT).as_posix() for p in (ROOT / "crates").rglob("*.rs")]
for rel in sorted(f for f in rs_files if is_production_file(f)):
    text = (ROOT / rel).read_text(encoding="utf-8")
    for lineno, line in strip_cfg_test(text):
        stripped = line.strip()
        if stripped.startswith("//") or stripped.startswith("///") or stripped.startswith("//!"):
            continue
        lits = string_literals(line)
        if rel != KAS_OWNER:
            for lit in lits:
                for tok in KAS_LITERALS:
                    if tok in lit:
                        failures.append(f"C21 FAIL {rel}:{lineno} R1: KAS literal {tok!r} outside {KAS_OWNER}")
        if rel != LABEL_OWNER:
            for lit in lits:
                for tok in LABEL_MARKERS:
                    if tok in lit:
                        failures.append(f"C21 FAIL {rel}:{lineno} R2: header/notice text {tok!r} outside {LABEL_OWNER}")
        if rel.startswith("crates/cyril-ui/src/") and ("acp::" in line or "agent_client_protocol" in line):
            failures.append(f"C21 FAIL {rel}:{lineno} R5: ACP type in cyril-ui")

for rel, cap in PROTECTED.items():
    now = len(production_lines((ROOT / rel).read_text(encoding="utf-8")))
    base = len(production_lines(base_text(rel)))
    if now - base > cap:
        failures.append(f"C21 FAIL {rel} R3: prod delta {now - base} > {cap} (base {base}, now {now})")

if production_lines((ROOT / FROZEN).read_text(encoding="utf-8")) != production_lines(base_text(FROZEN)):
    failures.append(f"C21 FAIL {FROZEN} R4: production text changed")

for f in failures: print(f)
if failures:
    sys.exit(1)
deltas = {rel: len(production_lines((ROOT / rel).read_text(encoding='utf-8'))) - len(production_lines(base_text(rel))) for rel in PROTECTED}
print(f"C21 PASS (base {BASE[:8]} on {DEFAULT}; protected-parent prod deltas {deltas})")
