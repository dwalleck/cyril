#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Offline, cross-platform self-test for the code-review-max tooling. No KAS, no model.

    uv run --script experiments/code-review-workflow/selftest_crtool.py
    python3 experiments/code-review-workflow/selftest_crtool.py

1. The recipe regenerates from build_recipe.py at the node cap, and every crtool
   command line in it is double-quoted (single quotes mean nothing to cmd.exe and
   a bare `a,b,c` becomes an array in PowerShell).
2. Those exact command lines, with the recipe's templates filled in, run through
   this platform's shell - PowerShell on Windows, as KAS uses there; bash
   elsewhere - against a throwaway git repository.
3. Every crtool subcommand runs end to end on synthetic finder, verifier, ballot,
   ranking and comment files, and the outputs are checked.

Exit status 0 on success; the first failed check raises.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

for _stream in (sys.stdout, sys.stderr):  # Windows legacy code pages
    if hasattr(_stream, "reconfigure"):
        _stream.reconfigure(encoding="utf-8", errors="replace")

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
CRTOOL = os.path.join(REPO, ".kiro", "code-review", "crtool.py")
RECIPE = os.path.join(REPO, ".kiro", "workflows", "code-review-max.workflow.json")
IS_WINDOWS = os.name == "nt"
ANGLES = ["a-line-scan", "b-removed-behavior", "c-cross-file", "d-language-pitfalls", "e-wrapper-proxy",
          "cleanup", "altitude", "conventions"]
GIT_ENV = dict(os.environ, GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.com",
               GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@example.com")


def step(msg):
    print(f"-- {msg}", flush=True)


def check(cond, msg):
    if not cond:
        raise SystemExit(f"FAIL: {msg}")
    print(f"   ok  {msg}")


def run(argv, cwd, **kw):
    r = subprocess.run(argv, cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace",
                       env=GIT_ENV, **kw)
    if r.returncode != 0:
        raise SystemExit(f"FAIL: {argv if isinstance(argv, str) else ' '.join(argv)} -> exit {r.returncode}\n"
                         f"{r.stdout}\n{r.stderr}")
    return r.stdout


def crtool(ws, *args):
    return run([sys.executable, CRTOOL, *args], cwd=ws)


def shell_argv(command):
    """The shell KAS would use: PowerShell on Windows, bash (or sh) elsewhere."""
    if IS_WINDOWS:
        exe = shutil.which("pwsh") or shutil.which("powershell")
        check(exe is not None, "a PowerShell is on PATH")
        return [exe, "-NoProfile", "-NonInteractive", "-Command", command]
    return [shutil.which("bash") or "/bin/sh", "-c", command]


def write_json(path, obj):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f)


def recipe_commands():
    step("recipe regenerates at the node cap")
    out = run([sys.executable, os.path.join(HERE, "build_recipe.py"), "--out",
               os.path.join(TMP, "regen.workflow.json")], cwd=REPO)
    check("20/20 step nodes" in out, f"build_recipe reports 20/20 ({out.strip()})")
    with open(os.path.join(TMP, "regen.workflow.json"), encoding="utf-8") as f:
        regen = json.load(f)
    with open(RECIPE, encoding="utf-8") as f:
        check(json.load(f) == regen, "the committed recipe matches its generator")
    lines = {}

    def walk(nodes):
        for n in nodes:
            for line in (n.get("prompt") or "").splitlines():
                m = re.search(r"\{\{crtool\}\} .*", line)
                if m:
                    lines.setdefault(n["id"], []).append(m.group(0))
            walk(n.get("steps", []))
            walk(n.get("branches", []))
    walk(regen["steps"])
    check(set(lines) >= {"setup", "dedup", "ballots", "rank", "comment"}, f"crtool is called from {sorted(lines)}")
    for node, line in ((n, x) for n, xs in lines.items() for x in xs):
        check("'" not in line, f"[{node}] no single quotes: {line[:90]}")
        check(re.search(r"\s[A-Za-z0-9_-]+,[A-Za-z0-9_-]+", line) is None,
              f"[{node}] no bare comma list (PowerShell would make it an array)")
    return lines


def main():
    ws = os.path.join(TMP, "ws")
    os.makedirs(os.path.join(ws, "src"))
    step("throwaway git repository with one change")
    run(["git", "init", "-q", "-b", "main"], cwd=ws)
    with open(os.path.join(ws, "src", "lib.rs"), "w", encoding="utf-8", newline="\n") as f:
        f.write("pub fn old_helper() -> u32 {\n    1\n}\n")
    run(["git", "add", "-A"], cwd=ws)
    run(["git", "commit", "-q", "-m", "base"], cwd=ws)
    with open(os.path.join(ws, "src", "lib.rs"), "w", encoding="utf-8", newline="\n") as f:
        f.write("pub fn old_helper() -> u32 {\n    2\n}\n\npub fn new_helper(x: u32) -> u32 {\n    x - 1\n}\n\n"
                "pub fn caller() -> u32 {\n    new_helper(old_helper())\n}\n")
    with open(os.path.join(ws, "NOTES.md"), "w", encoding="utf-8") as f:
        f.write("design note — intent of the change\n")
    run(["git", "add", "-A"], cwd=ws)
    run(["git", "commit", "-q", "-m", "change"], cwd=ws)

    lines = recipe_commands()
    # A run directory with a space in it: the double quotes have to carry it.
    run_dir = os.path.join(ws, ".code-review", "self test").replace("\\", "/")
    fill = {"{{crtool}}": f'"{sys.executable}" "{CRTOOL}"' if not IS_WINDOWS
            else f'& "{sys.executable}" "{CRTOOL}"',
            "{{rundir}}": run_dir, "{{target}}": "HEAD~1...HEAD", "{{scope}}": "."}

    def through_shell(node, which=0):
        cmd = lines[node][which]
        for k, v in fill.items():
            cmd = cmd.replace(k, v)
        return run(shell_argv(cmd), cwd=ws)

    step(f"the recipe's own gather line, through {'PowerShell' if IS_WINDOWS else 'bash'}")
    out = through_shell("setup")
    check("gathered 1 files" in out or "gathered 2 files" in out, f"gather ran ({out.strip().splitlines()[-1]})")
    with open(os.path.join(run_dir, "manifest.json"), encoding="utf-8") as f:
        m = json.load(f)
    check(m["total_files"] == 2 and m["target"] == "HEAD~1...HEAD", "manifest: 2 files, target kept verbatim")
    check(any(d["path"] == "NOTES.md" for d in m["change_docs"]), "the changed doc is listed as a change_doc")
    with open(os.path.join(run_dir, "facts", "symbols.json"), encoding="utf-8") as f:
        syms = {s["name"]: s for s in json.load(f)}
    check("new_helper" in syms and syms["new_helper"]["status"] == "added", "facts: new_helper detected as added")
    check(any(u["file"] == "src/lib.rs" for u in syms["new_helper"]["usages"]), "facts: its caller is mapped")
    check(crtool(ws, "gather", run_dir, "HEAD~1...HEAD", ".").startswith("already gathered"), "gather is idempotent")

    step("finder output, then the recipe's own merge line (the comma list)")
    write_json(os.path.join(run_dir, "candidates", "a-line-scan.json"), {"angle": "a-line-scan", "candidates": [
        {"file": "src/lib.rs", "line": 6, "summary": "x - 1 underflows at 0", "failure_scenario": "x=0 -> panic",
         "category": "correctness", "evidence": "x - 1"}]})
    write_json(os.path.join(run_dir, "candidates", "d-language-pitfalls.json"), {"angle": "d-language-pitfalls",
        "candidates": [{"file": "src/lib.rs", "line": 6, "summary": "u32 underflow in new_helper",
                        "failure_scenario": "x=0 underflows", "category": "correctness"}]})
    write_json(os.path.join(run_dir, "candidates", "conventions.json"), {"angle": "conventions", "candidates": [
        {"file": "src/lib.rs", "line": 1, "summary": "rule broken", "failure_scenario": "n/a",
         "category": "conventions"}]})
    for a in ANGLES:
        if not os.path.exists(os.path.join(run_dir, "candidates", f"{a}.json")):
            write_json(os.path.join(run_dir, "candidates", f"{a}.json"), {"angle": a, "candidates": []})
    out = through_shell("dedup")
    check("from 8/8 angles" in out, "merge saw all 8 angles through the shell's argument parsing")
    with open(os.path.join(run_dir, "candidates", "digest-1.txt"), encoding="utf-8") as f:
        check("SAME location" in f.read(), "the digest flags the two candidates at src/lib.rs:6")

    step("dedup decision, shard, verdicts")
    write_json(os.path.join(run_dir, "deduped", "decisions.json"),
               {"groups": [{"pids": ["a-line-scan-1", "d-language-pitfalls-1"], "reason": "same underflow"}]})
    crtool(ws, "shard", run_dir, "--shards", "2")
    with open(os.path.join(run_dir, "deduped", "index.json"), encoding="utf-8") as f:
        idx = json.load(f)
    check(idx["deduped_count"] == 2, "3 candidates -> 2 after grouping")
    ids = {c["angle"]: c["id"] for c in idx["candidates"]}
    for q in ("queue-1.json", "queue-2.json"):
        with open(os.path.join(run_dir, "queues", q), encoding="utf-8") as f:
            vd = json.load(f)["verdict_dir"]
        check("\\" not in vd and os.path.isabs(vd), f"{q}: verdict_dir is absolute with forward slashes")
    bug, conv = ids["a-line-scan"], ids["conventions"]
    write_json(os.path.join(run_dir, "verdicts", "q1", f"{bug}.json"), {"id": bug, "verdict": "CONFIRMED",
                                                                        "reasoning": "x=0"})
    write_json(os.path.join(run_dir, "verdicts", "q2", f"{conv}.json"), {"id": conv, "verdict": "REFUTED",
                                                                         "reasoning": "not a rule"})
    write_json(os.path.join(run_dir, "candidates", "sweep.json"), {"angle": "sweep", "candidates": []})

    step("the recipe's own ballots line")
    out = through_shell("ballots")
    check("1 of 2 candidates get two more votes" in out, "the refuted conventions claim is balloted")
    for loop, tag, verdict in (("r1", "v2", "CONFIRMED"), ("r2", "v3", "CONFIRMED")):
        write_json(os.path.join(run_dir, "verdicts", loop, f"{conv}.{tag}.json"),
                   {"id": f"{conv}.{tag}", "verdict": verdict, "reasoning": "it is a rule"})

    step("collate (2-of-3 tally), finalize, comments")
    out = through_shell("rank")
    check("votes changed the outcome" in out, "REFUTED, CONFIRMED, CONFIRMED overturns to CONFIRMED")
    write_json(os.path.join(run_dir, "ranking.json"), {"order": [bug, conv]})
    crtool(ws, "finalize", run_dir)
    write_json(os.path.join(run_dir, "comments", f"{bug}.json"), {"label": "issue", "decorations": ["blocking"],
               "subject": "`new_helper(0)` underflows", "discussion": "Use `x.saturating_sub(1)` or check for 0."})
    out = through_shell("comment")
    check("1 model-written, 1 template" in out, "one model comment kept, the missing one templated")
    with open(os.path.join(run_dir, "comments.json"), encoding="utf-8") as f:
        comments = json.load(f)
    check(comments[0]["body"].startswith("**issue (blocking):**"), "Conventional Comments body rendered")
    with open(os.path.join(run_dir, "report.md"), encoding="utf-8") as f:
        check("# Review comments" in f.read(), "report.md carries the comments section")

    step("check_run on the finished run directory")
    # The synthetic queues were never marked done, so check_run exits 1 on those checks;
    # what matters here is that it reads a run directory on this platform and checks it.
    r = subprocess.run([sys.executable, os.path.join(HERE, "check_run.py"), run_dir], cwd=ws, capture_output=True,
                       text=True, encoding="utf-8", errors="replace")
    check("PASS  one comment entry per reported finding" in r.stdout, "check_run reads and checks the run")
    print("\nALL SELF-TESTS PASS")


if __name__ == "__main__":
    TMP = tempfile.mkdtemp(prefix="crtool-selftest-")
    try:
        main()
    finally:
        shutil.rmtree(TMP, ignore_errors=True)
