# oh-my-pi `/review` — implementation analysis and cyril feasibility

*Analyzed 2026-07-28 against the oh-my-pi checkout at `~/repos/oh-my-pi`. Companion
docs: [`omp-advisor-analysis.md`](omp-advisor-analysis.md),
[`omp-tui-takeaways.md`](omp-tui-takeaways.md).*

omp's `/review` is an interactive code-review launcher: a menu picks the review mode,
follow-up pickers select the target (base branch, commit, PR), then the command runs
the git diff **client-side**, computes stats, filters noise, and composes a rich prompt
that fans review work out across parallel reviewer agents.

**Key finding: it is not a skill and not agent-side.** It's a bundled *custom command*
(a TypeScript class in omp's extensibility system) whose entire output is one composed
user prompt. The agent never sees the menu — it receives a fully-specified review
request with the diff already parsed, filtered, and sized. All intelligence that can be
deterministic *is* deterministic; the model only does the judging.

## Source map (omp)

| File | Role |
|---|---|
| `extensibility/custom-commands/bundled/review/index.ts` (~700 ln) | The whole feature: menu, pickers, git/jj/gh dispatch, diff parsing, prompt composition |
| `prompts/review-request.md` | Main template: mode, file table, excluded list, distribution guidelines, reviewer instructions, inline diff |
| `prompts/review-custom-request.md`, `prompts/review-headless-request.md` | Custom-instructions and headless (`-p`) variants |
| `extensibility/hooks/types.ts` | The deliberately narrow UI surface commands get: `select(title, options)`, `editor(...)`, `notify(...)` |

## Flow

1. **Argument fast-paths.** A GitHub PR URL or `pr://owner/repo/N` ref in the args skips
   the menu entirely and reviews that PR (diff fetched via `gh`, cached). Headless mode
   (no UI) renders a generic template instead of prompting.
2. **Mode menu** via `ctx.ui.select("Review Mode", …)`, built dynamically:
   - up to 3 **PR refs scraped from the conversation transcript** (most recent first,
     deduped) — "Review PR owner/repo#123 from conversation"
   - static entries: review against a base branch (PR-style) / uncommitted changes /
     a specific commit / custom instructions
3. **Target pickers per mode:**
   - base branch → `git branch --all` list → second `select`; diff is three-dot
     `base...current`
   - commit → last 20 `git log --oneline` entries → `select`; diff is `git show`
   - uncommitted → no picker; staged + unstaged combined (jj repos get `jj diff`)
   - custom → multi-line `editor` overlay (Ctrl+G opens `$VISUAL`/`$EDITOR`)
4. **Deterministic pre-work (client-side, no model involvement):**
   - parse the unified diff into per-file `{path, +lines, -lines, hunks}`
   - **noise filter** with reasons: lock files, minified/generated/snapshots/source
     maps, build output, vendor, images/fonts/binaries — excluded files are still
     *listed* in the prompt with their reasons, so nothing silently disappears
   - **reviewer-count heuristic** from diff weight: <100 lines or ≤2 files → 1 agent;
     <500 → ≤2; <2000 → ≤4; <5000 → ≤8; else ≤16
5. **Prompt composition** (`review-request.md` template):
   - mode line, file table (`path | +/- | ext`), excluded list, totals
   - distribution guidelines: "spawn N reviewer agents in parallel", group by
     locality (same module, tests with implementation)
   - reviewer musts: focus only on assigned files; diff-source discipline; findings
     via incremental yield sections
   - the raw diff inline **only if** <50 k chars *and* ≤20 files; otherwise per-file
     previews (~100 lines budget split across files) plus "MUST run
     `git diff`/`git show` for assigned files" (PR mode: "MUST read `pr://…/diff`,
     NEVER local git")
6. The returned string is sent as the user message. Cancel at any picker aborts with no
   side effects.

## Transferable design points

- **The client does everything deterministic; the model only judges.** The agent never
  re-discovers the diff with tool calls — it's handed the diff, the stats, and the
  fan-out plan. Cheaper, faster, reproducible.
- **Menu options are context-aware**: scraping recent PR refs out of the conversation
  transcript makes the common case ("review the PR we were just discussing") one
  keystroke.
- **Noise filtering with visible receipts** — excluded files are listed with reasons in
  the prompt, so the reviewer knows what was skipped and why.
- **Size-adaptive prompt shape** — inline diff for small changes, per-file previews +
  fetch instructions for large ones. The 50 k-char/20-file thresholds are explicit
  constants.
- **Headless degrades gracefully** — same command, template swap, no interactivity.

## Cyril feasibility

The "form input" instinct is right; the target of the form is the thing to get right.
In cyril terms this is a **client-side slash command + picker chain that composes a
`session/prompt`** — not a Kiro skill:

- Kiro file-prompts/skills accept only `$ARGUMENTS` (one string; multi-arg needs MCP —
  see `reference_kiro_file_prompts`), are Kiro-only, and can't run git themselves — the
  agent would burn a turn rediscovering the diff with tools.
- A composed prompt is vendor-neutral: it works identically against any ACP agent,
  which is cyril's whole thesis. The skill system stays an option for *review
  priorities* (a WATCHDOG.md-style body appended to the composed prompt), not for the
  mechanism.

### What cyril has vs. needs

| Piece | Status |
|---|---|
| Slash-command registry, `CommandResult::ShowPicker`, filterable picker overlay | exists |
| Picker confirmation routed to a **local continuation** | **gap** — `App::handle_picker_key` hardcodes Enter → `BridgeCommand::ExecuteCommand { command: <picker title>, args: {value} }`; the picker title doubles as the agent command name. Needs a picker target enum (e.g. `AgentCommand(name)` vs `LocalFlow(step)`) |
| Multi-step wizard (mode → branch/commit → compose) | **gap** — a small `ReviewFlow` state machine in App; precedent exists in the two-phase approval overlay (`ApprovalPhase`) |
| Local git execution | **gap** — cyril never shells out today; needs a small async git helper (branch list, log, diff, show). Client concern → `cyril` binary crate or a core `platform` module |
| Diff parsing + noise filter + sizing heuristic | straight port of omp's logic (~200 ln, pure functions, very testable) |
| Prompt template + send | exists — compose string, submit via the normal prompt path |

One adaptation: omp's distribution guidance says "use the `task` tool with
`agent: 'reviewer'`" — omp-specific. Kiro's fan-out story differs per engine (v2
`agent_crew`, KAS `OrchestrateSubAgent`), so cyril's template should either phrase
fan-out engine-aware or stay neutral ("review independent groups in parallel where
supported") and let the agent choose its own mechanism.

Estimated scope: ~300–600 lines (picker-target enum + flow state machine + git helper +
parser/filter + template), no new wire surface, no agent-side cooperation required.
