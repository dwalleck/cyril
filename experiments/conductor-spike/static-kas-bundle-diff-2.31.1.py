#!/usr/bin/env python3
"""Structural diff of two minified KAS acp-server.js bundles.

    python3 -I static-kas-bundle-diff-2.31.1.py OLD.js NEW.js OUTDIR

Lexes each bundle (strings, template literals with nested ${}, comments, regex
literals by heuristic), splits into "statements" after ; { } tokens, and writes
  OUTDIR/old.raw  OUTDIR/new.raw    one statement per line, original text
  OUTDIR/old.norm OUTDIR/new.norm   same lines with minifier-renameable
                                    identifiers replaced by `$` (property names
                                    after `.`, object keys before `:`, keywords
                                    and string contents are preserved)
Then run `diff old.norm new.norm` and map hunks back to *.raw line numbers.
Executes nothing from the bundle.
"""
import re, sys
from pathlib import Path

KW = set('''break case catch class const continue debugger default delete do else export extends
false finally for function if import in instanceof new null return super switch this throw true try
typeof var void while with yield let static async await of get set undefined NaN Infinity
Object Array String Number Boolean Promise Map Set Symbol JSON Math Date Error RegExp process
console require module exports globalThis Buffer'''.split())
REGEX_PREV = set('( , = : [ ! & | ? { } ; + - * % < > ~ ^ return typeof case do else in of new delete void throw yield await =>'.split()) | {'&&','||','??','==','===','!=','!==','<=','>=','+=','-=','*=','/=','%=','&=','|=','^=','??=','||=','&&=','**','<<','>>','>>>','=>'}
ID = re.compile(r'[A-Za-z_$\u0080-￿][A-Za-z0-9_$\u0080-￿]*')
NUM = re.compile(r'0[xX][0-9a-fA-F_]+n?|0[bB][01_]+n?|0[oO][0-7_]+n?|(?:\d[\d_]*\.?[\d_]*|\.\d[\d_]*)(?:[eE][+-]?\d+)?n?')
PUNCT = sorted('>>>= ... === !== **= <<= >>= >>> ??= ||= &&= => == != <= >= && || ?? ?. ++ -- += -= *= /= %= &= |= ^= ** << >>'.split(), key=len, reverse=True)

def lex(s):
    i, n = 0, len(s)
    toks = []  # (kind, text)
    prev = None  # last significant token text
    stack = []   # template nesting: brace depth counters
    while i < n:
        c = s[i]
        if c in ' \t\r\n':
            j = i
            while j < n and s[j] in ' \t\r\n': j += 1
            if '\n' in s[i:j]: toks.append(('nl', '\n'))
            i = j; continue
        if s.startswith('//', i):
            j = s.find('\n', i); j = n if j < 0 else j
            i = j; continue
        if s.startswith('/*', i):
            j = s.find('*/', i + 2); j = n if j < 0 else j + 2
            i = j; continue
        if c in '"\'':
            j = i + 1
            while j < n and s[j] != c:
                if s[j] == '\\': j += 1
                elif s[j] == '\n': break
                j += 1
            toks.append(('str', s[i:j+1])); prev = 'str'; i = j + 1; continue
        if c == '`' or (c == '}' and stack and stack[-1] == 0):
            # template literal chunk: from ` or } (closing ${) to next ${ or `
            if c == '}': stack.pop()
            j = i + 1
            while j < n:
                if s[j] == '\\': j += 2; continue
                if s[j] == '`': j += 1; break
                if s.startswith('${', j): j += 2; stack.append(0); break
                j += 1
            toks.append(('tpl', s[i:j])); prev = 'tpl'; i = j; continue
        if c == '/' and (prev is None or prev in REGEX_PREV):
            j = i + 1; cls = False
            while j < n:
                ch = s[j]
                if ch == '\\': j += 2; continue
                if ch == '[': cls = True
                elif ch == ']': cls = False
                elif ch == '/' and not cls: break
                elif ch == '\n': break
                j += 1
            j += 1
            while j < n and (s[j].isalnum()): j += 1
            toks.append(('re', s[i:j])); prev = 're'; i = j; continue
        m = ID.match(s, i)
        if m:
            t = m.group(0); toks.append(('id', t)); prev = t if t in KW else 'id'; i = m.end(); continue
        if c.isdigit() or (c == '.' and i + 1 < n and s[i+1].isdigit()):
            m = NUM.match(s, i); t = m.group(0) if m and m.group(0) else c
            toks.append(('num', t)); prev = 'num'; i += len(t); continue
        for p in PUNCT:
            if s.startswith(p, i):
                toks.append(('p', p)); prev = p; i += len(p); break
        else:
            if c == '{' and stack: stack[-1] += 1
            elif c == '}' and stack: stack[-1] -= 1
            toks.append(('p', c)); prev = c; i += 1
    return toks

def render(toks):
    raw, norm = [], []
    cur_r, cur_n = [], []
    last = None
    for k, (kind, t) in enumerate(toks):
        if kind == 'nl':
            continue
        nr = t
        if kind == 'id' and t not in KW:
            after_dot = last in ('.', '?.')
            nxt = None
            for kk in range(k + 1, min(k + 3, len(toks))):
                if toks[kk][0] != 'nl': nxt = toks[kk][1]; break
            is_key = nxt == ':' and last in ('{', ',')
            if not (after_dot or is_key): nr = '$'
        sep = ' ' if (cur_r and kind in ('id', 'num') and toks[k-1][0] in ('id', 'num')) else ''
        cur_r.append(sep + t); cur_n.append(sep + nr)
        last = t
        if kind == 'p' and t in (';', '{', '}'):
            raw.append(''.join(cur_r)); norm.append(''.join(cur_n)); cur_r, cur_n = [], []
    if cur_r: raw.append(''.join(cur_r)); norm.append(''.join(cur_n))
    return raw, norm

def main():
    old, new, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    out.mkdir(parents=True, exist_ok=True)
    for tag, p in (('old', old), ('new', new)):
        raw, norm = render(lex(p.read_text(encoding='utf-8', errors='replace')))
        (out / f'{tag}.raw').write_text('\n'.join(r.replace('\n', '\\n') for r in raw) + '\n')
        (out / f'{tag}.norm').write_text('\n'.join(r.replace('\n', '\\n') for r in norm) + '\n')
        print(tag, len(raw), 'statements')

if __name__ == '__main__':
    main()
