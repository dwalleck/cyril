# Issue tracker: Rivets

Issues, PRDs, and implementation tickets for this repo live in **Rivets**, a
local Rust-based issue tracker with JSONL storage. Use the `rivets` CLI.
Storage is on-disk and in-repo; GitHub Issues is not the source of truth.

The repository is initialized with:

- Database: `.rivets/issues.jsonl`
- Configuration: `.rivets/config.yaml`
- Issue prefix: `cyril`

## Core model

- **Issue IDs** look like `cyril-abc`. Commands that act on issues take one or
  more IDs space-separated.
- **Status** state machine: `open → in_progress → blocked → closed`
  (`rivets reopen` brings a closed issue back).
- **Priority**: `0`=critical, `1`=high, `2`=medium (default), `3`=low,
  `4`=backlog.
- **Kind**: `bug`, `feature`, `task` (default), `epic`, `chore`.
- **Labels**: free-form, comma-separated. Triage roles are labels, not the
  status field; see `triage-labels.md`.
- **Dependencies**: typed relationships — `blocks`, `related`, `parent-child`,
  and `discovered-from`.
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

For a PRD or epic, use `-k epic` and link child issues with
`--deps "parent-child:<epic-id>"`, or use `rivets dep add` afterward. Record an
upstream link or ROADMAP phase with `--external-ref`.

## ROADMAP traceability

Every issue derived from [`docs/ROADMAP.md`](../ROADMAP.md) must carry its
milestone ID in `--external-ref`, formatted `ROADMAP:<milestone-id>` — for
example, `ROADMAP:KAS-2a` or `ROADMAP:K1b`.

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
rivets ready
rivets list -l ready-for-agent
```

## Triage and status transitions

Apply or remove a triage role:

```sh
rivets label add <label> <issue-id>
rivets label remove <label> <issue-id>
```

For multiple issues, use `--ids <issue-id> <issue-id>...`.

Move status or close an issue:

```sh
rivets update <issue-id> -s in_progress
rivets update <issue-id> -s blocked
rivets close <issue-id> -r "<reason>"
rivets reopen <issue-id>
```

See `triage-labels.md` for the canonical role-to-label mapping.

## Useful queries

```sh
rivets list -s open
rivets list -k bug
rivets list -l needs-triage
rivets blocked
rivets stale
rivets stats
rivets dep tree <issue-id>
```
