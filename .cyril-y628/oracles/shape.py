#!/usr/bin/env python3
"""C7: standalone ownership census for the approved rejection-feedback ledger."""
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]


def git(*args):
    return subprocess.check_output(["git", "-C", str(ROOT), *args], text=True).strip()


def production(text):
    # Probe declarations at the start of convert/mod.rs are not its test body.
    match = re.search(r"(?m)^#\[cfg\(test\)\]\n(?:pub\([^\n]*\) )?mod [^{;]+\{", text)
    return text[:match.start()] if match else text


def source(path):
    return production((ROOT / path).read_text())


def functions(text):
    return set(re.findall(r"\bfn\s+(\w+)\s*[(<]", text))


def fail(path, rule):
    failures.append(f"C7 FAIL {path}: {rule}")


failures = []
upstream = git("symbolic-ref", "refs/remotes/origin/HEAD")
base = git("merge-base", "HEAD", upstream)
allowed = {
    "crates/cyril-core/src/types/event.rs",
    "crates/cyril-core/src/protocol/convert/mod.rs",
    "crates/cyril-core/src/protocol/convert/kas.rs",
    "crates/cyril-core/src/protocol/domain_mediator/mod.rs",
    "crates/cyril-ui/src/feedback_editor.rs",
    "crates/cyril-ui/src/lib.rs",
    "crates/cyril-ui/src/traits.rs",
    "crates/cyril-ui/src/state.rs",
    "crates/cyril-ui/src/widgets/input.rs",
    "crates/cyril-ui/src/widgets/approval.rs",
    "crates/cyril/src/app.rs",
}
fixture_only = {
    "crates/cyril-ui/src/floor_tests.rs",
    "crates/cyril-ui/src/render.rs",
    "crates/cyril-ui/src/widgets/toolbar.rs",
    "crates/cyril-core/src/protocol/convert/probe_qo13.rs",
}
changed = set(git("diff", "--name-only", base).splitlines())
changed.update(git("ls-files", "--others", "--exclude-standard").splitlines())
for path in sorted(changed):
    if not path.startswith("crates/") or not path.endswith(".rs"):
        continue
    if "/tests/" in path or path in allowed or "/examples/" in path:
        continue
    # These entire files are loaded only by #[cfg(test)] parent declarations.
    if path in {"crates/cyril-ui/src/floor_tests.rs", "crates/cyril-core/src/protocol/convert/probe_qo13.rs"}:
        continue
    try:
        before = production(git("show", f"{base}:{path}"))
    except subprocess.CalledProcessError:
        before = ""
    if path in fixture_only and before.strip() == source(path).strip():
        continue
    fail(path, "production change outside approved owner ledger")

for path in sorted(allowed):
    if not (ROOT / path).is_file():
        fail(path, "required module missing")
        continue
    text = source(path)
    if "cyril-ui/" in path and re.search(r"agent_client_protocol|\bacp::|cyril_core::protocol", text):
        fail(path, "SDK/protocol dependency crosses UI seam")
    if path.endswith("convert/mod.rs") and '"rejectionReason"' in text:
        fail(path, "KAS key belongs in convert/kas.rs")

editor = "crates/cyril-ui/src/feedback_editor.rs"
traits = "crates/cyril-ui/src/traits.rs"
lib = "crates/cyril-ui/src/lib.rs"
if (ROOT / editor).exists():
    text = source(editor)
    for forbidden in ["PermissionResponse", "oneshot", "serde_json", "BridgeCommand"]:
        if forbidden in text:
            fail(editor, f"editor must not own {forbidden}")
    if not re.search(r"pub struct RejectionFeedback\s*\{", text):
        fail(editor, "opaque public RejectionFeedback value missing")
    fields = re.search(r"pub struct RejectionFeedback\s*\{([^}]+)\}", text, re.S)
    if fields and re.search(r"(?m)^\s*pub(?:\([^)]*\))?\s+\w+\s*:", fields[1]):
        fail(editor, "editor state fields must be private")
if not re.search(r"(?m)^mod feedback_editor;", source(lib)):
    fail(lib, "editor module must be private")
if "EnterReason" not in source(traits) or "RejectionFeedback" not in source(traits):
    fail(traits, "phase-owned public feedback value missing")

parents = {
    "crates/cyril/src/app.rs": (35, False),
    "crates/cyril-core/src/protocol/domain_mediator/mod.rs": (12, False),
    "crates/cyril-core/src/protocol/convert/mod.rs": (40, False),
    "crates/cyril-ui/src/state.rs": (130, True),
}
for path, (limit, approval_methods) in parents.items():
    old = production(git("show", f"{base}:{path}"))
    new = source(path)
    added = functions(new) - functions(old)
    # Approved generic conversion may factor its shared exact-ID warning and
    # feature-gated dispatch; neither helper owns KAS encoding.
    shared_conversion = {"warn_if_foreign_permission_option", "attach_rejection_metadata"} if path.endswith("convert/mod.rs") else set()
    illegal = {name for name in added if name not in shared_conversion and not (approval_methods and name.startswith("approval_"))}
    if illegal:
        fail(path, f"new responsibility functions: {sorted(illegal)}")
    delta = len(new.splitlines()) - len(old.splitlines())
    print(f"C7 census {path}: delta={delta}, projection={limit}")
    if delta > limit:
        fail(path, f"growth tripwire {delta} > {limit}; inspect placement and update owning plan")
    if path.endswith("state.rs"):
        for field in ["approval_feedback", "feedback_editor"]:
            pattern = rf"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?{field}\s*:"
            if re.search(pattern, new) and not re.search(pattern, old):
                fail(path, f"parallel editor state {field}; editor must live in phase")

print(f"C7 baseline {base}, upstream {upstream}")
if failures:
    print("\n".join(failures), file=sys.stderr)
    sys.exit(1)
print("C7 PASS approved ownership and protected-parent census")
