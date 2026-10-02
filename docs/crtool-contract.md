# crtool contract

What the native `cyril crtool` must do for the `code-review-max` workflow, and
how exact it must be. This page replaces the "byte-identical to the Python
crtool" requirement (ROADMAP W3; cyril-5gb3 user story 56). Algorithms are
not restated here: `.kiro/code-review/crtool.py` stays the reference
implementation, and this page decides which of its outputs must match and
which may differ.

## What crtool is

crtool moves data between the workflow's model steps so that no model retypes
another's text. It reads the JSON files agents write, reshapes them, and writes
JSON plus text pages sized for one tool read. Its only outside-world work is
`gather`: a scoped `git diff`, split per file, plus a `git grep` usage map.

Its readers are the four review agents (finder, verifier, clerk, commenter) and
cyril's `/review` (it parses `manifest.json` and `findings.json`).

## Fidelity rule

The reason for parity is the recipe's measured calibration (14/20): the port
must not change what the models see. Exactness therefore follows the reader.

| Output | Must match Python |
|---|---|
| **Model-read text**: each command's stdout, every digest/usages/brief page, `report.md`, `comments.md`, the `body` strings in `comments.json` | Same text for ordinary inputs |
| **JSON files** | Same keys, value types and values. Indentation, key order and escaping are free |
| **Exit status** | Same codes (below) |

**Ordinary inputs:** a git work tree with UTF-8 file names that contain no
newline or tab, on Linux, macOS or Windows. Outside that, crtool may decode
names lossily or refuse with exit 2; it must never crash or silently write a
different file.

**Newlines:** crtool writes LF on every OS, including Windows (Python's text
mode would write CRLF there). Decoding git output and agent files uses
`\n`/`\r\n` line splitting, not Python's `str.splitlines` separator set.

### Deliberate deviations from `crtool.py`

| Area | Python | Native |
|---|---|---|
| Usage search | `git grep` limited to the cwd subtree | Whole repository (`:(top)` pathspecs) |
| Usage search cost | One `git grep` per symbol | One `git grep -e … -e …` for all symbols |
| Diff output | Affected by user config (`diff.submodule`, `diff.relative`, color, ext-diff) | Independent of user diff config (with the git CLI: `--no-relative --no-color --no-ext-diff --no-textconv --submodule=short`) |
| Self-exclusion | Skips hits under `.code-review/` | Skips hits under the actual run directory |
| Odd values in agent records | Python `str()`/`repr()`/truthiness: `None`, `0` and `[]` count as empty, a float line is truncated | Strings and integers as agents are told to write them; any other value prints as its JSON text; only absent, `null` and `""` count as empty; a line is an integer or an integer string |
| Parse-error text | Python `json` messages inside `unreadable` warnings | serde_json messages |
| Per-file patch | `git diff -- <path>`, read as a glob (`src/[id].tsx` matches `src/i.tsx`) | `:(literal)<path>` |
| `scope` | One space-split string | Same on the command line (the recipe passes one string); pathspecs containing spaces are unsupported |

Adding a deviation requires a row here.

## Invocation

```
cyril crtool gather   <rundir> <target> [<scope>]
cyril crtool facts    <rundir>
cyril crtool merge    <rundir> --expect <angle,angle,...>
cyril crtool shard    <rundir> [--shards N]          # default 4
cyril crtool ballots  <rundir>
cyril crtool collate  <rundir>
cyril crtool finalize <rundir>
cyril crtool comments <rundir> [--no-trailer]
```

- Hidden from `--help`; dispatched before any config, log or terminal setup.
- The argv grammar must match the recipe's command lines exactly: the review
  policy allowlists shell commands by parsing them.
- **cwd is the repository root.** `/review` refuses to start anywhere else, and
  the recipe runs every command "from the workspace root". crtool refuses to
  run anywhere else (exit 2). Paths in all outputs
  are repository-root-relative with forward slashes. `verdict_dir` values in
  queue files are absolute, with forward slashes.
- **Exit codes:** `0` success; `2` any error, printed as one line
  `crtool: error: <message>` on stderr; `3` empty diff (`gather`).
- **Version stamp:** `gather` writes `crtool_version` (the cyril version) into
  `manifest.json`. Every later subcommand refuses a missing or different stamp
  with exit 2.
- **No concurrency:** steps on one run directory never overlap. `/review`
  runs `gather` and then the check command before the workflow starts. Each
  crtool step runs alone in its workflow step. crtool takes no locks. Writes
  are atomic (temp file + rename) so a crash never leaves a half-written file.

## Run directory

`<rundir>` is created by `/review` (or the caller). `W` = writer, `R` = readers.

| File | W | R | Shape (see `crtool.py` for every field) |
|---|---|---|---|
| `manifest.json` | gather, facts | all agents, `/review` | `requested_target, target, scope[], head, worktree_matches_diff_head, gathered_at, crtool_version, total_files, total_patch_bytes, warnings[], files[{index, path, status, insertions?, deletions?, binary, patch, patch_bytes}], change_docs[{path, bytes}], facts{symbols, usages_pages[], diagnostics?, diagnostics_status?}` |
| `diff.patch`, `patches/NNN.patch`, `changed-files.txt` | gather | finder, verifier | raw `git diff` bytes per file |
| `facts/symbols.json`, `facts/usages-N.txt` | gather/facts | finder, verifier | symbols with usages; usage pages |
| `facts/diagnostics.txt`, `facts/diagnostics-raw.txt` | `/review` check | finder, verifier | see **Check command** |
| `candidates/<angle>.json` | finder | merge | `{"angle", "candidates":[{file, line, summary, failure_scenario, evidence, category}]}` or a bare list |
| `candidates/all.json`, `raw/<pid>.json`, `digest-N.txt` | merge | clerk | pids `<angle>-<n>` |
| `deduped/decisions.json` | clerk | shard | `{"groups":[{pids[], keep?, reason}]}` (older `{"duplicates":[{drop, keep, reason}]}` also accepted) |
| `deduped/index.json`, `Cnn.json`, `digest-N.txt`; `queues/queue-K.json`; `verdicts/qK/` | shard | verifier, sweep | ids `C01…`; queue `{done, ids[], verdict_dir}` |
| `candidates/sweep.json`, `queues/queue-sweep.json` | sweep finder | ballots, collate, verifier | ids `S01…` |
| `<verdict_dir>/<id>.json` | verifier | ballots, collate | `{id, verdict, evidence, reasoning, would_confirm?, by_design?, corrected_line?}` |
| `ballots.json`, `deduped/<id>.v2/.v3.json`, `queues/queue-r1/r2.json` | ballots | verifier, collate | |
| `verified.json`, `verified-digest-N.txt` | collate | clerk, finalize, comments | `{kept[], refuted[], stats, warnings[]}` |
| `ranking.json` | clerk | finalize | `{"order":[ids], "notes":{id: text}}` |
| `findings.json`, `report.md`, `comments/brief-N.txt` | finalize | commenter, `/review` | top 15 |
| `comments/<id>.json` | commenter | comments | `{label, decorations[], subject, discussion}` or `{duplicate_of}` |
| `comments.json`, `comments.md` (+ `findings.json`, `report.md` updated) | comments | `/review`, people | |

**Fixed numbers** (match Python): 8 candidates per angle, 15 reported findings,
20,000 characters per text page, usage caps 25 → 12 → 6 → 3 until the usage
map fits in 4 pages, 140-character usage lines, at most 40 `change_docs`.

**Agent-written files are untrusted input.** A missing, unreadable or
malformed agent file is never a crash. crtool degrades exactly as `crtool.py`
does (a warning in the run's `warnings`, a template comment, `UNVERIFIED`), and
those warnings must reach the model-read output.

## Check command (not a crtool subcommand)

The repository's `[review] check_cmd` runs once, in `/review`, after `gather`
and before the workflow starts. It is a library function in `cyril-review`,
never a crtool subcommand, so the review policy can never allowlist an
arbitrary command.

- POSIX: split the command with shell-word rules (the `shlex` crate). Windows:
  pass the string to `CreateProcess` unchanged. No shell either way.
- Drop empty `CARGO_*`/`RUST*` environment variables (an empty
  `CARGO_TARGET_DIR` breaks cargo).
- Stdin is null. Stdout and stderr are captured in full.
- Timeout from `[review] check_timeout_s` (default 1800 s). The caller can
  cancel. On timeout or cancel, kill the direct child and keep the output
  collected so far. A failure to kill is reported in the result; it never
  discards that output.
- A stream still open 2 seconds after the command ended (a background
  grandchild holding the pipe) is abandoned rather than waited on; the output
  read so far is kept and the report says so.
- Writes `facts/diagnostics-raw.txt` (stdout, newline, stderr) and
  `facts/diagnostics.txt` (command, status, elapsed, HEAD; up to 200 lines
  mentioning a changed file; the last 15 lines), and sets `facts.diagnostics`
  and `facts.diagnostics_status` in `manifest.json`. Status text: `clean`,
  `FAILED (exit N)`, `TIMED OUT`, `CANCELLED`.

Implementation: one code path on every OS, with no polling loop and no
platform-specific process code. With tokio that is `tokio::process::Command`,
two `read_to_end` tasks for the streams, and `child.wait()` in a `select!`
against the deadline and a cancellation token. Without tokio it is
`std::process::Command`, one reader thread per stream, and a blocking wait the
cancel path can interrupt by killing the child.

## Implementation constraints

- **Reach for a library when it removes code.** `git2`/`gix` and `tokio` are
  approved for this crate when they simplify the work. The test is net code and
  behaviour to maintain, not dependency count.
- **One git engine.** Use either the git CLI (explicit argv, never a shell) or
  a git library for the diff and the per-file split. Never produce text with one
  and re-parse it with the other.
- **Use the library, not a reimplementation.** For example, take shell-word
  splitting from the `shlex` crate and timestamps from a time crate, rather than
  copying Python's behaviour by hand.
- No Windows-only crates. The same code runs on every OS.
- Size budget: about 2,000 lines of non-test Rust for all eight subcommands and
  the check function. A change that would exceed it needs a stated reason.

## Testing

- **Golden runs:** a few fixture repositories with recorded agent outputs
  (candidates, decisions, verdicts, ranking, comments). Each covers a normal
  review, a missing finder angle, malformed agent files, ballots that overturn
  a verdict, and a sweep. The Python outputs are captured once and committed.
  The native outputs are compared under the fidelity rule.
- **Unit tests** for the pure parts: vote tallying, duplicate grouping, page
  splitting, `one_line`, the template comment, comment validation.
- **Edge cases outside "ordinary inputs" get no dedicated tests.** A failing
  edge case found in use becomes a bug with its own test.
- Runs on Linux and Windows in CI, plus macOS for `gather`.
- `crtool.py` stays in the repo as the reference until W3 ships. No live
  Python differential oracle runs in CI.
