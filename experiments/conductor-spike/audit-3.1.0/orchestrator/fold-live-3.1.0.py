import pathlib, re, sys
p=pathlib.Path("docs/kiro-3.1.0-wire-audit.md"); s=p.read_text(); miss=[]
def rep(old,new):
    global s
    if old in s: s=s.replace(old,new,1)
    else: miss.append(old[:100])
exec(open(sys.argv[1]).read())   # the replacement list
p.write_text(s); print("written;", len(s.splitlines()), "lines; misses:", len(miss)); [print("  MISS:", m) for m in miss]
