# KAS modes over ACP — `plan` / `bug-fix` / `quick-spec` (2.10.0 live probe)

**Analyzed:** 2026-07-02 · **Binary:** kiro-cli-chat 2.10.0 (archived, BUILD_HASH `a5da0295…`), `acp --agent-engine v3`, embedded `@kiro/agent` **0.3.299** (byte-frozen since 2.9.0 — so these findings hold for 2.9.0's KAS too).
**Method:** live per-mode A/B probe, one fresh session per mode in a throwaway git workspace seeded with a deliberately buggy `calc.py` (`add` returns `a - b`). `plan` and `bug-fix` received the **identical prompt** ("There is a bug in calc.py. Fix it.") so the write/no-write delta is attributable to the mode. Oracle = `git status --porcelain` + file-content comparison, not chat text. Client advertised `_meta.kiro.userInput` only (no fs/terminal — file+shell I/O in-process). Auth: IAM IdC token from the sqlite store, `profileArn` from the `state` table.
**Probe:** `experiments/conductor-spike/probe-kas-modes-2.10.0.py` · **Full wire dumps:** `experiments/conductor-spike/kas-modes-dumps/{plan,bug-fix,quick-spec}.json`

This closes three of the four "modes not exercised" from the 2.7.1 audit (`autonomous` remains unexercised — it auto-approves all tools and needs a sandboxed run design).

**Verdict for cyril:** all three modes are fully drivable over ACP via `session/set_config_option` — no hidden TUI-only surface. Six client-contract updates below (§ Wire-contract deltas), two of which correct the 2.7.1 audit: config-option entries key on **`value`** (not `id`), and an explicit set **does** emit a rebuild broadcast on 0.3.299. Directly feeds ROADMAP KAS-4 (configOptions/modes) and the diff-preview design (permission→tool_call join).

---

## Results at a glance

| | `plan` | `bug-fix` | `quick-spec` |
|---|---|---|---|
| set took effect | ✓ (`config_option_update` → `plan`) | ✓ (set response → `bug-fix`) | ✓ (set response → `quick-spec`) |
| `calc.py` modified | **no** | **no** | **no** |
| files created | none (git clean) | `.kiro/specs/add-function-wrong-operator/{.config.kiro, bugfix.md, design.md, tasks.md}` | `.kiro/specs/subtract-function/{.config.kiro, requirements.md, design.md, tasks.md}` |
| tool_calls | 2 (read-only: File Search, Read File) | 11 | 17 |
| `_kiro/userInput` | 0 | 0 | **3** |
| `session/request_permission` | 0 | 5 (shell mkdir + 4× Write File) | 5 (shell mkdir + 4× Write File) |
| turn end | `session_info_update` turn_end, `stopReason: end_turn` | same | same |

## Per-mode behavior

### `plan` — read-only is enforced, not advisory

Same "fix the bug" prompt as bug-fix, yet the agent ran only read-only tools (`File Search` kind `search`, `Read File` kind `read`), described the fix in chat, and left the workspace **byte-identical** (empty porcelain). No permission requests, no writes attempted. The "Plan-only mode, no changes" contract holds behaviorally on the wire.

### `bug-fix` — a docs-first spec-flow variant; the fix is NOT applied in turn one

The headline surprise: given a one-character bug and an explicit "fix it", bug-fix mode **does not touch `calc.py`**. Turn one is investigate → diagnose → document:

- Creates `.kiro/specs/<generated-slug>/` containing `bugfix.md` (a requirements doc with numbered *Current / Expected / Unchanged behavior* WHEN/THEN/SHALL clauses), `design.md` (root-cause + testing strategy), and `tasks.md` — a rigorous 4-task property-based-testing plan (bug-condition test that MUST FAIL on unfixed code → preservation tests for the `b == 0` degenerate case → the one-char fix → checkpoint).
- `.config.kiro` sidecar: `{"specId": "<uuid>", "workflowType": "requirements-first", "specType": "bugfix"}` — bug-fix is a spec-workflow variant, discriminated by `specType`.
- Ends by asking *"are you good to proceed with executing the tasks?"* — **in plain chat, not `_kiro/userInput`**, consistent with the covenant note that `get_user_input` is spec-tagged. Resolution requires a follow-up turn (or the `spec/*` task-execution surface).

### `quick-spec` — clarify via userInput, generate, stop at approved plan

Prompt: "Add a subtract(a, b) function to calc.py, with a simple test file."

- **Three `_kiro/userInput` callbacks**, the first live captures of the quick-spec flow: (1) "pytest or unittest?" — options `[pytest, unittest, Skip questions]`; (2) "cover both functions or only subtract?"; (3) a **plan-approval gate**: *"Does this plan look good…? Once approved, you can run the tasks from tasks.md to implement"* — options `[Looks good, let's go / I'd like to adjust something]`. Note: **no option carried `recommended: true`** in any of the three (the 2.7.1 spec-flow capture had recommended flags; don't assume they're always present), and questions include an explicit "Skip questions" escape.
- Generates `.kiro/specs/subtract-function/{requirements.md, design.md, tasks.md}`; sidecar `{"workflowType": "fast-task", "specType": "feature"}` (vs bug-fix's `requirements-first`/`bugfix`).
- **Even after the approval answer, the turn ends without implementing** — final message: *"The spec is locked in. You can now run the tasks from tasks.md to implement."* Generation and execution are separate flows; a client offering quick-spec needs a follow-through affordance (next prompt or `spec/runAllTasks`).
- Nice touch observed: the generated task plan includes a checkpoint that would surface the pre-existing `add()` bug ("The tests will flag the add() bug when you get to the checkpoint") — the clarify phase read the surrounding code, not just the ask.

## Wire-contract deltas vs the 2.7.1 audit (0.3.299)

1. **Config-option entries are `{value, name}` — the settable id is `options[].value`.** There is no `id` on the entry. A client keying on `.id` renders seven `None`s. (The option *group* still has `id: "mode"`, `currentValue`, `type: "select"`, plus a `category` field.)
2. **An explicit `session/set_config_option` now triggers a rebuild broadcast** — observed cluster after the set, before any turn content: `available_commands_update` + `config_option_update` (+ `_kiro/tools/didChange`, `_kiro/mcp/status`). This **supersedes 2.7.1 caveat (a)** ("no notification echo on explicit set"). The set *response* (rebuilt `configOptions`) remains the authoritative read; the broadcast means mode switches re-scope commands/tools and clients get told.
3. **`request_permission`'s `toolCall` is a stub** — `{status, title, toolCallId}` only, **no `rawInput`**. To render a diff preview at the approval gate, join by `toolCallId` to the earlier `tool_call` notification, which carries full `rawInput` — for writes: `{path, text}` (full content, kind `edit`). This is the load-bearing fact for any review-before-apply UX built on permissions rather than fs callbacks.
4. **Consent `_meta` shape drifted:** `{capability, resource, askType, triggeringResource, workspaceRoot}` — `triggeringResource` added, `consentRound` gone (vs the 2.7.1 capture).
5. **Permission options are now four:** `accept`/`always-accept`/`reject`/`always-reject` with kinds `allow_once`/`allow_always`/`reject_once`/`reject_always` (2.7.1-era captures showed three).
6. **`turn_end` meta is minimal here:** `_meta.kiro = {kind: "turn_end", stopReason, turnEnd: {stopReason}}` — no `promptTurnSummaries` in any of the three runs (earlier probes read `promptTurnSummaries[].usedTools`; it may be delegation-turn-only — don't depend on it for turn accounting).
7. **`tool_call._meta.kiro.toolOrigin`** distinguishes `"default"` vs `"acp"` tool identities (e.g. File Search = `default`, Read/Write File = `acp`).
8. **Session-setup notification storm** (before any prompt): `_kiro/mcp/status`, `_kiro/governance/state`, `_kiro/tools/didChange`, `_kiro/steering/documents_changed`, `_kiro/sessions/changed`, `_kiro/policy/changed`, `_kiro/powers/items_changed`, `_kiro/progressive_context/items_changed`, plus `available_commands_update` and several `session_info_update`s. A client must tolerate (or use) all of these prior to first turn.

## Open questions

- **Autopilot didn't suppress prompts:** `autopilot` was `on` (default) in every session, yet shell (`mkdir`) and all four file writes fired `session/request_permission` in bug-fix and quick-spec. Candidate explanations: spec-flow modes force supervised behavior; 0.3.299 policy layer changes (the 2.9.0 `policyDenial`/safety refinements); or `.kiro/`-path writes are special-cased. Needs a vibe-mode control run with the same write workload before concluding autopilot semantics changed.
- **`autonomous` mode still unexercised** — auto-approves everything except MCP; run it only with a sandbox story (`--sandbox` flags exist on the KAS server; see the launch-contract memory).
- **Spec-execution surface:** neither run exercised `spec/runAllTasks` / task execution; the generated `tasks.md` + `.config.kiro` (`specId`) are presumably the inputs. Follow-up probe candidate.

## Reproduction

```sh
kiro-cli login   # IAM IdC; logout DELETES the auth_kv token row (row-absent = logged out)
python3 experiments/conductor-spike/probe-kas-modes-2.10.0.py
# dumps land in experiments/conductor-spike/kas-modes-dumps/<mode>.json
```

Prior art: `docs/kiro-2.7.1-wire-audit.md` (KAS landing audit; "Settings / modes not exercised" list this closes), `docs/kiro-2.10.0-wire-audit.md` (KAS byte-frozen proof), `docs/kiro-kas-acp-covenant.md` (`_kiro/*` type contract). Methodology: `reference_kiro_wire_audit_methodology`.

## Addendum (2026-07-05): plan mode × subagents, re-probed on 2.11.0 (@kiro/agent 0.8.0)

The 2.10.0 runs above only exercised **direct** tools in plan mode; whether a
plan session could route writes through a *subagent* was untested. Probe:
`experiments/conductor-spike/probe-kas-plan-subagent-2.11.0.py`, dumps in
`kas-plan-subagent-dumps/` (A/B/C: plan+delegate, vibe+delegate control,
plan+direct baseline; oracle = git porcelain, `subagentOrchestration: true`
in the handshake).

**Result: no hole — plan's read-only binds the subagent layer structurally.**

- **plan + "spawn a subagent and have IT edit calc.py"**: zero
  `agent-subtask` tool_calls, zero permissions, workspace byte-identical.
  The agent's own explanation names the mechanism: in plan mode the subagent
  roster is scoped to read-only agents (`semantic_reviewer`,
  `context-gatherer`) — the write-capable `general-task-execution` subagent
  is not registered at all. Enforcement is roster-scoping, not a runtime
  veto.
- **vibe control (identical prompt, identical caps)**: 4 subtask calls
  (`Sub-agent: general-task-execution` + its Replace/Read/Run children),
  2 permissions, `M calc.py` — proving the harness can produce a writing
  subagent when the mode allows, so the plan null is attributable to the
  mode.
- **plan + direct fix (0.8.0 re-baseline)**: read-only still enforced
  (1 read tool, no writes, clean porcelain) — the 2.10.0 §plan result holds
  across the 0.3.299→0.8.0 renumber, and the advertised mode list is
  unchanged (same 7 values incl. `semantic_reviewer`).

cyril implication: `mode=plan` can be treated as a genuine write-barrier in
UI affordances (e.g. a future plan-mode indicator) — subject to the standing
caveat that `autonomous` mode and `spec/runAllTasks` execution remain
unexercised.
