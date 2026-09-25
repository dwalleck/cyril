#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Offline, cross-platform self-test for the code-review-max tooling. No KAS, no model.

    uv run --script experiments/code-review-workflow/selftest_crtool.py
    python3 experiments/code-review-workflow/selftest_crtool.py

0. The driver's pure decisions (review_policy.py): kiro-cli data dir per platform,
   path containment, the permission policy (including every known way around a
   blacklist: redirects, newlines, subexpressions, a smuggled `cd`, a command in
   another field), crtool-command and input validation.
1. The recipe regenerates from build_recipe.py at the node cap, and every crtool
   command line in it is double-quoted (single quotes mean nothing to cmd.exe and
   a bare `a,b,c` becomes an array in PowerShell).
2. Every one of those command lines, with {{crtool}} filled in exactly as the
   driver fills it (uv, and the platform's python when it is a real interpreter),
   runs through this platform's shell - PowerShell on Windows, as KAS uses there;
   bash elsewhere - against a throwaway git repository, after the driver's
   permission policy has allowed it for every crtool form. Separately, each line
   runs through EVERY shell on the machine (pwsh 7 and Windows PowerShell 5.1 on
   Windows) into an argv echo, which must match what the policy parsed.
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

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", ".."))
CRTOOL = os.path.join(REPO, ".kiro", "code-review", "crtool.py")
RECIPE = os.path.join(REPO, ".kiro", "workflows", "code-review-max.workflow.json")
IS_WINDOWS = os.name == "nt"
sys.path.insert(0, HERE)
import review_policy as policy  # noqa: E402

policy.utf8_stdio()
ANGLES = ["a-line-scan", "b-removed-behavior", "c-cross-file", "d-language-pitfalls", "e-wrapper-proxy",
          "cleanup", "altitude", "conventions"]
# Fully faked git: no global or system config (gpgsign, hooksPath, noprefix, color) and
# no inherited repository variables reach the throwaway repo.
GIT_ENV = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
GIT_ENV.update(GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.com", GIT_COMMITTER_NAME="t",
               GIT_COMMITTER_EMAIL="t@example.com", GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1")


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


def shells():
    """Every shell KAS could hand a command to here: both PowerShells on Windows (5.1
    drops empty arguments and mangles `\\"`, 7 does not), bash elsewhere."""
    if IS_WINDOWS:
        found = [x for x in (shutil.which("pwsh"), shutil.which("powershell")) if x]
        check(bool(found), "a PowerShell is on PATH")
        return found
    return [shutil.which("bash") or "/bin/sh"]


def shell_argv(command, exe=None):
    """`command` under the shell KAS would use (the first of shells(), unless given)."""
    exe = exe or shells()[0]
    if IS_WINDOWS:
        return [exe, "-NoProfile", "-NonInteractive", "-Command", command]
    return [exe, "-c", command]


def write_json(path, obj):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(obj, f)


def shell_request(cmd, command=None, title=None):
    """A session/request_permission shaped like KAS's run_command consent."""
    return {"toolCall": {"title": title or cmd}, "_meta": {"kiro": {"toolId": "run_command",
            "command": cmd if command is None else command, "consent": {"capability": "shell", "resource": cmd}}}}


def policy_tests():
    step("driver policy (review_policy.py)")
    home = "/h"
    check(policy.kiro_data_dir({"LOCALAPPDATA": r"C:\Users\u\AppData\Local"}, "win32", home=home)
          .endswith("Kiro-Cli"), "Windows: %LOCALAPPDATA%\\Kiro-Cli")
    check(policy.kiro_data_dir({}, "linux", home=home) == os.path.join(home, ".local", "share", "kiro-cli"),
          "Linux: ~/.local/share/kiro-cli")
    check(policy.kiro_data_dir({"XDG_DATA_HOME": "/x"}, "linux", home=home) == os.path.join("/x", "kiro-cli"),
          "Linux: $XDG_DATA_HOME/kiro-cli")
    check("Application Support" in policy.kiro_data_dir({}, "darwin", exists=lambda p: False, home=home),
          "macOS: ~/Library/Application Support/kiro-cli by default")
    xdg_db = os.path.join(home, ".local", "share", "kiro-cli", "data.sqlite3")
    check(policy.kiro_data_dir({}, "darwin", exists=lambda p: p == xdg_db, home=home).endswith(
          os.path.join(".local", "share", "kiro-cli")), "macOS: the XDG location when only it holds a store")
    check(policy.kiro_data_dir({"KIRO_DATA_DIR": "/d"}, "win32", home=home) == "/d", "KIRO_DATA_DIR overrides")

    ws = os.path.join(TMP, "policy-ws")
    rundir = os.path.join(ws, ".code-review", "r")
    os.makedirs(rundir)
    check(policy.under(os.path.join(rundir, "verdicts", "x.json"), rundir), "under: a file inside the run dir")
    check(not policy.under(os.path.join(ws, "src", "x.rs"), rundir), "under: a file outside the run dir")
    if IS_WINDOWS:
        check(policy.under(os.path.join(rundir, "X.JSON").upper(), rundir), "under: case-insensitive on Windows")
        check(not policy.under(r"Z:\\elsewhere", rundir), "under: another drive is outside")
    write = lambda res: {"_meta": {"kiro": {"toolId": "fs_write", "consent": {
        "capability": "fs_write", "resource": res, "workspaceRoot": ws}}}}
    check(policy.decide(write(os.path.join(rundir, "a.json")), ws, rundir, "")[0], "decide: write into the run dir")
    check(not policy.decide(write(os.path.join(ws, "src", "lib.rs")), ws, rundir, "")[0], "decide: no write to source")
    check(policy.decide({"_meta": {"kiro": {"toolId": "read_file", "consent": {"capability": "fs_read",
          "resource": "src/lib.rs", "workspaceRoot": ws}}}}, ws, rundir, "")[0], "decide: relative read in workspace")

    rd = policy.posix_path(rundir)
    for runner, uv in (("uv", True), ("python", False), ("auto", True), ("auto", False)):
        for win in (True, False):
            cmd = policy.crtool_command(runner, uv, win)
            check(policy.crtool_command_problem(cmd, win) is None and policy.decide(
                  shell_request(f'{cmd} gather "{rd}"'), ws, rundir, cmd)[0],
                  f"runner={runner} uv={uv} windows={win}: {cmd!r} passes the policy")
    quoted = '& "C:/Program Files/Python/python.exe" .kiro/code-review/crtool.py'
    check(policy.crtool_command_problem(quoted, True) is None, "Windows: PowerShell's `& \"exe\"` call form is fine")
    check(policy.crtool_command_problem(quoted, False) is not None, "bash: the `& \"exe\"` form is refused")
    check(policy.crtool_command_problem(quoted[2:], True) is not None,
          "Windows: a quoted program without `& ` is refused (PowerShell reads it as a string)")
    check(policy.crtool_command_problem(quoted[2:], False) is None, "bash: a quoted program path is fine")
    check(policy.crtool_command_problem("python3 x.py", False) is not None, "a command without crtool.py is refused")

    crt = "python3 .kiro/code-review/crtool.py"
    allowed = lambda cmd, **kw: policy.decide(shell_request(cmd, **kw), ws, rundir, crt)[0]
    check(allowed(f'{crt} merge "{rd}" --expect "a,b"'), "decide: a recipe line is allowed")
    check(allowed(f'cd "{ws}" && {crt} collate "{rd}"'), "decide: `cd <workspace> && ` is allowed")
    for why, cmd in (
            ("a chained command", f'{crt} gather "{rd}"; rm -rf /'),
            ("an output redirect", f'{crt} gather "{rd}" > src/lib.rs'),
            ("a redirect on an unquoted word", f"{crt} gather x > src/lib.rs"),
            ("a second line", f'{crt} gather "{rd}"\nrm -rf src'),
            ("a PowerShell subexpression", f'{crt} gather "{rd}" (Remove-Item -Recurse -Force src)'),
            ("a command substitution in quotes", f'{crt} gather "{rd}" "$(rm -rf src)"'),
            ("a redirect hidden in the cd prefix", f'cd . > src/lib.rs && {crt} merge "{rd}"'),
            ("a second line hidden in the cd prefix", f'cd x\nrm -rf src && {crt} merge "{rd}"'),
            ("a cd outside the workspace", f'cd / && {crt} merge "{rd}"'),
            ("another program naming crtool.py", "rm -rf src .kiro/code-review/crtool.py"),
            ("the diagnostics subcommand (runs any command)", f'{crt} diagnostics "{rd}" "rm -rf src"'),
            ("another run directory", f'{crt} gather "{ws}/src"'),
            ("an escaped closing quote", f'{crt} gather "{rd}" "a\\"'),
            ("a word glued to a quoted string", f'{crt} gather "{rd}"x')):
        check(not allowed(cmd), f"decide: {why} is refused")
    check(not allowed("rm -rf src", command="rm -rf src", title=f"run {crt}"),
          "decide: a command elsewhere than the title is what gets checked")
    check(not allowed(f'{crt} gather "{rd}"', command="rm -rf src"),
          "decide: every command field must hold an allowed call")
    check(not policy.decide({"toolCall": {"title": f'{crt} gather "{rd}"'}, "_meta": {"kiro": {
          "toolId": "run_command", "consent": {"capability": "shell"}}}}, ws, rundir, crt)[0],
          "decide: a shell request with no command field is refused")

    check(policy.input_problem("rundir", "C:/w/r") is None, "input: a forward-slash path is fine")
    check(policy.input_problem("rundir", "//wsl$/Ubuntu/home/u/r") is None, "input: a \\\\wsl$ share path is fine")
    for bad in ("", "a$b", "a$", "a`b", 'a"b', "a\u201db", "C:\\w", "a\nb"):
        check(policy.input_problem("x", bad) is not None, f"input: {bad!r} is refused")
    check("\\" not in policy.posix_path(TMP), "posix_path uses forward slashes")


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


def argv_probe(lines):
    """Each recipe line, under EVERY shell here, into a program that echoes its argv: what
    arrives must be exactly the words the permission policy parsed. Values are the hard
    cases the inputs allow: a space, a `$` before `/` (as in \\\\wsl$), a range target."""
    echo = os.path.join(TMP, "echo_argv.py")
    with open(echo, "w", encoding="utf-8") as f:
        f.write("import json, sys\nsys.stdout.write(json.dumps(sys.argv[1:]))\n")
    prog = (f'& "{sys.executable}" "{echo}"' if IS_WINDOWS else f'"{sys.executable}" "{echo}"')
    values = {"{{rundir}}": policy.posix_path(os.path.join(TMP, "probe dir", "wsl$", "r")),
              "{{target}}": "main...HEAD", "{{scope}}": "crates docs/x"}
    for key, value in values.items():
        check(policy.input_problem(key, value) is None, f"probe value {value!r} is a valid input")
    for exe in shells():
        step(f"argv probe through {exe}")
        for node, line in ((n, x) for n, xs in lines.items() for x in xs):
            cmd = line.replace("{{crtool}}", prog)
            for k, v in values.items():
                cmd = cmd.replace(k, v)
            want = policy.split_args(cmd[len(prog):])
            got = json.loads(run(shell_argv(cmd, exe), cwd=TMP))
            check(got == want, f"[{node}] {line.split()[1]}: the shell passed {len(got)} args as parsed")


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

    policy_tests()
    lines = recipe_commands()
    # The driver's crtool commands are relative to the workspace root, so crtool lives there.
    os.makedirs(os.path.join(ws, ".kiro", "code-review"))
    shutil.copy2(CRTOOL, os.path.join(ws, ".kiro", "code-review", "crtool.py"))
    # A run directory with a space in it: the double quotes have to carry it.
    run_dir = policy.posix_path(os.path.join(ws, ".code-review", "self test"))
    # {{crtool}} exactly as the driver fills it. Every form KAS could be handed on this
    # machine is used for at least one step; `& "exe"` is the --crtool-cmd quoted form.
    forms = []
    if shutil.which("uv"):
        forms.append(policy.crtool_command("uv", True, IS_WINDOWS))
    py = shutil.which("python" if IS_WINDOWS else "python3")
    if py and subprocess.run([py, "-c", "import sys"], capture_output=True).returncode == 0:
        forms.append(policy.crtool_command("python", False, IS_WINDOWS))
    forms.append(f'& "{sys.executable}" .kiro/code-review/crtool.py' if IS_WINDOWS
                 else f'"{sys.executable}" .kiro/code-review/crtool.py')
    print(f"   ..  crtool forms under test: {forms}")
    fill = {"{{rundir}}": run_dir, "{{target}}": "HEAD~1...HEAD", "{{scope}}": "."}
    used = []

    def filled(line, form, values=fill):
        cmd = line.replace("{{crtool}}", form)
        for k, v in values.items():
            cmd = cmd.replace(k, v)
        return cmd

    step("the policy allows every recipe line under every crtool form")
    for node, line in ((n, x) for n, xs in lines.items() for x in xs):
        for form in forms:
            allowed, why = policy.decide(shell_request(filled(line, form)), ws, run_dir, form)
            check(allowed, f"[{node}] {form.split()[0]}: {filled(line, form)[len(form):][:70]}")
    argv_probe(lines)

    def through_shell(node, which):
        """The recipe's own line `which` of `node`, run through the shell. Forms rotate."""
        form = forms[len(used) % len(forms)]
        used.append(form)
        return run(shell_argv(filled(lines[node][which], form)), cwd=ws)

    step(f"the recipe's own gather line, through {'PowerShell' if IS_WINDOWS else 'bash'}")
    out = through_shell("setup", 0)
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
    out = through_shell("dedup", 0)
    check("from 8/8 angles" in out, "merge saw all 8 angles through the shell's argument parsing")
    with open(os.path.join(run_dir, "candidates", "digest-1.txt"), encoding="utf-8") as f:
        check("SAME location" in f.read(), "the digest flags the two candidates at src/lib.rs:6")

    step("dedup decision, then the recipe's own shard line, verdicts")
    write_json(os.path.join(run_dir, "deduped", "decisions.json"),
               {"groups": [{"pids": ["a-line-scan-1", "d-language-pitfalls-1"], "reason": "same underflow"}]})
    through_shell("dedup", 1)
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
    out = through_shell("ballots", 0)
    check("1 of 2 candidates get two more votes" in out, "the refuted conventions claim is balloted")
    for loop, tag, verdict in (("r1", "v2", "CONFIRMED"), ("r2", "v3", "CONFIRMED")):
        write_json(os.path.join(run_dir, "verdicts", loop, f"{conv}.{tag}.json"),
                   {"id": f"{conv}.{tag}", "verdict": verdict, "reasoning": "it is a rule"})

    step("the recipe's own collate (2-of-3 tally), finalize and comments lines")
    out = through_shell("rank", 0)
    check("votes changed the outcome" in out, "REFUTED, CONFIRMED, CONFIRMED overturns to CONFIRMED")
    write_json(os.path.join(run_dir, "ranking.json"), {"order": [bug, conv]})
    through_shell("rank", 1)
    write_json(os.path.join(run_dir, "comments", f"{bug}.json"), {"label": "issue", "decorations": ["blocking"],
               "subject": "`new_helper(0)` underflows", "discussion": "Use `x.saturating_sub(1)` or check for 0."})
    out = through_shell("comment", 0)
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
    total = sum(len(xs) for xs in lines.values())
    check(len(used) == total, f"every recipe crtool line ran through the shell ({len(used)}/{total})")
    check(set(used) == set(forms), "every crtool form ran at least once")
    print("\nALL SELF-TESTS PASS")


if __name__ == "__main__":
    TMP = tempfile.mkdtemp(prefix="crtool-selftest-")
    try:
        main()
    finally:
        shutil.rmtree(TMP, ignore_errors=True)
