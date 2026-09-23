#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Generate `.kiro/workflows/code-review-max.workflow.json`.

The recipe is a port of Claude Code's max-effort `/code-review` prompt
(finder fan-out -> 3-state verify -> gap sweep -> ranked findings) onto the
KAS workflow engine. It is generated rather than hand-written because the
finder steps share one template and the verify loops share another; the angle
texts below are the single source of truth.

Shape (step nodes count toward the engine's hard cap of 20):

    setup                                   1   crtool gather
    parallel[allSettled] find-*          8/10   one peer session per angle
    dedup                                   1   crtool merge -> decisions -> crtool shard
    parallel[allSettled] repeat(verify-K)   N   queue drain: ONE candidate per fresh session
    sweep                                   1   gap sweep over the already-raised list
    repeat(verify-sweep)                    1
    ballots                                 1   crtool ballots: refuted + conventions claims get 2 more votes
    parallel[allSettled] repeat(verify-rK)  2   the second and third votes, same verifier, blind to the first
    rank                                    1   crtool collate (2-of-3 tally) -> ranking -> crtool finalize
    comment                                 1   one Conventional Comments review comment per reported finding

With the comment step the recipe needs 3 verify shards to fit the cap (4 would be 21 nodes).

Data flows through files under {{rundir}} only. `{{id.output}}` is never used:
the engine captures a step's LAST message, which for a step that writes a file
and signs off is the sign-off (cyril-srp6).

Usage: build_recipe.py [--shards 3] [--split-cleanup] [--replay] [--only ID[,ID]] [--out PATH]
"""
import argparse
import json
import os

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
DEFAULT_OUT = os.path.join(REPO, ".kiro", "workflows", "code-review-max.workflow.json")
# How crtool is launched differs per machine (`python3` / `python` / `uv run --script`),
# so it is a workflow input the client fills in. Every argument is DOUBLE-quoted: on
# Windows KAS runs commands in PowerShell, where a bare `a,b,c` becomes an array and
# single quotes mean nothing to cmd.exe; double quotes work in bash, pwsh and cmd.
TOOL = "{{crtool}}"
STEP_CAP = 20

# --- angle texts ----------------------------------------------------------
# A-E, Efficiency, Altitude and Conventions are verbatim from the assembled
# prompt. Reuse and Simplification are reconstructions there too (marked with
# a dagger in the source document).

ANGLE_A = """Read every hunk in the diff, line by line. Then Read the enclosing function for
each hunk — bugs in unchanged lines of a touched function are in scope (the PR
re-exposes or fails to fix them). For every line ask: what input, state, timing,
or platform makes this line wrong? Look for inverted/wrong conditions,
off-by-one, null/undefined deref, missing `await`, falsy-zero checks,
wrong-variable copy-paste, error swallowed in catch, unescaped regex metachars."""

ANGLE_B = """For every line the diff DELETES or replaces, name the invariant or behavior it
enforced, then search the new code for where that invariant is re-established.
If you can't find it, that's a candidate: a removed guard, a dropped error
path, a narrowed validation, a deleted test that was covering a real case."""

ANGLE_C = """For each function the diff changes, find its callers (search for the symbol) and
check whether the change breaks any call site: a new precondition, a changed
return shape, a new exception, a timing/ordering dependency. Also check callees:
does a parallel change in the same PR make a call unsafe?"""

ANGLE_D = """Scan for the classic pitfalls of the diff's language/framework — for example:
JS falsy-zero, `==` coercion, closure-captured loop var; Python mutable default
args, late-binding closures; Go nil-map write, range-var capture; SQL injection;
timezone/DST drift; float equality. Flag any instance the diff introduces.
Identify the diff's actual language(s) first and apply THAT language's pitfalls
(for Rust: integer overflow/underflow and `as` truncation, slice/str indexing
panics and char-boundary slicing, `unwrap`/`expect` on reachable None/Err,
RefCell double-borrow, lock held across `.await`, iterator invalidation via
index arithmetic, `saturating_sub` masking a logic error, Ord/Eq/Hash
inconsistency)."""

ANGLE_E = """When the PR adds or modifies a type that wraps another (cache, proxy, decorator,
adapter): check that every method routes to the wrapped instance and not back
through a registry/session/global — e.g. a caching provider holding a
`delegate` field that resolves IDs via `session.get(...)` instead of
`delegate.get(...)` will re-enter the cache or recurse. Also check that the
wrapper forwards all the methods the callers actually use. Trait impls,
delegating methods, newtypes and state wrappers all count. If the diff has no
wrapping types at all, write an empty candidates list."""

REUSE = """Flag code the diff adds that duplicates an existing helper, utility, constant,
or type already present in the codebase. For each duplicated block, point to the
existing implementation that should be reused instead, and name the concrete
cost (drift between copies, double maintenance)."""

SIMPLIFICATION = """Flag changed code that is more complex than it needs to be: redundant branches,
needless intermediate state, conditionals that collapse to a simpler form,
abstractions with a single caller, and dead code the diff leaves behind. Name
the simpler equivalent for each."""

EFFICIENCY = """Flag wasted work the diff introduces: redundant computation or repeated I/O,
independent operations run sequentially, blocking work added to startup or
hot paths. Also flag long-lived objects built from closures or captured
environments — they keep the entire enclosing scope alive for the object's
lifetime (a memory leak when that scope holds large values); prefer a
class/struct that copies only the fields it needs. Name the cheaper
alternative."""

ALTITUDE = """Check that each change is implemented at the right depth, not as a fragile
bandaid. Special cases layered on shared infrastructure are a sign the fix
isn't deep enough — prefer generalizing the underlying mechanism over adding
special cases."""

CONVENTIONS = """Find the rules files that govern the changed code: the repo-root CLAUDE.md and
AGENTS.md, any `.kiro/steering/*.md`, the user-level ~/.claude/CLAUDE.md, plus
any CLAUDE.md, CLAUDE.local.md or AGENTS.md in a directory that is an ancestor
of a changed file (a directory's rules file only applies to files at or below
it). Read each one that exists — skip silently any that is absent or
unreadable — then check the diff for clear violations of the rules they state.

Only flag a violation when you can quote the exact rule and the exact line
that breaks it — no style preferences, no vague "spirit of the doc"
inferences. In the finding, name the rules file path and quote the rule so the
report can cite it. If no rules file applies, write an empty candidates list."""

SWEEP = """You are a fresh reviewer who has the list of everything the first pass already
raised. That list is {{rundir}}/deduped/digest-1.txt (and digest-2.txt, ... if
present — read every page): one line per already-raised candidate. Do NOT read
deduped/index.json; it is too large for one read. For the full text of one
entry, read {{rundir}}/deduped/<id>.json. (This step is the stated exception to
the rule about not reading other candidates. The list includes candidates that
may since have been refuted; do not re-raise those either.)

Re-read the diff and enclosing functions looking ONLY for defects not already
listed. Do not re-derive or re-confirm anything already there — the job is
gaps. Focus on what the first pass tends to miss: error and exception paths,
concurrency and ordering, resource cleanup and leaks, boundary and empty-input
cases, and interactions between separate hunks of the same diff.

Surface up to 8 additional candidates, each naming a defect not already on
the list. If nothing new, return an empty sweep — do not pad."""


def angles(split_cleanup):
    out = [
        ("a-line-scan", "A — line-by-line diff scan", ANGLE_A),
        ("b-removed-behavior", "B — removed-behavior auditor", ANGLE_B),
        ("c-cross-file", "C — cross-file tracer", ANGLE_C),
        ("d-language-pitfalls", "D — language-pitfall specialist", ANGLE_D),
        ("e-wrapper-proxy", "E — wrapper/proxy correctness", ANGLE_E),
    ]
    if split_cleanup:
        out += [("reuse", "Reuse", REUSE),
                ("simplification", "Simplification", SIMPLIFICATION),
                ("efficiency", "Efficiency", EFFICIENCY)]
    else:
        out.append(("cleanup", "Cleanup — reuse, simplification, efficiency",
                    "This angle has three sub-topics; cover all three, 8 candidates total.\n\n"
                    f"REUSE. {REUSE}\n\nSIMPLIFICATION. {SIMPLIFICATION}\n\nEFFICIENCY. {EFFICIENCY}"))
    out += [("altitude", "Altitude", ALTITUDE),
            ("conventions", "Conventions (rules files)", CONVENTIONS)]
    return out


def akey(key):
    return key.replace("-", "_")


def finder_step(key, title, text):
    return {
        "type": "step", "id": f"find-{key}", "agent": "cr-finder",
        "prompt": (f"ANGLE KEY: {key}\nANGLE: {title}\n\n{text}\n\n"
                   "Run directory: {{rundir}}\n"
                   f"Write your candidates to: {{{{rundir}}}}/candidates/{key}.json"),
        "artifacts": {f"candidates_{akey(key)}": f"{{{{rundir}}}}/candidates/{key}.json"},
    }


def verify_loop(name, max_iterations):
    return {
        "type": "repeat", "id": f"verify-loop-{name}",
        "maxIterations": max_iterations,
        # Recall mode: an exhausted loop must not fail the review. Whatever is
        # still pending is collated as UNVERIFIED and kept.
        "onMaxIterations": "continue",
        "stopCondition": {"fileCheck": {"path": f"{{{{rundir}}}}/queues/queue-{name}.json",
                                        "jsonPath": "done", "value": True}},
        "steps": [{
            "type": "step", "id": f"verify-{name}", "agent": "cr-verifier",
            "prompt": ("Run directory: {{rundir}}\n"
                       f"Queue file: {{{{rundir}}}}/queues/queue-{name}.json\n\n"
                       "Verify the first unfinished candidate in your queue, following your protocol.\n\n"
                       "Context — the authorities named by whoever started this review (see \"Authorities "
                       "outside the code\" in your instructions):\n{{context}}"),
        }],
    }


def build(shards, split_cleanup, replay=False, only=()):
    ang = angles(split_cleanup)
    keys = ",".join(k for k, _, _ in ang)
    # 8 candidates per angle, round-robin over the shards, plus slack for an
    # iteration that repairs the queue instead of verifying.
    per_shard = -(-len(ang) * 8 // shards) + 2

    steps = [
        {"type": "step", "id": "setup", "agent": "cr-clerk", "effortLevel": "low",
         "prompt": ("You are the setup step of a code review. From the workspace root, run exactly "
                    "this one command:\n\n"
                    f"{TOOL} gather \"{{{{rundir}}}}\" \"{{{{target}}}}\" \"{{{{scope}}}}\"\n\n"
                    "It gathers the diff under review into the run directory. If it fails, report its "
                    "error verbatim and signal failure — do not retry with different arguments and do "
                    "not gather the diff by hand."),
         "artifacts": {"manifest": "{{rundir}}/manifest.json"}},

        # allSettled: one throttled or failed angle must not abort the others.
        # `crtool merge` records which angles never reported.
        {"type": "parallel", "id": "find", "joinPolicy": "allSettled",
         "branches": [finder_step(*a) for a in ang]},

        {"type": "step", "id": "dedup", "agent": "cr-clerk",
         "prompt": ("Merge and dedup the finders' candidates.\n\n"
                    f"1. Run: {TOOL} merge \"{{{{rundir}}}}\" --expect \"{keys}\"\n"
                    "2. Read EVERY digest page the command names ({{rundir}}/candidates/digest-1.txt, ...). "
                    "One line per candidate: `pid | file:line | category | summary || fails: excerpt`, sorted "
                    "by location so probable duplicates sit on adjacent lines. Do NOT read all.json. When "
                    "a decision turns on one record's full text, read {{rundir}}/candidates/raw/<pid>.json. "
                    "If the command reported problems, mention them in your final reply; do not repair files.\n"
                    "3. Group the duplicates. Candidates are duplicates only when they point at the same "
                    "line/mechanism — the same underlying defect; a group may have two members or six. Two "
                    "candidates that flag the same line for DIFFERENT reasons are NOT duplicates: leave them "
                    "out. When unsure, leave them out — a duplicate costs one extra verification, a wrong "
                    "merge loses a finding. Go through the digest location by location so that no member of "
                    "a group is left behind.\n"
                    "   Write {{rundir}}/deduped/decisions.json as strict JSON:\n"
                    '   {"groups": [{"pids": ["<pid>", "<pid>", "..."], "keep": "<pid>", "reason": "<why these '
                    'are one defect>"}]}\n'
                    "   `keep` is optional: name the member with the most concrete failure scenario if you "
                    "compared them, otherwise omit it and the script keeps the most detailed one. Write "
                    '{"groups": []} if there are no duplicates.\n'
                    f"4. Run: {TOOL} shard \"{{{{rundir}}}}\" --shards {shards}"),
         "artifacts": {"deduped_index": "{{rundir}}/deduped/index.json"}},

        {"type": "parallel", "id": "verify", "joinPolicy": "allSettled",
         "branches": [verify_loop(str(k), per_shard) for k in range(1, shards + 1)]},

        {"type": "step", "id": "sweep", "agent": "cr-finder",
         "prompt": ("ANGLE KEY: sweep\nANGLE: Phase 3 — sweep for gaps\n\n" + SWEEP + "\n\n"
                    "Run directory: {{rundir}}\n"
                    "Write your candidates to: {{rundir}}/candidates/sweep.json\n"
                    'Give each candidate an extra "id" field — "S01", "S02", ... in order.\n'
                    "Then ALSO write {{rundir}}/queues/queue-sweep.json listing those ids, so they get "
                    'verified:\n{"done": false, "ids": ["S01", "S02"], "verdict_dir": "{{rundir}}/verdicts/sweep"}\n'
                    'If you found nothing, write {"done": true, "ids": [], "verdict_dir": "{{rundir}}/verdicts/sweep"} '
                    "instead."),
         "artifacts": {"candidates_sweep": "{{rundir}}/candidates/sweep.json"}},

        verify_loop("sweep", 10),

        {"type": "step", "id": "ballots", "agent": "cr-clerk", "effortLevel": "low",
         "prompt": ("One vote proved unstable exactly where it matters: a REFUTED verdict, and any claim that "
                    "turns on how a written rule is read. Those candidates get two more independent votes. From "
                    "the workspace root, run exactly this one command:\n\n"
                    f"{TOOL} ballots \"{{{{rundir}}}}\"\n\n"
                    "It decides which candidates qualify and writes their ballot queues. If it fails, report its "
                    "error verbatim and signal failure."),
         "artifacts": {"ballots": "{{rundir}}/ballots.json"}},

        # Same verifier, same protocol, fresh sessions that cannot see the first vote:
        # one loop per extra ballot, so a candidate's second and third votes run side by side.
        {"type": "parallel", "id": "revote", "joinPolicy": "allSettled",
         "branches": [verify_loop("r1", 30), verify_loop("r2", 30)]},

        {"type": "step", "id": "rank", "agent": "cr-clerk",
         "prompt": ("Final phase of a code review: rank the verified findings.\n\n"
                    f"1. Run: {TOOL} collate \"{{{{rundir}}}}\"\n"
                    "2. Read EVERY digest page the command names ({{rundir}}/verified-digest-1.txt, ...): "
                    "`id | verdict | angles | category | file:line | summary || fails: excerpt`, one line per "
                    "(`votes R/C/P` after a verdict means three verifiers voted and that is the tally's "
                    "result) "
                    "finding that survived verification — CONFIRMED, PLAUSIBLE, or UNVERIFIED (no verdict "
                    "was produced; kept because this is a recall-mode review). `xN` is how many finder "
                    "angles raised it independently. Do NOT read verified.json; it is too large for one "
                    "read. For one finding's full text read {{rundir}}/deduped/<id>.json and "
                    "{{rundir}}/verdicts/<id>.json.\n"
                    "3. Rank ALL kept findings, most severe first. Severity = consequence x likelihood: "
                    "crashes, data loss, security holes and silently wrong results first; then wrong "
                    "behaviour on reachable edge cases; then tests that cannot fail or miss a real case; "
                    "then rules-file violations; then cleanup (reuse, simplification, efficiency, "
                    "altitude). Within a tier, CONFIRMED outranks PLAUSIBLE outranks UNVERIFIED, and a finding marked "
                    "`[by design]` (real, but the author documented it as accepted) ranks below the others.\n"
                    "   Write {{rundir}}/ranking.json as strict JSON:\n"
                    '   {"order": ["<id>", "..."], "notes": {"<id>": "optional one-line reason for a '
                    'surprising placement"}}\n'
                    "   List every kept id exactly once. The script reports the top 15 and records the "
                    "rest below the cap.\n"
                    f"4. Run: {TOOL} finalize \"{{{{rundir}}}}\""),
         "artifacts": {"findings": "{{rundir}}/findings.json", "report": "{{rundir}}/report.md"}},

        {"type": "step", "id": "comment", "agent": "cr-commenter",
         "prompt": ("Write the review comment for every reported finding.\n\n"
                    "1. Read EVERY brief page: {{rundir}}/comments/brief-1.txt (and brief-2.txt, ... if present). One "
                    "block per finding, in rank order.\n"
                    "2. For each finding, write {{rundir}}/comments/<id>.json as your instructions describe — a "
                    "Conventional Comments label, decorations, subject and discussion, or `duplicate_of` when it is "
                    "the same defect at the same place as a better-ranked finding. Open the source before you "
                    "propose a fix.\n"
                    f"3. From the workspace root, run exactly: {TOOL} comments \"{{{{rundir}}}}\"\n"
                    "   It validates your files and renders {{rundir}}/comments.json and {{rundir}}/comments.md. If it "
                    "reports a warning about one of your files, fix that file and run the command again."),
         "artifacts": {"comments": "{{rundir}}/comments.json", "comments_md": "{{rundir}}/comments.md"}},
    ]

    if only:
        # A single phase over an existing run directory, e.g. `--only comment` to (re)write the
        # review comments of a finished run for the price of one session.
        steps = [n for n in steps if n["id"] in only]
        if not steps:
            raise SystemExit(f"--only matched no top-level node; ids are {[n['id'] for n in build(shards, split_cleanup)['steps']]}")
    elif replay:
        # Everything downstream of the finders and the sweep, for re-testing the
        # bookkeeping against an existing run's candidates/*.json and sweep.json.
        steps = [n for n in steps if n["id"] not in ("setup", "find", "sweep")]

    return {
        "name": ("code-review-" + "-".join(only)) if only else ("code-review-replay" if replay else "code-review-max"),
        "description": ("Max-effort, recall-mode code review: one peer session per finder angle, "
                        "one fresh session per candidate verification, a gap sweep, then ranked findings. "
                        "Inputs: rundir = fresh ABSOLUTE run directory inside the workspace (client-minted, "
                        "e.g. <ws>/.code-review/<timestamp>); context = free text naming the documents that are "
                        "authoritative for this change (its spec/design docs, protocol references) — verifiers "
                        "consult them before confirming; crtool = the command that runs .kiro/code-review/crtool.py "
                        "on this machine, e.g. `uv run --script .kiro/code-review/crtool.py` or "
                        "`python .kiro/code-review/crtool.py`; target = a git diff target such as "
                        "`main...HEAD`, `<base>...<head>`, a commit, or `auto`; scope = space-separated git "
                        "pathspecs (`.` for everything). Results: <rundir>/findings.json and "
                        "<rundir>/report.md, plus <rundir>/comments.json and comments.md — one postable review "
                        "comment per reported finding in the Conventional Comments format. Requires "
                        ".kiro/code-review/crtool.py and the cr-finder, cr-verifier, cr-clerk, cr-commenter agents. "
                        "Generated by "
                        "experiments/code-review-workflow/build_recipe.py — edit that, not this file."),
        "inputs": {"rundir": "string", "target": "string", "scope": "string", "context": "prompt", "crtool": "string"},
        "steps": steps,
    }


def count_steps(nodes):
    n = 0
    for node in nodes:
        if node["type"] == "step":
            n += 1
        n += count_steps(node.get("steps", [])) + count_steps(node.get("branches", []))
    return n


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--shards", type=int, default=3,
                   help="parallel verify loops (default 3: with the comment step, 4 would be 21 of 20 nodes)")
    p.add_argument("--only", default="", help="emit only these top-level node ids, comma-separated (e.g. comment)")
    p.add_argument("--split-cleanup", action="store_true",
                   help="run reuse/simplification/efficiency as three finders instead of one")
    p.add_argument("--replay", action="store_true",
                   help="emit only dedup -> verify -> verify-sweep -> rank, to re-run the bookkeeping over "
                        "an existing run directory's finder and sweep outputs (no opus sessions)")
    p.add_argument("--out", default=DEFAULT_OUT)
    a = p.parse_args()

    recipe = build(a.shards, a.split_cleanup, a.replay, tuple(x for x in a.only.split(',') if x))
    n = count_steps(recipe["steps"])
    if n > STEP_CAP:
        raise SystemExit(f"{n} step nodes exceeds the engine cap of {STEP_CAP}; lower --shards")
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    with open(a.out, "w", encoding="utf-8") as f:
        json.dump(recipe, f, indent=2, ensure_ascii=False)
        f.write("\n")
    print(f"wrote {a.out}: {n}/{STEP_CAP} step nodes")


if __name__ == "__main__":
    main()
