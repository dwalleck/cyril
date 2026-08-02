# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those
roles to the actual label strings used in this repo's Rivets tracker.

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |

When a skill mentions a role, use the corresponding tracker label from this
table.

## Applying labels in Rivets

Triage roles are Rivets labels, kept separate from the `status` field
(`open`, `in_progress`, `blocked`, or `closed`). An issue can be `in_progress`
and carry a triage role at the same time.

```sh
rivets label add ready-for-agent <issue-id>
rivets label remove needs-triage <issue-id>
rivets label list <issue-id>
rivets list -l ready-for-agent
```

A triage transition normally removes the old role and adds the new role:

```sh
rivets label remove needs-triage <issue-id>
rivets label add ready-for-agent <issue-id>
```

Edit the right-hand column of the table above if the tracker vocabulary changes.

## Area and milestone labels

Work groupings are labels applied alongside a triage role. An issue normally
carries one role label plus one or more area labels.

| Label           | Grouping                                                                  |
| --------------- | ------------------------------------------------------------------------- |
| `kas`           | KAS engine integration track (ROADMAP KAS-1…8)                            |
| `usability`     | Theme and responsive-layout work                                          |
| `code-health`   | Cleanup, hygiene, dead API, and cache-correctness work                     |
| `steering`      | Queue-steering subsystem                                                   |
| `bridge`        | Bridge lifecycle, notification ordering, and turn-completion races        |
| `acp`           | ACP RPC-layer concerns, often paired with `bridge`                         |
| `docs`          | Documentation synchronization work                                        |
| `dev-workflow`  | Local gates, CI, and audit tooling                                         |
| `release-watch` | Audit tripwires; no role label until the watched signal fires              |

Conventions:

- `release-watch` is a disposition, not an area. Those issues deliberately
  carry no role label until their watched behavior appears.
- ROADMAP milestone IDs live in `external-ref` as `ROADMAP:<id>`, not in labels.
- Keep new area names lowercase-kebab and document them here to avoid synonyms.
