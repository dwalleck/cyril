#!/usr/bin/env python3
"""carve-frames.py — inventory and carve the zstd payloads embedded in kiro-cli-chat (>= 2.23.0).

    carve-frames.py <kiro-cli-chat> --frames-out F.json [--kas-dir DIR] [--tui-out FILE.js]

Every `28 b5 2f fd` magic is decompressed FROM ITS OFFSET TO EOF (zstd stops at the frame end;
`zstandard` reports the exact consumed size), so a magic inside an incompressible RAW block
(Bun's embedded modules) can never truncate the enclosing frame. Candidates inside an earlier
frame's compressed span are recorded as nested. Kinds: tar (KAS bundle), elf (Node, Bun),
tui (`#!/usr/bin/env bun`), js, unknown.

--kas-dir extracts the tar frame there (directory must be empty or absent);
--tui-out writes the TUI frame plus a .sha256 sidecar.
Requires `pip install zstandard` (falls back to the `zstd` CLI without csize, i.e. no nesting detection).
"""
import argparse, hashlib, json, os, subprocess

MAGIC = b"\x28\xb5\x2f\xfd"
try:
    import zstandard as zstd_mod
except Exception:
    zstd_mod = None

ap = argparse.ArgumentParser()
ap.add_argument("binary")
ap.add_argument("--frames-out", required=True)
ap.add_argument("--kas-dir")
ap.add_argument("--tui-out")
args = ap.parse_args()

data = open(args.binary, "rb").read()
offs = []
i = data.find(MAGIC)
while i != -1:
    offs.append(i); i = data.find(MAGIC, i + 1)
print(f"{args.binary}: {len(data)} bytes, {len(offs)} magic candidates", flush=True)

def decode(off):
    if zstd_mod:
        d = zstd_mod.ZstdDecompressor().decompressobj()
        out = bytearray(); pos = off; csize = None
        while pos < len(data):
            chunk = data[pos:pos + (4 << 20)]
            try:
                out += d.decompress(chunk)
            except Exception:
                return None, None
            pos += len(chunk)
            if d.eof:
                csize = pos - off - len(d.unused_data); break
        return bytes(out), csize
    proc = subprocess.run(["zstd", "-dc", "-q"], input=data[off:], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    return proc.stdout, None

def classify(out):
    head = out[:48]
    if head.startswith(b"#!/usr/bin/env bun"): return "tui"
    if head.startswith(b"\x7fELF"): return "elf"
    if head[:2] == b"./" or b"ustar" in out[:512]: return "tar"
    if head[:4] in (b"var ", b"func"): return "js"
    return "unknown"

frames = []; cur_end = -1
for off in offs:
    if off < cur_end:
        frames.append({"start": off, "nested": True}); continue
    out, csize = decode(off)
    if not out or len(out) < 1024:
        continue
    kind = classify(out)
    rec = {"start": off, "csize": csize, "dsize": len(out), "kind": kind,
           "head": out[:24].hex(), "sha256": hashlib.sha256(out).hexdigest()}
    frames.append(rec)
    if csize: cur_end = off + csize
    print(f"@{off:>10} csize={csize} dsize={len(out):>10} {kind}", flush=True)
    if kind == "tar" and args.kas_dir:
        os.makedirs(args.kas_dir, exist_ok=True)
        if os.listdir(args.kas_dir):
            print(f"  {args.kas_dir} is not empty; tar not extracted")
        else:
            subprocess.run(["tar", "-x", "-C", args.kas_dir], input=out, check=True)
            print(f"  extracted KAS tar -> {args.kas_dir}")
    elif kind == "tui" and args.tui_out:
        with open(args.tui_out, "wb") as f: f.write(out)
        with open(args.tui_out + ".sha256", "w") as f: f.write(rec["sha256"] + "\n")
        print(f"  wrote TUI bundle -> {args.tui_out}")

json.dump(frames, open(args.frames_out, "w"), indent=1)
top = [f for f in frames if not f.get("nested")]
print(f"top-level frames: {len(top)}  nested: {len(frames) - len(top)}  -> {args.frames_out}")
