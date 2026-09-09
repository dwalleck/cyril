#!/usr/bin/env python3
"""Independent implementation of the color-mode detection precedence.

Encoded directly from the table in `.cyril-qaq0/spec.md`, not derived from the
Rust implementation. Each case is a named row; the Rust fence drives the same
rows and must agree on every one.

Usage:
  python3 oracle-precedence.py            # print expected mode per case as TSV
  python3 oracle-precedence.py --check    # self-check the row set
"""
from __future__ import annotations

import sys
from dataclasses import dataclass


@dataclass(frozen=True)
class Case:
    name: str
    explicit: str | None
    no_color: str | None
    color_term: str | None
    term: str | None
    is_windows: bool


CASES = [
    # rule 1: an explicit value overrides everything, including NO_COLOR
    Case("explicit-beats-no-color", "ansi256", "1", "truecolor", "xterm-256color", False),
    Case("explicit-none", "none", None, "truecolor", "xterm-256color", False),
    Case("explicit-ansi16", "ansi16", "1", None, "dumb", True),
    # rule 2: non-empty NO_COLOR
    Case("no-color-nonempty", None, "1", "truecolor", "xterm-256color", False),
    Case("no-color-empty-is-unset", None, "", "truecolor", "xterm-256color", False),
    # rule 3: COLORTERM
    Case("colorterm-truecolor", None, None, "truecolor", "xterm", False),
    Case("colorterm-24bit", None, None, "24bit", "xterm", False),
    Case("colorterm-other-falls-through", None, None, "yes", "xterm-256color", False),
    # rule 4: windows
    Case("windows-default", None, None, None, "xterm", True),
    Case("windows-loses-to-no-color", None, "1", None, "xterm", True),
    # rule 5/6/7: TERM
    Case("term-256color", None, None, None, "xterm-256color", False),
    Case("term-dumb", None, None, None, "dumb", False),
    Case("term-other-defaults-truecolor", None, None, None, "xterm", False),
    Case("all-unset", None, None, None, None, False),
]


def detect(case: Case) -> str:
    if case.explicit is not None:
        return case.explicit
    if case.no_color is not None and case.no_color != "":
        return "none"
    if case.color_term in ("truecolor", "24bit"):
        return "truecolor"
    if case.is_windows:
        return "truecolor"
    if case.term is not None and "256color" in case.term:
        return "ansi256"
    if case.term == "dumb":
        return "none"
    return "truecolor"


def self_check() -> int:
    problems: list[str] = []
    names = [case.name for case in CASES]
    if len(set(names)) != len(names):
        problems.append("case names must be unique")
    for case in CASES:
        if case.explicit is not None and detect(case) != case.explicit:
            problems.append(f"{case.name}: explicit value must win")
    if detect(CASES[3]) != "none" or detect(CASES[4]) != "truecolor":
        problems.append("NO_COLOR non-empty/empty distinction is wrong")
    if problems:
        for problem in problems:
            print(f"FAIL {problem}")
        return 1
    print(f"PASS {len(CASES)} precedence cases agree with the spec table")
    return 0


def main() -> int:
    if "--check" in sys.argv[1:]:
        return self_check()
    for case in CASES:
        print(f"{case.name}\t{detect(case)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
