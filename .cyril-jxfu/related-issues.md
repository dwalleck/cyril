# cyril-jxfu — related issues (prove-it-prototype step 0)

Tracker searched 2026-08-10 (`rivets list -n 200`, keyword pass on
rout/workflow/subagent/session/stream). Direct hits:

- **cyril-tglp** (closed, P4) — origin of `classify_notification_route` and the
  Drop arm. Its design notes are quoted verbatim in this issue's design notes:
  the classifier stays TOTAL, no routing decision leaks to the caller. The truth
  table test it left behind is the one AC2 extends.
- **cyril-6beh** (closed, P2, blocks this) — shipped the `_kiro/workflow/*`
  converter + `WorkflowTracker` (PR #92). `apply_node_started` already
  merge-not-appends `session_id` per node (double-emit hazard handled at the
  tracker level). This issue is the first consumer of that data for routing.
- **cyril-a71q** (closed, P3) — C7 introduced the routing truth table as a
  stress fixture ("only the owned release mutates MAIN state").
- **cyril-fh06** (closed, P2) — prior misroute of the same shape: metadata
  frames lacking per-session routing stamped the main toolbar. Same bug class,
  different channel.
- **cyril-mys8** (open, P3) — arch review C03 wants routing+application
  concentrated behind one App-seam module. This change should not scatter new
  routing decisions; keep the classifier the single seam so mys8 stays cheap.
- **cyril-0qe6** (open, P2, blocked by this) — the /workflow command family and
  run lifecycle; consumes the route this issue adds.
- **cyril-ebqu / cyril-fjfu** (open) — explicitly NOT this mechanism
  (agent-subtask tool-call channel).
- **cyril-7sjs / cyril-sinu** (open, P3) — workflow-converter hardening; no
  overlap with routing but touch the same module family.

No open issue already covers the misroute itself — cyril-jxfu is the first
filing for it. No prior probe artifacts for routing exist beyond the truth
table test in `crates/cyril/src/app.rs`.
