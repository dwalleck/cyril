# Kiro CLI 2.4.1 — ACP Wire Audit

> **Captured 2026-05-21** using `experiments/conductor-spike/` against today's backend. Binary-isolated test: same backend, two binaries (2.3.0 archived vs 2.4.1 freshly installed). Methodology details in [`docs/kiro-acp-protocol.md`](kiro-acp-protocol.md) and the `wire-audit-methodology` reference in user auto-memory.

**Released**: 2026-05-21 (S3 tarball; manifest version bumped 2.3.0 → 2.4.1, with 2.4.0 also published on S3). Build hash `937fa9a6ff55382dd599408c80bb4b87517146ad` dated 2026-05-21.

## Audit setup

Binary-isolated, same-backend test:

- **Reference**: 2.3.0 (archived at `~/.local/share/kiro-research/binaries/2.3.0/`) — `kiro-cli-chat` sha256 `96ba63cb…`
- **Capture**: 2.4.1 (archived at `~/.local/share/kiro-research/binaries/2.4.1/`) — `kiro-cli-chat` sha256 `8d125143…`
- **Tarball**: `https://desktop-release.q.us-east-1.amazonaws.com/2.4.1/kirocli-x86_64-linux.tar.zst` (sha256 `91f41aff…`)
- **Wrappers**: `experiments/conductor-spike/conductor-wrapper-2.3.0.sh`, `conductor-wrapper-2.4.1.sh`
- **Harness output**: `experiments/conductor-spike/test_bridge-2.3.0.out`, `test_bridge-2.4.1.out`
- **Harness change**: added steps `9b` (`/stats` post-prompt) and `9c` (`/effort` options) to `crates/cyril/examples/test_bridge.rs` to exercise the two release-targeted areas. Existing steps unchanged; comparisons against pre-2.3.0 captures still hold for the original 10-step scenario.

## Exercised-wire result

Across the 14 ACP methods exercised by both runs, **one structural field delta**:

```
[_kiro.dev/commands/options (response)]
  + result.options
```

Reason: 2.4.1 returns a well-formed `commands/options` response for `command: "effort"` with `options: []`. 2.3.0 forwards the request to its kiro-cli-chat backend and the backend **silently never responds** — id 11 stays open and the next request never goes on the wire. The differ records this as "response present in 2.4.1, absent in 2.3.0," which is correct framing.

```
[_kiro.dev/commands/available (request/notif)] — no field deltas (17 fields)
[_kiro.dev/commands/execute (request/notif)] — no field deltas (4 fields)
[_kiro.dev/commands/execute (response)] — no field deltas (57 fields)
[_kiro.dev/commands/options (request/notif)] — no field deltas (3 fields)
[_kiro.dev/commands/options (response)] — + result.options
[_kiro.dev/metadata (request/notif)] — no field deltas (6 fields)
[_kiro.dev/subagent/list_update (request/notif)] — no field deltas (2 fields)
[initialize (request/notif)] — no field deltas (7 fields)
[initialize (response)] — no field deltas (12 fields)
[session/new (request/notif)] — no field deltas (2 fields)
[session/new (response)] — no field deltas (9 fields)
[session/prompt (request/notif)] — no field deltas (3 fields)
[session/prompt (response)] — no field deltas (1 fields)
[session/update (request/notif)] — no field deltas (4 fields)
```

Notable non-result: **`commands/execute (response)` jumped from 44 to 57 fields between 2.2.2/2.3.0 and 2.3.0/2.4.1.** That's not a binary change between 2.3.0 and 2.4.1 — both captures today report 57. Same-day backend behavior; the 44→57 happened sometime between the 2.3.0 audit (May 11) and today. Treat as a backend rollout, not a binary change.

## Slash-command additions

**Two new commands** in the `commands/available` notification — 21 → 23 in 2.4.1:

```
+ /effort   "Set thinking effort for this session"
            inputType: "selection", searchable: false
+ /rewind   "Rewind conversation to a previous turn (forks into a new session)"
            inputType: "panel"
```

### `/effort`

- **Slot in `commands/available`**: present in 2.4.1, absent in 2.3.0.
- **`commands/options { command: "effort" }` response is model-conditional.** Initial capture under `claude-haiku-4.5` returned `{ options: [], hasMore: false }`. Re-running with `claude-opus-4.7` active produced the actual options list:

  ```json
  {
    "hasMore": false,
    "options": [
      {"value":"low",    "label":"Low"},
      {"value":"medium", "label":"Medium"},
      {"value":"high",   "label":"High"},
      {"value":"xhigh",  "label":"xHigh  [active]"},
      {"value":"max",    "label":"Max"}
    ]
  }
  ```

  Five effort levels: `low | medium | high | xhigh | max`. xHigh is the default for Opus 4.7 (signaled by the `[active]` suffix on `label`, not a structured `current: true` field). The option schema is bare (`value` + `label` only — no `description` or `group` like other selection commands). Captures: `experiments/conductor-spike/test_bridge-2.4.1.out` (haiku, empty), `test_bridge-2.4.1-opus.out` (opus, populated).
- **Same request to 2.3.0**: backend hangs without responding (no `result`, no `error`). Subsequent client requests are blocked behind the in-flight id, breaking the session for the remainder of the run.
- **New wire-field strings in `kiro-cli-chat` 2.4.1**: `"effort"`, `"effort_update"`. Both absent in 2.3.0 (0 matches).
- **New Rust type**: `agent::agent::tui_commands::command::EffortArgs`, confirmed via mangled-symbol presence in 2.4.1 (`_ZN…tui_commands..command..EffortArgs..deserialize..`). Absent in 2.3.0 (0 matches for `EffortArgs` or `tui_commands..command..Effort`). Thinking-effort is net-new in the 2.4.x series — there is no 2.3.0 scaffolding precedent.
- **`_kiro.dev/metadata` carries `effort` field under thinking-capable models.** New in 2.4.1 and **also model-conditional**: under Opus 4.7 the metadata notification gained `"effort": "xhigh"`, emitted on model-switch and on every subsequent metadata notification (turn completion, post-`/effort`-query). Absent under haiku in the same binary (0 matches for `"effort":` in the haiku conductor log, 6+ in opus). Cyril's `_kiro.dev/metadata` parser will need to deserialize `effort: Option<String>` and surface it in the UI (likely the toolbar, alongside the model name).

### `/rewind`

- **`inputType: "panel"`** is a richer input mode than any prior command (selection/text-only). Suggests a multi-step UI flow (pick a turn, confirm fork).
- **New Rust module**: `crates/chat-cli-v2/src/agent/acp/commands/rewind.rs` (3 matches for `agent..acp..commands..rewind` mangled-symbol path in 2.4.1, 0 in 2.3.0).
- **New Rust types**: `RewindArgs` (4 matches in 2.4.1, 0 in 2.3.0). `/rewind` is net-new in 2.4.x — no Rust scaffolding in 2.3.0.
- Not exercised by this run's `commands/execute` — fork semantics aren't a same-session operation.

## `/stats` token counts — backend gate still closed

Sent `/stats` after the "Say hello in one word" turn on both binaries, same backend. Both responses identical in schema and in null-ness:

```json
{
  "stats": [{
    "duration_ms": 1273.84,        // populated (client-side measurement)
    "ttfc_ms":     1069.50,        // populated (client-side measurement)
    "input_tokens":  null,         // null on BOTH 2.3.0 and 2.4.1
    "output_tokens": null,         // null on BOTH 2.3.0 and 2.4.1
    "had_tool_use": false,
    "error": null,
    "request_id": "...",
    "status_code": null,
    "timestamp": "..."
  }],
  "summary": { "avg_ms": ..., "p90_ms": ..., "max_ms": ..., "errors": 0 }
}
```

**Conclusion**: token-count population is a backend-rollout question, not a binary question. Same-day same-backend isolation: if the binary were the variable, 2.4.1 would diverge; it doesn't. The `input_tokens`/`output_tokens` schema is present in both binaries (2.3.0's was the first to advertise it, per the `reference_kiro_2_3_0_diff.md` memory). Whether the backend will eventually populate these is observable via repeated capture against the same binary over time.

**Also confirmed not model-conditional**: re-ran `/stats` with `claude-opus-4.7` active. Tokens are still null. So the gate is purely backend, not "only the thinking-capable models report tokens." A turn under Opus 4.7 with xHigh effort costs ~$0.148 in metered credits (vs ~$0.019 for the same prompt under Haiku at default effort — 7.8× more, of which some is the model multiplier and some is the effort-level thinking budget), but the token totals that would let us split those costs into thinking-token vs response-token shares aren't on the wire yet.

`_kiro.dev/metadata` continues to emit `meteringUsage` (credits) and `turnDurationMs` but no per-turn token totals.

## KAS backend — assets still NOT embedded

Static binary analysis says KAS assets did not land in 2.4.1:

- **`kiro-cli-chat` size delta**: +2.36 MB (395.18 → 397.54). The KAS bundle as it ships in Kiro IDE is ~36 MB (see `reference_kiro_ide_agent_extension.md`). +2 MB is far too small to contain it.
- **`"KAS assets not embedded and KIRO_KAS_SERVER_PATH not set"`** string still present in 2.4.1 (same as 2.3.0).
- Extraction-side code matured: new tracing event fields `node_extracted`, `kas_extracted`, `asset_sha_path`, `asset_path`, `time_elapsed_ms` in `crates/chat-cli/src/embedded_tui.rs`. The plumbing is wired and waiting for assets.
- KAS namespace (`_kiro/*` per IDE evidence) never appears on the wire — the 2.4.1 binary still emits only `_kiro.dev/*`.

State: same as 2.3.0 — the engine flag (`--agent-engine kas`), env vars (`KIRO_AGENT_ENGINE`, `KIRO_KAS_SERVER_PATH`), and extraction code exist; assets are externally supplied via `KIRO_KAS_SERVER_PATH` only.

## Binary delta

| Binary          | 2.3.0 size  | 2.4.1 size  | Δ        |
|-----------------|-------------|-------------|----------|
| `kiro-cli`      | 117.86 MB   | 118.24 MB   | +0.37 MB |
| `kiro-cli-chat` | 395.18 MB   | 397.54 MB   | +2.36 MB |
| `kiro-cli-term` | 85.89 MB    | 86.06 MB    | +0.16 MB |

Two new files in the tarball that weren't in 2.3.0:

- `bin/q` (65 bytes) — `sh -c 'kiro-cli --show-legacy-warning "$@"'`
- `bin/qchat` (70 bytes) — `sh -c 'kiro-cli --show-legacy-warning chat "$@"'`

Backward-compat shims for Amazon Q legacy users. The new `--show-legacy-warning` flag confirms a UX path that prints a deprecation notice when users invoke the q-named entry points.

## Rust-module deltas

`crates/(chat-cli|chat-cli-v2|agent)/src/*.rs` source paths embedded in `kiro-cli-chat`:

**New in 2.4.1:**

- `crates/chat-cli/src/launch.rs` — entry-point reorg
- `crates/chat-cli-v2/src/agent/acp/commands/rewind.rs` — `/rewind` handler
- `crates/chat-cli-v2/src/cli/chat/legacy/additional_fields.rs` — chat-cli-v2 legacy compat

**Removed from 2.4.1:**

- `crates/chat-cli/src/api_client/internal_redirect_interceptor.rs`
- `crates/chat-cli-v2/src/api_client/internal_redirect_interceptor.rs`

The redirect-interceptor removal in both crates suggests AWS simplified backend routing — likely no longer transparently following internal redirects. Not visible on the ACP wire.

## Cyril impact

- **`/effort` and `/rewind` slash commands**: cyril surfaces `commands/available` to the UI today. The picker should already render `/effort` once it appears in `commands/available`. `/rewind`'s `inputType: "panel"` is a new input mode — cyril likely defaults to text input or `inputType: "selection"` handling and may need to special-case panel-mode commands.
- **`/effort` options-list semantics**: today returns `options: []`. Cyril's picker should handle "command listed but no values yet" gracefully (avoid empty-picker confusion). Worth testing.
- **Avoid sending `/effort` against pre-2.4.0 binaries**: 2.3.0's silent-hang behavior breaks the session for the rest of the run. Cyril should gate `/effort` usage on `commands/available` presence (which it presumably does — but worth verifying).
- **Token-count rendering**: cyril's `/stats` handling should already treat `input_tokens`/`output_tokens` as nullable. When the backend starts populating them, the UI will start showing real numbers without binary upgrade.
- **No client info / capability changes** — no migration needed.

## Status quo for `effort` and `tokens`

This audit answered both targeted questions:

1. **Is thinking effort exposed in ACP?** Yes, as of 2.4.1 — `/effort` is a real `commands/available` entry with `"effort"`/`"effort_update"` wire fields in the binary. Options-list is empty today (backend-gated, model-dependent).
2. **Does `/stats` carry token counts in newer releases?** Schema yes (since 2.3.0). Values still null on the wire as of today's backend. Same-day same-backend isolation rules out 2.4.1 as the variable.

Next observable change is most likely a backend rollout (token values flipping non-null) rather than a binary upgrade.

## Methodology note

Word-boundary string grep against `kiro-cli-chat` produces a lot of false positives because the binary embeds the entire `tui.js` JavaScript bundle plus V8, SQLite, Bun, and other vendored runtimes. Naive `\bEffort\b` matches things like V8's `Profiler.getBestEffortCoverage`, the regex library's `bestEffortTarget: "ES2025"`, SQLite's `Rewind` opcode, and Squirrel scripting `ctrlAutoScrollRewind` — none of which have anything to do with Kiro's slash-command system. To confirm a Rust-side feature is actually present, filter to mangled-symbol paths (e.g., `tui_commands..command..EffortArgs`, `agent..acp..commands..rewind`) or to fully qualified module paths under `chat_cli`/`chat_cli_v2`/`agent`. The findings above were re-verified against mangled symbols after the initial word-boundary grep produced false matches in 2.3.0 that suggested an effort/rewind scaffolding pattern that does not exist.

---

## Non-trivial prompt: deep wire-surface exposure

The original 2.4.1 capture exercised the standard 14-method scenario with a one-word prompt; the resulting surfaces are well-characterized by the v1.29.0 ACP doc. To probe wire behavior that only manifests with real agent activity — tool calls, permission requests, structured tool outputs, multi-request turns — a follow-up capture sent

> "Code review crates/cyril-core/src/protocol/bridge.rs and suggest meaningful improvements"

first under Opus 4.7 at the default xHigh effort and then again at max. Both runs are against the same binary, same backend, same day. Artifacts: `experiments/conductor-spike/test_bridge-2.4.1-codereview{,-max}.out`, `logs/conductor-2.4.1-codereview{,-max}.log`.

### Wire schema drift vs `docs/kiro-acp-protocol.md` (v1.29.0 baseline)

The first three notification types in the protocol reference have drifted under the surface; tool_call_chunk was added with the correct shape.

| Notification | Doc says | Actual wire (2.4.1) | Status |
|---|---|---|---|
| `session/update` → `tool_call` | `toolCallId`, `name`, `status`, `rawInput`, `title` | `toolCallId`, `kind`, `title`, `locations`, `rawInput` | **drift**: `name` → `kind`, `status` only on update, `locations` undocumented |
| `session/update` → `tool_call_update` | `toolCallId`, `status`, `output: "<string>"` | `toolCallId`, `kind`, `status`, `title`, `locations`, `rawInput`, `rawOutput: { items: [{Text\|Json: ...}] }` | **drift**: scalar `output` → tagged-union `rawOutput.items[]`; many adjacent fields undocumented |
| `_kiro.dev/session/update` → `tool_call_chunk` | `toolCallId`, `title`, `kind` | same | **matches** |

The drift is most likely accumulated across the 14 binary releases since 2026-04-11 (v1.29.0 → 2.4.1), not 2.4.1-specific. `docs/kiro-acp-protocol.md` needs a refresh based on these captures.

### `rawOutput` is a tagged union, not a string

The biggest schema departure from the doc. Each `tool_call_update` carries `rawOutput.items[]` where each item is a tagged variant. Two variants observed:

- **`Text`** — file-read content
  ```json
  { "Text": "use std::path::PathBuf;\nuse std::sync::Arc;\n..." }
  ```
- **`Json`** — shell exec, web search, anything else with structured output. Examples:
  ```json
  { "Json": { "stdout": "...", "stderr": "...", "exit_status": 0 } }
  { "Json": { "results": [
      { "url": "...", "snippet": "...", "title": "...",
        "domain": "...", "publishedDate": "...",
        "publicDomain": true, "maxVerbatimWordLimit": ...,
        "totalResults": ... }
  ] } }
  ```

Other variants likely exist (image/binary results) but weren't exercised by code review. The discriminator is the top-level key inside each item — `Text` and `Json` are the tag names.

### `rawInput` shape variants

Beyond the `{path}` shape we saw on file reads, max-effort exercised:

- `rawInput.command` — shell command string (`find ... | head`)
- `rawInput.query` — search query string
- `rawInput.max_matches_per_file` — grep limit
- `rawInput.__tool_use_purpose` — double-underscore prefix; almost certainly an internal/debug field the agent attaches to its own tool invocations. Worth grepping the binary's mangled symbols to confirm what it carries.

### `session/request_permission` first observed live

The v1.29.0 doc describes this method, but no prior capture had ever triggered it (the trivial prompts all exited without using permission-requiring tools). The max-effort code-review fired **7 permission requests**. None fired at xHigh.

Triggering tools:

| Kind | Count | Reason |
|---|---|---|
| Searching | 4 | grep over project files |
| Reading | 2 | files OUTSIDE the workspace (`~/.cargo/registry/src/agent-client-protocol-0.10.*`) — workspace-boundary crossing |
| Running | 1 | shell `find ... \| head` |

Basic request schema matches the doc:

```json
{
  "method": "session/request_permission",
  "params": {
    "sessionId": "...",
    "toolCall": { "toolCallId": "tooluse_...", "title": "Searching the web" },
    "options": [
      { "optionId": "allow_once",   "name": "Yes",    "kind": "allow_once"   },
      { "optionId": "allow_always", "name": "Always", "kind": "allow_always" },
      { "optionId": "reject_once",  "name": "No",     "kind": "reject_once"  }
    ]
  }
}
```

Response:

```json
{ "result": { "outcome": { "outcome": "selected", "optionId": "allow_once" } } }
```

### `_meta.trustOptions[]` — entirely new wire surface

**5 of 7** permission requests also carried a previously-undocumented `_meta.trustOptions[]` sub-structure. Example payload (from a shell-command request):

```json
"_meta": {
  "trustOptions": [
    {
      "label":       "Full command",
      "display":     "find ~/.cargo/registry/src -name \"agent-client-protocol-0.10*\" -type d , head -3",
      "setting_key": "allowedCommands",
      "patterns": [
        "find \\~/\\.cargo/registry/src \\-name \"agent\\-client\\-protocol\\-0\\.10\\*\" \\-type d",
        "head \\-3"
      ]
    },
    {
      "label":       "Partial command",
      "display":     "find ~/.cargo/registry/src * , head *",
      "setting_key": "allowedCommands",
      "patterns": ["find \\~/\\.cargo/registry/src( .*)?", "head( .*)?"]
    },
    {
      "label":       "Base command",
      "display":     "find * , head *",
      "setting_key": "allowedCommands",
      "patterns": ["find( .*)?", "head( .*)?"]
    }
  ]
}
```

Three nested permission tiers Kiro proposes when the user wants "Always":

- **Full** — exact command text
- **Partial** — fixed prefix + wildcard suffix
- **Base** — binary name + wildcards

Each carries a `setting_key` indicating where the client should persist the chosen pattern (here `allowedCommands`), a pre-escaped regex `patterns[]` array ready for storage, and `label`/`display` for UX. The web-search permission request omits `_meta` — atomic permission, no decomposition possible.

The 2 requests **without** `_meta`: web-search (atomic) and one of the internal greps. Shell exec and out-of-workspace file reads always carry it.

### Still unexercised after max-effort + non-trivial prompt

- **`agent_thought_chunk`** — 0 emitted across BOTH runs (xHigh and max). Most likely conclusion: Kiro's ACP doesn't expose model-side thinking content on the wire. Reasoning happens backend-side and only the final response streams via `agent_message_chunk`. The `effort` knob affects backend behavior (more thinking, larger token budget) but doesn't widen the wire surface. The v1.29.0 doc admitted this was unobserved; same conclusion 14 releases later.
- **`plan`** — agent didn't generate a plan in either run. Probably tied to prompts where the agent self-decomposes ("implement X with steps 1, 2, 3") rather than analyzes existing code. Untested.

### Cost comparison: xHigh vs max

Same prompt, same binary, same backend, same model (Opus 4.7), only effort varies:

| Metric           | xHigh (default) | Max     | Ratio  |
|------------------|----------------:|--------:|-------:|
| Credits          | 4.04            | 13.88   | 3.43×  |
| Wall-clock       | 117 s           | 276 s   | 2.36×  |
| Backend requests | 8               | 29      | 3.62×  |
| `tool_call` events | 22            | 54      | 2.45×  |
| Credits / second | $0.035/s        | $0.050/s| 1.45×  |

The 3.43× total cost decomposes roughly as `1.45× (denser thinking per second) × 2.36× (more wall-clock)`. Max doesn't just "think more per response" — it triggers more independent reasoning rounds. At max, the agent also ventured outside the workspace to inspect `~/.cargo/registry/src/agent-client-protocol-0.10.*` and used the web-search tool. Neither happened at xHigh — likely a side effect of both the higher token budget allowing more thorough exploration and the larger thinking budget exposing the model's "I'm uncertain, let me verify" loops.

### `/stats` token counts at every effort level

Three-way confirmation that the token-population gate is purely backend, independent of model AND effort level:

| Setting                          | input_tokens | output_tokens |
|----------------------------------|--------------|---------------|
| Haiku 4.5 (default effort)       | null         | null          |
| Opus 4.7 + xHigh                 | null         | null          |
| Opus 4.7 + max                   | null         | null          |

`/stats` shape is per-backend-request, not per-user-turn: 1 entry for trivial, 8 for xHigh code-review, 29 for max code-review. The wider implication: when AWS does flip the token-population gate, every `/stats` consumer will start seeing values regardless of model choice or effort setting.

### Cyril impact additions

Beyond the items in the earlier "Cyril impact" section:

- **`_meta.trustOptions[]` is the most product-shaped finding.** Cyril's current approval overlay surfaces only the bare `options[]` (AllowOnce / AllowAlways / RejectOnce). The `trustOptions[]` payload is the right surface for a "save this command as always-allowed" UX with three persistence tiers, regex patterns pre-built. The `setting_key` hints at a per-key persistence file that Kiro expects clients to maintain — likely an `allowedCommands` entry in cyril's config. Implementing this would match the IDE's approve-and-remember UX off the wire alone, no separate hook layer needed.
- **`rawOutput.items[]` is a tagged union — cyril's deserializer needs both `Text` and `Json` arms.** A `Text` variant carrying file content versus a `Json` variant carrying structured tool data is a real fork in handling: file-content goes into the syntax-highlighted code panel; JSON output (web results, shell stdout, etc.) wants different rendering. If cyril's `convert/kiro.rs` treats `rawOutput` as a string today, it's wrong for everything except file reads.
- **`rawInput` shape is tool-kind-dependent.** Cyril's tool-call display logic (`primary_path()`, `command_text()` in `TrackedToolCall`) should switch on `kind` and surface the right field: `command` for shell, `query` for search, `path` (or `operations[].path`) for read. Worth a pass over the existing display code to make sure all branches are covered.
- **Max-effort cost warning.** If cyril ever surfaces an effort picker, max needs UX friction — a typical code-review prompt cost 3.43× more at max than at xHigh, $13.88 vs $4.04 on this specific run. Not a budget-breaker but a real bill surprise without a hint.
- **The `_kiro.dev/metadata` parser must tolerate every shape combination.** Observed shapes in the deep capture: `{sessionId}` alone (3× as bare keep-alives), `{sessionId, effort}`, `{sessionId, contextUsagePercentage}`, `{sessionId, contextUsagePercentage, effort}`, and the full turn-completion shape `{sessionId, contextUsagePercentage, meteringUsage[], turnDurationMs, effort}`. All fields after `sessionId` are optional.
