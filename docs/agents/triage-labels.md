# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those
roles to the actual label strings used in this repo's issue tracker (rivets).

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |

When a skill mentions a role (e.g. "apply the AFK-ready triage label"), use the
corresponding label string from this table.

## Applying labels in rivets

Triage roles are **rivets labels**, kept separate from the `status` field
(`open`/`in_progress`/`blocked`/`closed`). A ticket can be `in_progress` *and*
`ready-for-human` at the same time.

```sh
rivets label add <issue-id> ready-for-agent      # move a role on
rivets label remove <issue-id> needs-triage      # move a role off
rivets label list <issue-id>                      # roles on one issue
rivets list -l ready-for-agent                    # find issues in a role
```

A triage transition is usually "remove the old role, add the new one" — e.g.
graduating from triage to AFK-ready:

```sh
rivets label remove <issue-id> needs-triage
rivets label add    <issue-id> ready-for-agent
```

Edit the right-hand column of the table above to match whatever vocabulary you
actually use.

## Area / milestone labels

Rivets has no milestone field, so work groupings are also labels, applied
alongside the triage role. A triaged issue normally carries **one role label
plus one or more area labels**. Labels are sets, not partitions — an issue may
sit in several areas (e.g. `kas` + `bridge`).

| Label           | Grouping                                                                  |
| --------------- | ------------------------------------------------------------------------- |
| `kas`           | KAS engine integration track (ROADMAP KAS-1…8)                            |
| `usability`     | Theme + responsive-layout milestone; capstone is the visual-regression contract (cyril-8r3u) |
| `code-health`   | Cleanup / hygiene batches (review findings, dead API, cache correctness)  |
| `steering`      | Queue-steering subsystem (K1 track: chip, echoes, clear)                  |
| `bridge`        | Bridge lifecycle, notification ordering, turn-completion races            |
| `acp`           | ACP rpc-layer concerns (often paired with `bridge`)                       |
| `docs`          | Documentation sync work                                                   |
| `dev-workflow`  | Local gates, CI, audit tooling                                            |
| `release-watch` | Audit tripwires — no role label; re-checked during each kiro-cli release audit, actioned only when the watched signal fires |

Conventions:

- `release-watch` is a **disposition**, not an area: those issues deliberately
  carry no role label because there is nothing to do until the watched
  behavior appears on the wire.
- ROADMAP milestone ids do NOT go in labels — they live in `external-ref` as
  `ROADMAP:<id>` (see `issue-tracker.md`).
- Query a milestone with `rivets list -l usability`; combine with a role to
  find work, e.g. `rivets list -l kas -l ready-for-agent`.
- New areas are cheap: keep names lowercase-kebab, add a row here so future
  triage passes reuse the same buckets instead of minting synonyms.
