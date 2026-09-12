#!/usr/bin/env python3
"""Pre-flight for `mutations.sh`: every `prove` anchor must occur exactly once.

A mutation whose `from` text no longer matches aborts the run *after* the slow
proofs before it have already run their fences (and after any earlier mutation
in that block was applied and restored), so a stale anchor costs a full oracle
round trip. This checker is the 0.2-second version: it resolves the `$VAR`
file variables, extracts each `prove` invocation's `from` argument, and reports
every anchor whose count in its file is not exactly 1.

Usage: python3 .cyril-v19o/oracles/anchor_check.py [name-filter]
Exit status is non-zero when any anchor is stale, ambiguous, or missing.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
SCRIPT = ROOT / ".cyril-v19o" / "oracles" / "mutations.sh"


def file_vars(text: str) -> dict[str, str]:
    """`NAME=path` assignments at the top of the script."""
    return dict(re.findall(r"^([A-Z_]+)=(crates/[^\s]+)$", text, re.MULTILINE))


def scan_quote(text: str, quote: str | None = None) -> str | None:
    """The quote character still open at the end of `text`, if any.

    Single quotes have no escapes (bash), so a `"` inside a `'…'` anchor is
    literal — which is exactly how a `#[serde(deserialize_with = "…")]` anchor
    survives this scan. Pass the previous line's result to continue a scan.
    """
    escaped = False
    for char in text:
        if escaped:
            escaped = False
            continue
        if char == "\\" and quote != "'":
            escaped = True
            continue
        if quote:
            if char == quote:
                quote = None
        elif char in "'\"":
            quote = char
    return quote


def prove_blocks(text: str) -> list[list[str]]:
    """Each `prove` invocation as its argument lines, continuations joined.

    A block ends at the first line that neither ends in a continuation
    backslash nor leaves a quote open — anchors span lines, so the backslash
    alone is not enough to find the end.
    """
    lines = text.split("\n")
    blocks: list[list[str]] = []
    current: list[str] | None = None
    for line in lines:
        if current is None:
            if line.startswith("prove "):
                current = [line[len("prove ") :]]
            continue
        current.append(line)
        if scan_quote("".join(current)) is None and not line.rstrip().endswith("\\"):
            blocks.append(current)
            current = None
    return blocks


def join_lines(block: list[str]) -> str:
    """The block's source text: continuation backslashes dropped, nothing else.

    A backslash at the end of a line continues the shell command only OUTSIDE
    quotes — inside a single-quoted anchor it is anchor text, and dropping it
    would compare a string the file never contained.
    """
    out: list[str] = []
    quote: str | None = None
    for part in block:
        if quote is None and part.endswith("\\"):
            part = part[:-1]
        quote = scan_quote(part, quote)
        out.append(part)
    return "\n".join(out)


def tokens(block: list[str]) -> list[str]:
    """Split the block's lines into arguments, keeping quoted strings whole.

    Continuation backslashes are dropped; the lines' own newlines and leading
    whitespace survive, because an anchor is compared against file text.
    """
    joined = join_lines(block)
    out: list[str] = []
    buf: list[str] = []
    quote: str | None = None
    for char in joined:
        if quote:
            if char == quote:
                quote = None
            else:
                buf.append(char)
            continue
        if char in "'\"":
            quote = char
            continue
        if char.isspace():
            if buf:
                out.append("".join(buf))
                buf = []
            continue
        buf.append(char)
    if buf:
        out.append("".join(buf))
    return out


def main() -> int:
    name_filter = sys.argv[1] if len(sys.argv) > 1 else ""
    text = SCRIPT.read_text()
    variables = file_vars(text)
    stale = 0
    checked = 0
    for block in prove_blocks(text):
        args = tokens(block)
        # prove <claim> <name> <file> <from> <to> <fence...>
        if len(args) < 5:
            print(f"UNPARSED\t{' '.join(args)[:80]}")
            stale += 1
            continue
        claim, name, file_arg, from_arg = args[0], args[1], args[2], args[3]
        if name_filter and name_filter not in name:
            continue
        checked += 1
        resolved = file_arg
        if file_arg.startswith("$"):
            resolved = variables.get(file_arg[1:], "")
            if not resolved:
                print(f"MISSING-VAR\t{claim}\t{name}\t{file_arg}")
                stale += 1
                continue
        path = ROOT / resolved
        if not path.is_file():
            print(f"NO-FILE\t{claim}\t{name}\t{resolved}")
            stale += 1
            continue
        count = path.read_text().count(from_arg)
        if count != 1:
            print(f"STALE-ANCHOR\t{claim}\t{name}\t{resolved}\toccurs {count}x")
            print(f"    from: {from_arg[:120]!r}")
            stale += 1
    print(f"checked {checked} anchors, {stale} stale")
    return 1 if stale else 0


if __name__ == "__main__":
    raise SystemExit(main())
