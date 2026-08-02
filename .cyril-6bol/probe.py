#!/usr/bin/env python3
"""Probe one genuine KAS terminal/create request through the proposed Unix shell path."""
import json
import os
import re
from pathlib import Path
import shlex
import shutil
import subprocess

root = Path(__file__).resolve().parents[1]
fixture = root / ".cyril-7bdu/fixtures/terminal__create.json"
params = json.loads(fixture.read_text())["params"]
env_shell = os.environ.get("SHELL", "")
basename = Path(env_shell).name
supported = {"bash", "sh", "dash", "zsh", "ksh", "fish"}
if basename in supported and os.access(env_shell, os.X_OK):
    shell = env_shell
else:
    shell = shutil.which("bash")
if shell is None:
    raise SystemExit("no runnable Unix shell")
family = "fish" if Path(shell).name == "fish" else "posix"
tokens = [params["command"], *params.get("args", [])]
command = shlex.join(tokens)
operators = {"|", ">", ">>", "<", "&&", "||"}
variable = re.compile(r"\$(?:[A-Za-z_][A-Za-z0-9_]*|\{[A-Za-z_][A-Za-z0-9_]*\})")
def render(items):
    return " ".join(
        token if token in operators or variable.fullmatch(token) else shlex.quote(token)
        for token in items
    )
operator_command = render([*tokens, "|", "tr", "a-z", "A-Z"])
expansion_command = render([params["command"], "$CYRIL_SHELL_PROBE"])
def run(source):
    return subprocess.run(
        [shell, "-lc", source], cwd=root, stdin=subprocess.DEVNULL,
        env={**os.environ, "CYRIL_SHELL_PROBE": "expanded-42"},
        text=True, capture_output=True, check=False,
    )
result = run(command)
operator_result = run(operator_command)
expansion_result = run(expansion_command)
print(json.dumps({
    "source": str(fixture.relative_to(root)),
    "shell_env": env_shell or None,
    "shell": shell,
    "family": family,
    "command": command,
    "stdout": result.stdout,
    "stderr": result.stderr,
    "exit": result.returncode,
    "operator_command": operator_command,
    "operator_stdout": operator_result.stdout,
    "operator_stderr": operator_result.stderr,
    "operator_exit": operator_result.returncode,
    "expansion_command": expansion_command,
    "expansion_stdout": expansion_result.stdout,
    "expansion_stderr": expansion_result.stderr,
    "expansion_exit": expansion_result.returncode,
}, sort_keys=True))
