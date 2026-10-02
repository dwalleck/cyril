#!/usr/bin/env python3
"""Capture the Python crtool's outputs for every golden case.

Run from anywhere:  python3 crates/cyril-review/tests/goldens/capture.py

For each cases/<name>/case.json this builds the fixture repository, runs the
case's steps through .kiro/code-review/crtool.py, and replaces
cases/<name>/expected/ with the run directory plus steps.json (each step's
exit code and stdout). tests/goldens.rs builds the same repository and checks
the native crtool against these files under docs/crtool-contract.md.
Recapture only when crtool.py's behaviour changes on purpose.
"""
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile

HERE = pathlib.Path(__file__).resolve().parent
CRTOOL = HERE.parents[3] / ".kiro" / "code-review" / "crtool.py"


def git_env(home):
    config = home / "gitconfig"
    config.write_text("")
    env = dict(os.environ)
    env.update({
        "GIT_CONFIG_GLOBAL": str(config), "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_AUTHOR_NAME": "Golden", "GIT_AUTHOR_EMAIL": "golden@example.invalid",
        "GIT_COMMITTER_NAME": "Golden", "GIT_COMMITTER_EMAIL": "golden@example.invalid",
    })
    return env


def git(repo, env, *args):
    subprocess.run(["git", *args], cwd=repo, env=env, check=True, capture_output=True)


def apply(repo, change):
    for path, text in change.get("files", {}).items():
        target = repo / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(text.encode("utf-8"))
    for path, data in change.get("binary", {}).items():
        target = repo / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(bytes(data))
    for path in change.get("delete", []):
        (repo / path).unlink()


def build(repo, env, case):
    """Must stay in step with build_repo() in tests/goldens.rs."""
    repo.mkdir()
    git(repo, env, "init", "-q", "-b", "main")
    git(repo, env, "config", "core.autocrlf", "false")
    for number, commit in enumerate(case["commits"], 1):
        if "branch" in commit:
            git(repo, env, "checkout", "-q", "-b", commit["branch"])
        apply(repo, commit)
        git(repo, env, "add", "-A")
        date = f"2026-01-{number:02d}T00:00:00Z"
        commit_env = dict(env, GIT_AUTHOR_DATE=date, GIT_COMMITTER_DATE=date)
        subprocess.run(["git", "commit", "-q", "--no-verify", "-m", commit.get("message", f"commit {number}")],
                       cwd=repo, env=commit_env, check=True, capture_output=True)
    apply(repo, case.get("worktree", {}))


def capture(case_dir):
    case = json.loads((case_dir / "case.json").read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory() as tmp:
        tmp = pathlib.Path(tmp)
        env = git_env(tmp)
        repo, run = tmp / "repo", tmp / "run"
        build(repo, env, case)
        steps = []
        for step in case["steps"]:
            argv = [sys.executable, str(CRTOOL), step[0], str(run), *step[1:]]
            result = subprocess.run(argv, cwd=repo, env=env, capture_output=True)
            steps.append({"step": step, "exit": result.returncode,
                          "stdout": result.stdout.decode("utf-8")})
        expected = case_dir / "expected"
        shutil.rmtree(expected, ignore_errors=True)
        (expected / "run").mkdir(parents=True)
        if run.exists():
            shutil.copytree(run, expected / "run", dirs_exist_ok=True)
        (expected / "steps.json").write_text(json.dumps(steps, indent=2, ensure_ascii=False) + "\n",
                                             encoding="utf-8")
    print(f"{case_dir.name}: {len(steps)} steps captured")


def main():
    for case_dir in sorted((HERE / "cases").iterdir()):
        if (case_dir / "case.json").exists():
            capture(case_dir)


if __name__ == "__main__":
    main()
