# cyril-dn91 — prove-it findings (2026-08-02)

## Smallest question

In a `--features kas` build with **V2Engine** bound, which handled host-callback
methods answer vs refuse?

## Probe

`crates/cyril-core/src/protocol/probe_dn91.rs` — two characterization tests run
with `cargo test -p cyril-core --features kas probe_dn91`. Constructs a real
`KiroClient` (V2Engine binding mirrors `bridge.rs::resolve_host_shell`: V2 → no
host shell) and fires one representative per callback family through the real
`acp::Client` entry points.

## Oracle

`.cyril-dn91/oracle.sh` (output: `oracle-output.txt`) — an independent
**source-text census** via sed/grep/awk over `client.rs`'s production region:
enumerates dispatch arms and counts engine consults inside dispatch bodies.
Different mechanism (reads the source; the probe runs it), same conclusion.

First oracle draft disagreed (reported 3 engine references): it had leaked into
`mod tests` and counted a doc comment. Cause was oracle imprecision (skill
cause #4); fixed by restricting to the production region and excluding comment
lines. Post-fix: **agreement on every row.**

## The matrix (probe = runtime, oracle = source census; both agree)

| Family | Advertised by V2? | V2-bound behavior today | Engine consult on path? |
|---|---|---|---|
| `kiro/auth/getAccessToken` | no (caps empty) | **Answered** — responder runs, reads real credential store | none |
| `fs/read_text_file` / `write_text_file` (typed) | no | **Answered** — returns/writes real file content | none |
| `_kiro/fs/*` ×5 (read/write/stat/readDir/delete) | no | **Answered** (stat probed; all 5 share `op_for_method` dispatch) | none |
| `kiro/hooks/list` | no | **Answered** — serves (empty) registry | construction-time only (`hooks_mode` picks registry contents) |
| `kiro/hooks/executeHook` | no | **Answered — EXECUTES the wire-supplied command** (`exitCode: 0` observed) | none (no registry consult either) |
| `kiro/hooks/sessionStart` | no | (same dispatch arm family; not separately probed) | none |
| `terminal/*` ×5 + `shell_type` | no | **Refused by the responder** ("no resolved host shell") — an *indirect* engine gate via `resolve_host_shell(V2)=None`; NOT method-not-found | indirect (bridge-side host-shell resolution) |
| hooks `cancel` / `didChange` notifications | n/a | consumed | none |
| unknown ext method | n/a | protocol-default null (dcc6 F15 breadcrumb) | n/a |

Handled-variant census: **7 typed overrides + 5 ext arms + 5 `_kiro/fs/*` ops +
2 control notifications = 19** — matches cyril-g9vt's "~19" sizing exactly.

## Probe slice 2 — the AC4 per-direction corner

Under `KasEngine { hooks_mode: Kas }` (outbound mode: agent executes hooks
itself, ADR-0010; cyril installs an empty registry): inbound `hooks/list` still
**serves** `{hooks: []}` and `executeHook` still **runs arbitrary wire
commands**. There is no direction gate today — only the registry's emptiness,
which is exactly the sentinel shape the issue forbids reconciling with.

## What I learned (that I didn't know before)

The hooks family is the highest-stakes row and the least gated: `executeHook`
runs an arbitrary wire-supplied command with **no registry consult, no
engine consult, and no direction gate** — even in outbound (`Kas`) hooks mode —
while terminals are *already* indirectly engine-gated via host-shell resolution,
so the design must add a gate to hooks/auth/fs without double-gating terminals
into a different refusal shape than they have today.
