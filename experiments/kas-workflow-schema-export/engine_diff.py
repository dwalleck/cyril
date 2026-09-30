#!/usr/bin/env python3
"""Differential check: the generated schema's verdicts vs the LIVE engine's.

Each engine-cases/<name>.workflow.json is registered with `_kiro/workflow/new`
(never invoked: zero credits) through run_review.py --validate-only --recipe, one
case at a time. Never run cases in parallel: concurrent kiro-cli token renewals
race on the single-use refresh token and log the user out.

The expected engine verdict is in the file name: -PASS- = accepted, -FAIL- =
rejected, -SCHEMA-PASS- = the schema deliberately tolerates it but the engine
rejects it at run creation (registry lookups: watch handler, agent names).
Prints each rejection's engine message so a reviewer can confirm it failed for
the intended reason, not an unrelated one.
"""
import pathlib, re, subprocess, sys

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent.parent
RUNNER = REPO / "experiments" / "code-review-workflow" / "run_review.py"

bad = 0
for case in sorted((HERE / "engine-cases").glob("*.workflow.json")):
    name = case.name.removesuffix(".workflow.json")
    expect = "ACCEPT" if "-PASS-" in name and "-SCHEMA-PASS-" not in name else "REJECT"
    out = subprocess.run(
        [sys.executable, str(RUNNER), "--workspace", str(REPO), "--validate-only", "--recipe", str(case)],
        capture_output=True, text=True, timeout=150,
    ).stdout
    got = "ACCEPT" if "ENGINE ACCEPTED" in out else "REJECT" if "ENGINE REJECTED" in out else "NO-VERDICT"
    detail = re.search(r'"details":\s*"(.*?)(?<!\\)"', out, re.S)
    reason = re.sub(r"(\\n|\s)+", " ", detail.group(1))[:150] if detail else ""
    ok = got == expect
    bad += not ok
    print(f"{'OK      ' if ok else 'MISMATCH'} {name:36} {got:10} {reason}")
print("ALL AGREE" if bad == 0 else f"{bad} DISAGREEMENTS")
sys.exit(1 if bad else 0)
