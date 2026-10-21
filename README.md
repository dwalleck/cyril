# Cyril

*The polished TUI for the [Agent Client Protocol](https://agentclientprotocol.com) ecosystem.*

## What is this?

Cyril is a polished terminal interface for the Agent Client Protocol ecosystem. Run any of 37+ registered agents — Claude, Cursor, Codex, Cline, Goose, Kiro, and more — through a single interface. Beneath the TUI, composable proxy stages add behaviors no agent ships natively: skill systems, transcript audit, organizational permission policies, persistent memory across sessions, multi-client observers. Vendor neutrality is a feature, not a roadmap; stages are how cyril compounds value over time.

> **Status:** Alpha. Today cyril works against [Kiro CLI](https://kiro.dev); vendor-neutral agent selection and the proxy-stage layer are in active development. The features and usage documentation below describe the current Kiro-focused implementation.

## Features

- **Streaming TUI** — ratatui-based interface with real-time markdown rendering (headings, bold, italic, code blocks with syntax highlighting, tables, lists, blockquotes)
- **Cross-platform** — runs natively on Linux, macOS, and Windows
- **Slash commands** — autocomplete-enabled commands from both the client and the Kiro agent
- **Tool call display** — see what the agent is doing in real time with inline diffs
- **Approval prompts** — review and approve command execution with Yes/Always/No options
- **Session management** — create, load, and resume previous sessions via `/chat`
- **Agent/model switching** — switch agents (`/agent`) and models (`/model`) via picker UI
- **Live activity indicator** — animated spinner with elapsed time and current tool activity in the toolbar
- **Context bar** — visual gauge showing context window usage
- **Opt-in local memory runtime** — teach explicit per-project lessons for a fresh session's first ordinary chat prompt, backed by private versioned stores and authenticated health reporting, without making chat depend on memory availability
- **@-file references** — reference files in ordinary chat prompts with `@path/to/file` autocomplete

## Prerequisites

- [Kiro CLI](https://kiro.dev/docs/cli/) installed and authenticated (`kiro-cli login`), and resolvable on PATH (cyril also checks `~/.local/bin`, `/usr/local/bin` and `/opt/homebrew/bin`; KAS sessions get their token from it) — Kiro ships native Linux, macOS and Windows binaries; see the platform qualification note below
- [Node.js](https://nodejs.org/) on `PATH` for the default KAS launch
- [Rust toolchain](https://rustup.rs/) (for building from source)

## Installation

```sh
cargo install cyril
```

Or build from source:

```sh
git clone https://github.com/dwalleck/cyril.git
cd cyril
cargo build -p cyril --release --bins
```

The build produces `target/release/cyril` and
`target/release/cyril-memory-runtime` (with `.exe` suffixes on Windows).
Keep both executables in the same directory; Cyril launches the memory runtime
by canonical absolute sibling path when `[memory] enabled = true`.

## Usage

Launch the interactive TUI:

```sh
cyril
```

Send a one-shot prompt:

```sh
cyril --prompt "Explain what this project does"
```

Specify a working directory:

```sh
cyril -d /path/to/project        # Linux
cyril -d C:\Users\you\project    # Windows
```

### Engines and first run

Normal production builds of the `cyril` package enable `kas` and select KAS when
engine configuration is absent. The library `cyril-core` keeps its `kas` feature
opt-in. Explicit v2 selection remains supported:

```sh
cyril --agent-engine v2
```

Or set the engine in `~/.config/cyril/config.toml`:

```toml
[agent]
engine = "v2"
```

CLI engine selection still wins over configuration. `kas` and `v3` both select
KAS. To build a V2-only binary without the KAS credential-reading code, use an
isolated selected-package build:

```sh
cargo build -p cyril --no-default-features
```

That build defaults to v2. Runtime `--agent-engine v2` does **not** remove the
credential-reading capability from a normal binary. A workspace build is not
proof of exclusion: `cyril-workbench` enables core KAS through feature
unification. See [ADR-0002](docs/adr/0002-kas-cargo-feature-gate.md).

On v2, a tool rejected during argument validation is shown as a failed call with
its validation error, even when the agent sends only an input chunk and a failed
update without a start. Error text supplied in the update's content is displayed
when no usable raw-output error is available. Empty or whitespace-only raw
strings and error fields do not suppress that content fallback. Nonblank raw
errors retain precedence; the original raw output remains unchanged. A chunk
alone remains activity, not a fabricated tool result.

The default KAS `free` spawn runs discovered Node.js plus the extracted
`acp-server.js` directly, with callback authentication: on each KAS token
callback Cyril runs `kiro-cli chat _ get-kas-token` (cyril-zbu7), so kiro-cli
resolves and renews the token itself; Cyril reads Kiro's credential store only
as a fallback for a kiro-cli that lacks that subcommand (never renewed, so such
sessions expire hourly). “Free” means no Kiro wrapper, not credential-free.
It ignores `--agent-command`; an override alone does not select a WSL agent.

If the bundle is missing, the error reports the searched path and instructs
`kiro-cli acp --agent-engine v3` once to extract it, or `KIRO_KAS_SERVER_PATH`
to select an explicit `acp-server.js`. Missing Node has its own instruction:
install Node.js on `PATH` or set `KIRO_AGENT_PATH` to its executable. Credential
failures are distinct; authenticate with `kiro-cli login`. Server/Node overrides
do not relocate credentials. There is no silent Wrapper or v2 fallback.

### KAS on native platforms

By default, Cyril discovers Kiro's extracted KAS bundles under `<data dir>/kas`.
The stored-token fallback reads `<data dir>/data.sqlite3`. Native defaults are:

| Platform | Default Kiro data directory |
| --- | --- |
| Linux | `$XDG_DATA_HOME/kiro-cli` when absolute; otherwise `~/.local/share/kiro-cli` |
| macOS | `~/Library/Application Support/kiro-cli` |
| Native Windows | `%LOCALAPPDATA%\Kiro-Cli`, falling back to Windows' local-app-data known folder when `LOCALAPPDATA` is unset or not absolute |

`KIRO_DATA_DIR` overrides **bundle discovery only**. A value that is nonblank
after checking whitespace is used with its original spelling, including leading
or trailing spaces; Cyril does not append a `kiro-cli` or `Kiro-Cli` directory.
An unset, empty, or whitespace-only value uses the native default above. Relative
bundle roots resolve from Cyril's launch directory and stay bound there even
when the agent runs in a different `--cwd` workspace. The credential store always
keeps its native default location; neither `KIRO_DATA_DIR` nor `KIRO_HOME`
relocates it.

`XDG_DATA_HOME` does not redirect macOS or native Windows KAS discovery.
macOS also ignores `LOCALAPPDATA`; native Windows ignores `HOME` and does not
require WSL. Explicit v2 inside WSL requires both engine and command selection,
for example `cyril --agent-engine v2 --agent-command wsl kiro-cli acp`, plus a
Kiro login inside WSL. Path translation follows the resolved agent location,
not merely Cyril's host OS.

The macOS data-root correction from
[PR #167](https://github.com/dwalleck/cyril/pull/167) is integrated.
Live native macOS/Windows KAS qualification is not claimed here. See the
[ROADMAP parity table](docs/ROADMAP.md#current-default-policy-and-parity-cyril-4c4d).

### Kiro profile roots

Cyril uses one validated Kiro profile root for KAS settings, sidecar reads,
global agent configurations and trust writes, and global host hooks. It defaults
to `<home>/.kiro`; `KIRO_HOME` selects another profile without relocating KAS
bundles or credentials.

`KIRO_HOME` has leading/trailing whitespace trimmed; an unset, empty, or
whitespace-only value uses the default. A nonempty value must be absolute on Unix
or rooted on Windows (drive-relative paths such as `C:profile` are rejected).
Filesystem roots and paths equal to or containing the user's home are also
rejected, including symlink aliases. Rejected overrides produce a warning and
fall back to the default profile. Workspace agents still shadow globals, and
workspace hooks still merge with global hooks.

### Local memory runtime

Cyril's local memory runtime is disabled by default. It is separate from native
KAS memory: upstream KAS defaults are preserved, with possible overlap and costs
explicitly accepted. Cyril's `/memory` does **not** control native KAS memory.
Enable the local runtime in Cyril's `~/.config/cyril/config.toml`:

```toml
[memory]
enabled = true
```

Use `/memory status` to inspect runtime health, store schema versions, and how
the current workspace is bound to a project (a Git common dir, or the
canonical path outside Git).

Cyril lessons are explicit: the local runtime captures nothing from
conversations. `/memory teach <text>` stores a project lesson (secrets such as `password=…`, `ghp_…`, `AKIA…`,
`sk-…`, and PEM private keys are redacted before storage); `/memory list` and
`/memory inspect <id>` read them back; `/memory teach --replace <id> <text>`
supersedes one. Active lessons for the bound project are prepended, in a
`<CYRIL_LESSONS>` block, to the **first ordinary chat prompt of each fresh
session** and never shown in the transcript. Registered KAS agent-command
prompts do not receive this block; if a command comes first, the lessons remain
pending for the next ordinary chat prompt. If the memory runtime is unavailable
the prompt goes out without them; chat never waits on memory.


### Keyboard shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Shift+Enter` | Newline in input, when the terminal reports the Shift modifier |
| `Tab` | Accept autocomplete suggestion |
| `Esc` | Cancel current request |
| `Ctrl+M` | Toggle mouse capture (off = copy mode) |
| `Ctrl+C` / `Ctrl+Q` | Quit |

Key presses and held-key repeats are handled; key releases are ignored to prevent
duplicate input on Windows. Windows `Alt+numpad` character entry is unsupported
because Crossterm reports those characters only on release. Paste the character
instead; Unicode paste remains supported.

### Themes and Markdown rendering

Use `/theme` to preview a bundled palette. `Esc` cancels the preview and
restores the previous colors; `Enter` applies the selection for the current
session. Committed and streaming Markdown, including syntax-highlighted code,
use the selected palette without changing the preview/cancel/commit behavior.

Highlighting and Markdown caches identify their output by the concrete style
inputs their renderers use, rather than every role in the full theme.
Highlighting depends on the resolved syntax palette and fallback text color;
Markdown also includes its own formatting colors and render width. Unrelated
theme-role changes can reuse cached output, while changes to a used style
remain isolated. Adding a rendering dependency to a style input automatically
includes it in that input's derived identity.

Display budgets use the released `ratatui-core 0.1.2` /
`ratatui-widgets 0.3.2` physical-cell widths for Markdown tables, code frames
and affected popup/status text. Halfwidth sound marks occupy their painted
columns; combining marks and original input bytes are preserved. The input
editor retains character wrapping and its character-cursor contract.

The [current diff-reuse integration binding](.cyril-zj0m/current-integration.json)
separates this source state and its outstanding qualification from the frozen
earlier benchmark and consumer evidence.

### Tool call history

Tool rows stay at their first observed position in the conversation. An update
without a preceding start creates a row from the fields it supplies, including
KAS **Tool Load** and **Subagent Response** updates. Later frames for the same
call within a session merge into that row rather than adding duplicates,
preserving the text before and after it. A new session can reuse a tool ID
without changing the previous session's history.

Completed Write calls use the **first diff** for both the `+added -removed`
summary and the inline body. An unchanged first diff shows no change counts,
even if a later diff in the same call changed. The body keeps one context line
and at most 20 rows, followed by an ellipsis when truncated.

The tracked call retains one theme-independent derivation for unchanged diff
text. Redraws, resizes and palette changes reuse it while applying the current
colors; replacing the selected source refreshes the summary and body together.
Pending approval comparisons use the same diff renderer without requiring the
tool to be completed.

For successful KAS commands, string-valued `rawOutput.output` is shown beneath
the completed row, including completions that omit `kind`. Existing output
formats keep precedence; non-string `output` values and the formatted `message`
field are not used as substitutes.

### Rejecting a tool with feedback (KAS)

Select the agent's one-time rejection option, then press `r` to **Reject with
reason**. Pressing `Enter` on the option itself still rejects immediately
without feedback.

The feedback editor accepts multiline paste. `Ctrl+J` inserts a newline,
`Enter` submits, and `Esc` discards the draft and returns to the same approval
choice without answering it. Explicitly reported `Shift+Enter` also inserts a
newline, but many terminals send it as plain Enter; use `Ctrl+J` reliably.
Empty or whitespace-only feedback sends a plain rejection.

Reasons are limited to 4,096 Unicode characters. An insertion that would exceed
the limit is refused in full with a visible notice; it is never silently
truncated. The editor does not alter the chat draft or a queued approval.
KAS 0.66.8 and newer receive the reason through permission-response metadata;
older KAS may ignore it. The action is not shown for v2 or always-reject options.

### Host hooks (KAS)

KAS is the default engine in normal production builds. In the default KAS hooks
mode, `[agent] kas_hooks = "host"`, Cyril loads command hooks from the
workspace's `.kiro/hooks/*.json` and `~/.kiro/hooks/*.json`.

For example, a `.kiro/hooks/shell.json` hook can select tools tagged `shell`:

```json
{
  "version": "v1",
  "hooks": [{
    "name": "shell-hook",
    "trigger": "PreToolUse",
    "matcher": "shell",
    "action": {"type": "command", "command": "echo shell hook"}
  }]
}
```

For `PreToolUse` and `PostToolUse`, matching follows KAS's selector-plus-ID-regex
rules:

- Matchers without any of `\ ^ $ ( ) [ ] { } | +` act as tool selectors.
  Literals match exact supplied tool IDs or tags, such as `shell`, `read`,
  `@builtin`, and `@mcp`; source tags are not inferred from ID prefixes.
  `*` matches zero or more characters and `?` matches one, across the whole ID
  or tag. Other characters are literal in these wildcard patterns, and `*`
  alone is valid.
- Exact ID selectors also recognize `execute_bash`/`execute_pwsh` and
  `control_bash_process`/`control_pwsh_process` as aliases in both directions.
- Independently, a valid matcher retains legacy unanchored Rust-regex matching
  against the tool ID only. For example, `fs_.*` still matches `fs_write`.
  Regex-only forms such as `^mcp_.*$` never match tags.

On these two triggers a configured matcher requires a tool ID, even if tags
are supplied. Missing or empty tags leave ID matching available. A malformed
`toolTags` value is logged and its non-string members are dropped; the request
is still answered, because KAS runs none of a tool's hooks when the list call
fails.

`SessionStart`, `UserPromptSubmit` and `Stop` have no tool to test, so, as in
KAS, a valid matcher on them selects every invocation. On any trigger, an
omitted or empty matcher selects every invocation, and an invalid matcher is
warned about at load and never matches. Matchers compile as Rust regexes, so
JavaScript-only syntax that KAS accepts, such as lookaround or backreferences,
is invalid here.

Tag matching does not change hook execution authority or approval behavior.
A pre-tool command hook exiting with code 2 blocks the tool. Other triggers
keep their existing execution behavior. `kas_hooks = "kas"` remains the
separate KAS-owned registry/execution mode; this Host-mode repair does not
change it.

### Reviewing a branch (KAS)

`/review`, run from the repository root, reviews changes with Kiro's
workflow engine. The form picks the Review target with ←/→:

- `auto` — HEAD against its upstream, `main` or `master`, plus uncommitted
  changes;
- `vs <branch>` — what a PR would contain, `<branch>...HEAD` (Tab moves to the
  branch field, ←/→ picks the branch);
- `uncommitted changes` — HEAD against the working tree;
- `the commit at HEAD` — that commit alone.

Every target ends at the checked-out HEAD, because the reviewers read the
working tree. The scope is a row of checkboxes: one per top-level directory
the diff touches, plus each touched root file, all checked unless the
repository's `[review] scope` says otherwise. Tab moves between the target, the
base branch and the paths; on the paths ←/→ moves and Space toggles, with the
file count following. Unchecking every path is refused ("select at least one
path"). The form also shows the expected cost (about 50–65 model sessions and
40–50 minutes) and what the run may do. `Esc` backs out without writing
anything; `Enter` starts it. An empty diff for the chosen target and paths
stops there with "nothing to review".

Arguments prefill the form, which still asks for `Enter`:
`/review [auto | uncommitted | head | base [<branch>]] [-- <path>…]`. The
cursor starts on the first field the arguments left out.

Starting installs the `cyril-review` recipe and its four agents into
`~/.kiro/workflows` and `~/.kiro/agents`, rewriting only files that differ.
It refuses if the workspace defines an agent with one of those names. Results
go to a new `.code-review/<YYYYMMDD-HHMMSS>-<hex>/` directory, whose parent
ignores itself in git. While the review runs, cyril answers its step
sessions' permission requests itself:

- reads only inside the workspace;
- writes only inside the run directory;
- shell only for the run's own `cyril crtool` step calls.

Everything else is denied, without a prompt, and listed in `denied.log`.

`/review resume` continues a failed or paused run, also after restarting
cyril: with no argument the newest run the agent reports as failed or paused,
or a run directory name or workflow id. It shows the stored run read-only and
continues after `Enter` (failed runs are retried, paused runs resumed). It
never gathers or checks again, and refuses a run started by a different cyril
binary or version.

`/review cancel` stops the review at any phase. Before the workflow exists
it abandons the launch, killing a running check; once the workflow exists it
withdraws the run's authorization at once and cancels the workflow, and says
whether the agent confirmed it.

A repository can commit settings for every reviewer in `.cyril/config.toml`:

```toml
[review]
check_cmd = "cargo clippy --workspace --all-targets -- -D warnings"
check_timeout_s = 900          # default 1800
context_file = "docs/review-context.md"   # or: context = "..."
scope = ["crates", "docs"]     # default: everything; no spaces in paths
```

The settings are read fresh on every `/review`. An unknown key, a wrong
type, setting both `context` and `context_file`, or a path outside the
repository refuses the review with an error naming the problem. The form
shows the check as `will run: <command>`, or `no check configured`; it never
guesses a check. The check runs once, after gathering and before the workflow
starts ("review: running check…"). A failing or timed-out check is recorded
for the reviewers and the review goes on. A command that cannot start stops
the review before any workflow exists. `Esc` during the check abandons the launch and kills
the check command's own process (not processes it started in the
background). `context_file` is read when the review starts and its
contents go to the verifiers. The
chat stays usable throughout, and `/workflow status <id>` shows progress.
When the run ends, one message lists the verdict counts, the top ten
findings and the path to `report.md`.

### Slash commands

**Local commands** (handled by Cyril):

| Command | Description |
|---------|-------------|
| `/help` | Show available commands |
| `/new` | Start a new session |
| `/load <id>` | Load a session by ID |
| `/clear` | Clear the chat |
| `/mode <id>` | Switch agent mode |
| `/model [id]` | Switch model (opens picker if no ID given) |
| `/theme` | Preview a palette; `Enter` applies it for this session, `Esc` cancels |
| `/memory status` | Show local memory runtime health, store versions, and project binding |
| `/memory teach <text>` | Store an explicit lesson for the bound project |
| `/memory teach --replace <id> <text>` | Supersede one lesson with new text |
| `/memory list` | List active project lessons (newest first) |
| `/memory inspect <id>` | Show one lesson, active or replaced |
| `/review` | Review changes with a multi-agent workflow (KAS); `/review resume`, `/review cancel` |
| `/quit` | Quit |

**V2 agent commands** (forwarded to Kiro via ACP; examples, not a KAS parity guarantee):

| Command | Description |
|---------|-------------|
| `/agent` | Switch agent (picker) |
| `/chat` | Resume a previous session (picker) |
| `/compact` | Compact conversation history |
| `/context` | Show context/token usage breakdown |
| `/knowledge` | Manage knowledge bases |
| `/mcp` | Show configured MCP servers |
| `/plan` | Switch to planning agent |
| `/prompts` | Select from available prompts (picker) |
| `/tools` | Show available agent tools |
| `/usage` | Show billing and usage info |

V2 retains its existing command execution and selection-option requests.
Each advertisement is the agent's complete command list: a command it no longer
lists is withdrawn from autocomplete and dispatch, and a re-advertised command
takes its new behaviour. Advertised commands never replace Cyril's builtins.

**KAS agent commands:** when KAS advertises a custom agent, skill or saved
prompt (`custom-agent`, `skill`, `prompt`), Cyril registers it for ordinary ACP
prompt submission. For example, if advertised,
`/general-task-execution "inspect foo"` starts a prompt turn rather than a V2
command RPC. The invocation is sent as exactly one original text block,
preserving the slash, quotes, and whitespace as typed, including commands with
no arguments. As in KAS, the command name ends at the first whitespace of any
kind, so a tab or newline after the name still invokes the command.

Recognized KAS command prompts receive **no automatic local lesson or
`@file` expansion text**: those additions would change KAS's command arguments.
First-prompt local lessons remain pending for ordinary chat instead. Ordinary
chat and unregistered slash fallback keep their existing context/file behavior.
If a turn is already busy, the invocation follows normal steering behavior.

Unsupported KAS command types are not registered. The exact workflow commands
`workflow-run`, `workflow-status`, `workflow-cancel`, and `workflow-resume` also
remain excluded; Cyril retains its own workflow control plane.

## Project structure

```
crates/
  cyril/          # TUI application (binary)
  cyril-core/     # Protocol logic, path translation, session state
  cyril-review/   # Native crtool (gather, facts) and the review check command
docs/
  kiro-acp-protocol.md  # Comprehensive Kiro ACP protocol reference
```

Core library consumers can construct a safe executable prefix with
`cyril_core::review::CrtoolPrefix::current(dialect)`. Obtain the dialect from
`BridgeHandle::review_shell()` before splitting a KAS bridge handle; it reflects
the host shell already resolved for that bridge, not an OS-based guess. The
constructor canonicalizes the executable, normalizes Windows drive/UNC spelling,
and refuses shell-active characters rather than attempting fallback quoting.
This API constructs a command prefix; it does not add a `/review` command or
execute a review.

The internal `cyril crtool` subcommands (`gather`, `facts`, `merge`, `shard`,
`ballots`, `collate`, `finalize`, `comments`) are the native crtool that the
review workflow runs. They need Git on `PATH`, run from the
repository root (anywhere else is refused), and are hidden from `--help`.
[`docs/crtool-contract.md`](docs/crtool-contract.md) specifies their outputs:
the same text the Python `crtool.py` gives the models, value-equal JSON, and the
same exit codes (3 for an empty diff, 2 for any other error). Every run is
stamped with the cyril version, and a run stamped by another version is refused.

`cyril_review::ASSETS` embeds the canonical review workflow
(`cyril-review.workflow.json`) and its four `cyril-review-*` agents, and
`cyril_review::read_findings` reads a finished run's `findings.json`, keeping a
missing file and a corrupt one as distinct errors. The experiment's
`code-review-max` recipe and `cr-*` agents are generated from the same sources
by `experiments/code-review-workflow/build_recipe.py`.

`cyril_review::run_check` runs a repository's check command once for a review,
on the caller's Tokio runtime, and writes `facts/diagnostics.txt` and
`facts/diagnostics-raw.txt`. It is not a crtool subcommand. It reports clean,
failed (with the exit code), timed out or cancelled; a timeout or cancellation
kills the command and keeps the output collected so far.

## License

[MIT](LICENSE)
