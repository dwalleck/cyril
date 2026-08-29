# Cyril queries and toggles KAS's own hook registry; opting into agent-side hooks is the trust decision

Status: accepted (2026-08-01, cyril-gk17) — extends [ADR-0003](0003-defer-proxy-stack-for-host-callbacks.md) and [ADR-0004](0004-bridge-loop-mediates-both-acp-directions.md)

## Context

ADR-0003 named hooks (KAS-7) as one of the side-effect concerns host callbacks
would deliver. That framing assumed one direction: the agent asks, cyril
executes. The v2 hook generation inverts it.

The two models do not compose per session — KAS's `buildSessionHooks` is
winner-take-all. With `_meta.kiro.hooks = {enabled: true, v2: true}` the
agent's own file-watched `.kiro/hooks` loader **replaces** the host callbacks
wholesale, and the agent executes the commands itself via its own
`processRunner`. `_kiro/hooks/executeHook` is never sent to the client. So
`_kiro/hooks/list` is genuinely **bidirectional**, and which direction is live
depends on which hook generation the client advertised.

`kas_hooks = "kas"` (cyril-jiyn) already shipped that advertisement — including
the object-vs-boolean shape two earlier probes got wrong. What was missing is
the client→agent direction: in that mode cyril had no way to see or change what
would execute on its own host.

## Decision

- **Cyril sends `_kiro/hooks/list` and `_kiro/hooks/setEnabled` outbound**, as
  `BridgeCommand`s, while `host` mode continues to *serve* `_kiro/hooks/list`
  inbound. Same method name, opposite direction, different registry — which is
  the concrete case ADR-0004 anticipated in saying the bridge loop mediates both
  ACP directions.
- **`/hooks` is registered only for (KAS engine × `kas` hooks mode).** The
  (KAS, `host`) cell is the one that matters: cyril owns the registry there, so
  asking the agent would query a registry it does not have. Under v2 registering
  it would shadow the agent's own advertised command.
- **`didChange` replaces the registry rather than merging it.** A hook deleted
  on disk must stop being addressable.
- **An ambiguous `/hooks enable|disable <name>` refuses and lists candidates.**
  `setEnabled` rewrites the `enabled` flag in a file on the user's disk;
  guessing which file is not an acceptable failure mode.
- **Default stays `host`.** Agent-side execution is opt-in.

## The trust decision

cyril-gk17 required a deliberate answer to two questions before advertising
`hooks.v2`: how cyril establishes workspace trust, and whether it surfaces hook
execution in the transcript.

- **Workspace trust: cyril does not implement a prompt, and this is a knowingly
  partial answer.** It has no trust store to consult, and inventing one would
  give a false assurance. KAS's own `workspaceTrusted` flag — which feeds
  `disabledReason: "untrusted-workspace"` — is the mechanism that belongs in
  that role; wiring it is **cyril-mq15**. Until then the compensating control is
  that the mode is opt-in.
- **The honest limit of that control:** `kas_hooks` is a **global** config knob,
  so a single opt-in trusts every workspace cyril is ever launched in, including
  one cloned later. This is weaker than the per-workspace guard the upstream
  flag provides, and it is the reason cyril-mq15 is not optional polish.
- **Transcript surfacing: yes, for every state KAS reports** — `hook_update`
  frames become `Notification::HookExecuted`, including `running` and
  `awaiting_approval`, so a hook that hangs still leaves a record.
- **The audit trail is nonetheless incomplete, and the gap is upstream.** In
  `kas-v2hooks-2.16.0.jsonl` two hooks executed (host-side evidence file) but
  KAS emitted exactly **one** `hook_update`, for the `preToolUse` hook; the
  `sessionStart` hook reported nothing. Cyril cannot surface a frame that is
  never sent. Opting in therefore accepts some hook execution that leaves no
  record at all — stated here rather than left as an unexamined claim that
  execution is "never silent".
- **`HookExecuted` is a record, never a gate.** Nothing in this path can refuse
  a hook. Under `kas_hooks = "kas"` the agent runs `.kiro/hooks/*.json` shell
  commands on this host; no `session/request_permission` precedes them
  (live-verified — the capture holds zero permission frames).

## Considered options

- **Advertise `v2` and say nothing about trust** — rejected: gk17 conditioned
  the advertisement on a recorded decision, and an unrecorded one is how a
  security posture becomes accidental.
- **Invent a cyril-side trust prompt** — rejected: no store to back it, and a
  prompt that cannot remember its answer trains users to dismiss it.
- **Refuse to support `v2` until cyril-mq15 lands** — rejected: the mode is
  reachable and useful today, and users who opt in currently get *no*
  visibility at all, which is strictly worse than opt-in plus a partial trail.

## Consequences

- Wire→display translation covers the three file/wire divergences (composite
  `"<filePath>#hook-N"` id, `runCommand` action tag, PascalCase `_meta.trigger`),
  fixtured on the live 2.16.0 capture.
- `HookInfo` gains `id`/`name`/`enabled` as `Option`s with
  `skip_serializing_if`, so the v2 three-field projection still round-trips
  unchanged. `enabled: None` — every v2 hook — is *not* rendered as disabled:
  unknown is not disabled.
- `didChange` refreshes an already-open panel and is otherwise inert: it fires
  on any hook-file edit, and an overlay that opens itself over the user's input
  is worse than a stale one.
- **Follow-up (cyril-mq15) is load-bearing, not optional.** Until
  `workspaceTrusted` is wired, the trust story is "you opted in globally" plus
  an incomplete transcript.
- **The exit-2 gate is now live-verified (2026-08-29).** This ADR's premise —
  that `host` mode buys an org write/exec-policy gate — rested on a 2.7.1
  capture absent from the repo (`.cyril-jiyn/findings.md` caveat 1). A matched
  observe/block pair on 2.20.1 / KAS 0.54.3 confirms it: exit 2 stops the tool
  on three independent oracles, and the hook's `output` reaches the model
  verbatim as the denial reason, treated as non-retryable — so a host-mode hook
  can redirect, not only refuse. Two riders qualify the gate: an exit-0 hook
  returning no verdict still blocks the *first* attempt and the model retries
  (so answering `executeHook` for non-matching commands costs a round trip per
  tool call), and `_kiro/hooks/*` is absent from 0.54.3's advertised
  `extensionMethods` while remaining fully functional. Evidence and probe:
  `docs/kiro-2.20.1-wire-audit.md` §9.
