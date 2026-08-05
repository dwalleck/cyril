#!/usr/bin/env python3
"""cyril-jxmv cheapest falsifier — claims C4 (detection heuristic) + C6
(location derives from the RESOLVED spawn command).

Implements the PROPOSED heuristic (manual basename split on both separators,
ASCII-lowercase, optional .exe strip, exact == "wsl") and runs it over every
spawn-command shape the codebase actually produces, plus near-misses.

Oracle (independent of cyril): Microsoft ships the WSL launcher as
%SystemRoot%\\System32\\wsl.exe, invoked as `wsl` (CreateProcess appends .exe;
NTFS name lookup is case-insensitive). Anything that is not that launcher runs
where cyril runs. Expected values below are annotated with that ground truth,
not derived from the heuristic.

Falsified if: any EXPECT mismatches, or the pre/post-resolve KAS-free pair
does NOT diverge (which would make C6's placement claim vacuous).
"""

def classify(program: str) -> str:
    base = program.replace("\\", "/").rsplit("/", 1)[-1].lower()
    if base.endswith(".exe"):
        base = base[:-4]
    return "wsl" if base == "wsl" else "native"

CASES = [
    # (program, expected, ground)
    ("kiro-cli", "native", "v2 default (main.rs:27); native exe on PATH both OSes"),
    ("kiro-cli.exe", "native", "explicit .exe form of the native MSI binary"),
    ("wsl", "wsl", "MS launcher invoked bare; CreateProcess appends .exe"),
    ("wsl.exe", "wsl", "MS launcher with extension"),
    ("WSL.EXE", "wsl", "NTFS case-insensitive lookup"),
    (r"C:\Windows\System32\wsl.exe", "wsl", "canonical full launcher path"),
    (r"c:\windows\system32\WSL.exe", "wsl", "case-varied full path"),
    ("/usr/bin/node", "native", "KAS free path (discovery.rs:228), Linux"),
    (r"C:\Program Files\nodejs\node.exe", "native", "KAS free path, Windows"),
    ("sh", "native", "transport.rs test stubs"),
    ("wslkiro", "native", "not the launcher: exact-match only"),
    ("my-wsl-wrapper.exe", "native", "not the launcher"),
    ("wsl2", "native", "not the launcher"),
    ("wsl ", "native", "literal, no trim (CYRIL_WSL_DISTRO precedent)"),
]

fail = False
for program, expect, ground in CASES:
    got = classify(program)
    mark = "ok" if got == expect else "FALSIFIED"
    if got != expect:
        fail = True
    print(f"C4|{mark}|{program!r}|expect={expect}|got={got}|{ground}")

# C6 — pre- vs post-resolve divergence for KAS free path: user says
# `--agent-command wsl kiro-cli acp`, engine kas free; resolve_spawn_command
# (bridge.rs:539) discards the CLI argv and spawns node directly.
pre = classify("wsl")               # CLI argv program
post = classify("/usr/bin/node")    # resolved spawn program (discovery.rs:228)
div = "ok" if (pre, post) == ("wsl", "native") else "FALSIFIED"
if div != "ok":
    fail = True
print(f"C6|{div}|pre-resolve={pre}|post-resolve={post}|divergence proves placement matters")

# C6 — wrapper path preserves the program (version.rs:72-78): wsl stays wsl.
wrap = classify("wsl")
print(f"C6|{'ok' if wrap == 'wsl' else 'FALSIFIED'}|wrapper preserves program -> {wrap}")

print("FALSIFIER-FAILED" if fail else "FALSIFIER-PASSED")
raise SystemExit(1 if fail else 0)
