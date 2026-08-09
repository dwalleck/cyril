# cyril-a5wo prove-it-prototype findings

## Question

Can archived kiro-cli 2.16.2 emit a live KAS subagent tool-call sequence in which `session/cancel` interrupts the call while ACP `rawInput` is incomplete, then recover the same `toolCallId`?

## Provenance

- Binary: `~/.local/share/kiro-research/binaries/2.16.2/kiro-cli-chat`
- Version: `2.16.2`
- Archive source: `https://desktop-release.q.us-east-1.amazonaws.com/2.16.2/kirocli-x86_64-linux.tar.zst`
- Manifest-verified archive SHA-256: `8bcd1d939604ce3b8adafe7c321459a9a9f27fe9afaaedf519d6d4e21e8e0b4f`
- Authentication: refreshed immediately before the bounded run.
- Probe: `probe-subagent-interrupt.py` (100 lines).
- Captures: `captures/attempt-{1,2,3}.jsonl`; credentials scrubbed during emission.

## Probe result

Three fresh authenticated attempts each launched a KAS subagent and injected `session/cancel` immediately after the first `agent-subtask` `tool_call` frame:

| Attempt | Cancel injected | Session updates | Same-ID recovery frames | Incomplete ACP rawInput |
|---|---:|---:|---|---|
| 1 | yes | 14 | pending → in_progress → failed | no |
| 2 | yes | 14 | pending → in_progress → failed | no |
| 3 | yes | 14 | pending → in_progress → failed | no |

Attempt 1's ordered evidence:

1. `tool_call` id `invoke_subagent_tooluse_coj4cd8N6SgFbzPA2Voz9R`, status `pending`, kind `agent-subtask`.
2. Client `session/cancel` for the owning session.
3. Same-id `tool_call_update`, status `in_progress`.
4. Same-id `tool_call_update`, status `failed`.
5. KAS `turn_completion` status `aborted` and `turn_end` stop reason `cancelled`.

The initial frame and both updates all carried the same complete `rawInput` object with keys `contextFiles`, `explanation`, `name`, and `prompt`. No frame exposed a partial object or an absent `rawInput` for the interrupted call.

## Oracle

`oracle-subagent-interrupt.py` independently parses only JSONL frame predicates. For each capture it reports:

- whether a client-to-agent `session/cancel` exists;
- subagent tool-call frames grouped by exact `toolCallId`;
- whether any grouped frame has absent or structurally incomplete `rawInput`;
- whether the same id receives recovery/update frames.

All three outputs were `FAIL`: cancel and same-id recovery were present, but `partial_raw_input` was empty. The oracle therefore does not agree with the required feature premise.

## What I learned

KAS 2.16.2 exposes a deterministic cancelled-subagent lifecycle on ACP—`pending → in_progress → failed` for one `toolCallId`—but by the earliest observable `agent-subtask` frame its argument object is already complete; immediate client cancellation cannot reproduce the release-note's internal “mid-arguments” condition at this ACP seam.

## Gate result

**BLOCKED.** The signed spec requires stopping after three fresh attempts if the subagent mid-arguments sequence is absent. The captures are valid supplemental cancellation/recovery evidence, but they do not satisfy the live partial-rawInput criterion and cannot advance to falsifiable design.
