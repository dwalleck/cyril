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
