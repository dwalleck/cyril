#!/usr/bin/env python3
"""cyril-a14l ORACLE for input char-wrap (design claims C2/C3, slices 5-6).

Independently computes the visual-row layout the input widget must produce:
insert the cursor block at the (byte-)cursor, split on newlines, then
char-wrap each logical line at the given cell width using East-Asian-Width
rules (W/F = 2 cells, combining marks = 0, else 1). A wide char that would
straddle the boundary moves whole to the next row.

Regenerates crates/cyril-ui/tests/fixtures/input-wrap-oracle.tsv:
    case \t width \t cursor_byte \t cursor_row \t rows (joined by U+001F)

Run: python3 .cyril-a14l/oracle-input-wrap.py > crates/cyril-ui/tests/fixtures/input-wrap-oracle.tsv
"""

import unicodedata

BLOCK = "█"
SEP = ""


def cell_width(ch: str) -> int:
    if unicodedata.combining(ch):
        return 0
    if unicodedata.east_asian_width(ch) in ("W", "F"):
        return 2
    return 1


def wrap_line(line: str, width: int) -> list[str]:
    width = max(width, 1)
    rows: list[str] = []
    current = ""
    used = 0
    for ch in line:
        w = cell_width(ch)
        if w > 0 and used + w > width and current:
            rows.append(current)
            current = ""
            used = 0
        current += ch
        used += w
    rows.append(current)
    return rows


def layout(text: str, cursor_byte: int) -> str:
    raw = text.encode("utf-8")
    cursor_byte = min(cursor_byte, len(raw))
    # Floor to a char boundary, mirroring the Rust clamp.
    while 0 < cursor_byte < len(raw) and (raw[cursor_byte] & 0xC0) == 0x80:
        cursor_byte -= 1
    prefix_chars = len(raw[:cursor_byte].decode("utf-8"))
    return text[:prefix_chars] + BLOCK + text[prefix_chars:]


CASES = [
    ("empty", "", [0]),
    ("ascii", "ascii", [0, 2, 5]),
    ("long-line", "ab " * 100, [0, 150, 300]),
    ("draft-10", "\n".join(f"draft-{i}" for i in range(1, 11)), [0, 32, 79]),
    ("wide", "世界" * 40, [0, 40, 160]),
    ("wide-straddle", "ab世cd", [0, 2, 7]),
    ("combining", "a\u0301bc", [0, 3, 5]),
]

WIDTHS = [1, 2, 3, 10, 58]


def main() -> None:
    print("case\twidth\tcursor_byte\tcursor_row\tcursor_col\trows_0x1f_joined")
    for name, text, cursors in CASES:
        for cursor in cursors:
            decorated = layout(text, cursor)
            for width in WIDTHS:
                rows: list[str] = []
                for line in decorated.split("\n"):
                    rows.extend(wrap_line(line, width))
                cursor_row = next(i for i, r in enumerate(rows) if BLOCK in r)
                cursor_col = rows[cursor_row].index(BLOCK)
                print(
                    f"{name}\t{width}\t{cursor}\t{cursor_row}\t{cursor_col}\t{SEP.join(rows)}"
                )


if __name__ == "__main__":
    main()
