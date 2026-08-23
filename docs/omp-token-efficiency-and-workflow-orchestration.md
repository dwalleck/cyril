# OMP token efficiency and workflow orchestration

Status: investigation record and current decisions as of 2026-08-23.

This document records the evidence, corrections, configuration changes, and workflow-design options from the Cyril/Gilfoyle token-efficiency investigation. It separates measured facts from proposed changes and identifies which controls exist in OMP today.

## Executive conclusion

The measured cost was dominated by an expensive supervising `gpt-5.6-sol` session repeatedly replaying a very large cached context for single-tool turns. The child agents were separate sessions and inexpensive; the problem was not user-driven scope expansion or merely "too many subagents." Gilfoyle started and supervised the work, while the supervisor retained the accumulated transcript, tool results, child summaries, advisor updates, validation output, and polling results.

The strongest measured control is earlier context maintenance. The supervisor issued 1,422 requests before its only compaction, averaging 406,111 tokens per request. The 63 requests after compaction averaged 66,204 tokens, a 6.13x reduction.

Two project controls are now active:

- Mid-turn compaction at 250,000 tokens, retaining 20,000 recent tokens.
- Fail-safe OMP Task isolation for every non-plan task subagent, enforced by a pre-tool hook.

Timeout and request-budget controls exist but have not yet been changed. A balanced future setting would be a 60-minute hard runtime and a 150-request soft budget per subagent. True inactivity-based stall detection is not currently an OMP setting.

A third-party extension, [`zerx-lab/omp-dynamic-workflows`](https://github.com/zerx-lab/omp-dynamic-workflows), already implements most of the workflow runtime discussed here. It should be audited before building a new OMP core interface. It is a young OMP port with important fail-open persistence/isolation and licensing caveats.

## Measurement source and scope

The OMP statistics dashboard at `http://127.0.0.1:3847` reads from:

```text
~/.omp/stats.db
```

Relevant tables:

- `messages`: per-model-request tokens, cost, latency, model, agent type, timestamp, and session file.
- `tool_calls`: tool name, agent type, timestamp, argument/result sizes, and error status.

The measured supervisor family was identified by session UUID:

```text
01a02b3e-a198-7634-bc48-a07e9fb851a2
```

The family includes the supervising session and nested child/advisor session paths. The children were independent sessions; grouping them under the supervisor UUID is attribution, not evidence that they shared one child context.

Measurement window used for the full-session snapshot:

```text
2026-08-22 20:53:31 UTC to 2026-08-23 16:08:43 UTC
Elapsed span: 19.25 hours
```

A later event-gap analysis found one 6.31-hour idle gap. Capping inter-request gaps at five minutes produced approximately 12.1 active hours, matching the operator's estimate of active task time.

The dashboard UI supports 1h and 24h ranges but not 12h. Calling `/api/stats/overview?range=12h` silently behaved like a 24-hour query. Exact 12-hour figures were therefore computed directly from SQLite using `MAX(timestamp) - 43,200,000`.

## Full supervisor-family usage

| Agent type | Requests | Uncached input | Cache reads | Output | Conversation tokens | Cost |
|---|---:|---:|---:|---:|---:|---:|
| Main | 1,485 | 5,819,892 | 575,461,120 | 379,549 | 581,660,561 | $611.154637 |
| Subagents | 1,971 | 6,129,700 | 207,689,216 | 526,175 | 214,345,091 | $6.498180 |
| Advisor | 1,561 | 9,175,444 | 125,664,000 | 774,888 | 135,614,332 | $5.761874 |
| **Total** | **5,017** | **21,125,036** | **908,814,336** | **1,680,612** | **931,619,984** | **$623.414690** |

Key ratios:

- Main-agent share of cost: 98.03%.
- Main-agent share of conversation tokens: 62.44%.
- Cache reads as a share of main-agent conversation tokens: 98.93%.
- Cache reads as a share of all conversation tokens: 97.55%.
- Newly processed input plus generated output: approximately 22.8 million tokens.

Cache reads were the dominant cost driver. They were much cheaper than uncached input but not free.

## Exact 12-hour snapshot

For the supervising session family only:

| Agent type | Requests | Uncached input | Cache reads | Output | Conversation tokens | Cost |
|---|---:|---:|---:|---:|---:|---:|
| Main | 359 | 3,671,774 | 204,710,144 | 77,643 | 208,459,561 | $242.109221 |
| Advisor | 125 | 4,897,856 | 19,340,288 | 146,622 | 24,384,766 | $1.962921 |
| **Total** | **484** | **8,569,630** | **224,050,432** | **224,265** | **232,844,327** | **$244.072142** |

Across every OMP session during the same exact cutoff:

- 518 requests.
- 233,456,872 conversation tokens.
- $244.663438.

The difference came from separate verification sessions launched during the later configuration work.

## Context replay and compaction

Main-agent request characteristics before the configuration change:

| Metric | Value |
|---|---:|
| Requests | 1,485 |
| Average tokens per request | 391,691 |
| Maximum tokens in one request | 850,050 |
| Average cache-read tokens per request | 387,516 |
| Average uncached input per request | 3,919 |
| Average output per request | 256 |

The session contained one compaction event:

```text
2026-08-23 14:42:27 UTC
Tokens before: 850,050
Tokens after: 72,497
Method: remote
```

Measured before/after behavior on the same main model:

| Period | Requests | Average tokens/request | Total tokens | Cost |
|---|---:|---:|---:|---:|
| Before compaction | 1,422 | 406,111 | 577,489,685 | $608.10 |
| After compaction | 63 | 66,204 | 4,170,876 | $3.05 |

The workloads differed, so the dollar delta is not a clean counterfactual. The 6.13x context-size reduction is direct evidence that waiting until roughly 850k tokens was too late for cost control.

### Applied compaction configuration

Project file: `.omp/config.yml`

```yaml
compaction:
  enabled: true
  midTurnEnabled: true
  thresholdTokens: 250000
  keepRecentTokens: 20000
```

OMP validated `compaction.thresholdTokens` as a positive fixed token limit that overrides percentage-based triggering.

This is project-local to Cyril. New sessions inherit it. A handoff/restart is the reliable way to ensure an already-running parent runtime adopts it.

## Main tool-loop evidence

Every recorded main, advisor, and subagent tool row had `calls_in_turn = 1`. The expensive main model therefore performed one outer tool call per provider turn.

Main-agent tool distribution:

| Tool | Calls | Tokens on invoking turns | Cost on invoking turns |
|---|---:|---:|---:|
| Bash | 427 | 186,522,777 | $201.73 |
| Read | 341 | 120,667,051 | $124.50 |
| Edit | 213 | 85,131,031 | $85.67 |
| Hub | 166 | 55,789,161 | $55.08 |
| Write/tool wrappers | 127 | 48,238,134 | $49.57 |
| Grep | 98 | 36,194,970 | $35.55 |
| Todo | 69 | 27,753,461 | $34.52 |
| Eval | 40 | 11,534,361 | $11.95 |
| Task | 14 | 3,337,774 | $3.28 |
| Glob | 9 | 2,078,766 | $2.24 |

These are model-turn costs associated with each tool, not intrinsic prices charged by the tools.

Exact duplicate calls were uncommon. The failure mode was not primarily an infinite identical-command loop. It was fine-grained operations repeatedly waking a large-context main model.

### Polling evidence

`hub` operations:

- 92 `wait`.
- 42 `send`.
- 10 `logs`.
- 9 `cancel`.
- 7 `start`.
- 3 `stop`.
- 2 `jobs`.
- 1 `list`.

The 92 wait turns consumed:

- 29,880,236 tokens.
- 29,771,776 cache-read tokens.
- $27.535569.

Wait timeout arguments:

- 46 at 30 seconds.
- 26 at 60 seconds.
- 15 implicit/configured waits.
- 3 at one second.
- 1 at ten seconds.
- 1 at twenty seconds.

This polling was often a symptom of subagents not returning, not gratuitous user interaction. The avoidable part was waking the supervisor at every short timeout. `async.pollWaitDuration` affects only implicit waits; explicit `timeoutMs` values override it.

When all parent work is exhausted, the lower-cost pattern is one `hub wait` with `timeoutMs: 0`. It remains active until a message/job completion, runtime abort, or user interruption, while the tool can continue streaming progress snapshots without another model turn.

### Validation and rebase churn

Shell activity:

| Category | Calls | Failed calls | Recorded command runtime |
|---|---:|---:|---:|
| Cargo/build/test | 191 | 71 | 33.6 minutes |
| Git rebase | 17 | 10 | negligible |
| Other Git | 119 | 1 | 2.4 minutes |
| GitHub CLI | 43 | 2 | 1.7 minutes |
| Rivets | 13 | 0 | negligible |

Of the Cargo calls, 70 contained broad workspace or CI-equivalent validation; 26 failed. Some failures were legitimate red-green feedback. Others were full gates run while syntax, types, formatting, or rebase conflicts were still unsettled.

Recommended validation ladder:

1. Exact affected test during red-green work.
2. Affected crate/check during local correction.
3. Repository-required test and Clippy at a completed logical slice.
4. One full gate after a rebase is fully resolved.
5. One CI-equivalent gate at the final PR head.

The repository's requirement to validate each logical change remains authoritative. The efficiency improvement is to make the logical checkpoint explicit and avoid broad gates during known-broken intermediate states.

## Subagent and advisor attribution

| Agent type | Distinct sessions | Requests | Conversation tokens | Cost |
|---|---:|---:|---:|---:|
| Main | 1 | 1,485 | 581,660,561 | $611.15 |
| Subagent | 33 | 1,971 | 214,345,091 | $6.50 |
| Advisor | 9 | 1,561 | 135,614,332 | $5.76 |

Subagents and advisors accounted for 37.6% of conversation tokens but only 1.97% of cost because they ran on `gpt-5.6-luna`. Removing them wholesale would not address the dominant cost and could move more work onto the expensive main model.

The operator clarified that the PR stacks were separate child sessions launched by one Gilfoyle invocation. That correction matters:

- This was not user-driven scope creep.
- Creating additional child sessions would not fix the supervisor context.
- Gilfoyle needs internal supervisor compaction/checkpoints and completion-driven child management.
- Child-session separation already existed.

## Advisor limits for stall detection

OMP advisors review transcript deltas at primary updates. A silent stall produces no delta, so the advisor has no event to inspect.

An advisor can identify an active non-progress loop when the agent continues producing turns. It cannot reliably enforce:

- Provider-stream silence.
- A tool that never returns.
- A child that emits no more updates.
- A child that fails to yield.
- Hard cancellation of another agent.

Runtime ownership should be:

| Concern | Owner |
|---|---|
| Incorrect reasoning or risky change | Advisor |
| Excessive model-request loop | `task.softRequestBudget` |
| Total child runtime | `task.maxRuntimeMs` |
| Silent provider stream | Provider stream watchdog |
| No useful activity for an interval | Missing progress watchdog |
| Manual inspection/termination | Agent Hub / `hub cancel` |

Adding an advisor specifically to watch stalls would add usage without covering silent failure.

## Configurable timeout and progress controls

Current OMP settings and defaults:

| Setting | Default | Meaning |
|---|---:|---|
| `task.maxRuntimeMs` | `0` | Hard per-subagent wall-clock limit; zero disables it |
| `task.softRequestBudget` | `200` | Wrap-up notice at the limit; forced stop at 1.5x |
| `task.softRequestBudgetNotice` | `true` | Enables the wrap-up steering notice |
| `task.maxConcurrency` | `32` | Maximum concurrent subagents |
| `task.agentIdleTtlMs` | `420000` | Parks completed idle agents; not a running timeout |
| `async.pollWaitDuration` | `smart` | Implicit wait duration; explicit waits override it |
| `providers.streamFirstEventTimeoutSeconds` | `-1` | Provider/env default for first stream event |
| `providers.streamIdleTimeoutSeconds` | `-1` | Provider/env default for stream silence |

Observed successful subagent spans:

- `MemoryPaths`: 48.5 minutes, 63 requests.
- `MemoryStore`: 47.7 minutes, 108 requests.
- `RuntimeDomain`: 31.3 minutes, 30 requests.
- `RuntimeProcessTests`: 25.2 minutes, 47 requests.
- `WindowsIpcLibrarian`: 20.2 minutes, 188 requests.

Evidence-backed balanced proposal, not yet applied:

```yaml
async:
  pollWaitDuration: "5m"

task:
  maxRuntimeMs: 3600000
  softRequestBudget: 150
  softRequestBudgetNotice: true
```

Consequences against the observed sample:

- Notice at 150 requests.
- Forced stop at 225 requests.
- Hard stop at 60 minutes.
- No successful observed child would have been force-stopped.

OMP does not currently expose a `task.stallTimeoutMs` setting. A proper implementation would reset an inactivity clock on model output, tool start/result, or explicit progress; steer once after the threshold; allow a grace period; and then abort while distinguishing a long-running tool that is still reporting progress.

## Writer isolation investigation

### Existing repository guard

`.githooks/pre-commit` prevents feature-branch commits from the primary checkout. `scripts/session-worktree.sh` provisions branch-specific linked worktrees. Git prevents the same branch from being checked out in two worktrees.

Those controls do not prevent two processes from editing the same linked worktree before commit. Git hooks are commit-time backstops, not write isolation.

### OMP Task isolation

OMP supports per-spawn isolation when:

```yaml
task:
  isolation:
    mode: auto
```

An isolated task runs in a separate workspace, returns a patch/branch result, and is torn down afterward.

### Applied project hook

Project file: `.omp/hooks/pre/isolate-writers.ts`

Current policy:

- Intercept every `task` tool call.
- Outside plan mode, force `isolated: true` for every flat or batched task item.
- Override an explicit `isolated: false`.
- Treat agent names as untrusted; project/user definitions can override bundled names.
- In plan mode, leave task input unchanged because OMP restricts children to read-only tools and rejects per-spawn isolation controls.

The hook determines plan mode from the latest active-branch `mode_change` entry.

Why the policy changed during review:

1. The initial read-only-name allowlist was unsafe because OMP agent discovery is first-wins; a custom writable agent could shadow `scout` or `reviewer`.
2. Isolating every task without a plan-mode exception broke plan-mode task spawning because OMP rejects isolation controls there.
3. The final policy isolates all non-plan task subagents and exempts current plan mode.

Behavioral verification:

- Default writer received `isolated: true`.
- Explicit `isolated: false` was overridden.
- Batched tasks were all isolated.
- A custom/read-only-named agent could not bypass isolation.
- Plan-mode task input was not modified.
- Isolation resumed after a later `mode_change: none`.
- Real writer path: `/home/dwalleck/.omp/wt/t55f13fc1e/m`.
- Real scout path after fail-safe change: `/home/dwalleck/.omp/wt/t99c28e52b/m`.
- Real `--plan-yolo` scout completed without an isolation-parameter rejection.

Scope limitation:

- Covers OMP `task` subagents.
- Does not stop the main agent, another independently launched process, a human, or non-OMP tooling from modifying the primary checkout.
- Hard enforcement for arbitrary writers requires launch/OS isolation.

## OMP workflow layers

OMP has several workflow layers but no first-party fully declarative, durable DAG engine.

| Layer | What it supplies | Enforcement level |
|---|---|---|
| Skill | Reusable runbook and engineering judgment | Prompt contract |
| Task-agent definition | Role, model, tools, spawn policy, schema | Partial runtime policy |
| Task batch | Concurrent independent agents | Runtime fan-out |
| Eval `parallel()` | Bounded concurrent functions | Deterministic in-session helper |
| Eval `pipeline()` | Barriered stages across items | Deterministic in-session helper |
| `workflowz` keyword | Contract to use Eval workflow helpers | Prompt contract |
| Extension command | Programmatic command, events, timers, persistent entries | Executable controller substrate |
| Todo/plan/goal | Visible state and budgets | Tracking, not a generic scheduler |

`workflowz` is more deterministic than an ordinary skill because it directs the model toward real Eval helpers. The model still authors the pipeline at runtime, and the pipeline lacks durable stage state across process restart.

Useful first-party implementation references:

- Autoresearch: durable SQLite experiment state, automatic continuation, timeout/cancellation, stale-run recovery.
- Goal runtime: persisted pause/resume/drop/complete state plus token and wall-time accounting.
- Cleanse: bounded discover -> parallel repair -> verify flow.
- Commit orchestration: staged single-run pipeline with reminders and confirmation.
- Task/structured-subagent/AsyncJobManager: policy, isolation, progress, timeout, cancellation, completion delivery, and structured results without durable workflow stages.

## Proposed generic extension subagent interface

A clean generic interface discussed during the investigation was:

```ts
interface ExtensionSubagentController {
  spawn(request: ExtensionSubagentRequest): Promise<ExtensionSubagentHandle>;
}

interface ExtensionSubagentHandle {
  readonly id: string;
  readonly result: Promise<ExtensionSubagentResult>;
  snapshot(): ExtensionSubagentProgress;
  subscribe(listener: (progress: ExtensionSubagentProgress) => void): () => void;
  cancel(reason?: string): void;
}
```

OMP internally already has `runStructuredSubagent()`, including policy resolution, schemas, isolation, timeout, abort, progress, artifacts, and cleanup. Extension contexts do not receive the internal `ToolSession` needed to call it, nor public spawn/await/cancel methods. A thin core adapter would be architecturally cleaner than fabricating `ToolSession` in an extension.

The hard part is lifecycle correctness rather than raw implementation size:

- Results cannot enter the wrong session after switch/resume.
- Shutdown cancels children and cleans isolated workspaces.
- Extension-handler timeouts do not kill detached children.
- Cancellation is idempotent and promises settle once.
- Spawn/depth/approval/plan/isolation policies match Task and Eval.
- Progress listeners are contained.

This core change should not be implemented before evaluating the existing workflow plugin described below.

## KAS workflow inspiration

The relevant KAS feature is the persisted `_kiro/workflow/*` engine, not the model-elected `OrchestrateSubAgent` tool.

KAS workflow capabilities documented in `docs/kiro-2.16.0-wire-audit.md` (the client-authored DAG and core lifecycle were live-verified; some control paths were reconstructed from the bundled implementation):

- Client-authored DAGs.
- Persisted workspace-scoped runs.
- Full peer sessions per workflow step.
- `step`, `sequence`, `repeat`, `parallel`, and `watch` nodes.
- Pause/resume/cancel/retry/list/load/inspect/update/delete controls.
- Bounded repeats and watch idle timeouts.
- GitHub PR watch handler.
- Nine lifecycle notifications.
- Artifacts, captured outputs, and run snapshots.
- Reattachment after client restart.
- Stale-running liveness converted to resumable pause.

Cyril ROADMAP distinction:

- W1: KAS-only persisted peer-session workflow DAG and Cyril `/workflow` control plane.
- W2: unscheduled cross-vendor/client-owned scheduler.

Current Cyril W1 surface includes typed lifecycle conversion, `WorkflowTracker`, peer-session routing, approval queue, and `/workflow recipes|list|run|attach|status|cancel|resume`. Current gaps include no dedicated workflow progress renderer, no automatic reattach, no rich workflow user-input gate, incomplete control methods, and in-memory Cyril tracker state that relies on KAS for persistence.

Gilfoyle can be expressed as a KAS-specific recipe, but KAS alone does not provide:

- OMP/Cyril writer worktree isolation.
- Rivets-specific state and close semantics.
- Gilfoyle's exact design/merge approval contract.
- Cross-vendor stages.

KAS's strongest reusable ideas are persisted run/node state, canonical node identity, explicit paused versus terminal status, peer-session claims, non-LLM watch nodes, and liveness-driven pause.

## Existing workflow plugin

Direct candidate: [`zerx-lab/omp-dynamic-workflows`](https://github.com/zerx-lab/omp-dynamic-workflows).

Audited source revision: [`421f4bbd1bca40d88c18137693c9dd9049926831`](https://github.com/zerx-lab/omp-dynamic-workflows/tree/421f4bbd1bca40d88c18137693c9dd9049926831), inspected 2026-08-23. Capability and caveat claims below refer to that immutable revision; the repository's default branch may change.

Declared compatibility:

- Plugin requirement: `@oh-my-pi/pi-coding-agent >= 17.2.4`.
- Installed OMP at investigation time: `18.0.3`.

The plugin is not installed. Configured marketplaces currently contain Grafana skills and Matt Pocock skills only. Direct install syntax from its README:

```bash
omp plugin install github:zerx-lab/omp-dynamic-workflows
```

### Capabilities

- JavaScript-defined workflows.
- `agent()`, `parallel()`, `pipeline()`, nested saved workflows.
- Phases, retries, gates, judge panels, verification, completeness checks.
- Background execution and completion-driven result delivery.
- TUI progress panel and navigator.
- Optional loopback web console.
- Durable run records with script, args, journal, lease, limits, usage, result, and backup.
- Pause/resume/stop/delete.
- Resume by replaying unchanged call-prefix journal entries.
- Per-agent timeout and retries.
- Run/phase token budgets.
- Model and agent-type routing.
- Saved workflows registered as slash commands.
- Journaled human confirmation checkpoints.
- Worktree isolation option.
- Compact delivered result previews, limiting parent-context growth.

This is close to the Gilfoyle runtime discussed here. A saved, reviewed workflow could own the stage machine while existing Gilfoyle references remain stage runbooks.

### Architectural correction

The plugin proves that extension-only child execution is possible using OMP's public SDK `createAgentSession()`. It also confirms the duplication concern: `src/agent.ts` implements its own session creation, settings/model registry, tool policy, structured-output repair, usage collection, abort handling, identity, persistence option, and disposal rather than using native `runStructuredSubagent()`.

Therefore:

- A generic core controller is cleaner.
- It is not a prerequisite for an extension-only prototype or deployment.
- The existing plugin should be evaluated before building either path.

### Adoption caveats

- OMP port version is `0.1.0`, with a very small current user signal.
- `package.json` is marked private.
- No `LICENSE` file or package license declaration was found. Clarify permission before copying, modifying, or redistributing.
- Worktree isolation is documented as best effort and continues unisolated on failure. Gilfoyle writers require fail-closed isolation.
- Run persistence catches/logs failures and continues in memory. Load-bearing workflow transitions should fail or pause when persistence fails.
- `checkpoint()` confirm/headless paths exist, while declared input/select/timeout paths are not fully wired.
- Per-agent timeout exists; inactivity-based stall timeout does not.
- Workflow scripts run inside the OMP process. Only trusted, committed scripts should drive merges.
- Child transcripts are not persisted by default.

Lineage references:

- Original Pi plugin: [`Michaelliv/pi-dynamic-workflows`](https://github.com/Michaelliv/pi-dynamic-workflows), MIT, mature community signal, but no persisted/resumable runs in its documented prototype status.
- Feature-rich Pi fork: [`QuintinShaw/pi-dynamic-workflows`](https://github.com/QuintinShaw/pi-dynamic-workflows), MIT, persistent/resumable features, but targets `@earendil-works/*` rather than OMP.

The lineage's maturity does not automatically transfer to the young OMP port.

## Recommended next steps

### Already applied

1. Project compaction at 250,000 tokens.
2. Mid-turn compaction enabled.
3. All non-plan OMP Task subagents isolated through a fail-safe hook.

### Proposed configuration experiment

1. Set `task.maxRuntimeMs: 3600000`.
2. Set `task.softRequestBudget: 150` with notices enabled.
3. Remove Gilfoyle's explicit short `hub wait` timeouts; when fully blocked, wait indefinitely for completion/runtime abort.
4. Measure a comparable workflow before changing thresholds again.

### Workflow path

1. Audit `omp-dynamic-workflows` for security, compatibility, persistence, cancellation, and isolation behavior.
2. Resolve its licensing status.
3. Run a non-destructive local proof.
4. Encode Gilfoyle as a trusted saved workflow while retaining stage runbooks as separate prompt assets.
5. Make writer isolation and workflow persistence fail closed.
6. Add inactivity detection if real runs still stall.
7. Add a generic OMP core subagent controller only if duplicated lifecycle maintenance becomes a demonstrated problem.

## Decision summary

- Cache reads are the dominant measured cost; they are not free.
- Child sessions were already separate; supervisor context maintenance is the main token lever.
- Short polling was a return/latency symptom. Completion-driven waits plus runtime enforcement are the fix.
- Advisors review correctness; they are not silent-stall watchdogs.
- OMP has configurable hard runtime and request budgets, but no inactivity watchdog.
- OMP has workflow primitives but no first-party durable declarative DAG engine.
- KAS W1 is a useful persisted-workflow reference but is KAS-only and does not solve cross-vendor Gilfoyle.
- A third-party OMP workflow extension already solves most of the target problem and should be evaluated before new core work.
