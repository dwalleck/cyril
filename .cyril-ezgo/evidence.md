# Evidence: cyril-ezgo

## Premise checklist

| ID | Candidate premise | Smallest question | Verdict |
|----|-------------------|-------------------|---------|
| P1 | A canonical Git common directory can serve as one stable project identity for a repository's primary checkout and linked worktrees while each workspace retains its own canonical display path; a non-Git workspace can use its canonical workspace path for both. | Do a real primary checkout and linked worktree resolve to the same canonical Git common directory, do their canonical display paths remain distinct, and does a non-Git directory resolve to itself? | PASS |
| N1 | Lesson schema, validation, redaction, ordering, budgeting, and prompt-injection behavior. | N/A — these are feature behavior and design claims; no implementation exists to probe. | N/A — feature behavior, not an existing-system premise |

## Data

- Source: production-shaped
- Shape: the real Cyril primary checkout, the real linked feature worktree created by `scripts/session-worktree.sh`, and the real non-Git `/tmp` directory; these cover primary-worktree `.git` directory, linked-worktree `.git` indirection plus `commondir`, and no-Git fallback shapes.
- Safety: both mechanisms perform read-only path and Git-metadata inspection. They do not write, mutate, or delete repository, tracker, Git, memory-runtime, or user data.

## Probe

- File: `probe.py`
- Mechanism: a standalone Python resolver canonicalizes the workspace, walks ancestors, follows a `.git` file's `gitdir:` target and `commondir` marker, and otherwise returns the canonical workspace.
- Run: `python .cyril-ezgo/probe.py main /home/dwalleck/repos/cyril linked /home/dwalleck/repos/cyril-wt-feat-cyril-ezgo nongit /tmp`

## Oracle

- Mechanism: Git's own `rev-parse --path-format=absolute --git-common-dir` computes repository identity independently of the probe's marker parser; the separate `realpath` executable computes canonical display and non-Git paths. The production implementation will be Rust domain code, so neither oracle shares its implementation failure mechanism.
- Run: `git -C /home/dwalleck/repos/cyril rev-parse --path-format=absolute --git-common-dir && git -C /home/dwalleck/repos/cyril-wt-feat-cyril-ezgo rev-parse --path-format=absolute --git-common-dir && realpath /home/dwalleck/repos/cyril && realpath /home/dwalleck/repos/cyril-wt-feat-cyril-ezgo && realpath /tmp`

## Comparisons

| ID | Probe output | Oracle output | Verdict |
|----|--------------|---------------|---------|
| P1 | `main` project `/home/dwalleck/repos/cyril/.git`, display `/home/dwalleck/repos/cyril`; `linked` project `/home/dwalleck/repos/cyril/.git`, display `/home/dwalleck/repos/cyril-wt-feat-cyril-ezgo`; `nongit` project/display `/tmp`. | Git returned `/home/dwalleck/repos/cyril/.git` for both primary and linked worktrees. `realpath` returned distinct canonical workspace paths and `/tmp` for the non-Git directory. | PASS |

## Validated / learned

- P1: validated prior understanding — the real primary checkout and linked worktree share the canonical Git common directory while preserving distinct display paths, and a non-Git canonical workspace is a stable self-identity.

## Related issues

- Consulted: `cyril-ct0y` establishes project-scoped memory and original-prompt separation; `cyril-ezgo` owns explicit-lesson project identity and first-prompt injection; `cyril-n3j7` reuses the same project scope for later derived episodes and requires injected context exclusion; `cyril-3dqf` later binds scoped recall capabilities to the resolved project without trusting agent-supplied identity.
- Filed: none — probe and oracle agree; no underlying-system defect or deferred work was discovered.
