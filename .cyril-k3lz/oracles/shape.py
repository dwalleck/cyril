"""cyril-k3lz module-shape fence (design C13). Issue-local, non-production.

Run from the repo root:  python .cyril-k3lz/oracles/shape.py
Exit 0 = PASS; exit 1 prints every violated rule as `C13/<rule>: <path>: <detail>`.

S1  crates/cyril/src/app.rs has zero delta against the merge base (protected parent).
S2  thinking derivation lives only in types/thinking.rs: the production sections of
    session.rs, state.rs and builtin.rs contain no `"thinking"` config-key comparison,
    no `ReasoningSupport::` match and no `thinkingEnabled` literal.
S3  no engine-kind matching in thinking code: types/thinking.rs and the
    ThinkingCommand block contain no `AgentEngine` / `.kind()`.
S4  the `thinkingEnabled` wire key appears in production code only under protocol/.
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
violations = []


def git(*args):
    return subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout.strip()


def merge_base():
    # Branch discovery per the workflow contract: the upstream default branch.
    try:
        default = git("symbolic-ref", "--short", "refs/remotes/origin/HEAD")
    except subprocess.CalledProcessError:
        default = "main"
    return git("merge-base", "HEAD", default)


def production(path):
    """Source text before the first `#[cfg(test)]` test module."""
    text = (ROOT / path).read_text(encoding="utf-8")
    match = re.search(r"^#\[cfg\(test\)\]\s*\n(\s*#!?\[[^\n]*\]\s*\n)*\s*mod tests", text, re.M)
    return text[: match.start()] if match else text


def fail(rule, path, detail):
    violations.append(f"C13/{rule}: {path}: {detail}")


# S1
base = merge_base()
app_delta = git("diff", base, "--", "crates/cyril/src/app.rs")
if app_delta:
    fail("S1", "crates/cyril/src/app.rs", f"{len(app_delta.splitlines())} diff lines vs {base[:10]}")

# S2
for path in (
    "crates/cyril-core/src/session.rs",
    "crates/cyril-ui/src/state.rs",
    "crates/cyril-core/src/commands/builtin.rs",
):
    src = production(path)
    for pattern, what in (
        (r'==\s*"thinking"|"thinking"\s*==|key\s*==\s*THINKING', "thinking config-key comparison"),
        (r"ReasoningSupport::", "reasoning-support match"),
        (r'"thinkingEnabled"', "thinkingEnabled wire key"),
    ):
        for m in re.finditer(pattern, src):
            line = src.count("\n", 0, m.start()) + 1
            fail("S2", path, f"line {line}: {what}")

# S3
thinking = ROOT / "crates/cyril-core/src/types/thinking.rs"
if thinking.exists():
    src = production("crates/cyril-core/src/types/thinking.rs")
    for m in re.finditer(r"AgentEngine|\.kind\(\)", src):
        fail("S3", "crates/cyril-core/src/types/thinking.rs", f"line {src.count(chr(10), 0, m.start()) + 1}")
builtin = production("crates/cyril-core/src/commands/builtin.rs")
block = re.search(r"pub struct ThinkingCommand.*?(?=\n/// |\npub struct |\Z)", builtin, re.S)
if block and re.search(r"AgentEngine|\.kind\(\)", block.group(0)):
    fail("S3", "crates/cyril-core/src/commands/builtin.rs", "ThinkingCommand matches engine kind")

# S4
for path in sorted((ROOT / "crates").rglob("*.rs")):
    rel = path.relative_to(ROOT).as_posix()
    if "/tests/" in rel or "/examples/" in rel or "/protocol/" in rel:
        continue
    src = production(rel)
    if '"thinkingEnabled"' in src:
        fail("S4", rel, "thinkingEnabled wire key outside protocol/")

# S5 (added after the final isolated review found an unfenced edge): cyril-ui
# takes types, not command-layer internals, from cyril-core.
for path in sorted((ROOT / "crates/cyril-ui/src").rglob("*.rs")):
    rel = path.relative_to(ROOT).as_posix()
    src = production(rel)
    for m in re.finditer(r"cyril_core::commands::builtin", src):
        fail("S5", rel, f"line {src.count(chr(10), 0, m.start()) + 1}: imports command-layer internals")

if violations:
    print("\n".join(violations))
    sys.exit(1)
print(f"C13 PASS (base {base[:10]})")
