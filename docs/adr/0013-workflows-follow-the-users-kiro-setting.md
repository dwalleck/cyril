# KAS workflows follow the user's Kiro setting; cyril keeps its own control plane beside the model's launch path

Status: accepted (2026-10-18)

Supersedes: [ADR-0011](0011-ungated-client-driven-workflow-control-plane.md), carrying its cyril-lki9 amendment forward

Decision approval: grilling session, 2026-10-18 (Q1–Q26); shared understanding confirmed by the user

## Context

ADR-0011 (2026-08-08, kiro-cli 2.16.0) kept KAS's **workflow gate** off in every cyril session, so the model
never received `run_workflow`, `update_workflow`, `save_workflow_definition`, `inspect_workflow` or
`send_message`, and only cyril started workflow runs. Its reasons were facts about Kiro in August: model
launches were unsupervised and "for no gain"; turning the gate on would have been cyril's first non-parity row
in `kas/settings.rs`; Kiro's four workflow slash commands dispatched through a v2-only method; and workflows
mattered only behind the default-off `kas` cargo feature.

By kiro-cli 2.32.0 (KAS 0.66.43) those reasons no longer hold:

- **Kiro recommends workflows.** The rollout row `workflows` is generally available ("opt in via
  chat.enableWorkflows in /settings features"), and Kiro's TUI tips tell users to turn them on. Kiro's TUI
  builds `{available: rollout("workflows"), enabled: available && chat.enableWorkflows}` and, when available,
  sends `settings.workflows` **and** `settings.goal` set to that value (`ZOn` in the 2.32.0 bundle). Default off.
- **Workflows are Kiro's delegation path when enabled.** A workflow-enabled session does not get
  `invoke_sub_agent` ("it delegates through run_workflow"), and the `run_workflow` description steers the model
  toward the `workflowPrompt` form so complex work runs as a planned workflow outside the main context.
- **Model launches are supervised where it matters.** A `workflowPrompt` launch starts a `create-workflow` step
  whose plan reaches the user as a `session/request_permission` (`_meta.kiro.workflowPlan`) before any work step
  runs; plain-language edits of a running workflow are approved the same way.
- **Parity now points the other way.** cyril's settings marshal lacks the `chat.enableWorkflows` row Kiro's TUI
  has; KAS command dispatch was fixed (cyril-oieu, #169); KAS is becoming cyril's default engine (cyril-4c4d).

Evidence: [kiro-2.32.0-wire-audit.md](../kiro-2.32.0-wire-audit.md) and the grilling fact-finding
(`experiments/conductor-spike/audit-2.32.0/live/kas-lead-wfplan-2.32.0.jsonl`, `kas-clientcreator-2.32.0.jsonl`,
probe `experiments/conductor-spike/probe-kas-clientcreator-2.32.0.py`).

What still holds from ADR-0011: every `_kiro/workflow/*` method works with the gate off, so cyril can launch and
control runs itself in any session.

## Decision

1. **The workflow gate follows the user's Kiro setting.** cyril reads `chat.enableWorkflows` from Kiro's
   `cli.json` at startup and marshals `settings.workflows` and `settings.goal` exactly as Kiro's TUI does. A
   user who enabled workflows as Kiro's docs suggest gets workflow delegation in cyril too. cyril adds no toggle;
   Kiro owns the setting, and a change applies to the next agent process.
2. **The model may launch workflow runs whenever the gate is on.** cyril adds no confirmation of its own. KAS's
   plan approval for prompt-created workflows is the human checkpoint, and cyril must render it.
3. **The parent model may answer a workflow step's question**, as Kiro designs it. cyril makes that visible: the
   step's question and the parent's answer appear as a labelled, costed agent-initiated turn.
4. **cyril keeps its own control plane** — `/workflow run|list|status|cancel|resume|attach`, `/review`,
   `/workflow new`, `/goal` — beside the model's launch path, for two reasons: a vendor-neutral workflow
   substrate that a future cross-vendor engine (ROADMAP W2) reuses, and deterministic commands the user invokes.
   Runs cyril launches use `_kiro/workflow/new` + `invoke` with the user's main session as `parentSessionId`,
   whatever the gate, so their reports and wakes land in the conversation the user is working in. The gate is
   connection-wide, so with workflows on that parent can answer their steps too; point 3 applies to them as it
   does to model-launched runs.
5. **`/workflow new <prompt>` always launches Kiro's creator from the client:** `_kiro/workflow/new` with an
   inline copy of Kiro's `workflow-from-prompt` recipe (one step `create-workflow`, agent `wf-workflow-creator`,
   Kiro's brief wrapper), so its plan approval has `kind:"launch"` like a model launch. Verified live gate-off
   (2026-10-18). A rejection carries the user's reason as `_meta.kiro.rejectionReason`. The creator runs at
   Kiro's default effort.

## Considered options

- **Keep ADR-0011 (gate off everywhere).** Rejected: it withholds Kiro's recommended delegation path from users
  who followed Kiro's docs, and the agent's prompt and tools are tuned for that path when enabled.
- **Gate on only in a dedicated workflow session.** Rejected: workflow delegation exists to keep the *everyday*
  conversation's context clean; confining it defeats the purpose.
- **Gate on by default in cyril.** Rejected: goes beyond Kiro's own default; following the user's Kiro setting
  keeps one source of truth.
- **A separate gate-off parent session for cyril-launched runs** (a per-session `workflows:{enabled:false}`
  override, untested live), so only the user can answer their steps. Rejected: it moves completion reports and
  wakes into a hidden session, reversing cyril-lki9's decision to keep the user's session as the parent.
- **A cyril-side confirmation of every model launch.** Feasible: `run_workflow` raises no permission request and
  has no permission capability, but all workflow tools pass KAS's PreToolUse gate and a cyril-hosted hook reply
  can block a call before it runs (KAS waits with no timeout). Not adopted: it adds friction to the agent's
  normal delegation, and plan approval already guards prompt-created runs. It remains the known route if
  model-launch control is ever needed (needs a built-in hook and a bridge→UI approval path; unproven live).
- **`/workflow new` via Kiro's canned prompt** (the TUI's approach). Rejected: it works only gate-on, and in the
  2.26.0 probe the model reached a valid recipe only through recovery. **Via `agent://wf-workflow-creator`.**
  Not chosen: it also works gate-off, but its plan arrives as `kind:"replace"` rather than Kiro's tested
  `launch` path.

## Consequences

- **Plan approvals are now reachable** in everyday chat (model launches) and from `/workflow new`; rendering the
  plan (today "Preview unavailable", cyril-8wh0) is a prerequisite of both.
- **Agent-initiated turns multiply** when the gate is on: launch, creator, and step-answer turns join the
  completion and pause wakes that occur either way. cyril-lki9's policy stands — accept every wake, label it with
  its run and trigger — and every agent-initiated turn shows its credits; a workflow's completion line shows a
  per-run total. Auto-cancelling wakes is revisited only after step questions can be answered inline.
- **A step's question is shown with its run and step**, and the user can answer it inline (`session/prompt` to the
  step session). With workflows off, only the user can answer; with workflows on, the parent may answer first,
  and the inline answer is how the user overrides it.
- **A gate-on session loses `invoke_sub_agent`.** Sub-agent display work (cyril-ebqu, cyril-1v25) still matters
  for users with workflows off, which remains the default.
- **The parent can answer a step before the human sees the question**: KAS wakes the parent within milliseconds of
  a step's `send_message`, and offers no lever to route the question elsewhere. Accepted and made visible, not
  prevented.
- **Copying `workflow-from-prompt` creates drift risk.** The release-audit skill diffs the copied recipe shape and
  brief wrapper every release.
- **Live checks before `/workflow new` ships:** a plan rejection on a client-launched run, and a gate-on parity
  probe (KAS accepts the marshalled `workflows`/`goal` keys and registers the workflow tools).
- ROADMAP W2 is restated as cross-vendor orchestration only; for Kiro-only runs cyril drives Kiro's engine.
