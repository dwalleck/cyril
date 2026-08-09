# oh-my-pi ↔ cyril — feature comparison and transfer analysis

*Analyzed 2026-08-02 against the oh-my-pi checkout at `~/repos/oh-my-pi` (`packages/coding-agent` @ 17.2.4 + unreleased) and cyril `main` @ `5c27b58`. Companion docs — the three depth-dives this breadth doc sits above: [`omp-advisor-analysis.md`](omp-advisor-analysis.md), [`omp-review-command-analysis.md`](omp-review-command-analysis.md), [`omp-tui-takeaways.md`](omp-tui-takeaways.md).*

oh-my-pi (omp) is a full agent harness: it owns the provider connections, the agent
loop, the tools, the prompts, and the conversation state. cyril is an ACP client. The
naive reading of that difference — "omp can do anything, cyril can only render" — is
wrong in a way that matters, and this doc exists to replace it with a sharper model.

**Thesis: omp's most distinctive work is concentrated in exactly the two places cyril
also has full authority.** The 32 tools, 75 provider descriptors, and 11 wire dialects
are real engineering but they are commodity harness plumbing; the ideas worth stealing
live in prompt composition, rendering, and the approval/host seat.

---

## The transfer model

Every omp capability falls into one of three buckets, and the bucket — not the size of
the implementation — determines what it costs cyril to borrow.

### Bucket A — the composition gap

Everything between *what the user typed* and *what goes into `session/prompt`*, plus
everything cyril renders on the way back.

Cyril owns this as completely as omp does. There is no authority gap at all: a client
that composes a prompt has the same power as a harness that composes a prompt. Anything
omp does here transfers at near-zero protocol cost — the only work is the implementation.

### Bucket B — the host seat

Where the agent asks cyril for something and cyril answers:

- `session/request_permission` (both engines)
- KAS fs + terminal host callbacks (KAS-5a/5b, shipped)
- KAS hooks host, including exit-2 `preToolUse` blocking (KAS-7, shipped)

For these operations **cyril is the executor**, which puts it in precisely omp's seat.
omp's decision-layer engineering applies literally here, not by analogy.

### Bucket C — inside the loop

Model routing, tool implementations, per-model prompt tuning, context and compaction.
The agent owns these. Reachable only via an MCP server cyril exposes to the agent, or a
wire-rewriting proxy stage — both real options, both expensive, neither free.

> **Reading rule for the rest of this doc:** a Bucket A item is a scheduling decision.
> A Bucket B item is a scheduling decision *plus* a correctness obligation. A Bucket C
> item needs an architecture decision before it needs an estimate.

---

## Scale calibration

Before any recommendation, the honest comparison:

| | oh-my-pi | cyril |
|---|---|---|
| Design docs | 70+ top-level, plus `docs/tools/` (31 per-tool refs) and `docs/toolconv/` (per-dialect) | ROADMAP + ADRs + protocol refs |
| Built-in slash commands | 69 | 12 client-side (one conditionally registered) |
| Tools | 32 built-in, 14 LSP ops over 53 servers, 28 DAP ops over 14 adapters | none — the agent's |
| Providers | ~75 descriptors, 11 wire dialects | none — cyril is multi-*vendor* instead |
| Themes | multiple, live-preview picker | one, hardcoded |
| Editor | `packages/tui/src/components/editor.ts`, 3,301 lines | insert / backspace / delete / arrows / home / end |
| Issue numbers | 7,300s | 107 open |
| Core | ~55k lines Rust behind a TS harness | 62.5k lines Rust, four crates |

**The first lesson is negative: do not chase surface area.** omp can afford a plugin
marketplace, a Chrome extension, and a terminal LaTeX layout engine. Cyril extracting
the right twelve things beats cyril reimplementing two hundred. Every recommendation
below is filtered for leverage, not completeness.

---

## Source map (omp)

Where the transferable work lives, for anyone reading the source directly.

| Concern | Path | Bucket |
|---|---|---|
| Slash-command registry (one spec, two dispatchers) | `src/slash-commands/builtin-registry.ts`, `types.ts` | A |
| Command source merge (builtin/skill/extension/custom/mcp/file) | `src/slash-commands/available-commands.ts` | A |
| Editor | `packages/tui/src/components/editor.ts`, `kill-ring.ts` | A |
| Autocomplete chain | `src/modes/prompt-action-autocomplete.ts`, `packages/tui/src/autocomplete.ts` | A |
| Magic keywords | `src/modes/magic-keywords.ts`, `magic-keyword-boundary.ts`, `docs/magic-keywords.md` | A |
| Paste (bracketed, tmux re-encode, OSC 5522, triage menu) | `packages/tui/src/bracketed-paste.ts`, `src/utils/enhanced-paste.ts`, `src/modes/controllers/input-controller.ts` | A |
| `@` import expansion | `src/discovery/at-imports.ts`, `docs/context-files.md` | A |
| Session persistence (append-only tree) | `src/session/session-manager.ts`, `session-entries.ts`, `session-paths.ts`, `blob-store.ts` | A |
| Prompt history | `src/session/history-storage.ts` | A |
| Context accounting (anchor model) | `src/session/session-stats.ts`, `src/modes/utils/context-usage.ts` | A |
| Context thresholds / status line | `src/modes/components/status-line/context-thresholds.ts` | A |
| Diff rendering | `src/modes/components/diff.ts`, `src/edit/renderer.ts` | A |
| Terminal capabilities / notifications | `packages/tui/src/terminal-capabilities.ts`, `desktop-notify.ts` | A |
| Hyperlinks (OSC 8) | `src/tui/hyperlink.ts` | A |
| Subagent display channels | `src/task/types.ts`, `src/task/render.ts`, `src/modes/session-observer-registry.ts` | A |
| Stats dashboard | `packages/stats/src/` (`parser.ts`, `sync-worker.ts`, `user-metrics.ts`) | A |
| Approval resolution | `src/tools/approval.ts`, `docs/approval-mode.md` | B |
| Approval enforcement + revision ordering | `src/extensibility/extensions/wrapper.ts:170-340` | B |
| Bash critical patterns + `bash.patterns` | `src/tools/bash.ts:109-230` | B |
| Capability + discovery-provider lattice | `src/capability/*.ts`, `src/discovery/*.ts` | C→A |
| Advisor | `src/advisor/`, `docs/advisor-watchdog.md` | A/B |
| Compaction | `src/session/session-maintenance.ts`, `packages/agent/src/compaction/`, `packages/snapcompact/` | C |

---

## Feature-by-feature comparison

| Area | oh-my-pi | cyril today | Bucket |
|---|---|---|---|
| Input editor | word motion, `ctrl+a/e/u/k/w`, kill ring with yank-pop, undo, atomic tokens, decorator hook | insert/backspace/delete/arrows/home/end only | **A** |
| Prompt history | SQLite + FTS5, `ctrl+R` fuzzy, merged into session-picker ranking | none | **A** |
| Multiline | `Shift+Enter`, plus platform-aware `ctrl+q` fallback | paste only — `Shift+Enter` documented in README, unimplemented | **A** |
| Autocomplete | 6 trigger families: `#PR`, `#action-palette`, `scheme://`, `:emoji:`, `/cmd`, `@file` | 2: `/cmd` (prefix-only), `@file` (fuzzy over `git ls-files`) | **A** |
| Paste | bracketed + tmux re-encode decoder + OSC 5522 images + large-paste triage menu | bracketed, raw insert | **A** |
| Diff rendering | live streaming diff with pinned tail window, intra-line word diff, indent viz, LSP diagnostics folded in, OSC 8 file links | `similar`, 20-line cap, 1 context line, first block per call only | **A** |
| Notifications | per-terminal protocol matrix, tmux/Zellij quirks, `notify-send`/`gdbus` fallback, Warp OSC 777 | none (CN1 not started) | **A** |
| Theme | multiple, live-preview picker, capability-driven | 29-role contract × 4 color-mode projections — one theme, hardcoded TrueColor | **A** |
| Transcript persistence | append-only JSONL tree, blobs, migrations, artifacts | **none**; 500-message in-memory cap, oldest dropped | **A** |
| Session ops | resume / continue / fork / branch / tree / move / export / share / collab | `/load <id>` — requires already knowing the id | **A** |
| Status line | 25 composable Powerline segments, 5 presets | fixed toolbar + status bar | **A** |
| Subagent display | 3 event channels; inline block + anchored HUD + hub + transcript viewer; park/revive lifecycle | crew panel; drill-in built but unreachable; KAS crews render as nothing (KAS-3) | **A** |
| Observability | `omp stats` dashboard, 10 routes, incremental JSONL ingest, no separate telemetry | none | **A** |
| Approval | tiers × modes, asymmetric `bash.patterns`; **no** trust persistence, `yolo` default | two-phase approval, trust tier persisted into `.kiro/agents/*.json` | **B** |
| Host-side execution | in-process `brush` shell, ~45 coreutils, per-tool output minimizer | KAS `host_shell` / `terminal_io` / `kiro_fs` | **B** |
| Hooks | file-discovered, block / revise-input / rewrite-output / replace-context | KAS-7 host, exit-2 `preToolUse` block | **B** |
| Compaction | 5 strategies incl. snapcompact bitmap frames | agent-side only | **C** |
| Tools / LSP / DAP | 32 / 14 ops / 28 ops | the agent's | **C** |
| Providers | ~75 descriptors, 11 dialects | the agent's | **C** |
| Memory | 4 backends incl. a full local SQLite engine | none | A/B |
| Advisor | in-process second model | feasible via a second ACP session — **and vendor-neutral** | A/B |
| Multi-client | encrypted relay, browser guest, QR join | none (ROADMAP: fan-out observer stage) | C |

---

## Tier 1 — client-side gaps worth closing

Ordered by leverage. All Bucket A.

### 1. Client-side transcript persistence — the keystone

Cyril persists nothing about a session; conversation state is entirely agent-side. The
only client-side write today is the trust grant into `.kiro/agents/*.json`.

That single absence blocks: export/share, transcript search, a stats view, session
listing that doesn't require memorizing an id, and the advisor's delta feed
([`omp-advisor-analysis.md`](omp-advisor-analysis.md) already identifies a core-side
transcript recorder as new build surface). **Nearly every other item in this doc is
downstream of it**, which is why it leads.

omp's design, worth copying closely (`src/session/session-manager.ts`,
`session-entries.ts`):

- Append-only **JSONL, one entry per line**, first line a header.
- Every non-header entry carries `{ id, parentId, timestamp }`; the manager holds a
  mutable `leaf` pointer. Appends create a child of the current leaf.
- Full rewrites are atomic (temp-write + rename, with an EPERM move-aside fallback).
- Base64 payloads ≥1024 chars externalize to a content-addressed blob store and resolve
  back on load.
- Versioned migrations run at load, set a rewrite flag, and flush on the next write.
  Corrupt or missing files are treated as empty and re-initialized — recovery, not failure.

Two details that are pure production scar tissue:

- **Persistence is deferred until the first assistant message exists.** Sessions that
  never got a response never touch disk.
- **Listing reads a 4 KB prefix plus a bounded 32 KB tail per file**, never the whole
  transcript. Prefix supplies metadata, tail supplies lifecycle status. The documented
  cost is that search text misses content beyond the prefix — an accepted, stated tradeoff.

### 2. Never destroy history — move a pointer

`ui.max_messages = 500` currently *drops* the oldest messages. omp's session model is
append-only throughout: `/tree` and `/branch` navigate by relocating a leaf, and the
abandoned path stays on disk. Even without adopting tree navigation, adopt the invariant.

### 3. Prompt history

Cyril has none — verified, no history buffer of any kind. omp keeps a separate SQLite DB
(`src/session/history-storage.ts`) with an FTS5 index and trigger sync, consecutive-dupe
dedup, and batched inserts on a ~100 ms drain queue. Up-arrow on an empty editor browses;
`ctrl+R` opens a fuzzy search overlay.

### 4. Editor competence — `cyril-4vvw`

Beyond the obvious emacs bindings, two structural ideas carry most of the value:

- **Atomic placeholder tokens.** `[Image #1, 800x600]` and `[Paste #2, +30 lines]` are
  indivisible: backspace anywhere inside deletes the whole token rather than corrupting it.
- **A decorator hook** that post-processes displayed text *after* layout, so highlighting
  never disturbs pre-measured widths.

Also ship `Shift+Enter` — the README already claims it works. omp's keybinding defaults
carry the platform reasoning worth inheriting: bind `ctrl+q` alongside because Windows
Terminal never emits a distinct `Ctrl+Enter`, and `shift+up` alongside `alt+up` because
macOS Terminal.app eats Option for composition.

### 5. Large-paste triage menu

Past a line threshold, omp offers three choices: wrap as an `<attachment>` block, **write
to a file and insert a reference the agent can read on demand**, or paste inline — with
Esc falling back to inline so content is never lost.

Cyril already has `@file` reference expansion on submit, so the middle branch is nearly
free, and it is the single best context-economy win available to a client.

### 6. Harden `@` expansion

Cyril's `parse_file_references` should adopt omp's import rules wholesale
(`src/discovery/at-imports.ts`) — each one exists because of a false positive:

- Relative paths resolve from the **importing file's** directory; `~` and absolute supported.
- Tokens inside fenced blocks and inline code stay literal.
- `git@github.com:…` and `user@example.com` are excluded by requiring line-start or a
  preceding space/tab.
- Trailing sentence punctuation (`. , ; : ! ? ) ] } " '`) is trimmed.
- Recursion caps at 5 hops; cycles are skipped.
- A missing target leaves the literal `@token` rather than erroring.

> The same class of care governs omp's magic keywords, which ignore fenced blocks and
> inline code spans so `orchestrate.ts` in a path never fires. That spec is worth
> copying verbatim rather than re-deriving — it was written against real false positives.

### 7. Notifications (CN1) — copy, don't re-derive

`packages/tui/src/desktop-notify.ts` and `terminal-capabilities.ts` encode expensive
ecosystem knowledge, and the header comments document which protocols were tried and why
each failed:

- Under **tmux**, OSC 9/99 must be wrapped in DCS passthrough *and* have a real BEL
  appended — a bare OSC doesn't trip tmux's own `monitor-bell`.
- **Zellij** drops OSC 9/99 entirely and has no DCS envelope, but raises its own bell.
- OSC 99 rich fields are used only after a **runtime probe** confirms support; payloads
  are base64'd if they contain unsafe C0/C1 bytes and chunked at 2048 bytes.
- The whole VTE family (GNOME Terminal, Ptyxis, Tilix, Terminator), Alacritty, and xterm
  report Bell-only, so Linux falls through to `notify-send`, then `gdbus` with the
  `s u s s s as a{sv} i` signature.

Adjacent and cheap: OSC 0 title updates, and OSC 9;4 indeterminate progress **on a
keepalive interval** because some hosts time it out.

### 8. Wire up the subagent drill-in

`UiState::focus_subagent` (`crates/cyril-ui/src/state.rs:1529`) is referenced only by its
own tests; `app.rs:616` binds only `unfocus_subagent`. The per-subagent stream view is
implemented, tested, and reachable by nobody. This is a one-keybinding fix for an
already-paid-for feature.

### 9. Context accounting — replace last-writer-wins

Two independent sources (`kiro.dev/metadata` percentage, ACP `usage_update` counts)
currently write the same field on a last-writer-wins basis. omp's `SessionStatsTracker`
is strictly better and directly portable:

- Walk the branch backwards (stopping at the last compaction) for the newest assistant
  message carrying **real provider-reported usage** — that's the anchor.
- `used = anchorPromptTokens + delta(nonMessageTokens) + Σ estimate(messages after anchor)`.
- Expose **`anchored: bool`** so the UI can distinguish truth from estimate.
- `recordAnchoredHistoryRewrite(tokensRemoved)` lets a prune or compaction correct the
  anchor immediately instead of lagging a full turn.

Display ideas worth taking with it: escalate colour on
`min(percent threshold, absolute-token threshold as percent of window)` so a 1M-window
model still warns at 150k; and render `<tokens>/?` when the window is unknown, rather
than a `0.0%/0` that falsely implies an empty context.

### 10. Smaller wins

- **OSC 8 hyperlinks** on tool-call paths with `?line=N&col=M`. omp's enablement cascade
  is the wisdom: env override → per-terminal capability → *screen anywhere in the path
  disables* → tmux ≥3.4 by version parse → screen-family `TERM` always off.
- **Theme activation** (`cyril-qaq0`, `cyril-fkke`). The 29-role contract and all four
  colour-mode projections already exist; `theme.rs:474` asserts `ThemeId::ALL` has exactly
  one entry and `state.rs` hardcodes TrueColor. Add capability detection and a picker with
  live preview as the cursor moves.

---

## Tier 2 — design lessons

Principles, not features. These prevent future pain rather than closing present gaps.

**Do everything deterministic client-side; the model only judges.** The thesis of omp's
`/review` (see [`omp-review-command-analysis.md`](omp-review-command-analysis.md)) and
cyril's structural advantage generalized. A client that hands the agent a parsed,
filtered, pre-sized diff beats one that makes the agent burn a turn rediscovering it.

**Degrade visibly, never silently.** omp writes `[Output truncated - N tokens]`,
`[Uneventful result elided]`, and lists excluded files *with reasons*. Cyril has two live
violations of its own CLAUDE.md rule ("Log before returning `None`"):

- `convert_tool_call_content` (`crates/cyril-core/src/protocol/convert/mod.rs:135`) is a
  `filter_map` with a bare `_ => None` and a silent `else { None }` — Image, Audio,
  ResourceLink, EmbeddedResource, and Terminal content vanish with no log. (ROADMAP K2.)
- The KAS 5-bucket context breakdown is dropped without trace when the status line
  doesn't fit.

**Measure the cost of your own instrumentation.** omp's `MIN_PRUNE_TOKENS = 50` exists
because the `[Output truncated]` placeholder itself costs ~8 tokens — pruning below the
floor *grows* context and churns the provider cache. Generalizes to any truncation,
placeholder, or annotation cyril adds.

**Approval must bind to the thing that actually executes.** omp lands hook input-revisions
*before* the approval gate (`extensions/wrapper.ts:200-263`) so the user approves exactly
the command that runs, and re-resolves if a revision newly denies. Directly relevant to
the KAS-7 hooks host.

**Asymmetric matching for allow vs deny** (`src/tools/bash.ts:150-230`). `deny`/`prompt`
fire when the pattern matches the whole command **or any single segment** of a compound
line (split on `&&`, `||`, `;`, `|`, `&`, subshells, newlines); `allow` must match the
**entire** command and never rides a compound line. So `rm -rf *` still denies
`cd /tmp && rm -rf build`, while `git *` cannot vouch for `git status && rm -rf /`.
Relevant to trust persistence into `execute_bash.allowedCommands`.

**Fail closed on the gates that matter.** omp's provider safety checks are documented as
bypassable by no setting and no mode. A throwing `tool_call` handler blocks execution
rather than being swallowed.

**Declarative subcommand metadata gives autocomplete for free.** omp's command specs
declare `subcommands` and `inlineHint`; dropdown completions and dim ghost text are
auto-derived. Cyril's `/help` is a hardcoded name list that already drifts from the
registry — declarative metadata fixes the drift and the autocomplete in one move.

**One spec, two dispatchers.** omp commands carry a TUI-agnostic `handle` plus an optional
`handleTui` override, which is how one registry serves both the TUI and their ACP mode.
Cyril's `--prompt` is parsed and never read (`cyril-0ffy`); this is the shape that makes
headless mode cheap instead of a fork.

**Capture full rollback state before a risky transition.** omp's `switchSession()` captures
session, messages, queued steer/follow-up, model/thinking/tier, MCP selections, tools, and
system prompt before mutating anything, and restores on any post-capture failure.

**Comment invariants with their issue number.** omp does this pervasively — semaphore
resize-in-place, provider-concurrency deadlock, xdev demotion. Cyril already has rivets
IDs; same move, near-zero cost, and it makes non-obvious constraints survive refactors.

---

## Tier 3 — strategic

### The capability + discovery-provider lattice

The idea worth thinking hardest about.

omp natively ingests Claude Code, Codex, Gemini CLI, Cursor, VS Code, Windsurf, Cline,
OpenCode, and GitHub Copilot config layouts — skills, commands, hooks, MCP servers,
instruction files — through one registry (`src/capability/`, `src/discovery/`). Providers
sort by numeric priority; each capability declares a dedup `key`; first item with a given
key wins, and shadowed duplicates are **retained** with a `_shadowed` marker rather than
discarded.

Notice what that actually is:

> **omp is vendor-neutral at the configuration layer. cyril is vendor-neutral at the
> protocol layer.** These are orthogonal axes, and they compose.

A cyril stage that harvests skills, `AGENTS.md`/`CLAUDE.md` context files, and MCP
definitions from every ecosystem's layout and feeds them to *any* ACP agent gives agents
capabilities they do not ship — including agents with no skill system at all. That is
ROADMAP Phase 5's "skill resolver, context injector" and the enhancing-proxy thesis,
stated concretely. omp demonstrates the ingestion layer is tractable and hands over a
working precedence model.

Two adjacent details worth lifting if this is pursued:

- **Sticky rules vs one-shot context.** omp's `RULES.md` loads as an always-apply rule
  re-attached near the current turn, not as a context file injected once at session start —
  so it survives long conversations. Cyril composes prompts, so it can do this.
- **Whole-provider kill switches share an id namespace with model providers**, which omp's
  own docs flag as a footgun (disabling `claude` drops CLAUDE.md *and* Claude-discovered
  MCP servers, commands, skills, hooks). Worth designing around rather than inheriting.

### The advisor

Already analyzed in [`omp-advisor-analysis.md`](omp-advisor-analysis.md) and found very
feasible — every load-bearing primitive exists on the wire today, and K1 steering shipped.
The strategic point bears repeating here: **the vendor-neutral version is something no
single-vendor harness can ship.** Kiro primary with a Claude advisor is a genuinely
independent second opinion; omp's advisor is always the same vendor as the primary.

### `/btw` — side questions without transcript pollution

omp answers an aside against current session context in a dedicated panel *without*
appending to the transcript; if the aside turns out to be real work, it promotes to a
full background agent. For cyril this is a short-lived second ACP session against the same
cwd — the same primitive the advisor needs, so it comes nearly free once that runner exists.

### Stats, once transcripts exist

`omp stats` derives everything from the JSONL transcripts already on disk — no separate
telemetry pipeline — with incremental ingest via a `file_offsets` table so re-sync reads
only appended bytes. Cyril additionally has session sidecar metering
(`~/.kiro/sessions/cli/{uuid}.json`) as a credit source. Cheap to follow Tier 1 item 1.

---

## What not to copy

- **The tool layer, providers, and wire dialects.** 32 tools, ~75 provider descriptors,
  11 dialects, 53 LSP servers, 14 DAP adapters. Chasing any of it abandons the thesis
  that makes cyril worth building.
- **Snapcompact.** The cleverest thing in the repo — it rasterizes discarded history into
  dense pixel-font PNG frames that vision models read back, with frame geometry chosen per
  provider from recall evals against real billing. It requires owning the context, so it
  isn't cyril's.
- **The 5,876-line settings schema.** Cyril's two-field `ui` block with a schema-lock test
  asserting it stays exactly two fields is *better* discipline. Defend it.
- **The plugin marketplace and ~4,100 lines of legacy compatibility shims.** Pure
  surface-area debt inherited from a predecessor ecosystem.
- **The unsandboxed in-process extension model.** omp is explicit that extensions are not
  sandboxed; its managed-timer wrapper exists specifically because a raw `setInterval`
  callback that throws becomes an `uncaughtException` and tears down the session.
- **`yolo` as the default approval mode.**

---

## Where cyril is already ahead

Worth stating plainly, because a size comparison obscures it.

- **Trust persistence.** omp has *none* — a two-option Approve/Deny prompt, no "don't ask
  again", no learned allowlist, no folder trust. Persistent permission lives entirely in
  declarative config, with `yolo` as the default posture. Cyril's two-phase approval with
  trust-tier selection written back to the agent's own config is the richer design.
- **Workspace safety rails.** `unsafe_code = "forbid"`, `unwrap_used = "deny"`, zero
  `#[allow]`, pinned toolchain, centralized dependency versions. No omp equivalent.
- **Vendor neutrality at the protocol layer**, which enables the different-vendor advisor.
- **Maintenance surface.** Cyril carries none of the provider/tool/prompt burden that
  generates the bulk of omp's issue volume.

---

## Suggested order

1. **Client-side transcript persistence** — unblocks items 3, 6, 8, and both Tier 3 stats
   and advisor work. Do this first or most of the list stays blocked.
2. **Editor competence + prompt history + `Shift+Enter`** (`cyril-4vvw`) — the most
   user-visible quality-of-life delta, independent of everything else.
3. **Context-accounting anchor model** — replaces a live last-writer-wins defect.
4. **Notifications** (CN1) — self-contained, and the terminal matrix is copy-not-derive.
5. **Wire up subagent drill-in** — one keybinding for already-built, already-tested code.
6. **Large-paste triage + `@` expansion hardening** — small, high context-economy return.
7. **Theme activation** (`cyril-qaq0`, `cyril-fkke`) — machinery exists, needs a picker
   and capability detection.
8. **Declarative command metadata** — fixes `/help` drift and autocomplete together.
9. **Capability/discovery lattice** — only after Phase 3/4 make multi-vendor real; it is
   an architecture decision, not a feature.

Tier 2 lessons are not scheduled items — they are review criteria to apply to whatever
lands.

---

## Verification notes

Claims about cyril's code marked below were verified directly against the tree at
`5c27b58`; everything else in the cyril columns comes from a code survey and should be
re-checked before being used as a work estimate.

| Claim | Status |
|---|---|
| `convert_tool_call_content` silently drops non-Text/Diff content | **verified** — `crates/cyril-core/src/protocol/convert/mod.rs:135`, `filter_map` with bare `_ => None` and silent `else { None }`, no logging |
| Subagent drill-in unreachable | **verified** — `focus_subagent` at `crates/cyril-ui/src/state.rs:1529` referenced only by tests at 4601–4634; `crates/cyril/src/app.rs:616` binds only `unfocus_subagent` |
| Exactly one theme, hardcoded | **verified** — `crates/cyril-ui/src/theme.rs:474` asserts `ThemeId::ALL == &[ThemeId::CyrilDark]` |

omp claims are sourced from the checkout plus its `docs/` tree, which tracks the code
closely. Two known drift points found during the survey: `docs/tools/edit.md` still
documents the superseded `SWAP`/`INS.*` opcode set, and the README's tool list names
`ssh`, `search`, and `find` — the first is a URL scheme rather than a registered tool, the
latter two are legacy aliases normalized to `grep`/`glob`.
