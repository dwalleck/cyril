import re,sys
ID=re.compile(r'(?<![\w$])[A-Za-z_$][\w$]*(?![\w$])')
KEEP=set('return if else for let const var function new typeof this null true false void await async case switch break default try catch throw in of instanceof delete do while yield class extends super import export from as continue'.split())
def n(s): return ID.sub(lambda m: m.group(0) if (m.group(0) in KEEP or len(m.group(0))>6) else '_', s)
hunks=[];cur=None
for line in open(sys.argv[1]):
    if re.match(r'^\d',line):
        cur={'h':line.strip(),'a':[],'b':[]};hunks.append(cur)
    elif line.startswith('< '): cur['a'].append(line[2:].rstrip('\n'))
    elif line.startswith('> '): cur['b'].append(line[2:].rstrip('\n'))
out=0
for h in hunks:
    a=''.join(n(x) for x in h['a']); b=''.join(n(x) for x in h['b'])
    if a==b: continue
    out+=1
    print('@@',h['h'])
    for x in h['a']: print('- '+x[:int(sys.argv[2])])
    for x in h['b']: print('+ '+x[:int(sys.argv[2])])
print('real hunks',out,'of',len(hunks),file=sys.stderr)
