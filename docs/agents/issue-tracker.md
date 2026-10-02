# Issue tracker: Rivets

Issues, PRDs, and implementation tickets for this repo live in **Rivets**, a
local Rust-based issue tracker with JSONL storage. Use the `rivets` CLI.
Storage is on-disk and in-repo; GitHub Issues is not the source of truth.

The repository is initialized with:

- Database: `.rivets/issues.jsonl`
- Configuration: `.rivets/config.yaml`
- Issue prefix: `cyril`

Every mutation rewrites `.rivets/issues.jsonl`. Commit it right away, on `main`
from the primary checkout, as `chore(rivets): …`.

## Core model

- **Issue IDs** look like `cyril-abc`. Commands that act on several issues take
  the IDs space-separated.
- **Status**: `open`, `in_progress`, `closed`. Moving an issue into progress
  takes two steps: `claim` assigns it, then `start` moves it (see below).
  `rivets reopen` brings a closed issue back. "Blocked" is not a status: an
  issue is blocked while it has an open blocking prerequisite (`rivets blocked`).
- **Priority**: `0`=critical, `1`=high, `2`=medium (default), `3`=low,
  `4`=backlog.
- **Kind**: `bug`, `feature`, `task` (default), `epic`, `chore`.
- **Labels**: lowercase alphanumeric with hyphens/underscores; comma-separated
  on `create -l`. Triage roles are labels, not the status field; see
  `triage-labels.md`.
- **Relationships**: each kind has its own command with explicit roles:
  `blocking-dependency` (dependent → prerequisite, blocks readiness),
  `parent` (child → epic), `related` (symmetric), and `discovery`
  (discovered issue → the issue whose work surfaced it).
- **Resources**: typed links (repo path or URL) attached with `rivets resource`.
  These replace the old `--external-ref` field.
- Prefer `--json` when a skill needs to parse output. Use `-y` for
  non-interactive mutations.

## When a skill says "create an issue" or "publish to the issue tracker"

```sh
rivets create --json -y \
  --title "<title>" \
  -k <bug|feature|task|epic|chore> \
  -p <0-4> \
  -l "needs-triage" \
  -D "<description>" \
  --acceptance "<acceptance criteria>"
```

`create` also takes `--design`, `--notes`, and `--prerequisite <issue-id>`
(repeat it for several blocking prerequisites). It has no flags for parents,
discovery origins, or resources. Add those after the issue exists:

```sh
rivets parent set -y --child <issue-id> --parent <epic-id>
rivets discovery add -y --discovered <issue-id> --source <origin-issue-id>
rivets related add -y --issue <issue-id> --related <other-issue-id>
rivets blocking-dependency add -y --dependent <issue-id> --prerequisite <other-issue-id>
rivets resource add -y --role <implementation|documentation|evidence|successor|reference> \
  (--path <repo-relative-path> | --url <https-url>) [--label "<label>"] <issue-id>
```

For a PRD or epic, create it with `-k epic`, then attach each child with
`rivets parent set`.

For long descriptions, write the text to a file and pass `-D "$(cat file)"`
rather than inlining a heredoc.

## ROADMAP traceability

Every issue derived from [`docs/ROADMAP.md`](../ROADMAP.md) must carry its
milestone ID as a documentation resource labelled `ROADMAP:<milestone-id>` —
for example, `ROADMAP:KAS-2a` or `ROADMAP:K1b`:

```sh
rivets resource add -y --role documentation --path docs/ROADMAP.md \
  --label "ROADMAP:<milestone-id>" <issue-id>
```

Milestones deferred rather than filed individually live as checklist items in
a **tail epic** (`-k epic`). That epic is the worklist for the next breakdown
pass, so nothing is silently dropped.

## When a skill says "fetch the relevant ticket"

```sh
rivets show <issue-id>
rivets show <issue-id> --json
```

The user will normally pass the issue ID directly.

## When a skill says "find ready work" or "AFK-ready"

```sh
rivets ready                      # open, unblocked, unassigned
rivets ready --all-assignees
rivets list -l ready-for-agent -n 50
```

## Triage and status transitions

Apply or remove a triage role (label first, then the issue ID):

```sh
rivets label add <label> <issue-id>
rivets label remove <label> <issue-id>
```

For multiple issues, use `--ids <issue-id> <issue-id>...`.

Move status or close an issue:

```sh
rivets claim -a <assignee> <issue-id>      # assign an open, unblocked issue
rivets start <issue-id>                    # assigned open → in_progress
rivets return-to-open <issue-id>           # in_progress → open
rivets release -a <assignee> <issue-id>    # drop the assignment of an open issue
rivets close <issue-id> -r "<reason>"
rivets reopen <issue-id>
```

Record progress without touching the description:

```sh
rivets note append --content "<note>" <issue-id>
```

`rivets update` changes `--title`, `-D`, `-p`, `-k`, `--design`, and
`--acceptance` only. It replaces each field you pass outright, so never pass an
empty string such as `-D ""`: that erases the field.

See `triage-labels.md` for the canonical role-to-label mapping.

## Useful queries

`rivets list` requires `-n/--limit`.

```sh
rivets list -s open -n 50
rivets list -k bug -n 50
rivets list -l needs-triage -n 50
rivets blocked
rivets stale
rivets stats
rivets blocking-dependency tree --dependent <issue-id>
rivets parent show --child <issue-id>
rivets discovery list --discovered <issue-id>
rivets resource list <issue-id>
```
