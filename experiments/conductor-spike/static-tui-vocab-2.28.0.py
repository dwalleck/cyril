#!/usr/bin/env python3
"""Static TUI-bundle vocabulary differ (kiro-cli 2.26.0 -> 2.28.0 audit).

Kiro's v2 TUI ships as a minified bun bundle (`tui.js`). Identifiers are mangled
between builds, but STRING LITERALS and PROPERTY NAMES survive minification. This
script lexes each bundle (a small JS tokenizer: strings, nested template literals,
comments, regex literals) and diffs both vocabularies as SETS across consecutive
release pairs, so new wire surface is found without grepping a fixed token list.

Usage:
  python3 -I static-tui-vocab-2.28.0.py OUTDIR BUNDLE_A BUNDLE_B [BUNDLE_C ...]

Outputs (per consecutive pair A->B) under OUTDIR:
  <a>__<b>.strings.added / .removed  -- literals absent (as raw text) from the other bundle
  <a>__<b>.props.added  / .removed   -- property names (`.x` / `{x:` / `,x:`) likewise
  <a>__<b>.buckets.txt               -- added/removed literals bucketed by wire-relevant shape
  <a>__<b>.context.txt               -- +/-300 chars of context for bucketed adds
Bundles are read-only inputs; nothing is executed.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

KEYWORDS_BEFORE_REGEX = {
    "return", "typeof", "case", "do", "else", "in", "of", "new", "delete", "void",
    "throw", "instanceof", "yield", "await",
}
CODE_CHUNK = re.compile(r'[^"\'`/{}]+')
IDENT_TAIL = re.compile(r'([A-Za-z_$][\w$]*)\s*$')
PROP_RE = re.compile(r'(?:\?\.|\.)([A-Za-z_$][\w$]{2,60})\b|[{,]\s*([A-Za-z_$][\w$]{2,60})\s*:')


class Lexed:
    def __init__(self) -> None:
        self.strings: set[str] = set()
        self.code: list[str] = []


def _regex_allowed(prev: str) -> bool:
    """prev = code text immediately before the '/' (already rstripped)."""
    if not prev:
        return True
    c = prev[-1]
    if c in "(,=:[!&|?{};+-*%<>~^":
        return True
    if c.isalnum() or c in "_$":
        m = IDENT_TAIL.search(prev)
        return bool(m and m.group(1) in KEYWORDS_BEFORE_REGEX)
    return False  # ) ] } . etc -> division


def lex(text: str) -> Lexed:
    out = Lexed()
    n = len(text)
    i = 0
    brace: list[str] = []  # 'b' = code brace, 't' = template ${ }
    prev_code = ""  # tail of recent code for regex disambiguation
    tpl_pending = False

    def scan_template(i: int) -> int:
        """i points just after a backtick or a template-closing '}'; returns index after
        the template part terminator; pushes 't' if it stopped at '${'."""
        buf = []
        while i < n:
            ch = text[i]
            if ch == "\\":
                buf.append(text[i:i + 2])
                i += 2
                continue
            if ch == "`":
                part = "".join(buf).strip()
                if part:
                    out.strings.add(part)
                return i + 1
            if ch == "$" and i + 1 < n and text[i + 1] == "{":
                part = "".join(buf).strip()
                if part:
                    out.strings.add(part)
                brace.append("t")
                return i + 2
            buf.append(ch)
            i += 1
        return i

    while i < n:
        m = CODE_CHUNK.match(text, i)
        if m:
            chunk = m.group(0)
            out.code.append(chunk)
            prev_code = (prev_code + chunk)[-64:]
            i = m.end()
            continue
        ch = text[i]
        if ch in "\"'":
            j = i + 1
            while j < n:
                c = text[j]
                if c == "\\":
                    j += 2
                    continue
                if c == ch or c == "\n":
                    break
                j += 1
            s = text[i + 1:j]
            if len(s) <= 2000:
                out.strings.add(s)
            out.code.append('""')
            prev_code = (prev_code + '""')[-64:]
            i = j + 1
        elif ch == "`":
            i = scan_template(i + 1)
            out.code.append('""')
            prev_code = (prev_code + '""')[-64:]
        elif ch == "{":
            brace.append("b")
            out.code.append("{")
            prev_code = (prev_code + "{")[-64:]
            i += 1
        elif ch == "}":
            top = brace.pop() if brace else "b"
            if top == "t":
                i = scan_template(i + 1)
                out.code.append('""')
                prev_code = (prev_code + '""')[-64:]
            else:
                out.code.append("}")
                prev_code = (prev_code + "}")[-64:]
                i += 1
        elif ch == "/":
            nxt = text[i + 1] if i + 1 < n else ""
            if nxt == "/":
                j = text.find("\n", i)
                i = n if j < 0 else j
            elif nxt == "*":
                j = text.find("*/", i + 2)
                i = n if j < 0 else j + 2
            elif _regex_allowed(prev_code.rstrip()):
                j = i + 1
                in_cls = False
                while j < n:
                    c = text[j]
                    if c == "\\":
                        j += 2
                        continue
                    if c == "\n":
                        break
                    if in_cls:
                        if c == "]":
                            in_cls = False
                    elif c == "[":
                        in_cls = True
                    elif c == "/":
                        break
                    j += 1
                j += 1
                while j < n and (text[j].isalpha()):
                    j += 1
                out.code.append("/r/")
                prev_code = (prev_code + "/r/")[-64:]
                i = j
            else:
                out.code.append("/")
                prev_code = (prev_code + "/")[-64:]
                i += 1
        else:  # unreachable given CODE_CHUNK, kept for safety
            out.code.append(ch)
            i += 1
    _ = tpl_pending
    return out


BUCKETS: list[tuple[str, re.Pattern[str]]] = [
    ("method:_kiro/kiro.dev", re.compile(r'^_?kiro(\.dev)?/[\w./-]+$')),
    ("method:acp", re.compile(r'^(session|fs|terminal|authenticate|initialize)(/[\w_]+)+$')),
    ("path-like a/b", re.compile(r'^[a-z][\w-]*(/[a-zA-Z_][\w-]*){1,4}$')),
    ("env-like", re.compile(r'^(KIRO|Q|AMAZON_Q|AWS|NODE|BUN)_[A-Z0-9_]{2,}$')),
    ("setting:dotted", re.compile(r'^[a-z][A-Za-z]{1,20}\.[a-z][A-Za-z]{1,40}(\.[a-z][A-Za-z]{1,40})?$')),
    ("SCREAMING_CONST", re.compile(r'^[A-Z][A-Z0-9]*(_[A-Z0-9]+){1,8}$')),
    ("slash-command", re.compile(r'^/[a-z][\w-]{1,40}$')),
    ("snake_kind", re.compile(r'^[a-z][a-z0-9]*(_[a-z0-9]+){1,6}$')),
    ("kebab-id", re.compile(r'^[a-z][a-z0-9]*(-[a-z0-9]+){1,6}$')),
    ("camelIdent", re.compile(r'^[a-z][a-z0-9]*([A-Z][a-z0-9]*){1,8}$')),
]
ENV_RE = re.compile(r'process\.env\.([A-Z][A-Z0-9_]{2,80})|process\.env\[\s*"([A-Z][A-Z0-9_]{2,80})"\s*\]')


def ctx(text: str, needle: str, width: int = 300, limit: int = 2) -> list[str]:
    res = []
    for q in (f'"{needle}"', f"'{needle}'", needle):
        start = 0
        while len(res) < limit:
            i = text.find(q, start)
            if i < 0:
                break
            res.append(text[max(0, i - width): i + len(q) + width].replace("\n", " "))
            start = i + len(q)
        if res:
            break
    return res


def ver(p: Path) -> str:
    m = re.search(r'(\d+\.\d+\.\d+)', p.name)
    return m.group(1) if m else p.stem


def main() -> int:
    if len(sys.argv) < 4:
        print(__doc__)
        return 2
    outdir = Path(sys.argv[1])
    outdir.mkdir(parents=True, exist_ok=True)
    bundles = [Path(a) for a in sys.argv[2:]]
    texts: dict[Path, str] = {}
    lits: dict[Path, set[str]] = {}
    prs: dict[Path, set[str]] = {}
    evs: dict[Path, set[str]] = {}
    for b in bundles:
        t = b.read_text(encoding="utf-8", errors="replace")
        texts[b] = t
        lx = lex(t)
        lits[b] = lx.strings
        code = "\n".join(lx.code)
        prs[b] = {m.group(1) or m.group(2) for m in PROP_RE.finditer(code)}
        evs[b] = {m.group(1) or m.group(2) for m in ENV_RE.finditer(t)}
        (outdir / f"{ver(b)}.strings").write_text("\n".join(sorted(s.replace("\n", "\\n") for s in lits[b])) + "\n")
        (outdir / f"{ver(b)}.props").write_text("\n".join(sorted(prs[b])) + "\n")
        print(f"{ver(b)}: {len(lits[b])} literals, {len(prs[b])} props, {len(evs[b])} env reads", flush=True)
    for a, b in zip(bundles, bundles[1:]):
        tag = f"{ver(a)}__{ver(b)}"
        ta, tb = texts[a], texts[b]
        # "genuinely new" = literal absent as raw text from the other bundle (defeats
        # re-tokenisation noise); same for props.
        added = sorted(s for s in lits[b] - lits[a] if s not in ta)
        removed = sorted(s for s in lits[a] - lits[b] if s not in tb)
        padd = sorted(p for p in prs[b] - prs[a] if p not in ta)
        prem = sorted(p for p in prs[a] - prs[b] if p not in tb)
        esc = lambda xs: "\n".join(x.replace("\n", "\\n") for x in xs) + "\n"  # noqa: E731
        (outdir / f"{tag}.strings.added").write_text(esc(added))
        (outdir / f"{tag}.strings.removed").write_text(esc(removed))
        (outdir / f"{tag}.props.added").write_text(esc(padd))
        (outdir / f"{tag}.props.removed").write_text(esc(prem))
        eadd = sorted(evs[b] - evs[a])
        erem = sorted(evs[a] - evs[b])
        lines = [f"# {tag}: +{len(added)} -{len(removed)} literals; +{len(padd)} -{len(prem)} props",
                 f"## env reads added: {eadd}", f"## env reads removed: {erem}"]
        cl = []
        for name, rx in BUCKETS:
            hits = [s for s in added if rx.match(s)]
            lines.append(f"## {name} (+{len(hits)})")
            lines.extend(f"  {h}" for h in hits)
            for h in hits:
                for c in ctx(tb, h):
                    cl.append(f"=== [{name}] {h}\n{c}\n")
            rh = [s for s in removed if rx.match(s)]
            lines.append(f"## {name} REMOVED (-{len(rh)})")
            lines.extend(f"  -{h}" for h in rh)
        for e in eadd:
            for c in ctx(tb, e):
                cl.append(f"=== [env] {e}\n{c}\n")
        (outdir / f"{tag}.buckets.txt").write_text("\n".join(lines) + "\n")
        (outdir / f"{tag}.context.txt").write_text("\n".join(cl))
        print(f"{tag}: +{len(added)} -{len(removed)} literals, +{len(padd)} -{len(prem)} props, env +{eadd} -{erem}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
