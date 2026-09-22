#!/usr/bin/env python3
"""Deterministic data movement for the `code-review-max` KAS workflow.

The workflow's LLM steps make judgment calls only (find candidates, decide
duplicates, verify, rank). Every mechanical transformation lives here, so a
candidate's text is written once by the finder that raised it and never
retyped by a later model.

Run directory layout (all paths relative to <rundir>):

    manifest.json            gather   changed files + per-file patch index
    diff.patch               gather   full unified diff (can be large)
    changed-files.txt        gather
    patches/NNN.patch        gather   one unified diff per changed file
    facts/symbols.json       gather   every symbol the diff adds or changes + its usages
    facts/usages-N.txt       gather   the same, one read per page
    facts/diagnostics.txt    diagnostics  the repo's check command, run ONCE for the review
    candidates/<angle>.json  finders  {"angle": ..., "candidates": [...]}
    candidates/all.json      merge    every candidate, provisional ids
    candidates/raw/<pid>.json merge   one full record per candidate (for lookups)
    candidates/digest-N.txt  merge    one line per candidate, sorted by location
    deduped/decisions.json   dedup    {"groups": [{"pids": [...], "keep"?, "reason"}]}
    deduped/index.json       shard    surviving candidates, final ids C01..
    deduped/Cnn.json         shard    one record per candidate
    deduped/digest-N.txt     shard    one line per surviving candidate
    queues/queue-K.json      shard    {"done": bool, "ids": [ids]}   ids are IMMUTABLE
    candidates/sweep.json    sweep    gap-sweep candidates, ids S01..
    queues/queue-sweep.json  sweep
    verdicts/<id>.json       verify   {"id","verdict","evidence","reasoning",...}
    ballots.json             ballots  which candidates get two more votes, and why
    deduped/<id>.v2|v3.json  ballots  the same record under a ballot id
    queues/queue-r1|r2.json  ballots  the second and third votes, one loop each
    verified.json            collate  kept / refuted / stats
    verified-digest-N.txt    collate  one line per kept finding
    ranking.json             rank     {"order": [ids], "notes": {id: str}}
    findings.json            finalize top findings, most severe first
    report.md                finalize human-readable report
    comments/brief-N.txt     finalize everything a commenter needs about each reported finding
    comments/<id>.json       comment  {label, decorations, subject, discussion} | {duplicate_of}
    comments.json            comments postable review comments: file, line, body (Conventional Comments)
    comments.md              comments the same, for reading

Digests exist because a tool result over ~30,000 characters is truncated: the
JSON files above routinely exceed that, and a step that cannot read its input
in one call reaches for ad-hoc shell. Every digest page fits in a single read.

A queue's `ids` never change. "Pending" is derived, by the verifier, from which
verdicts/<id>.json exist -- so a skipped id is picked up by the next iteration
instead of being lost, and the only mutation a queue ever sees is `done`.

Code intelligence is computed ONCE per review, here, not per session: a usage
map for every changed symbol (grep-grade: `git grep -w`, so comments and
same-named symbols are included) and, optionally, the repo's own check command.

A REFUTED verdict and any conventions claim get two more independent votes
(`ballots`); `collate` tallies them and REFUTED stands only with 2 of 3.

Every reported finding ends as a postable review comment in the Conventional
Comments format (conventionalcomments.org): a model chooses the label and writes
the prose, `comments` validates the vocabulary and renders the body, and a finding
the model skipped gets a template comment rather than none.

Subcommands: gather, facts, diagnostics, merge, shard, ballots, collate, finalize, comments.
"""
import argparse
import collections
import datetime
import json
import os
import re
import shlex
import subprocess
import sys
import time

MAX_PER_ANGLE = 8
MAX_FINDINGS = 15
VERDICTS = ("CONFIRMED", "PLAUSIBLE", "REFUTED")


def die(msg, code=2):
    print(f"crtool: error: {msg}", file=sys.stderr)
    sys.exit(code)


def read_json(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def write_json(path, obj):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(obj, f, indent=2, ensure_ascii=False)
        f.write("\n")
    os.replace(tmp, path)


PAGE_BUDGET = 20000  # chars per digest page; the tool-result cap is ~30,000


def one_line(text, limit):
    t = " ".join(str(text or "").split()).replace("|", "/")
    return t if len(t) <= limit else t[:limit - 1].rstrip() + "…"


def write_pages(run, stem, header, lines):
    """Write `lines` to <stem>-N.txt pages that each fit one tool read. Returns rel paths."""
    for old in sorted(os.listdir(os.path.dirname(os.path.join(run, stem)) or run)):
        if re.fullmatch(re.escape(os.path.basename(stem)) + r"-\d+\.txt", old):
            os.remove(os.path.join(os.path.dirname(os.path.join(run, stem)), old))
    pages, cur, size = [], [], 0
    for ln in lines:
        if cur and size + len(ln) + 1 > PAGE_BUDGET:
            pages.append(cur)
            cur, size = [], 0
        cur.append(ln)
        size += len(ln) + 1
    pages.append(cur)
    rels = []
    for n, body in enumerate(pages, 1):
        rel = f"{stem}-{n}.txt"
        with open(os.path.join(run, rel), "w", encoding="utf-8") as f:
            f.write(f"# {header} -- page {n} of {len(pages)}\n" + "\n".join(body) + "\n")
        rels.append(rel)
    return rels


def where(c):
    return f"{c.get('file', '?')}:{c.get('line') if c.get('line') is not None else '?'}"


def git_bytes(*args):
    r = subprocess.run(["git", *args], capture_output=True)
    if r.returncode != 0:
        die(f"git {' '.join(args)} failed: {r.stderr.decode(errors='replace').strip()}")
    return r.stdout


def git_text(*args):
    return git_bytes(*args).decode(errors="replace")


def rev_ok(rev):
    r = subprocess.run(["git", "rev-parse", "--verify", "--quiet", rev + "^{commit}"],
                       capture_output=True)
    return r.returncode == 0


# --------------------------------------------------------------------------
# gather
# --------------------------------------------------------------------------

def resolve_target(target):
    """Phase 0 of the review prompt: pick the diff when the caller says `auto`."""
    if target != "auto":
        return target, None
    base = next((c for c in ("@{upstream}", "main", "master") if rev_ok(c)), None)
    dirty = bool(git_text("status", "--porcelain", "--untracked-files=no").strip())
    if base is None:
        return ("HEAD" if dirty else "HEAD~1"), "no upstream/main: fell back to " + ("HEAD" if dirty else "HEAD~1")
    rng = f"{base}...HEAD"
    empty = not git_text("diff", "--name-only", rng).strip()
    if dirty or empty:
        mb = git_text("merge-base", base, "HEAD").strip()
        return mb, f"working tree included (dirty={dirty}, empty_range={empty}); diffing against merge-base {mb[:12]}"
    return rng, None


def head_side(target):
    for sep in ("...", ".."):
        if sep in target:
            return target.split(sep, 1)[1] or "HEAD"
    return None  # single commit: diff is against the working tree


def cmd_gather(a):
    run = os.path.abspath(a.rundir)
    mpath = os.path.join(run, "manifest.json")
    if os.path.exists(mpath):
        # A client may gather before invoking (so a slow check command never sits
        # inside an LLM tool call); the workflow's setup step then finds its work done.
        prior = read_json(mpath)
        if prior.get("requested_target") == a.target and prior.get("scope") == (a.scope.split() or ["."]):
            if not os.path.exists(os.path.join(run, "facts", "symbols.json")):
                build_facts(run, prior)
            print(f"already gathered: {prior['total_files']} files, target={prior['target']}")
            return
        die(f"{run} already holds a different gathered run; use a fresh run directory")
    for sub in ("patches", "candidates", "deduped", "queues", "verdicts", "facts"):
        os.makedirs(os.path.join(run, sub), exist_ok=True)

    target, note = resolve_target(a.target)
    scope = a.scope.split() or ["."]
    warnings = [note] if note else []

    hs = head_side(target)
    matches_head = True
    if hs is not None:
        matches_head = git_text("rev-parse", hs + "^{commit}").strip() == git_text("rev-parse", "HEAD").strip()
        if not matches_head:
            warnings.append(f"working tree (HEAD) is not at the diff's head ({hs}); "
                            "source files may not match the patches")

    base_args = ["diff", "--no-renames", target, "--", *scope]
    full = git_bytes(*base_args)
    if not full.strip():
        die(f"empty diff for target={target!r} scope={scope!r}", code=3)
    with open(os.path.join(run, "diff.patch"), "wb") as f:
        f.write(full)

    status = {}
    parts = git_bytes("diff", "--no-renames", "--name-status", "-z", target, "--", *scope).split(b"\0")
    for i in range(0, len(parts) - 1, 2):
        status[parts[i + 1].decode(errors="replace")] = parts[i].decode()

    files = []
    for rec in git_bytes("diff", "--no-renames", "--numstat", "-z", target, "--", *scope).split(b"\0"):
        if not rec:
            continue
        ins, dele, path = rec.decode(errors="replace").split("\t", 2)
        idx = len(files) + 1
        patch_rel = f"patches/{idx:03d}.patch"
        patch = git_bytes("diff", "--no-renames", target, "--", path)
        with open(os.path.join(run, patch_rel), "wb") as f:
            f.write(patch)
        binary = ins == "-"
        files.append({
            "index": idx, "path": path, "status": status.get(path, "?"),
            "insertions": None if binary else int(ins),
            "deletions": None if binary else int(dele),
            "binary": binary, "patch": patch_rel, "patch_bytes": len(patch),
        })

    with open(os.path.join(run, "changed-files.txt"), "w", encoding="utf-8") as f:
        f.write("".join(x["path"] + "\n" for x in files))

    manifest = {
        "requested_target": a.target, "target": target, "scope": scope,
        "head": git_text("rev-parse", "HEAD").strip(),
        "worktree_matches_diff_head": matches_head,
        "gathered_at": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "total_files": len(files), "total_patch_bytes": len(full),
        "warnings": warnings, "files": files,
    }
    write_json(mpath, manifest)
    build_facts(run, manifest)
    print(f"gathered {len(files)} files, {len(full)} patch bytes, target={target}"
          + (f" | WARNINGS: {'; '.join(warnings)}" if warnings else ""))


# --------------------------------------------------------------------------
# facts: code intelligence computed once per review
# --------------------------------------------------------------------------

DOC_EXT = (".md", ".rst", ".adoc", ".txt")
DEFS = {
    (".rs",): r"(?:pub(?:\([^)]*\))?\s+)?(?:(?:async|const|unsafe|default)\s+)*(?:extern\s+\"[^\"]*\"\s+)?"
              r"(fn|struct|enum|trait|type|const|static|mod|union)\s+([A-Za-z_][A-Za-z0-9_]*)",
    (".py",): r"(?:async\s+)?(def|class)\s+([A-Za-z_]\w*)",
    (".js", ".jsx", ".ts", ".tsx", ".mjs"): r"(?:export\s+)?(?:default\s+)?(?:async\s+)?"
                                            r"(function|class|interface|type|enum|const)\s+([A-Za-z_$][\w$]*)",
    (".go",): r"(func|type)\s+(?:\([^)]*\)\s*)?([A-Za-z_]\w*)",
}
TEST_ATTR = re.compile(r"#\[(?:[a-z_:]+::)?test\b|@pytest|\bit\(|\btest\(")
# Names that match half the codebase: their usages are listed short and flagged.
COMMON = {"new", "default", "fmt", "from", "into", "main", "run", "get", "set", "render", "update", "init", "len",
          "is_empty", "clone", "drop", "build", "name", "id", "parse", "apply", "handle", "tests", "test", "value"}
MAX_USAGES = 40


def def_regex(path):
    for exts, rx in DEFS.items():
        if path.endswith(exts):
            return re.compile(rx)
    return None


def changed_symbols(run, files):
    """Symbols the diff ADDS (a `+` definition line) or MODIFIES (a hunk's enclosing definition)."""
    found = {}
    for f in files:
        rx = def_regex(f["path"])
        if rx is None or f["binary"]:
            continue
        with open(os.path.join(run, f["patch"]), encoding="utf-8", errors="replace") as fh:
            lines = fh.read().splitlines()
        new_line, prev_added = 0, ""
        for ln in lines:
            hunk = re.match(r"@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@ ?(.*)", ln)
            if hunk:
                new_line = int(hunk.group(1)) - 1
                m = rx.search(hunk.group(2))
                if m:
                    found.setdefault((f["path"], m.group(2)), {"name": m.group(2), "kind": m.group(1),
                                     "file": f["path"], "line": None, "status": "modified"})
                prev_added = ""
                continue
            if ln.startswith(("+++", "---")) or new_line == 0 and not ln.startswith(("+", " ")):
                continue
            if ln.startswith("-"):
                continue
            new_line += 1
            if ln.startswith("+"):
                m = rx.match(ln[1:].lstrip())
                if m and not TEST_ATTR.search(prev_added):
                    found[(f["path"], m.group(2))] = {"name": m.group(2), "kind": m.group(1), "file": f["path"],
                                                      "line": new_line, "status": "added"}
                prev_added = ln[1:].strip() or prev_added
            else:
                prev_added = ""
    return sorted(found.values(), key=lambda x: (x["file"], x["line"] or 0, x["name"]))


def build_facts(run, manifest):
    os.makedirs(os.path.join(run, "facts"), exist_ok=True)
    target = manifest["target"]
    # The change's own documents: what the same diff says about its intent. A
    # narrow review scope hides them from the patches; verifiers need them.
    docs = []
    for path in git_text("diff", "--no-renames", "--name-only", target).splitlines():
        if path.lower().endswith(DOC_EXT) and not path.startswith(".code-review/") and os.path.isfile(path):
            docs.append({"path": path, "bytes": os.path.getsize(path)})
    manifest["change_docs"] = docs[:40]

    symbols = [x for x in changed_symbols(run, manifest["files"]) if not (x["kind"] == "mod" and x["name"] in COMMON)]
    for sym in symbols:
        # Usages in CODE of the same language only: a design doc that mentions the
        # symbol forty times is not a caller.
        exts = next(e for e in DEFS if sym["file"].endswith(e))
        r = subprocess.run(["git", "grep", "-n", "-w", "-F", "-I", "--", sym["name"], "--",
                            *[f"*{e}" for e in exts]], capture_output=True)
        hits = []
        for raw in r.stdout.decode(errors="replace").splitlines():
            path, _, rest = raw.partition(":")
            num, _, text = rest.partition(":")
            if path.startswith(".code-review/") or not num.isdigit():
                continue
            if path == sym["file"] and sym["line"] is not None and int(num) == sym["line"]:
                continue
            hits.append({"file": path, "line": int(num), "text": text.strip()[:140]})
        sym["ambiguous"] = sym["name"] in COMMON or len(sym["name"]) < 4
        sym["usage_count"] = len(hits)
        sym["usages"] = hits
    write_json(os.path.join(run, "facts", "symbols.json"), symbols)

    def render(cap):
        out, unused, common = [], [], []
        for sym in symbols:
            at = f"{sym['file']}:{sym['line']}" if sym["line"] else sym["file"]
            if sym["ambiguous"]:
                common.append(f"{sym['name']} ({at}, {sym['usage_count']} textual hits)")
            elif not sym["usages"]:
                unused.append(f"{sym['name']} ({sym['kind']}, {at})")
            else:
                more = f", first {cap} shown" if sym["usage_count"] > cap else ""
                out.append(f"## {sym['name']}  ({sym['kind']}, {sym['status']}, {at}) — {sym['usage_count']} usages{more}")
                out += [f"{u['file']}:{u['line']}: {u['text']}" for u in sym["usages"][:cap]] + [""]
        if unused:
            out += ["## defined or changed by this diff, used NOWHERE else in code (tests have no callers; "
                    "for anything else this is worth a look):", "; ".join(unused), ""]
        if common:
            out += ["## names too common for a textual map to mean anything — search these yourself:", "; ".join(common)]
        return out or ["(no symbols detected)"]

    header = ("usages of every symbol this diff adds or changes — textual (git grep -w, same language), so a "
              "comment or a same-named symbol counts. Start here for callers instead of grepping.")
    for cap in (25, 12, 6, 3):  # shrink until the whole map is at most four reads
        body = render(cap)
        if sum(len(x) + 1 for x in body) <= 4 * PAGE_BUDGET or cap == 3:
            break
    pages = write_pages(run, "facts/usages", header, body)
    manifest["facts"] = dict(manifest.get("facts") or {}, symbols=len(symbols), usages_pages=pages)
    write_json(os.path.join(run, "manifest.json"), manifest)
    print(f"facts: {len(symbols)} changed symbols (usages capped at {cap} each), "
          f"{len(docs)} change docs -> {' '.join(pages)}")


def cmd_facts(a):
    run = os.path.abspath(a.rundir)
    build_facts(run, read_json(os.path.join(run, "manifest.json")))


def cmd_diagnostics(a):
    """Run the repo's check command ONCE and keep what concerns the changed files."""
    run = os.path.abspath(a.rundir)
    manifest = read_json(os.path.join(run, "manifest.json"))
    argv = shlex.split(a.command)
    os.makedirs(os.path.join(run, "facts"), exist_ok=True)
    # An EMPTY toolchain variable is never a setting, only a broken environment:
    # cargo refuses to start on CARGO_TARGET_DIR="" and the review would record
    # that as a failed build.
    env = {k: v for k, v in os.environ.items() if v or not k.startswith(("CARGO_", "RUST"))}
    t0 = time.time()
    try:
        r = subprocess.run(argv, capture_output=True, timeout=a.timeout, env=env)
        code, text = r.returncode, (r.stdout + b"\n" + r.stderr).decode(errors="replace")
    except subprocess.TimeoutExpired as e:
        code, text = None, ((e.stdout or b"") + b"\n" + (e.stderr or b"")).decode(errors="replace")
    except OSError as e:
        die(f"cannot run {argv[0]!r}: {e}")
    took = time.time() - t0
    with open(os.path.join(run, "facts", "diagnostics-raw.txt"), "w", encoding="utf-8") as f:
        f.write(text)
    lines = [ln for ln in text.splitlines() if ln.strip()]
    changed = [x["path"] for x in manifest["files"]]
    mine = [ln for ln in lines if any(p in ln for p in changed)]
    status = "TIMED OUT" if code is None else ("clean" if code == 0 else f"FAILED (exit {code})")
    body = [f"command: {a.command}", f"result: {status} in {took:.0f}s, on HEAD {manifest['head'][:12]}",
            f"{len(mine)} output line(s) mention a changed file" + (":" if mine else "."), *mine[:200], "",
            "last lines of output:", *lines[-15:]]
    with open(os.path.join(run, "facts", "diagnostics.txt"), "w", encoding="utf-8") as f:
        f.write("\n".join(body) + "\n")
    manifest["facts"] = dict(manifest.get("facts") or {}, diagnostics="facts/diagnostics.txt", diagnostics_status=status)
    write_json(os.path.join(run, "manifest.json"), manifest)
    print(f"diagnostics: {status} in {took:.0f}s; {len(mine)} line(s) on changed files -> facts/diagnostics.txt")


# --------------------------------------------------------------------------
# merge
# --------------------------------------------------------------------------

def to_line(v):
    try:
        return int(v)
    except (TypeError, ValueError):
        return None


def load_candidates(path):
    """Return (list_of_dicts, problem_or_None). Accepts {"candidates": [...]} or a bare list."""
    try:
        data = read_json(path)
    except (OSError, json.JSONDecodeError) as e:
        return [], f"unreadable: {e}"
    items = data.get("candidates") if isinstance(data, dict) else data
    if not isinstance(items, list):
        return [], "no `candidates` array"
    good = [c for c in items if isinstance(c, dict)]
    return good, (None if len(good) == len(items) else "non-object entries skipped")


def cmd_merge(a):
    run = os.path.abspath(a.rundir)
    expect = [x for x in a.expect.split(",") if x]
    reported, missing, problems, out = [], [], [], []
    for angle in expect:
        path = os.path.join(run, "candidates", f"{angle}.json")
        if not os.path.exists(path):
            missing.append(angle)
            continue
        items, problem = load_candidates(path)
        if problem:
            problems.append({"angle": angle, "file": f"candidates/{angle}.json", "problem": problem})
        if problem and not items:
            missing.append(angle)
            continue
        reported.append(angle)
        if len(items) > MAX_PER_ANGLE:
            problems.append({"angle": angle, "problem": f"{len(items)} candidates; kept first {MAX_PER_ANGLE}"})
            items = items[:MAX_PER_ANGLE]
        for n, c in enumerate(items, 1):
            rec = dict(c)
            rec["pid"] = f"{angle}-{n}"
            rec["angle"] = angle
            rec["line"] = to_line(c.get("line"))
            absent = [k for k in ("file", "summary", "failure_scenario") if not c.get(k)]
            if absent:
                rec["incomplete"] = absent
            out.append(rec)
    write_json(os.path.join(run, "candidates", "all.json"), {
        "angles_expected": expect, "angles_reported": reported, "angles_missing": missing,
        "problems": problems, "raw_count": len(out), "candidates": out,
    })
    for c in out:
        write_json(os.path.join(run, "candidates", "raw", f"{c['pid']}.json"), c)
    # Sorted by location so probable duplicates sit on adjacent lines.
    ordered = sorted(out, key=lambda c: (str(c.get("file")), c.get("line") or 0, c["pid"]))
    # A shared location is a mechanical fact, so state it: a clerk once left four
    # candidates at the identical file:line unmerged and they took four of the top 15.
    at = collections.Counter((str(c.get("file")), c.get("line")) for c in ordered)
    lines, flagged = [], set()
    for c in ordered:
        key = (str(c.get("file")), c.get("line"))
        if at[key] > 1 and key not in flagged:
            flagged.add(key)
            lines.append(f">>> the next {at[key]} candidates cite the SAME location, {where(c)}: if they describe one "
                         "defect they are ONE group (keep them apart only if the reasons genuinely differ)")
        lines.append(f"{c['pid']} | {where(c)} | {one_line(c.get('category'), 14)} | "
                     f"{one_line(c.get('summary'), 300)} || fails: {one_line(c.get('failure_scenario'), 160)}")
    pages = write_pages(run, "candidates/digest",
                        "pid | file:line | category | summary || fails: failure-scenario excerpt   "
                        "(full record: candidates/raw/<pid>.json)", lines)
    print(f"digest pages (read these, not all.json): {' '.join(pages)}")
    print(f"merged {len(out)} candidates from {len(reported)}/{len(expect)} angles"
          + (f" | MISSING: {','.join(missing)}" if missing else "")
          + (f" | {len(problems)} problem(s), see candidates/all.json" if problems else ""))


# --------------------------------------------------------------------------
# shard
# --------------------------------------------------------------------------

def cmd_shard(a):
    run = os.path.abspath(a.rundir)
    merged = read_json(os.path.join(run, "candidates", "all.json"))
    cands = {c["pid"]: c for c in merged["candidates"]}
    warnings = []

    dec_path = os.path.join(run, "deduped", "decisions.json")
    decisions = {}
    if os.path.exists(dec_path):
        try:
            decisions = read_json(dec_path)
            if not isinstance(decisions, dict):
                raise TypeError("top level is not an object")
        except (json.JSONDecodeError, TypeError) as e:
            decisions = {}
            warnings.append(f"decisions.json unreadable ({e}); no dedup applied")
    else:
        warnings.append("decisions.json missing; no dedup applied")

    # Accept groups ({"pids": [...], "keep"?, "reason"}) and the older pairs
    # ({"drop","keep","reason"}). Both reduce to "these pids are one defect".
    parent = {pid: pid for pid in cands}

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    preferred, reason_of = set(), {}
    entries = [dict(g, _pids=g.get("pids")) for g in decisions.get("groups") or [] if isinstance(g, dict)]
    entries += [dict(d, _pids=[d.get("drop"), d.get("keep")]) for d in decisions.get("duplicates") or []
                if isinstance(d, dict)]
    for e in entries:
        raw = e["_pids"] if isinstance(e["_pids"], list) else []
        pids = [x for x in raw if x in cands]
        if len(pids) != len(raw):
            warnings.append(f"decision names unknown pid(s) {[x for x in raw if x not in cands]}; ignored those")
        if len(set(pids)) < 2:
            continue
        for other in pids[1:]:
            parent[find(other)] = find(pids[0])
        if e.get("keep") in pids:
            preferred.add(e["keep"])
        for x in pids:
            reason_of.setdefault(x, e.get("reason", ""))

    order = {c["pid"]: n for n, c in enumerate(merged["candidates"])}
    members = {}
    for pid in cands:
        members.setdefault(find(pid), []).append(pid)

    dropped = []
    for group in members.values():
        if len(group) < 2:
            continue
        named = sorted((x for x in group if x in preferred), key=order.get)
        keeper = named[0] if named else max(
            group, key=lambda x: (len(str(cands[x].get("failure_scenario") or "")) +
                                  len(str(cands[x].get("evidence") or "")), -order[x]))
        for x in sorted(group, key=order.get):
            if x == keeper:
                continue
            cands[keeper].setdefault("also_flagged_by", []).append(
                {"pid": x, "angle": cands[x]["angle"], "summary": cands[x].get("summary", "")})
            dropped.append({"pid": x, "kept_as": keeper, "reason": reason_of.get(x, "")})
    dropped_pids = {d["pid"] for d in dropped}

    survivors = [c for c in merged["candidates"] if c["pid"] not in dropped_pids]
    for n, c in enumerate(survivors, 1):
        c["id"] = f"C{n:02d}"
        write_json(os.path.join(run, "deduped", f"{c['id']}.json"), c)

    queues = [[] for _ in range(a.shards)]
    for n, c in enumerate(survivors):
        queues[n % a.shards].append(c["id"])
    # `verdict_dir` is ABSOLUTE: given "verdicts/r1", one loop's sessions resolved it
    # against the queue file's folder and wrote to queues/verdicts/r1/.
    # One verdict directory PER queue. With a shared directory a verifier has to pick
    # its ids out of everyone's files, and `C32.v3.json` (another loop's) was once read
    # as `C32.v2.json`: the id was skipped and the queue marked done. In its own
    # directory every file is the loop's own, and "done" is a count.
    for k, pending in enumerate(queues, 1):
        os.makedirs(os.path.join(run, "verdicts", f"q{k}"), exist_ok=True)
        write_json(os.path.join(run, "queues", f"queue-{k}.json"),
                   {"done": not pending, "ids": pending, "verdict_dir": os.path.join(run, "verdicts", f"q{k}")})
    os.makedirs(os.path.join(run, "verdicts", "sweep"), exist_ok=True)

    write_json(os.path.join(run, "deduped", "index.json"), {
        "angles_reported": merged["angles_reported"], "angles_missing": merged["angles_missing"],
        "raw_count": merged["raw_count"], "deduped_count": len(survivors),
        "dropped": dropped, "warnings": warnings, "candidates": survivors,
    })
    pages = write_pages(run, "deduped/digest",
                        "id | file:line | angles | summary   (full record: deduped/<id>.json)",
                        [f"{c['id']} | {where(c)} | x{1 + len(c.get('also_flagged_by', []))} | "
                         f"{one_line(c.get('summary'), 300)}" for c in survivors])
    print(f"digest pages (the already-raised list): {' '.join(pages)}")
    print(f"sharded {len(survivors)} candidates ({len(dropped)} duplicates dropped) into "
          f"{a.shards} queues: {[len(q) for q in queues]}"
          + (f" | WARNINGS: {'; '.join(warnings)}" if warnings else ""))


# --------------------------------------------------------------------------
# ballots: two more independent votes where one vote proved unstable
# --------------------------------------------------------------------------

def all_candidates(run, warnings=None):
    """Deduped candidates plus the gap sweep's, as (list, sweep_count)."""
    index = read_json(os.path.join(run, "deduped", "index.json"))
    cands = list(index["candidates"])
    sweep_path = os.path.join(run, "candidates", "sweep.json")
    n_sweep = 0
    if os.path.exists(sweep_path):
        items, problem = load_candidates(sweep_path)
        if problem and warnings is not None:
            warnings.append(f"sweep.json: {problem}")
        for n, c in enumerate(items[:MAX_PER_ANGLE], 1):
            rec = dict(c)
            rec.setdefault("id", f"S{n:02d}")
            rec["angle"] = "sweep"
            rec["line"] = to_line(c.get("line"))
            cands.append(rec)
            n_sweep += 1
    elif warnings is not None:
        warnings.append("candidates/sweep.json missing: the gap sweep did not report")
    return index, cands, n_sweep


def is_conventions(c):
    return "convention" in str(c.get("category", "")).lower() or c.get("angle") == "conventions"


def cmd_ballots(a):
    """A single vote is unstable exactly where it matters most: identical runs flipped
    REFUTED <-> CONFIRMED on rule-interpretation claims. So every first-round REFUTED
    and every conventions claim gets two more sessions that cannot see the first."""
    run = os.path.abspath(a.rundir)
    _, cands, _ = all_candidates(run)
    chosen = []
    for c in cands:
        first = load_verdict(run, c["id"])["verdict"]
        why = [w for w, hit in (("first vote REFUTED", first == "REFUTED"), ("conventions claim", is_conventions(c))) if hit]
        if why:
            chosen.append({"id": c["id"], "first_vote": first, "reason": " + ".join(why)})
            for tag in ("v2", "v3"):
                write_json(os.path.join(run, "deduped", f"{c['id']}.{tag}.json"),
                           dict(c, id=f"{c['id']}.{tag}", ballot_of=c["id"]))
    for loop, tag in (("r1", "v2"), ("r2", "v3")):
        ids = [f"{b['id']}.{tag}" for b in chosen]
        os.makedirs(os.path.join(run, "verdicts", loop), exist_ok=True)
        write_json(os.path.join(run, "queues", f"queue-{loop}.json"),
                   {"done": not ids, "ids": ids, "verdict_dir": os.path.join(run, "verdicts", loop)})
    write_json(os.path.join(run, "ballots.json"), {"balloted": chosen})
    print(f"ballots: {len(chosen)} of {len(cands)} candidates get two more votes "
          f"({sum('REFUTED' in b['reason'] for b in chosen)} refuted, "
          f"{sum('conventions' in b['reason'] for b in chosen)} conventions) -> queues/queue-r1.json, queue-r2.json")


def tally(votes):
    """Final verdict from 1-3 votes. REFUTED needs two; so does CONFIRMED; a split is PLAUSIBLE."""
    valid = [v for v in votes if v in VERDICTS]
    if not valid:
        return "UNVERIFIED"
    if len(valid) == 1:
        return "PLAUSIBLE" if valid[0] == "REFUTED" else valid[0]
    for verdict in ("REFUTED", "CONFIRMED"):
        if valid.count(verdict) >= 2:
            return verdict
    return "PLAUSIBLE"


# --------------------------------------------------------------------------
# collate
# --------------------------------------------------------------------------

def load_verdict(run, cid):
    path = os.path.join(run, "verdicts", f"{cid}.json")
    if not os.path.exists(path):
        # Per-queue directories - and wherever under the run a loop really put them: a
        # verdict that exists must never be read as missing because of WHERE it was written.
        path = next((os.path.join(root, f"{cid}.json") for root, _, files in os.walk(run)
                     if f"{cid}.json" in files and "verdicts" in os.path.relpath(root, run).split(os.sep)), path)
    if not os.path.exists(path):
        return {"verdict": "UNVERIFIED", "reasoning": "no verdict file was written"}
    try:
        v = read_json(path)
    except (OSError, json.JSONDecodeError) as e:
        return {"verdict": "UNVERIFIED", "reasoning": f"verdict file unreadable: {e}"}
    if not isinstance(v, dict):
        return {"verdict": "UNVERIFIED", "reasoning": "verdict file is not a JSON object"}
    raw = str(v.get("verdict", "")).strip().upper()
    if raw not in VERDICTS:
        v["raw_verdict"] = v.get("verdict")
        v["verdict"] = "UNVERIFIED"
    else:
        v["verdict"] = raw
    return v


def cmd_collate(a):
    run = os.path.abspath(a.rundir)
    warnings = []
    index, cands, sweep_count = all_candidates(run, warnings)
    warnings = list(index.get("warnings", [])) + warnings
    bpath = os.path.join(run, "ballots.json")
    balloted = {b["id"] for b in read_json(bpath)["balloted"]} if os.path.exists(bpath) else set()

    kept, refuted, overturned = [], [], []
    for c in cands:
        ballots = [load_verdict(run, c["id"])]
        if c["id"] in balloted:
            ballots += [load_verdict(run, f"{c['id']}.{tag}") for tag in ("v2", "v3")]
        votes = [b["verdict"] for b in ballots]
        final = tally(votes) if c["id"] in balloted else votes[0]
        shown = next((b for b in ballots if b["verdict"] == final), ballots[0])
        rec = dict(c)
        rec["verdict"] = final
        rec["verification"] = {k: shown[k] for k in
                               ("evidence", "reasoning", "would_confirm", "corrected_line", "raw_verdict")
                               if shown.get(k) not in (None, "")}
        # An accepted limitation is still a finding: any vote's by_design note rides
        # along so the report can say so instead of the finding being refuted away.
        note = next((b["by_design"] for b in ballots if b.get("by_design")), None)
        if note and final != "REFUTED":
            rec["by_design"] = str(note)
        if c["id"] in balloted:
            rec["votes"] = votes
            if final != votes[0]:
                overturned.append(f"{c['id']}: {votes[0]} -> {final} ({', '.join(votes)})")
        (refuted if final == "REFUTED" else kept).append(rec)

    counts = {k: 0 for k in (*VERDICTS, "UNVERIFIED")}
    for c in kept + refuted:
        counts[c["verdict"]] += 1
    per_angle = {}
    for c in cands:
        per_angle[c["angle"]] = per_angle.get(c["angle"], 0) + 1

    write_json(os.path.join(run, "verified.json"), {
        "kept": kept, "refuted": refuted,
        "stats": {
            "angles_reported": index["angles_reported"], "angles_missing": index["angles_missing"],
            "raw_candidates": index["raw_count"], "after_dedup": index["deduped_count"],
            "sweep_candidates": sweep_count, "verdicts": counts,
            "balloted": len(balloted), "overturned_by_vote": overturned,
            "candidates_per_angle_after_dedup": per_angle,
            "unverified_ids": [c["id"] for c in kept if c["verdict"] == "UNVERIFIED"],
        },
        "warnings": warnings,
    })
    pages = write_pages(run, "verified-digest",
                        "id | verdict | angles | category | file:line | summary || fails: excerpt   "
                        "(full: deduped/<id>.json or candidates/sweep.json, verdicts/<id>.json)",
                        [f"{c['id']} | {c['verdict']}{' (votes ' + '/'.join(v[0] for v in c['votes']) + ')' if c.get('votes') else ''}"
                         f"{' [by design]' if c.get('by_design') else ''}"
                         f" | x{1 + len(c.get('also_flagged_by', []))} | "
                         f"{one_line(c.get('category'), 14)} | {where(c)} | {one_line(c.get('summary'), 300)} "
                         f"|| fails: {one_line(c.get('failure_scenario'), 160)}" for c in kept])
    print(f"digest pages (read these, not verified.json): {' '.join(pages)}")
    if overturned:
        print("votes changed the outcome: " + "; ".join(overturned))
    print(f"collated: kept {len(kept)} (" + ", ".join(f"{k}={v}" for k, v in counts.items() if k != "REFUTED")
          + f"), refuted {len(refuted)}")


# --------------------------------------------------------------------------
# finalize
# --------------------------------------------------------------------------

def best_line(c):
    return to_line(c.get("verification", {}).get("corrected_line")) or c.get("line")


def loc(c):
    line = best_line(c)
    return f"`{c.get('file', '?')}:{line}`" if line else f"`{c.get('file', '?')}`"


def cell(s):
    return str(s or "").replace("|", "\\|").replace("\n", " ").strip()


def cmd_finalize(a):
    run = os.path.abspath(a.rundir)
    verified = read_json(os.path.join(run, "verified.json"))
    manifest = read_json(os.path.join(run, "manifest.json"))
    kept = {c["id"]: c for c in verified["kept"]}
    warnings = list(verified.get("warnings", []))

    order, notes = [], {}
    rank_path = os.path.join(run, "ranking.json")
    if os.path.exists(rank_path):
        try:
            r = read_json(rank_path)
            order, notes = list(r.get("order", [])), dict(r.get("notes", {}))
        except (json.JSONDecodeError, AttributeError, TypeError) as e:
            warnings.append(f"ranking.json unreadable ({e}); findings are in discovery order")
    else:
        warnings.append("ranking.json missing; findings are in discovery order")

    ranked, seen = [], set()
    for cid in order:
        if cid in kept and cid not in seen:
            ranked.append(kept[cid])
            seen.add(cid)
        elif cid not in kept:
            warnings.append(f"ranking names unknown or refuted id {cid!r}; ignored")
    unranked = [c for cid, c in kept.items() if cid not in seen]
    if unranked:  # recall mode: an unranked finding is appended, never dropped
        warnings.append("not ranked, appended in discovery order: " + ", ".join(c["id"] for c in unranked))
        ranked.extend(unranked)

    top, rest = ranked[:MAX_FINDINGS], ranked[MAX_FINDINGS:]
    write_json(os.path.join(run, "findings.json"), [{
        "id": c["id"], "file": c.get("file"), "line": best_line(c),
        "summary": c.get("summary"), "failure_scenario": c.get("failure_scenario"),
        "verdict": c["verdict"],
        "angles": [c["angle"]] + [x["angle"] for x in c.get("also_flagged_by", [])],
    } for c in top])

    s = verified["stats"]
    md = [f"# Code review — `{manifest['target']}`", "",
          f"- **Head:** `{manifest['head'][:12]}`  **Scope:** `{' '.join(manifest['scope'])}`  "
          f"**Files:** {manifest['total_files']}  **Patch bytes:** {manifest['total_patch_bytes']}",
          f"- **Pipeline:** {len(s['angles_reported'])} finder angles reported"
          + (f" (**missing: {', '.join(s['angles_missing'])}**)" if s["angles_missing"] else "")
          + f" → {s['raw_candidates']} candidates → {s['after_dedup']} after dedup"
          f" + {s['sweep_candidates']} from the gap sweep",
          "- **Verdicts:** " + ", ".join(f"{k} {v}" for k, v in s["verdicts"].items())
          + (f" — {s['balloted']} candidates took three votes, {len(s['overturned_by_vote'])} changed outcome"
             if s.get("balloted") else ""),
          "- `CONFIRMED` = trigger and wrong outcome named, line quoted. `PLAUSIBLE` = mechanism real, "
          "trigger uncertain. `UNVERIFIED` = no verdict was produced; kept because this is a recall-mode review.",
          "", "## Summary", "", "| # | Id | Location | Verdict | Issue |", "|---|---|---|---|---|"]
    for n, c in enumerate(ranked, 1):
        md.append(f"| {n} | {c['id']} | {loc(c)} | {c['verdict']} | {cell(c.get('summary'))} |")
        if n == MAX_FINDINGS and rest:
            md.append(f"| | | | | *— reporting cap ({MAX_FINDINGS}); items below are recorded in rank order —* |")
    md += ["", "## Findings", ""]
    for n, c in enumerate(ranked, 1):
        v = c.get("verification", {})
        angles = [c["angle"]] + [x["angle"] for x in c.get("also_flagged_by", [])]
        md += [f"### {n}. {cell(c.get('summary'))}", "",
               f"- **Where:** {loc(c)}  **Verdict:** {c['verdict']}  **Id:** {c['id']}  **Angles:** {', '.join(angles)}",
               f"- **Failure scenario:** {c.get('failure_scenario') or '—'}"]
        if c.get("votes"):
            md.append(f"- **Votes:** {', '.join(c['votes'])} → {c['verdict']}")
        if c.get("by_design"):
            md.append(f"- **Accepted by design:** {c['by_design']}")
        if c.get("evidence"):
            md.append(f"- **Finder evidence:** {c['evidence']}")
        if v.get("evidence"):
            md.append(f"- **Verifier evidence:** {v['evidence']}")
        if v.get("reasoning"):
            md.append(f"- **Verifier reasoning:** {v['reasoning']}")
        if v.get("would_confirm"):
            md.append(f"- **Would confirm:** {v['would_confirm']}")
        if notes.get(c["id"]):
            md.append(f"- **Ranking note:** {notes[c['id']]}")
        md.append("")
    md += ["## Refuted during verification", "",
           "Listed so they are not re-derived later.", ""]
    if verified["refuted"]:
        for c in verified["refuted"]:
            v = c.get("verification", {})
            votes = f" *(votes: {', '.join(c['votes'])})*" if c.get("votes") else ""
            md.append(f"- **{c['id']}** {loc(c)} — {cell(c.get('summary'))}{votes}  \n"
                      f"  *Refuted:* {v.get('reasoning') or v.get('evidence') or 'no reasoning recorded'}")
    else:
        md.append("None.")
    if warnings:
        md += ["", "## Run warnings", ""] + [f"- {w}" for w in warnings]
    with open(os.path.join(run, "report.md"), "w", encoding="utf-8") as f:
        f.write("\n".join(md) + "\n")
    # Everything the commenter needs, in one place, so it never has to find a verdict
    # file or page through verified.json.
    brief = []
    for n, c in enumerate(top, 1):
        v = c.get("verification", {})
        angles = [c["angle"]] + [x["angle"] for x in c.get("also_flagged_by", [])]
        brief += [f"===== {c['id']}  (rank {n})  {c.get('file')}:{best_line(c)}",
                  f"verdict: {c['verdict']}" + (f"  votes: {', '.join(c['votes'])}" if c.get("votes") else "")
                  + f"  | category: {c.get('category', '?')}  | raised by {len(angles)} angle(s): {', '.join(angles)}",
                  f"summary: {one_line(c.get('summary'), 600)}",
                  f"failure scenario: {one_line(c.get('failure_scenario'), 900)}"]
        for key, label in (("evidence", "finder evidence"),):
            if c.get(key):
                brief.append(f"{label}: {one_line(c[key], 500)}")
        for key, label in (("evidence", "verifier evidence"), ("reasoning", "verifier reasoning"),
                           ("would_confirm", "would confirm")):
            if v.get(key):
                brief.append(f"{label}: {one_line(v[key], 900)}")
        if c.get("by_design"):
            brief.append(f"ACCEPTED BY DESIGN (the author documented this): {one_line(c['by_design'], 500)}")
        brief.append("")
    os.makedirs(os.path.join(run, "comments"), exist_ok=True)
    pages = write_pages(run, "comments/brief", "one block per reported finding, in rank order", brief or ["(no findings)"])
    print(f"comment briefs (one block per reported finding): {' '.join(pages)}")
    print(f"finalized: {len(top)} findings reported, {len(rest)} below the cap, "
          f"{len(verified['refuted'])} refuted -> findings.json, report.md")


# --------------------------------------------------------------------------
# comments: Conventional Comments (conventionalcomments.org)
# --------------------------------------------------------------------------

LABELS = ("praise", "nitpick", "suggestion", "issue", "todo", "question", "thought", "chore", "note",
          "typo", "polish", "quibble")
NEVER_BLOCKING = ("nitpick", "thought", "note", "praise")   # "non-blocking by nature" in the spec
DECORATION = re.compile(r"^[a-z][a-z0-9-]*$")


def template_comment(f, c):
    """The fallback when no model-written comment exists: mechanical, but never absent."""
    cat = str(c.get("category", "")).lower()
    if c.get("by_design"):
        label, decs = "thought", ["non-blocking"]
    elif f["verdict"] != "CONFIRMED":
        label, decs = "question", ["non-blocking"]
    elif cat in ("cleanup", "altitude"):
        label, decs = "suggestion", ["non-blocking"]
    else:
        label, decs = "issue", []
    subject = one_line(f.get("summary"), 240)
    parts = [str(f.get("failure_scenario") or "").strip()]
    if c.get("by_design"):
        parts.append(f"This looks deliberate: {c['by_design']}")
    return {"label": label, "decorations": decs, "subject": subject, "discussion": "\n\n".join(x for x in parts if x)}


def cmd_comments(a):
    run = os.path.abspath(a.rundir)
    findings = read_json(os.path.join(run, "findings.json"))
    verified = read_json(os.path.join(run, "verified.json"))
    kept = {c["id"]: c for c in verified["kept"]}
    ids = {f["id"] for f in findings}
    out, warnings, sources = [], [], collections.Counter()
    for f in findings:
        c = kept.get(f["id"], {})
        path = os.path.join(run, "comments", f"{f['id']}.json")
        raw, source = None, "template"
        if os.path.exists(path):
            try:
                raw = read_json(path)
                if not isinstance(raw, dict):
                    raise ValueError("not a JSON object")
                source = "model"
            except (json.JSONDecodeError, ValueError) as e:
                warnings.append(f"{f['id']}: comment file unreadable ({e}); template used")
                raw = None
        entry = {"id": f["id"], "file": f.get("file"), "line": f.get("line"), "verdict": f["verdict"]}
        dup = (raw or {}).get("duplicate_of")
        if dup:
            if dup in ids and dup != f["id"]:
                out.append(dict(entry, duplicate_of=dup, source=source))
                sources["duplicate"] += 1
                continue
            warnings.append(f"{f['id']}: duplicate_of names {dup!r}, which is not a reported finding; template used")
            raw, source = None, "template"
        if raw is not None:
            label = str(raw.get("label", "")).strip().lower().rstrip(":")
            subject = " ".join(str(raw.get("subject", "")).split())
            if label not in LABELS or not subject:
                warnings.append(f"{f['id']}: label {raw.get('label')!r} or subject invalid; template used")
                raw, source = None, "template"
        if raw is None:
            raw = template_comment(f, c)
            label, subject = raw["label"], raw["subject"]
        decs = []
        for d in raw.get("decorations") or []:
            d = str(d).strip().lower().strip("()")
            if DECORATION.match(d) and d not in decs:
                decs.append(d)
            elif d:
                warnings.append(f"{f['id']}: decoration {d!r} dropped (not a lowercase word)")
        if "blocking" in decs and (label in NEVER_BLOCKING or f["verdict"] != "CONFIRMED" or c.get("by_design")):
            # The spec makes these labels non-blocking by nature; and this pipeline does not
            # block a merge on a claim it could not confirm or that the author documented.
            decs = ["non-blocking" if d == "blocking" else d for d in decs]
            warnings.append(f"{f['id']}: `blocking` downgraded (label {label}, verdict {f['verdict']}"
                            f"{', by design' if c.get('by_design') else ''})")
        if "blocking" in decs and "non-blocking" in decs:
            decs.remove("non-blocking")
        if (f["verdict"] != "CONFIRMED" or c.get("by_design")) and "non-blocking" not in decs:
            decs.append("non-blocking")  # say so explicitly: a bare label reads as "must fix"
        discussion = str(raw.get("discussion") or "").strip()
        head = f"**{label}{' (' + ','.join(decs) + ')' if decs else ''}:** {subject}"
        angles = len(f.get("angles") or [])
        trailer = (f"_Automated review · {f['verdict'].lower()}"
                   + (f" by vote ({', '.join(v.lower() for v in c['votes'])})" if c.get("votes") else "")
                   + (f" · raised independently by {angles} review angles" if angles > 1 else "") + f" · {f['id']}_")
        body = "\n\n".join(x for x in (head, discussion, None if a.no_trailer else trailer) if x)
        out.append(dict(entry, label=label, decorations=decs, subject=subject, discussion=discussion,
                        body=body, source=source))
        sources[source] += 1

    write_json(os.path.join(run, "comments.json"), out)
    by_id = {o["id"]: o for o in out}
    for f in findings:
        o = by_id[f["id"]]
        f.pop("comment", None), f.pop("duplicate_of", None)
        f.update({"duplicate_of": o["duplicate_of"]} if o.get("duplicate_of") else {"comment": o["body"]})
    write_json(os.path.join(run, "findings.json"), findings)

    md = ["# Review comments", "",
          "Conventional Comments format (conventionalcomments.org). Each block is postable as-is at the location above it.", ""]
    for n, o in enumerate(out, 1):
        where_ = f"`{o['file']}:{o['line']}`" if o.get("line") else f"`{o['file']}`"
        if o.get("duplicate_of"):
            md += [f"## {n}. {where_} — {o['id']}", "", f"Same defect as {o['duplicate_of']}; no separate comment.", ""]
        else:
            md += [f"## {n}. {where_} — {o['id']}", "", o["body"], ""]
    text = "\n".join(md) + "\n"
    with open(os.path.join(run, "comments.md"), "w", encoding="utf-8") as fh:
        fh.write(text)
    rpath = os.path.join(run, "report.md")
    if os.path.exists(rpath):
        report = open(rpath, encoding="utf-8").read().split("\n# Review comments\n")[0].rstrip("\n")
        with open(rpath, "w", encoding="utf-8") as fh:
            fh.write(report + "\n\n" + text)
    labels = collections.Counter(o["label"] for o in out if o.get("label"))
    print(f"comments: {len(out)} findings -> {sources['model']} model-written, {sources['template']} template, "
          f"{sources['duplicate']} marked duplicate | labels {dict(labels)} | "
          f"blocking: {sum('blocking' in (o.get('decorations') or []) for o in out)}"
          + (f" | WARNINGS: {'; '.join(warnings)}" if warnings else "") + " -> comments.json, comments.md")


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    g = sub.add_parser("gather"); g.add_argument("rundir"); g.add_argument("target"); g.add_argument("scope", nargs="?", default=".")
    g.set_defaults(fn=cmd_gather)
    m = sub.add_parser("merge"); m.add_argument("rundir"); m.add_argument("--expect", required=True)
    m.set_defaults(fn=cmd_merge)
    s = sub.add_parser("shard"); s.add_argument("rundir"); s.add_argument("--shards", type=int, default=4)
    s.set_defaults(fn=cmd_shard)
    fa = sub.add_parser("facts"); fa.add_argument("rundir"); fa.set_defaults(fn=cmd_facts)
    d = sub.add_parser("diagnostics"); d.add_argument("rundir"); d.add_argument("command")
    d.add_argument("--timeout", type=int, default=1800); d.set_defaults(fn=cmd_diagnostics)
    b = sub.add_parser("ballots"); b.add_argument("rundir"); b.set_defaults(fn=cmd_ballots)
    c = sub.add_parser("collate"); c.add_argument("rundir"); c.set_defaults(fn=cmd_collate)
    f = sub.add_parser("finalize"); f.add_argument("rundir"); f.set_defaults(fn=cmd_finalize)
    cm = sub.add_parser("comments"); cm.add_argument("rundir")
    cm.add_argument("--no-trailer", action="store_true", help="omit the provenance line under each comment")
    cm.set_defaults(fn=cmd_comments)
    a = p.parse_args()
    a.fn(a)


if __name__ == "__main__":
    main()
