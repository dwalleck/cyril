# Live check of the queue release (C12) — 2026-10-18, binary 43f3ad4a

Setup is as in `live-summary.md`: private `HOME` at `<scratch>/live/queue/home`, `SHELL=fish`,
`--agent-engine kas`, KAS 0.66.43.

| Step | Observation | Receipt |
|---|---|---|
| Prompt: three parallel shell calls (`echo QA1/QB2/QC3`) | Only **one** approval pending (`echo QA1`). No queue formed. | `live-queue-1-first-prompt.txt` |
| Always allow → `Trust entire tool (run_command)` at session | All three commands ran and the turn ended with no further prompt | `live-queue-2-after-grant.txt` |
| KAS log | One `acp-peer.request_permission` for the whole turn | (KAS `kiro.log` under the private HOME) |

**Result: the release path could not be triggered live in this shape.**

On the main session, KAS 0.66.43 issued one permission request at a time even for parallel tool calls. Its own
session grant then covered the rest, so no approvals were waiting behind the granted one. A real queue needs
concurrent permission requests, which come from parallel workflow steps or sub-agents. Driving those live means a
multi-step parallel workflow run, which I did not set up.

C12 is therefore established by the `UiState` tests (`always_allow_tests`) and the red/green receipts in
`queue-red.txt`. C11 is established by the core coverage matrix. The live run does confirm two things on this
binary: the scope page works, and the whole-tool grant answers with no regression.
