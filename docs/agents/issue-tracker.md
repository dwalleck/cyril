# Issue tracker: rivets

Issues, PRDs, and implementation tickets for this repo live in **rivets**, a
local Rust-based issue tracker with JSONL storage. The CLI is `rivets`
(installed at `~/.cargo/bin/rivets`). Storage is on-disk and in-repo — no
network round-trips, no GitHub.

> **First-time setup:** this repo is not yet a rivets repository. Run
> `rivets init` once at the repo root before creating issues. Confirm with
> `rivets info`.

## Core model

- **Issue IDs** look like `rivets-abc`. Commands that act on issues take one or
  more IDs space-separated (e.g. `rivets update rivets-abc rivets-def -s closed`).
- **Status** state machine: `open → in_progress → blocked → closed`
  (`rivets reopen` brings a closed issue back).
- **Priority**: `0`=critical, `1`=high, `2`=medium (default), `3`=low, `4`=backlog.
- **Type**: `bug`, `feature`, `task` (default), `epic`, `chore`.
- **Labels**: free-form, comma-separated — this is where triage roles live
  (see `triage-labels.md`). Triage roles are labels, *not* the status field.
- **Dependencies**: typed relationships — `blocks`, `related`, `parent-child`,
  `discovered-from`.
- Every command accepts `--json` for programmatic use — prefer it when a skill
  needs to parse output rather than show it to a human.

## When a skill says "create an issue" / "publish to the issue tracker"

```sh
rivets create --title "<title>" \
  -t <bug|feature|task|epic|chore> \
  -p <0-4> \
  -l "needs-triage" \
  -D "<description>" \
  --acceptance "<acceptance criteria>"
```

For a PRD or epic, use `-t epic` and link child issues with
`--deps "parent-child:<epic-id>"` (or `rivets dep add` after the fact). Record
an upstream/external link (a GitHub URL, a ROADMAP phase) with `--external-ref`.

## ROADMAP traceability (convention)

Every issue derived from [`docs/ROADMAP.md`](../ROADMAP.md) **must** carry its
milestone id in `--external-ref`, formatted `ROADMAP:<milestone-id>` — e.g.
`ROADMAP:KAS-2a`, `ROADMAP:K1b`. This is the durable link between a ticket and
the roadmap, and it makes coverage queryable: the set of milestones that have
issues is exactly the `ROADMAP:` external-refs in the tracker.

**Coverage check** — "which ROADMAP milestones don't have issues yet?" is the
universe (milestone headers in ROADMAP.md) minus the filed set:

```sh
# filed: milestones that have an issue
rivets list --json | python3 -c "import json,sys; \
  [print(i['external_ref']) for i in json.load(sys.stdin) \
   if str(i.get('external_ref','')).startswith('ROADMAP:')]" | sort -u
# universe: milestone headers
grep -oE '### (KAS-[0-9a-d]+|K[0-9]|Phase [0-9])' docs/ROADMAP.md | sed 's/### //' | sort -u
```

Milestones deferred rather than filed individually live as checklist items in a
**tail epic** (`-t epic`); that epic is the worklist for the next breakdown pass,
so nothing is silently dropped.

## When a skill says "fetch the relevant ticket"

```sh
rivets show <issue-id>          # human-readable
rivets show <issue-id> --json   # for parsing
```

The user will normally pass the issue ID directly.

## When a skill says "find work that's ready" / "AFK-ready"

```sh
rivets ready                    # unblocked issues, ordered by priority
rivets list -l ready-for-agent  # AFK-ready (see triage-labels.md)
```

## Triage and status transitions

- Apply or change a triage role:
  `rivets label add <issue-id> <label>` / `rivets label remove <issue-id> <label>`.
- Move status: `rivets update <issue-id> -s in_progress` (or `blocked`).
- Close: `rivets close <issue-id> -r "<reason>"`. Reopen: `rivets reopen <issue-id>`.

See `triage-labels.md` for the canonical role → label mapping.

## Useful queries

```sh
rivets list -s open             # all open issues
rivets list -t bug              # filter by type
rivets list -l needs-triage     # filter by triage label
rivets blocked                  # issues blocked by dependencies
rivets stale                    # issues that have gone quiet
rivets stats                    # project statistics
rivets dep tree <issue-id>      # dependency tree for an issue
```
