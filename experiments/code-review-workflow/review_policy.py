# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Pure decisions the code-review driver makes, kept importable so they can be tested.

run_review.py imports this module (it sits next to it, so plain `python` and
`uv run --script` both find it); selftest_crtool.py tests it on every OS.
Nothing here spawns a process or touches the network.
"""
import os
import pathlib
import re
import sys

CRTOOL_REL = ".kiro/code-review/crtool.py"
# The crtool subcommands a workflow step may run. `diagnostics` runs an arbitrary
# command, so only the driver calls it, never a step.
STEP_SUBCOMMANDS = ("gather", "merge", "shard", "facts", "ballots", "collate", "finalize", "comments")
# Quotes PowerShell reads as `"`: a value holding one ends its argument early.
SMART_QUOTES = "\u201c\u201d\u201e"
# A value that is safe inside the recipe's double quotes, in bash and in both PowerShells:
# no quote of any kind, no backtick, no backslash (bash escapes with it, and 5.1 turns
# a trailing `\"` into an escaped quote), no control character, and `$` only where
# neither shell expands it - before `/`, as in `//wsl$/Ubuntu/...`.
UNSAFE_IN_QUOTES = re.compile(r'["`\\\x00-\x1f' + SMART_QUOTES + r']|\$(?!/)')
# One argument of a step's crtool call: a double-quoted string (a backslash is kept for
# Windows paths, but never right before the closing quote) or a bare word.
_ARG = re.compile(r'"((?:[^"`$\\\x00-\x1f' + SMART_QUOTES + r']|\\(?!")|\$(?=/))*)"|([A-Za-z0-9_./:=+-]+)')
_CD = re.compile(r'cd[ \t]+("[^"]*"|[^ \t]+)[ \t]+&&[ \t]+')


def utf8_stdio():
    """Windows consoles and pipes default to a legacy code page; pin UTF-8 rather
    than crash on the first arrow or dash."""
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", errors="replace")


def split_args(text):
    """The argument list a shell would build from `text`, or None when any part of it is
    not a plain word or a safely double-quoted string. Nothing that could redirect,
    chain, substitute or continue onto another line gets through."""
    out, pos = [], 0
    text = text.strip(" \t")
    while pos < len(text):
        m = _ARG.match(text, pos)
        if not m:
            return None
        out.append(m.group(1) if m.group(1) is not None else m.group(2))
        pos = m.end()
        if pos < len(text):
            gap = re.match(r"[ \t]+", text[pos:])
            if not gap:
                return None  # `"a"b` or `a"b"`: one word to the shell, two here
            pos += gap.end()
    return out


def same_path(path, target, base):
    """Whether `path` (relative to `base` when not absolute) names `target`."""
    try:
        norm = lambda x: os.path.normcase(os.path.realpath(x))
        return norm(os.path.join(base, path)) == norm(target)
    except (ValueError, OSError):
        return False


def posix_path(path):
    """One spelling for paths that leave Python: forward slashes, absolute.

    bash, pwsh, cmd, Python and node all accept `C:/x/y`; a backslash path copied
    into JSON by a model turns into an escaping bug.
    """
    return pathlib.Path(os.path.realpath(path)).as_posix()


def kiro_data_dir(environ=None, platform=None, exists=os.path.exists, home=None):
    """Where kiro-cli keeps data.sqlite3 (the auth store) and its kas/ bundles."""
    env = os.environ if environ is None else environ
    platform = sys.platform if platform is None else platform
    home = os.path.expanduser("~") if home is None else home
    if env.get("KIRO_DATA_DIR"):
        return env["KIRO_DATA_DIR"]
    if env.get("KIRO_XDG_DATA_HOME"):  # the older, Linux-only override
        return os.path.join(env["KIRO_XDG_DATA_HOME"], "kiro-cli")
    if platform == "win32":  # native Windows kiro-cli: %LOCALAPPDATA%\Kiro-Cli
        return os.path.join(env.get("LOCALAPPDATA") or os.path.join(home, "AppData", "Local"), "Kiro-Cli")
    xdg = os.path.join(env.get("XDG_DATA_HOME") or os.path.join(home, ".local", "share"), "kiro-cli")
    if platform == "darwin":
        # Rust's dirs::data_local_dir() is ~/Library/Application Support on macOS; prefer
        # it unless only the XDG location actually holds an auth store.
        mac = os.path.join(home, "Library", "Application Support", "kiro-cli")
        if exists(os.path.join(mac, "data.sqlite3")) or not exists(os.path.join(xdg, "data.sqlite3")):
            return mac
    return xdg


def crtool_command(runner, has_uv, windows):
    """How KAS's shell launches crtool, relative to the workspace root."""
    if runner == "auto":
        runner = "uv" if has_uv else "python"
    if runner == "uv":
        return f"uv run --script {CRTOOL_REL}"
    return f"{'python' if windows else 'python3'} {CRTOOL_REL}"


def crtool_command_problem(cmd, windows):
    """Why this crtool command would not run under the platform's shell (PowerShell on
    Windows, bash elsewhere), or None."""
    if "crtool.py" not in cmd:
        return "it does not name crtool.py"
    if re.search(r"[\x00-\x1f;|<>`" + SMART_QUOTES + r"]|\$\(|&&", cmd):
        return "it must be one simple command: no newline, ; | < > ` $( or &&"
    body = cmd.strip()
    if windows:
        if body[:1] in "\"'":
            return "PowerShell reads a leading quoted path as a string, not a program: prefix it with `& `"
        if "&" in body[1:] or (body.startswith("&") and not body.startswith("& ")):
            return "`&` may only appear as PowerShell's leading call operator `& `"
    elif "&" in body:
        return "`&` is PowerShell's call operator; under bash it backgrounds the command"
    return None


def input_problem(name, value):
    """Why a value cannot be placed inside the recipe's double quotes, or None."""
    if value == "":
        return f"{name} is empty (Windows PowerShell 5.1 drops an empty quoted argument)"
    if UNSAFE_IN_QUOTES.search(value):
        return (f"{name} contains a quote, a backtick, a backslash, a control character or a `$` that a "
                f"shell would expand (use forward slashes): {value!r}")
    return None


def under(path, root):
    try:
        path, root = os.path.normcase(os.path.realpath(path)), os.path.normcase(os.path.realpath(root))
        return os.path.commonpath([path, root]) == root
    except ValueError:  # different drives on Windows
        return False


def crtool_call_problem(cmd, ws, rundir, crtool):
    """Why a shell command is not exactly a workflow step's crtool call, or None.

    Allowed: an optional `cd <workspace> && `, then the driver's own crtool command,
    then a step subcommand whose first argument is the run directory, then plain or
    safely double-quoted words. Checked by construction (what runs), not by looking
    for dangerous characters, so no shell syntax can slip past."""
    body = cmd.strip(" \t")
    m = _CD.match(body)
    if m:
        where = split_args(m.group(1))
        if not where or len(where) != 1 or not same_path(where[0], ws, ws):
            return f"cd to somewhere other than the workspace: {m.group(1)[:80]}"
        body = body[m.end():]
    if not body.startswith(crtool + " "):
        return f"not the crtool command this run was started with ({crtool})"
    words = split_args(body[len(crtool) + 1:])
    if words is None:
        return "an argument is not a plain word or a safely double-quoted string"
    if not words or words[0] not in STEP_SUBCOMMANDS:
        return f"crtool subcommand {(words or ['(none)'])[0]!r} is not one a step runs"
    if len(words) < 2 or not same_path(words[1], rundir, ws):
        return "the first argument is not this run's directory"
    return None


def decide(p, ws, rundir, crtool, allow_caps=()):
    """(allow, why) for one session/request_permission. Reads inside the workspace,
    writes only under the run directory, shell only for this run's crtool calls."""
    kiro = (p.get("_meta") or {}).get("kiro") or {}
    consent = kiro.get("consent") or {}
    cap = consent.get("capability") or kiro.get("toolId") or ""
    res = str(consent.get("resource") or "")
    if cap in allow_caps:
        return True, f"{cap} {res[:120]} (--allow-cap)"
    if cap == "fs_read":
        # KAS asks an implicit fs_read consent for directory listings and for a
        # shell command's cwd (nested under the run_command call: denying it
        # rejects the command). Reading the workspace is the whole job.
        full = res if os.path.isabs(res) else os.path.join(consent.get("workspaceRoot") or ws, res)
        return under(full, ws), f"read {res}"
    if cap in ("fs_write", "str_replace") or kiro.get("toolId") in ("fs_write", "str_replace"):
        full = res if os.path.isabs(res) else os.path.join(consent.get("workspaceRoot") or ws, res)
        return under(full, rundir), f"write {res}"
    if "execute_bash" in (cap, kiro.get("toolId")) or cap in ("shell", "execute"):
        tc = p.get("toolCall") or {}
        raw = tc.get("rawInput") if isinstance(tc.get("rawInput"), dict) else {}
        # EVERY place the request carries a command must hold an allowed call: whichever
        # one KAS actually runs is then allowed. A title is display text, never checked.
        cmds = [v for v in (consent.get("resource"), kiro.get("command"), raw.get("command"), raw.get("cmd"))
                if isinstance(v, str) and v.strip()]
        if not cmds:
            return False, "shell request carries no command to check"
        for cmd in cmds:
            problem = crtool_call_problem(cmd, ws, rundir, crtool)
            if problem:
                return False, f"shell {cmd[:160]} ({problem})"
        return True, f"shell {cmds[0][:160]}"
    return False, f"unhandled capability {cap!r} {res[:120]}"
