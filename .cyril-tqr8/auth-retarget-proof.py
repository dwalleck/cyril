#!/usr/bin/env python3
"""Parent-run Linux diagnostic/mutation proof; never edits the supplied source tree.

Usage: python3 auth-retarget-proof.py SOURCE NEW_PROOF_ROOT NONEMPTY_TARGET_DIR
Run alone against the shared Cargo target. No invocation was run by the preparer.
"""

import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys


AUTH = Path("crates/cyril-core/src/protocol/kas/auth.rs")
TEST = "protocol::kas::auth::tests::fetch_serializes_across_a_symlink_retarget"
JOIN = "        let (first, second) = tokio::join!(before.fetch(false), after.fetch(false));\n"
PATCH_NAME = "auth-retarget-immutable-interpreter.patch"
PATCH_SHA256 = "ccaaab48c4e909666bfcc278fdf9c92308e7aac245d516e5a92587add7f206f4"
PROBE = """        drop(script_writer);
        let busy = [&first, &second].iter().any(|result| {
            matches!(result, Err(KasTokenError::Spawn { source, .. })
                if source.raw_os_error() == Some(26))
        });
        let overlap = [&first, &second].iter().any(|result| {
            matches!(result, Err(KasTokenError::NonZeroExit { status, .. })
                if status == "exit status: 9")
        });
        eprintln!("AUTH_RETARGET_PROOF busy={busy} overlap={overlap} success={}",
            first.is_ok() && second.is_ok());
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def replace_once(text, old, new):
    if text.count(old) != 1:
        raise RuntimeError(f"expected one exact source anchor, found {text.count(old)}: {old[:100]!r}")
    return text.replace(old, new, 1)


def fixture_hunks(patch):
    old, new = [], []
    active = False
    for line in patch.decode().splitlines(keepends=True):
        if line.startswith("@@"):
            if active:
                raise RuntimeError("expected the frozen single-hunk fixture patch")
            active = True
        elif active:
            if line[0] in " -":
                old.append(line[1:])
            if line[0] in " +":
                new.append(line[1:])
    if not old or not new:
        raise RuntimeError("missing fixture hunk")
    return "".join(old), "".join(new)


def held_writer(hunk, path):
    writer = (
        "        let script_writer = std::fs::OpenOptions::new()\n"
        "            .write(true)\n"
        f"            .open({path})\n"
        "            .unwrap();\n"
    )
    return replace_once(hunk, JOIN, writer + JOIN + PROBE)


def frozen_at_construction(text):
    # Faithful mutation: freeze ONLY gate identity at actual source construction;
    # keep the retargeted AgentCommand and dynamic identity assertions unchanged.
    text = replace_once(text, "pub(crate) struct KasTokenSource {\n",
                        "pub(crate) struct KasTokenSource {\n    frozen_lock_key: String,\n")
    text = replace_once(text,
        "    pub(crate) fn new(kiro_cli: AgentCommand, environment: SpawnEnvironment) -> Self {\n        Self {\n",
        "    pub(crate) fn new(kiro_cli: AgentCommand, environment: SpawnEnvironment) -> Self {\n        let mut source = Self {\n            frozen_lock_key: String::new(),\n")
    text = replace_once(text,
        "            fallback_warned: std::sync::atomic::AtomicBool::new(false),\n        }\n",
        "            fallback_warned: std::sync::atomic::AtomicBool::new(false),\n        };\n"
        "        source.frozen_lock_key = source.current_lock_key();\n        source\n")
    return replace_once(text, "        let key = self.current_lock_key();\n",
                        "        let key = self.frozen_lock_key.clone();\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("proof_root", type=Path)
    parser.add_argument("target_dir", type=Path)
    parser.add_argument("--timeout-seconds", type=int, default=900,
                        help="per Cargo invocation; kills its whole process group on expiry")
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("the deterministic OS-code-26 held-writer diagnostic is Linux-only")
    if not str(args.target_dir) or str(args.target_dir) == ".":
        parser.error("supply an explicit nonempty Cargo target directory")
    if args.timeout_seconds <= 0:
        parser.error("timeout must be positive")
    source = args.source.resolve(strict=True)
    root = args.proof_root.absolute()
    target = args.target_dir.absolute()
    if root.exists() or root.is_relative_to(source) or source.is_relative_to(root):
        parser.error("proof root must be new and outside the supplied source tree")
    if target.is_relative_to(root):
        parser.error("shared target must be outside the new proof root")
    patch = Path(__file__).with_name(PATCH_NAME).read_bytes()
    if sha(patch) != PATCH_SHA256:
        raise RuntimeError("fixture patch bytes differ from the frozen patch")
    old_hunk, new_hunk = fixture_hunks(patch)
    supplied = (source / AUTH).read_bytes()
    text = supplied.decode()
    if text.count(new_hunk) == 1:
        repaired = text
    else:
        repaired = replace_once(text, old_hunk, new_hunk)
    original = replace_once(repaired, new_hunk, old_hunk)
    repaired_held = replace_once(repaired, new_hunk, held_writer(new_hunk, "&fake.args()[0]"))
    original_held = replace_once(original, old_hunk,
                                 held_writer(old_hunk, 'dir.path().join("Cellar/v2/kiro-cli")'))
    # Drop the shared guard before the helper, removing effective serialization
    # without creating dead-code/compiler-failure false reds.
    no_gate = replace_once(repaired_held,
        "        let _serialized = gate.lock().await;\n", "        drop(gate.lock().await);\n")
    frozen = frozen_at_construction(repaired_held)
    if not (source / ".cyril-a5wo").is_dir():
        raise RuntimeError("source lacks .cyril-a5wo compile-time fixtures; use a complete source tree")
    root.mkdir(parents=True)
    tree = root / "source"
    # Copy ambient/uncommitted source, not Git HEAD. Keep hidden .cyril-a5wo
    # fixtures and all other source dependencies; omit only root Git/target data.
    shutil.copytree(source, tree, symlinks=True,
                    ignore=lambda directory, names: [".git", "target"] if Path(directory) == source else [])
    (root / "auth-supplied.rs").write_bytes(supplied)
    (root / "auth-repaired.rs").write_bytes(repaired.encode())
    entrypoints = sorted([*tree.glob("crates/*/src/lib.rs"), *tree.glob("crates/*/src/main.rs")])
    for entrypoint in entrypoints:
        entrypoint.touch()
    auth = tree / AUTH
    if auth.is_symlink():
        raise RuntimeError("refusing an auth source symlink that could escape the proof copy")
    env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_INCREMENTAL="0")
    command = ["cargo", "test", "-p", "cyril-core", "--features", "kas", "--lib", TEST,
               "--", "--exact", "--nocapture"]
    cases = [
        ("old-held-writer", original_held, 101, "busy=true overlap=false success=false"),
        ("repaired-held-writer", repaired_held, 0, "busy=false overlap=false success=true"),
        ("remove-shared-gate", no_gate, 101, "busy=false overlap=true success=false"),
        ("restore-after-no-gate", repaired_held, 0, "busy=false overlap=false success=true"),
        ("cached-construction-key", frozen, 101, "busy=false overlap=true success=false"),
        ("restore-after-cached-key", repaired_held, 0, "busy=false overlap=false success=true"),
    ]
    report = {
        "source": str(source), "copy": str(tree), "target": str(target),
        "supplied_sha256": sha(supplied), "original_sha256": sha(original.encode()),
        "repaired_sha256": sha(repaired.encode()), "patch_sha256": sha(patch),
        "a5wo_retained": (tree / ".cyril-a5wo").is_dir(),
        "refreshed_entrypoints": [str(p.relative_to(tree)) for p in entrypoints],
        "command": command, "timeout_seconds": args.timeout_seconds, "cases": [],
    }
    try:
        for name, variant, expected_exit, marker in cases:
            auth.write_bytes(variant.encode())
            (tree / "crates/cyril-core/src/lib.rs").touch()
            (root / f"{name}.patch").write_text("".join(difflib.unified_diff(
                repaired.splitlines(keepends=True), variant.splitlines(keepends=True),
                fromfile=f"a/{AUTH}", tofile=f"b/{AUTH}")))
            log_path = root / f"{name}.log"
            with log_path.open("wb") as log:
                process = subprocess.Popen(command, cwd=tree, env=env, stdout=log,
                                           stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    code = process.wait(timeout=args.timeout_seconds)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
                    raise RuntimeError(f"{name}: bounded run expired; see {log_path}")
            receipt = {
                "name": name, "source_sha256": sha(variant.encode()),
                "returncode": code, "log": str(log_path),
                "expected_marker": f"AUTH_RETARGET_PROOF {marker}",
            }
            log_text = log_path.read_text(errors="replace")
            receipt["pass"] = code == expected_exit and receipt["expected_marker"] in log_text
            report["cases"].append(receipt)
            (root / "receipt.json").write_text(json.dumps(report, indent=2) + "\n")
            print(json.dumps(receipt), flush=True)
            if not receipt["pass"]:
                raise RuntimeError(f"{name}: did not observe the required result; see {log_path}")
    finally:
        auth.write_bytes(repaired.encode())
        (tree / "crates/cyril-core/src/lib.rs").touch()
        report["restored_sha256"] = sha(auth.read_bytes())
        report["supplied_source_unchanged"] = (source / AUTH).read_bytes() == supplied
        (root / "receipt.json").write_text(json.dumps(report, indent=2) + "\n")
    if report["restored_sha256"] != report["repaired_sha256"] or not report["supplied_source_unchanged"]:
        raise RuntimeError("byte-exact source restoration/preservation failed")


if __name__ == "__main__":
    main()
