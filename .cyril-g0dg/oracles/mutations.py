#!/usr/bin/env python3
"""Validate the claim-local mutation ledger and checkpoint evidence.

A bare run performs the full static validation: ledger structure, checkpoint
cross-references, expected_failure locality (design C12: a generic entry names
no cell/byte/route and is rejected as non-local), and fence existence (a
renamed contract test would make `cargo test <old_name> -- --exact` run zero
tests and exit 0, silently voiding the red/green evidence chain).

`--verify-manifest` additionally EXECUTES every ledger fence command on the
current tree and requires each to actually run at least one passing test.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import shlex
import subprocess
import sys
from collections import Counter

ROOT = pathlib.Path(__file__).resolve().parents[2]
LEDGER = pathlib.Path(__file__).with_name("mutations.json")
EXPECTED_CLAIMS = {f"C{index}" for index in range(1, 14)}

# Words that carry no localization on their own. An expected_failure whose
# every word (beyond its own claim id) sits in this set names no cell, byte,
# event, or route — exactly the "generic substring" the design's named C12
# mutation must reject. Heuristic by construction: a specific locator token
# (a path, a cell name, a count, an identifier) is anything outside this set.
GENERIC_WORDS = frozenset(
    {
        "a", "an", "and", "the", "this", "that", "it", "in", "on", "of",
        "test", "tests", "suite", "fail", "fails", "failed", "failure",
        "with", "error", "errors", "assertion", "assert", "asserts",
        "panic", "panics", "panicked", "goes", "went", "is", "was", "must",
        "should", "broken", "wrong", "bad", "red", "green", "output",
        "exit", "nonzero", "non-zero", "status", "generic", "some", "any",
    }
)

CARGO_EXACT = re.compile(r"cargo test\s+(?:-p\s+\S+\s+)?([A-Za-z0-9_:]+)\s+--\s+--exact")
PASSED_COUNT = re.compile(r"(\d+) passed")


def load_json(path: pathlib.Path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"C12 cannot read {path.relative_to(ROOT)}: {error}") from error


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--verify-manifest",
        action="store_true",
        help="also execute every ledger fence command and require it to run "
        "at least one passing test on the current tree",
    )
    return parser.parse_args()


def is_generic(expected_failure: str, claim: str) -> bool:
    words = re.findall(r"[a-z0-9_.:'-]+", expected_failure.lower())
    meaningful = [
        word for word in words if word != claim.lower() and word not in GENERIC_WORDS
    ]
    return not meaningful


def rust_sources() -> list[str]:
    return [
        path.read_text(encoding="utf-8")
        for path in sorted((ROOT / "crates").rglob("*.rs"))
    ]


def fence_exists_errors(entries: list[dict], sources: list[str]) -> list[str]:
    errors: list[str] = []
    for entry in entries:
        mutation_id = entry.get("id")
        command = entry.get("command")
        if not isinstance(command, str):
            continue
        match = CARGO_EXACT.search(command)
        if match is not None:
            fn_name = match.group(1).rsplit("::", 1)[-1]
            if not any(f"fn {fn_name}(" in source for source in sources):
                errors.append(
                    f"C12 fence test not found in tree (renamed or removed?): "
                    f"{mutation_id} -> {fn_name}"
                )
        elif "cargo test" in command:
            errors.append(
                f"C12 cargo mutation command is not exact: {mutation_id}"
            )
        elif command.startswith("python3 "):
            script = command.split()[1]
            if not (ROOT / script).is_file():
                errors.append(
                    f"C12 fence oracle script missing: {mutation_id} -> {script}"
                )
    return errors


def run_manifest(entries: list[dict]) -> list[str]:
    """Execute each unique fence command; every one must actually run green.

    A cargo fence must report >=1 passed (an --exact filter that matches no
    test 'passes' with zero tests — the exact hole this closes). Commands
    invoking this script itself are skipped (recursion, and its evidence is
    this very run).
    """
    errors: list[str] = []
    for command in sorted({e["command"] for e in entries if isinstance(e.get("command"), str)}):
        if "mutations.py" in command:
            continue
        result = subprocess.run(
            shlex.split(command),
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            errors.append(f"C12 fence command failed on the current tree: {command}")
            continue
        if "cargo test" in command:
            passed = sum(int(count) for count in PASSED_COUNT.findall(result.stdout))
            if passed < 1:
                errors.append(
                    f"C12 fence command ran zero tests (stale --exact path?): {command}"
                )
    return errors


def validate() -> list[str]:
    errors: list[str] = []
    try:
        entries = load_json(LEDGER)
    except ValueError as error:
        return [str(error)]
    if not isinstance(entries, list):
        return ["C12 mutation ledger must be a JSON array"]

    ids: list[str] = []
    claims: list[str] = []
    checkpoints: dict[str, list[dict]] = {}
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            errors.append(f"C12 mutation row {index} is not an object")
            continue
        mutation_id = entry.get("id")
        claim = entry.get("claim")
        expected_failure = entry.get("expected_failure")
        command = entry.get("command")
        target = entry.get("target")
        checkpoint = entry.get("checkpoint")
        if not isinstance(mutation_id, str) or not mutation_id:
            errors.append(f"C12 mutation row {index} missing id")
        else:
            ids.append(mutation_id)
        if not isinstance(claim, str) or claim not in EXPECTED_CLAIMS:
            errors.append(f"C12 mutation {mutation_id!r} has invalid claim {claim!r}")
        else:
            claims.append(claim)
        if not isinstance(expected_failure, str) or not expected_failure.strip():
            errors.append(f"C12 mutation missing expected_failure: {mutation_id}")
        elif isinstance(claim, str) and is_generic(expected_failure, claim):
            errors.append(
                f"C12 expected_failure is generic — it names no cell, byte, "
                f"event, or route: {mutation_id} -> {expected_failure!r}"
            )
        if not isinstance(command, str) or not command.strip():
            errors.append(f"C12 mutation missing command: {mutation_id}")
        elif "cargo test" in command and "--exact" not in command:
            errors.append(f"C12 cargo mutation command is not exact: {mutation_id}")
        if not isinstance(target, str) or not target.strip():
            errors.append(f"C12 mutation missing target: {mutation_id}")
        elif claim != "C11" and not (ROOT / target).exists():
            errors.append(f"C12 mutation target does not exist: {mutation_id} -> {target}")
        if entry.get("observed") is not True:
            errors.append(f"C12 mutation not observed red: {mutation_id}")
        if not isinstance(checkpoint, str) or checkpoint not in {"S1", "S2", "S3", "S4"}:
            errors.append(f"C12 mutation has invalid checkpoint: {mutation_id}")
        elif isinstance(claim, str):
            checkpoints.setdefault(checkpoint, []).append(entry)

    duplicates = sorted(value for value, count in Counter(ids).items() if count > 1)
    errors.extend(f"C12 duplicate mutation id: {value}" for value in duplicates)
    missing_claims = sorted(EXPECTED_CLAIMS - set(claims), key=lambda value: int(value[1:]))
    errors.extend(f"C12 claim has no named mutation: {claim}" for claim in missing_claims)

    errors.extend(
        fence_exists_errors(
            [entry for entry in entries if isinstance(entry, dict)], rust_sources()
        )
    )

    for checkpoint, checkpoint_entries in sorted(checkpoints.items()):
        checkpoint_path = ROOT / ".cyril-g0dg" / "checkpoints" / f"{checkpoint}.json"
        if not checkpoint_path.is_file():
            errors.append(f"C12 checkpoint missing for mutation evidence: {checkpoint}")
            continue
        try:
            checkpoint_data = load_json(checkpoint_path)
        except ValueError as error:
            errors.append(str(error))
            continue
        if checkpoint_data.get("status") != "pass":
            errors.append(f"C12 checkpoint is not pass: {checkpoint}")
        recorded = {
            item.get("claim")
            for item in checkpoint_data.get("mutations", [])
            if isinstance(item, dict) and item.get("result") == "PASS"
        }
        for entry in checkpoint_entries:
            if entry["claim"] not in recorded:
                errors.append(
                    f"C12 checkpoint {checkpoint} lacks PASS mutation for {entry['claim']}"
                )

    return errors


def main() -> int:
    args = parse_args()
    errors = validate()
    if not errors and args.verify_manifest:
        entries = load_json(LEDGER)
        errors = run_manifest(entries)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    mode = "manifest verified live" if args.verify_manifest else "static"
    print(
        f"C12 mutation ledger PASS ({mode}): 13 claims, unique exact commands, "
        "local expected failures, extant fences, checkpointed red/green evidence"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
