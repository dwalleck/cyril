#!/usr/bin/env python3
"""Static, read-only Kiro 2.21.2 vs 2.21.1 release audit.

This script never runs a Kiro binary and never touches ~/.kiro. It extracts
strings, the embedded gzip KAS tar, selected registries, and documentation
manifests into a version-specific research directory. Inputs are explicit
binary paths; output is a JSON report plus carved bundles/manifests.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zlib
from pathlib import Path

OLD = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/home/dwalleck/.local/share/kiro-research/binaries/2.21.1/kiro-cli-chat")
NEW = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("/home/dwalleck/.local/share/kiro-research/binaries/2.21.2/kiro-cli-chat")
OUT = Path(sys.argv[3]) if len(sys.argv) > 3 else Path("experiments/conductor-spike/static-2.21.2")
OUT.mkdir(parents=True, exist_ok=True)
CARVE = Path("/home/dwalleck/.local/share/kiro-research/kas-carves/2.21.2")
CARVE.mkdir(parents=True, exist_ok=True)


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


def strings(path: Path) -> list[str]:
    # GNU strings is available on the audit workstation and avoids loading
    # another full copy of the 800+ MB executable in Python.
    p = subprocess.run(["strings", "-a", "-n", "4", str(path)], check=True,
                       capture_output=True, text=True, errors="replace")
    return p.stdout.splitlines()


def vocab(lines: list[str], pattern: str) -> list[str]:
    rx = re.compile(pattern)
    return sorted({m.group(0) for line in lines for m in rx.finditer(line)})


def gzip_carve(binary: Path, version: str) -> dict:
    data = binary.read_bytes()
    magic = b"\x1f\x8b\x08\x08"
    starts = [m.start() for m in re.finditer(re.escape(magic), data)]
    records = []
    for start in starts:
        # FNAME for the KAS asset is intentionally verified before inflate.
        fname_end = data.find(b"\0", start + 10)
        if fname_end < 0 or data[start + 10:fname_end] != b"kas-bundle.tar":
            continue
        try:
            dec = zlib.decompressobj(31)
            tar_bytes = dec.decompress(data[start:]) + dec.flush()
        except zlib.error:
            continue
        if not tar_bytes.startswith(b"./") and not tar_bytes.startswith(b"node_modules/"):
            # tarfile can still parse other prefixes, so only reject clearly
            # unrelated gzip streams by requiring a valid tar header below.
            pass
        dest = CARVE / version
        if dest.exists():
            shutil.rmtree(dest)
        dest.mkdir(parents=True)
        tar_path = dest / "kas-bundle.tar"
        tar_path.write_bytes(tar_bytes)
        with tarfile.open(tar_path) as tf:
            tf.extractall(dest / "tree", filter="data")
            names = tf.getnames()
        bundle = next((dest / "tree" / n for n in names if n.endswith("acp-server.js")), None)
        rec = {"offset": start, "gzip_filename": "kas-bundle.tar",
               "tar_bytes": len(tar_bytes), "tar_sha256": sha(tar_path),
               "members": len(names), "root": str(dest),
               "acp_server": str(bundle) if bundle else None}
        records.append(rec)
        break
    if not records:
        raise RuntimeError(f"no kas-bundle.tar gzip stream in {binary}")
    return records[0]


def brace_object(data: bytes, needle: bytes) -> dict | None:
    at = data.find(needle)
    if at < 0:
        return None
    # Find a nearby opening brace and then match strings/escapes exactly.
    start = data.rfind(b"{", 0, at)
    if start < 0:
        return None
    depth, in_str, esc = 0, False, False
    for i in range(start, len(data)):
        c = data[i]
        if in_str:
            if esc:
                esc = False
            elif c == 92:
                esc = True
            elif c == 34:
                in_str = False
            continue
        if c == 34:
            in_str = True
        elif c == 123:
            depth += 1
        elif c == 125:
            depth -= 1
            if depth == 0:
                try:
                    return json.loads(data[start:i + 1])
                except json.JSONDecodeError:
                    return None
    return None


def manifest_objects(data: bytes) -> list[dict]:
    out, spans = [], set()
    for m in re.finditer(b'"documents":', data):
        start = data.rfind(b"{", 0, m.start())
        if start < 0:
            continue
        # The embedded manifest is ASCII/UTF-8 JSON; brace match while
        # respecting escaped quoted strings.
        depth, in_str, esc = 0, False, False
        for i in range(start, len(data)):
            c = data[i]
            if in_str:
                if esc:
                    esc = False
                elif c == 92:
                    esc = True
                elif c == 34:
                    in_str = False
                continue
            if c == 34:
                in_str = True
            elif c == 123:
                depth += 1
            elif c == 125:
                depth -= 1
                if depth == 0:
                    if (start, i) in spans:
                        break
                    spans.add((start, i))
                    try:
                        obj = json.loads(data[start:i + 1])
                    except json.JSONDecodeError:
                        break
                    if isinstance(obj, dict) and isinstance(obj.get("documents"), list):
                        out.append(obj)
                    break
    return out


def quoted_method_literals(data: bytes) -> list[str]:
    """Extract only quote-delimited ACP method strings from raw bytes."""
    return sorted({
        value.decode("ascii")
        for value in re.findall(rb'"((?:_kiro|kiro\.dev)/[A-Za-z0-9_./:-]+)"', data)
    })


def collect(version: str, binary: Path) -> dict:
    lines = strings(binary)
    data = binary.read_bytes()
    rollout = brace_object(data, b'"cloud_config": {')
    out = {"version": version, "binary": str(binary), "bytes": binary.stat().st_size,
           "sha256": sha(binary),
           "quoted_method_literals": quoted_method_literals(data),
           # These remain unquoted string leads and are never used as a
           # method census; KAS's precise census is in static-kas-surface.
           "acp_method_literal_leads": vocab(lines, r"(?:_?kiro(?:\.dev)?|kiro\.dev|_kiro)/(?:[A-Za-z0-9_./-]+)"),
           "env_literals": vocab(lines, r"KIRO_[A-Z0-9_]+"),
           "host_symbols": sorted({line for line in lines if re.search(r"(?:chat_cli|chat_cli_v2|kiro_telemetry|launch::|pinned_bin|rollout|experiment)", line)}),
           "changelog_leads": [line for line in lines if any(x in line.lower() for x in ("changelog", "what's new", "release notes"))][:200]}
    out["rust_rollout_registry"] = rollout
    out["doc_manifests"] = []
    for idx, obj in enumerate(manifest_objects(data)):
        p = OUT / f"static-{version}-doc-manifest-{idx}.json"
        p.write_text(json.dumps(obj, indent=2, sort_keys=True), encoding="utf-8")
        out["doc_manifests"].append({"path": str(p), "generated_at": obj.get("generated_at"),
                                     "total_docs": obj.get("total_docs"),
                                     "documents": len(obj.get("documents", []))})
    out["kas"] = gzip_carve(binary, version)
    kas_path = Path(out["kas"]["acp_server"] or "")
    if kas_path.exists():
        kas = kas_path.read_text(encoding="utf-8", errors="replace")
        out["kas_methods"] = sorted(set(re.findall(r"_kiro/[A-Za-z0-9_./-]+", kas)))
        out["kas_env_literals"] = sorted(set(re.findall(r"KIRO_[A-Z0-9_]+", kas)))
        out["kas_version_leads"] = sorted(set(re.findall(r"(?:version|VERSION|@kiro/agent)[^\n]{0,120}", kas)))[:200]
        (OUT / f"static-{version}-kas-acp-server.sha256").write_text(sha(kas_path) + "  " + str(kas_path) + "\n")
    return out


reports = {"old": collect("2.21.1", OLD), "new": collect("2.21.2", NEW)}
# Set comparisons are the useful static evidence, not raw minified line counts.
reports["delta"] = {}
for key in ("quoted_method_literals", "acp_method_literal_leads", "env_literals", "host_symbols", "kas_methods", "kas_env_literals"):
    a, b = set(reports["old"].get(key, [])), set(reports["new"].get(key, []))
    reports["delta"][key] = {"added": sorted(b - a), "removed": sorted(a - b),
                              "old_count": len(a), "new_count": len(b)}
(OUT / "static-2.21.2-report.json").write_text(json.dumps(reports, indent=2, sort_keys=True), encoding="utf-8")
print(json.dumps({"old": {"sha256": reports["old"]["sha256"], "bytes": reports["old"]["bytes"]},
                  "new": {"sha256": reports["new"]["sha256"], "bytes": reports["new"]["bytes"]},
                  "delta": reports["delta"]}, indent=2))
