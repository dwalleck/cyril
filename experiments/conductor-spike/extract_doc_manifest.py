#!/usr/bin/env python3
"""Extract the embedded product-doc manifest(s) from a kiro-cli-chat binary.

kiro-cli-chat embeds one or more build-time documentation indexes shaped
`{generated_at, total_docs, by_category?, documents:[...]}`. Each documents[]
node = {path,file,title,description,category,keywords,related,validated,status,...}.
This is a build-time feature inventory and a SUPERSET of the public kiro.dev docs,
so it leaks unannounced features. See reference_kiro_embedded_docs_corpus.md and the
doc-manifest addendum in reference_kiro_wire_audit_methodology.md.

Usage: extract_doc_manifest.py <path-to-kiro-cli-chat> <out-prefix>
Writes <out-prefix>-<n>.json per manifest found and <out-prefix>-merged.json
(deduped union of all documents[] keyed by path).
"""
import json, sys, re

BIN = sys.argv[1]
OUT = sys.argv[2]

data = open(BIN, "rb").read()
needle = b'"documents":'
offsets = [m.start() for m in re.finditer(re.escape(needle), data)]
print(f"found {len(offsets)} '\"documents\":' occurrences")

def carve_object(buf, doc_kw_pos):
    """Walk backwards to the enclosing '{', then string-aware brace-match forward."""
    # find the '{' that opens the object containing this "documents": key
    i = doc_kw_pos
    depth = 0
    start = None
    while i >= 0:
        c = buf[i]
        if c == 0x7d:  # }
            depth += 1
        elif c == 0x7b:  # {
            if depth == 0:
                start = i
                break
            depth -= 1
        elif c < 0x09 and c != 0x00:  # hit a control byte that won't be in JSON
            break
        i -= 1
    if start is None:
        return None
    # forward string-aware brace match
    depth = 0
    in_str = False
    esc = False
    j = start
    n = len(buf)
    while j < n:
        c = buf[j]
        if in_str:
            if esc:
                esc = False
            elif c == 0x5c:  # backslash
                esc = True
            elif c == 0x22:  # "
                in_str = False
        else:
            if c == 0x22:
                in_str = True
            elif c == 0x7b:
                depth += 1
            elif c == 0x7d:
                depth -= 1
                if depth == 0:
                    return buf[start:j+1]
            elif c < 0x09 and c != 0x00:
                return None  # control byte outside string => not JSON
        j += 1
    return None

manifests = []
seen_spans = set()
for off in offsets:
    blob = carve_object(data, off)
    if not blob:
        continue
    try:
        text = blob.decode("utf-8")
    except UnicodeDecodeError:
        text = blob.decode("utf-8", "replace")
    try:
        obj = json.loads(text)
    except json.JSONDecodeError:
        continue
    if not isinstance(obj, dict) or "documents" not in obj:
        continue
    docs = obj.get("documents")
    if not isinstance(docs, list) or not docs:
        continue
    key = (obj.get("total_docs"), len(docs), obj.get("generated_at"))
    if key in seen_spans:
        continue
    seen_spans.add(key)
    manifests.append(obj)

print(f"parsed {len(manifests)} distinct manifest object(s)")
merged = {}
for k, obj in enumerate(manifests):
    docs = obj["documents"]
    path = f"{OUT}-{len(docs)}.json"
    with open(path, "w") as f:
        json.dump(obj, f, indent=2, sort_keys=True)
    print(f"  manifest[{k}]: total_docs={obj.get('total_docs')} documents={len(docs)} "
          f"generated_at={obj.get('generated_at')} -> {path}")
    for d in docs:
        if isinstance(d, dict):
            merged[d.get("path") or d.get("file") or json.dumps(d, sort_keys=True)] = d

with open(f"{OUT}-merged.json", "w") as f:
    json.dump(merged, f, indent=2, sort_keys=True)
print(f"merged union: {len(merged)} unique documents -> {OUT}-merged.json")
