#!/usr/bin/env python3
"""cyril-jxmv oracle — independent computation of what Windows-cyril's
translation layer produces, implemented from the DOCUMENTED contract only:

  - CLAUDE.md "Path Translation": drive mounts /mnt/c/... <-> C:\\... are
    UNCONDITIONAL on Windows.
  - path.rs doc comments: the \\?\\ extended-length prefix is stripped before
    the drive rule; wsl->win touches only /mnt/<single-letter> inputs, all
    other inputs pass through unchanged (no distro is configured here).

Deliberately NOT derived from the Rust implementation — different language,
different codebase, same published contract. Emits the same OUT|/IN| lines as
probe.rs for an item-by-item diff.
"""


def doc_win_to_wsl(p: str) -> str:
    if p.startswith("\\\\?\\"):
        p = p[4:]
    if len(p) >= 2 and p[1] == ":" and p[0].isalpha():
        drive = p[0].lower()
        rest = p[2:].replace("\\", "/").lstrip("/")
        return f"/mnt/{drive}/{rest}" if rest else f"/mnt/{drive}"
    return p.replace("\\", "/")


def doc_wsl_to_win(p: str) -> str:
    # Only /mnt/<single-letter> translates; everything else (including C:\\
    # inputs a native agent would send) passes through untouched.
    if p.startswith("/mnt/") and len(p) > 5:
        rest = p[5:]
        if len(rest) == 1 or (len(rest) > 1 and rest[1] == "/"):
            drive = rest[0].upper()
            tail = rest[2:] if len(rest) > 1 else ""
            return f"{drive}:\\{tail.replace('/', chr(92))}" if tail else f"{drive}:\\"
    return p


OUT_INPUTS = [
    "C:\\Users\\u\\repos\\proj",
    "C:\\",
    "D:\\data",
    "\\\\?\\C:\\Users\\u\\repos\\proj",
]
IN_INPUTS = ["C:\\Users\\u\\repos\\proj\\file.rs", "C:/Users/u/x.txt", "D:\\data\\f"]

for p in OUT_INPUTS:
    print(f"OUT|{p}|{doc_win_to_wsl(p)}")
for p in IN_INPUTS:
    print(f"IN|{p}|{doc_wsl_to_win(p)}")
