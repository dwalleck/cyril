# kiro-cli 2.16.2 wire audit (2026-08-08, vs 2.16.0)

**Verdict: SAFE for cyril on both engines.** Nothing was removed or renamed. Four additive
field-level changes, two new `_kiro/*` methods cyril does not speak, and one **workflow
semantics change that would have produced a shipped bug** had cyril built to the 2.16.0 spec
(see §"Workflow completion semantics"). The v2 (Rust) ACP surface is frozen apart from one
additive field.

This audit spans **two releases** since 2.16.0, in the 2.15.0 audit's tradition of folding point
releases into one document and attributing per version:

- **2.16.1** (2026-08-04) — `@kiro/agent` **0.27.8 → 0.30.9**, +23 KAS modules, +1 method
  (`_kiro/spec/phaseCheckpoint`). Absent from the versioned S3 origin's manifest index, so it
  was archived from the running install rather than downloaded.
- **2.16.2** (2026-08-05) — `@kiro/agent` **0.30.9 → 0.35.11**, +28 KAS modules, +1 method
  (`_kiro/powers/list`). `latest/manifest.json` reported this as newest stable on 2026-08-08.

> **2.16.3 does not exist on the stable origin.** `latest/manifest.json` → 2.16.2; the versioned
> path `2.16.3/…` returns **403, identical to a nonexistent 2.16.4**; AUR was on 2.16.0. Recorded
> because a 2.16.3 sighting prompted this audit — most likely the Kiro **IDE**, which ships on an
> independent version line.

## Headline: kiro-cli patch versions do not bound KAS change

`@kiro/agent` moved **eight minor versions across two kiro-cli patch releases** (0.27.8 →
0.35.11). The prior for this project — set by 2.11.1 (stealth hotfix, byte-identical KAS) and
2.14.2 (folded into the 2.15.0 audit) — was that point releases are cosmetic. **That prior is
dead for the 2.16.x line.** Treat a kiro-cli patch bump as an unbounded KAS delta until the
bundle version says otherwise.

| Axis | 2.16.0 | 2.16.1 | 2.16.2 |
|---|---|---|---|
| `@kiro/agent` | 0.27.8 | **0.30.9** | **0.35.11** |
| `_kiro/*` distinct literals | 107 | 108 | 109 |
| KAS bundle modules | 736 | 759 | 787 |
| `_kiro/workflow` references | 132 | 132 | **154** |
| `_kiro/workflow/*` method set | — | identical | **identical** |
| Doc manifest | 139 | 139 | 139 |
| Rust module paths | 219 | +`chat_cli::cli::crew` | +`chat_cli::launch::auto_migrate` |

(Literal/module counts use this audit's regex and are **not** comparable to the 2.16.0 audit's
absolute numbers; the deltas are, since both sides use the same method.)

## Wire deltas — four, all additive

Established by a **same-day** A/B (both legs captured 2026-08-08, four real turns each:
read → non-zero exit → write → subtask), so these are attributable to the **binary** axis.

### 1. `session/new` promotes the `model` config option into its initial response

The largest change, and the only one that adds a whole surface rather than a field.

| | `session/new` result `configOptions` | via later `config_option_update` |
|---|---|---|
| 2.16.0 | `[mode, autopilot, contentCollection]` | `[mode, model, autopilot, contentCollection]` |
| 2.16.2 | `[mode, **model**, autopilot, contentCollection]` | `[mode, model, autopilot, contentCollection]` |

Both versions *push* the model option; 2.16.2 also returns it **up front**. The 19 options each
carry `_meta.kiro.{hasEffort, rateMultiplier, rateUnit}`, and the 9 with `hasEffort: true` also
carry `effortLevels` + `defaultEffortLevel`.

This is exactly the case ROADMAP **KAS-4** anticipated: *"Read the initial `configOptions` from
the `session/new` response — `session_created_from_response` currently reads only
`modes`+`models` and ignores `config_options`."* On 2.16.2 that initial read now yields a
complete model picker without waiting for a push. Bears on **cyril-lxuo** (per-model capability
in the picker) and **cyril-4jt7** (effort levels).

### 2. `_kiro/sessions/changed` — `+ upserted[].updatedAt`

Continues the pattern the 2.16.0 audit flagged, where cloud/relay vocabulary keeps surfacing on
purely local runs (that frame already gained `source` and `executionTarget.kind` in 2.16.0).
Inert; cyril tolerates this notification and consumes nothing from it.

### 3. `session/update::agent_message_chunk` — `+ update._meta.kiro.replayId`

Per-chunk id on the main text stream, e.g. `"903ef876-f1ff-4a6b-ab21-cab7002369ed-say"` (UUID
plus a role-ish suffix). It is the per-chunk companion to the connection-level `replayMarking`
capability added in 2.15.0 and the `_meta.kiro.replay` flag on replayed `session/load` updates —
so the replay story now has two halves on the wire. If cyril ever de-duplicates or re-anchors a
replayed stream, this is the join key. Tracked on **cyril-99ds**.

### 4. v2 settle — `+ prompts[].serverName`

The one v2 change. `prompts` **is** parsed by cyril (`convert/kiro.rs`, into `PromptInfo`), but
selectively, so the new field is ignored rather than breaking. It would disambiguate
MCP-server-provided prompts if cyril ever wants that.

## Workflow completion semantics — the finding that matters

`_kiro/workflow/*`'s **method set is byte-identical** to 2.16.0, and the gate behaviour is
unchanged ([ADR-0011](adr/0011-ungated-client-driven-workflow-control-plane.md) re-verified on
2.16.2: discovery, authoring, `invoke` and the full lifecycle stream all work with
`workflowsEnabled: false`). But the **payload semantics moved underneath the frozen method set.**

2.16.2 adds a `NodeState` field:

```jsonc
completionSignal:       enum(["success","need_input","error"]).optional()
completionSignalSource: enum(["send_message","status_update"]).optional()   // NEW
```

Live-settled with three arms × 2 reps, zero within-arm variance
(`probe-kas-completion-signal-2.16.2.py`):

| arm | binary | step prompt | `completionSignal` | `completionSignalSource` |
|---|---|---|---|---|
| C (control) | 2.16.0 | neutral — never mentions send_message | `success` | *not in schema* |
| B | 2.16.2 | neutral — never mentions send_message | **None** | **None** |
| A | 2.16.2 | explicit send_message, severity success | `success` | **`send_message`** |

Same one-step DAG, same `wf-coder` agent, identical prompt text between B and C. All six runs
reached `status: completed` with **zero** `paused`/`node_paused` events.

**What changed:** 2.16.0 recorded `completionSignal: "success"` even when nothing asked for one.
2.16.2 leaves both fields unset unless the model genuinely calls `send_message`, and completes
regardless. `completionSignal` went from *precondition for completion* to *honest record of what
happened*, with `completionSignalSource` attributing it.

**Consumer rule for 2.16.2+:** an absent `completionSignal` on a **completed** node is NORMAL.
Do not read it as incomplete, hung, or awaiting input. Hazard 4 from the 2.16.0 audit — "a
step's outcome is decided by a model-issued tool call" — survives only in weakened form:
`send_message` is one path, the stale-run force-pause machinery is unchanged
(`STALE_RUNNING_PAUSE_REASON` present in both bundles), and a step can still stall awaiting
input. It is simply no longer true that a node must be signalled to complete. Recorded on
**cyril-6beh**, whose design notes encode the old rule.

**Unobserved live:** `completionSignalSource == "status_update"`. Static evidence is exact — the
bundle's failed-status transition sets it verbatim beside `completionSignal = "error"` — so it
reads as engine-set versus model-elected. Forcing a genuine engine-side node failure belongs
with the audit tradition's **elective-mechanism** limit, same class as `node_paused`.

Three workflow modules are new in the 2.16.1–2.16.2 range: `src/workflow/lifecycle-events.ts`
(the emitter, moved out of `workflow-notification-bridge.ts`), `node-state-transforms.ts`, and
`run-disk-operations.ts`. The last one touches run persistence and therefore the
reattach-on-demand model in **cyril-0qe6**.

## `kiro-cli crew` — a Crew launcher ships in the CLI (2.16.1)

The nm diff's lone 2.16.1 addition, `chat_cli::cli::crew`, is a real subcommand. From the
official notes: *"Launch Kiro Crew with `kiro-cli crew`. On macOS and Linux, Kiro CLI can
**install Crew when it's missing**; use `--yes`/`-y` to skip the installation prompt. Arguments
after `crew` are forwarded directly to Crew. On Windows, install Crew separately first."*

Off cyril's ACP path — it is a launcher, not a wire change — but strategically adjacent on two
counts. Kiro Crew is AWS's open-source ACP **orchestrator**, and per
`reference_kirocrew_platform` it drives `kiro-cli acp --agent` (v2, not KAS); cyril's **W track**
is building orchestration over the same ecosystem. And kiro-cli now *bootstraps* an installer for
a second ACP client on the user's machine. Nothing to implement; worth knowing the vendor is
shipping a first-party path to an orchestrator that competes for the same job.

## Two new methods, neither on cyril's path

- **`_kiro/spec/phaseCheckpoint`** (2.16.1) with emitter `src/spec/session/spec-phase-checkpoint-emitter.ts`.
  Matches 2.16.2's changelog line *"Spec sessions self-repair malformed spec artifacts via
  agent-side format validation"*, and arrives alongside a new `src/spec/symbolic/**` analyzer
  subsystem. Spec remains a **KAS-7 non-goal**.
- **`_kiro/powers/list`** (2.16.2) — **do not dismiss this one.** The embedded changelog says
  nothing about it; the *official* release notes do: *"Powers aligned with the **open Agent
  Plugin format** now load in V3 sessions. Kiro now supports plugins that bundle **skills and
  MCP**, making powers easier to share **across compatible agent tools**."* It arrives with
  `src/powers/agent-plugin-loader.ts` (new in 2.16.1) and joins the already-tolerated
  `_kiro/powers/items_changed` notification.

  An *open, cross-tool* plugin format bundling skills + MCP is squarely the territory of the
  platform vision's **Skill resolver** stage (ROADMAP Phase 5) — cyril would plausibly want to
  both *consume* such plugins and *supply* them. It also sits beside
  `reference_kiro_kas_command_surface` (KAS advertises only skills). Warrants its own
  investigation rather than a KAS-8 "record and defer": what the format is, whether it is
  genuinely open or Kiro-specific, and whether `list` is the only client-facing verb.

## Backend axis — measured, and flat

The 2.16.0 leg was **re-captured today** specifically to isolate the axes, giving a same-binary
pair eight days apart. Result: **zero field deltas and zero value deltas.** No backend rollout
touched this surface in that window.

This matters as method, not just result. The **cross-day** diff appeared to show the model
catalog churning (Claude Opus 5, GLM 5, MiniMax M2.5, Qwen3 Coder Next, …), which reads exactly
like the April→May 2026 `meteringUsage[]` rollout — a backend change with no version bump. It
was not. Those 19 names showed as "added" purely because 2.16.2 promotes `model` into
`session/new` (§1); the catalog itself is identical in both. **A cross-day diff cannot tell a
backend rollout from a binary change, and guessing which one it is produces confident wrong
answers.** The same-binary re-capture is what settles it, and it is cheap.

## Other verifications

- **Terminal falsifier still passes on 2.16.2**: host exits 3, client replies flat
  `{exitCode, signal}` (no `exitStatus` wrapper), agent reports *"The exit code was 3."*
  Re-confirms `reference_kiro_terminal_wait_exit_reply_shape` on both binaries.
- **Host-callback inventory identical** across the pair: `fs/read_text_file` ×3,
  `fs/write_text_file`, `terminal/{create,output,wait_for_exit,release}`,
  `session/request_permission` ×2, `_kiro/auth/getAccessToken`, `_kiro/terminal/shell_type`.
- **`session_info_update` kinds unchanged** — all eight still present.
- **Doc manifest frozen** at 139 documents across all three versions: no additions, removals, or
  revalidations. Still **no workflow doc** — the 2.16.0 audit's signal that Kiro intends to make
  workflows public has not fired.

## Methodology correction — the embedded changelog is NOT the release notes

This audit's step 2 read `kiro-cli version --changelog=<ver>` and treated it as the claimed
change set. **It under-reports features.** Compared against the official release notes:

| Release | Embedded changelog | Omitted from it |
|---|---|---|
| 2.16.1 | 9 entries | **`kiro-cli crew` launcher** (the headline feature); "some V3 sessions failing to continue after an interrupted tool call" |
| 2.16.2 | 20 entries | **Powers / open Agent Plugin format** (the headline feature) |

The pattern is consistent: the embedded list carries the **fixes** and drops the **features** —
precisely inverting what a wire audit most needs. Both omissions were the largest items in their
release, and both mapped to static findings this audit had already surfaced but under-weighted
(`chat_cli::cli::crew`, `_kiro/powers/list` + `agent-plugin-loader.ts`).

**Consequence for the audit method:** the embedded changelog is a cheap first signal, not the
claim set. Cross-check against the published release notes before deciding what a release
"claims" to change — and treat an unexplained nm/module/literal addition as more likely to be a
real feature than a refactor. Recorded in `reference_kiro_changelog_command`.

## Coverage — what this audit did NOT check

- **Two lanes, disjoint surfaces.** The turn A/B never runs a workflow, so
  `completionSignalSource` is invisible to it; the workflow probes never stream agent text, so
  `replayId` is invisible to them. Neither lane alone would have cleared 2.16.2. Anything
  exercised by *neither* scenario is unchecked by construction.
- **v2 command responses** were not re-swept (the 2.16.0 audit's 16-command sweep). The v2 settle
  surface and turn path were checked; a command-response change would be missed.
- **`_kiro/spec/*` and `_kiro/powers/list`** were recorded, not called.
- **The ten non-`invoke` workflow mutating verbs** (`cancel`, `pause`, `resume`, `resumeAll`,
  `retry`, `load`, `delete`, `update`) remain unverified gate-off, as in ADR-0011.
- **Tool-call interruption/recovery is unchecked**, and both releases touched it: 2.16.1 fixed
  *"some V3 sessions failing to continue after an interrupted tool call or inconsistent tool-call
  record"*; 2.16.2 fixed *"subagents recover from a tool call interrupted mid-arguments instead
  of faulting the turn"*. Cyril renders `tool_call`/`tool_call_update` and merges partial updates,
  so a change in which frames arrive on an interrupted call is on its path. No probe scenario here
  interrupts a tool call.
- **Powers / the Agent Plugin format** was recorded from release notes and module names only —
  `_kiro/powers/list` was never called.
- **2.16.1 was not independently A/B'd on the wire.** Its static delta is characterized here; the
  live A/B endpoints are 2.16.0 and 2.16.2. A field that appeared in 2.16.1 and was withdrawn by
  2.16.2 would be invisible — the `_kiro/frontendToolCall` failure mode (shipped 2.13.0, withdrawn
  2.14.0). Both binaries are archived, so this is reconstructible if it ever matters.

## What this means for cyril

1. **No breakage.** Every wire change is additive and cyril's parsers skip unknown keys by
   construction.
2. **One would-be shipped bug caught.** A workflow state machine built to the 2.16.0
   `completionSignal` rule would misread every neutral-prompt step on 2.16.2 as unfinished.
   **cyril-6beh** updated.
3. **ADR-0011 holds on 2.16.2** — the W-track's foundational decision survives the upgrade.
4. **KAS-4 gets easier**: the model picker is now available at `session/new` without waiting for
   a push (**cyril-lxuo**, **cyril-4jt7**).
5. **Upstream fixed a bug cyril worked around**: 2.16.2's *"[V3] Steering sent near the end of a
   turn is delivered at the turn boundary instead of dropped"* is the turn-tail steer drop behind
   **cyril-nvmh** / **cyril-7z7u**. Worth re-testing those against 2.16.2 before doing more
   client-side work.
6. **Powers / the open Agent Plugin format is a platform-vision item, not a KAS-8 footnote.**
   Plugins bundling skills + MCP, shareable across compatible agent tools, is what the Phase-5
   **Skill resolver** stage was scoped to do. Investigate before assuming it is Kiro-internal.

## Reproduction

```sh
B=~/.local/share/kiro-research/binaries

# static triage (all free, offline)
#   @kiro/agent version + bundle sha:  <kas-root>/<ver>-<sha>/node_modules/@kiro/agent/package.json
#   method literals:  grep -oE '"_kiro/[a-zA-Z0-9/_]+"' acp-server.js | sort -u | wc -l
#   modules:          grep -oE '^// src/[a-zA-Z0-9/_.-]+' acp-server.js | sort -u | wc -l
#   rust modules:     nm -C --defined-only kiro-cli-chat | grep -oE '\b(kiro_[a-z_]+|chat_cli)::[a-z_]+'
extract_doc_manifest.py $B/2.16.2/kiro-cli-chat docs/kiro-docs-index-2.16.2

# free wire lanes
probe-v2-surface-ab-2.11.0.py  $B/2.16.2/kiro-cli-chat v2-surface-2.16.2.jsonl
probe-kas-hostinit-2.15.0.py   $B/2.16.2/kiro-cli-chat kas-hostinit-2.16.2.jsonl
diff_fields.py v2-surface-2.16.0.jsonl v2-surface-2.16.2.jsonl

# paid: 4 real turns per leg. BOTH legs same-day, or the axes conflate.
probe-kas-turn-traffic-ab-2.16.0.py $B/2.16.2/kiro-cli-chat kas-turn-2.16.2.jsonl
probe-kas-turn-traffic-ab-2.16.0.py $B/2.16.0/kiro-cli-chat kas-turn-2.16.0-sameday.jsonl
diff-acp-wire.py kas-turn-2.16.0-sameday.jsonl kas-turn-2.16.2.jsonl \
    --label-old 2.16.0sd --label-new 2.16.2
# backend axis: same binary, different days
diff-acp-wire.py kas-turn-2.16.0.jsonl kas-turn-2.16.0-sameday.jsonl

# workflow lane (disjoint from the turn lane — run both)
probe-kas-workflow-gateoff-2.16.0.py        $B/2.16.2/kiro-cli-chat kas-workflow-gateoff-2.16.2.jsonl
probe-kas-workflow-diskrecipe-gateoff-2.16.0.py $B/2.16.2/kiro-cli-chat kas-workflow-diskrecipe-2.16.2.jsonl
probe-kas-completion-signal-2.16.2.py $B/2.16.2/kiro-cli-chat kas-csig-2.16.2-neutral.jsonl  neutral  2
probe-kas-completion-signal-2.16.2.py $B/2.16.0/kiro-cli-chat kas-csig-2.16.0-neutral.jsonl  neutral  2
probe-kas-completion-signal-2.16.2.py $B/2.16.2/kiro-cli-chat kas-csig-2.16.2-explicit.jsonl explicit 2
```
