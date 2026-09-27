#!/usr/bin/env python3
"""cyril-ulhs: list the diagnostics of interest from `cargo clippy --message-format=json` output.

Usage: probe-diag.py <cargo-json-log> [<code> ...]
Default codes: clippy::expect_used clippy::unwrap_used unfulfilled_lint_expectations

Prints one line per primary span: `<code> | <file>:<line> | <message>`, sorted, deduplicated.
Throwaway instrument for evidence.md; not part of the build.
"""

import json
import sys

DEFAULT_CODES = ("clippy::expect_used", "clippy::unwrap_used", "unfulfilled_lint_expectations")


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    path = sys.argv[1]
    codes = tuple(sys.argv[2:]) or DEFAULT_CODES
    rows = set()
    with open(path, encoding="utf-8") as fh:
        for raw in fh:
            raw = raw.strip()
            if not raw.startswith("{"):
                continue
            try:
                obj = json.loads(raw)
            except json.JSONDecodeError:
                continue
            if obj.get("reason") != "compiler-message":
                continue
            msg = obj.get("message") or {}
            code = (msg.get("code") or {}).get("code")
            if code not in codes:
                continue
            spans = [s for s in msg.get("spans", []) if s.get("is_primary")] or msg.get("spans", [])
            if not spans:
                rows.add((code, "<no span>", msg.get("message", "")))
                continue
            for span in spans:
                rows.add((code, f"{span.get('file_name')}:{span.get('line_start')}", msg.get("message", "")))
    for code, loc, text in sorted(rows):
        print(f"{code} | {loc} | {text}")
    print(f"# {len(rows)} diagnostic(s) for codes {', '.join(codes)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
