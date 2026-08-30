#!/usr/bin/env python3
"""Validate the claim-local mutation ledger and checkpoint evidence."""

from __future__ import annotations

import json
import pathlib
import sys
from collections import Counter

ROOT = pathlib.Path(__file__).resolve().parents[2]
LEDGER = pathlib.Path(__file__).with_name("mutations.json")
EXPECTED_CLAIMS = {f"C{index}" for index in range(1, 14)}


def load_json(path: pathlib.Path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"C12 cannot read {path.relative_to(ROOT)}: {error}") from error


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
    errors = validate()
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("C12 mutation ledger PASS: 13 claims, unique exact commands, checkpointed red/green evidence")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
