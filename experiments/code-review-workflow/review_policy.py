# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Pure decisions the code-review driver makes, kept importable so they can be tested.

run_review.py imports this module (it sits next to it, so plain `python` and
`uv run --script` both find it); selftest_crtool.py tests it on every OS.
Nothing here spawns a process or touches the network.
"""
import json
import os
import pathlib
import re
import sys

CRTOOL_REL = ".kiro/code-review/crtool.py"
# Characters that change meaning inside a double-quoted shell argument: `$` and a
# backtick expand in bash and PowerShell, `"` ends the argument, and PowerShell
# 5.1 turns a trailing `\"` into an escaped quote.
UNSAFE_IN_QUOTES = re.compile(r'[$`"]|\\$')
# PowerShell's call operator runs a quoted program path: `& "C:\py\python.exe" x`.
PS_CALL = re.compile(r"^\s*&\s+")
CD_PREFIX = re.compile(r"^\s*cd\s+[^;&|`$]+&&\s*")
SHELL_META = re.compile(r"[;&|`]|\$\(")


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


def crtool_command_problem(cmd):
    """Why the permission policy would refuse this crtool command, or None."""
    if "crtool.py" not in cmd:
        return "it does not name crtool.py, so every crtool step would be refused"
    body = PS_CALL.sub("", cmd)
    if SHELL_META.search(body):
        return "it contains shell metacharacters (; & | ` $( ), which the policy refuses"
    return None


def input_problem(name, value):
    """Why a value cannot be placed inside the recipe's double quotes, or None."""
    if value == "":
        return f"{name} is empty (Windows PowerShell 5.1 drops an empty quoted argument)"
    if UNSAFE_IN_QUOTES.search(value):
        return f"{name} contains $, a backtick, a double quote or a trailing backslash: {value!r}"
    return None


def under(path, root):
    try:
        path, root = os.path.normcase(os.path.realpath(path)), os.path.normcase(os.path.realpath(root))
        return os.path.commonpath([path, root]) == root
    except ValueError:  # different drives on Windows
        return False


def decide(p, ws, rundir, allow_caps=()):
    """(allow, why) for one session/request_permission. Reads inside the workspace,
    writes only under the run directory, shell only for a crtool command."""
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
        cmd = next((v for v in (res, kiro.get("command"), raw.get("command"), raw.get("cmd"), tc.get("title"))
                    if isinstance(v, str) and "crtool.py" in v), None)
        if cmd is None:
            return "crtool.py" in json.dumps(p), "shell (command location unknown; matched crtool.py in request)"
        body = PS_CALL.sub("", CD_PREFIX.sub("", cmd))  # tolerate `cd <ws> &&` and PowerShell's `& "exe"`
        return not SHELL_META.search(body), f"shell {cmd[:160]}"
    return False, f"unhandled capability {cap!r} {res[:120]}"
