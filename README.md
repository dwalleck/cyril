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
- **Opt-in local memory runtime** — teach explicit per-project lessons that are injected into a fresh session's first prompt, backed by private versioned stores and authenticated health reporting, without making chat depend on memory availability
- **@-file references** — reference files in prompts with `@path/to/file` autocomplete

## Prerequisites

- [Kiro CLI](https://kiro.dev/docs/cli/) installed and authenticated (`kiro-cli login`) — Linux, macOS, and Windows native binaries are all supported
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
### Local memory runtime

Memory is disabled by default. Enable the local runtime in Cyril's
`~/.config/cyril/config.toml`:

```toml
[memory]
enabled = true
```

Use `/memory status` to inspect runtime health, store schema versions, and how
the current workspace is bound to a project (a Git common dir, or the
canonical path outside Git).

Lessons are explicit: nothing is captured from conversations. `/memory teach
<text>` stores a project lesson (secrets such as `password=…`, `ghp_…`, `AKIA…`,
`sk-…`, and PEM private keys are redacted before storage); `/memory list` and
`/memory inspect <id>` read them back; `/memory teach --replace <id> <text>`
supersedes one. Active lessons for the bound project are prepended, in a
`<CYRIL_LESSONS>` block, to the **first prompt of each fresh session** and never
shown in the transcript. If the memory runtime is unavailable the prompt goes
out without them; chat never waits on memory.


### Keyboard shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Shift+Enter` | Newline in input, when the terminal reports the Shift modifier |
| `Tab` | Accept autocomplete suggestion |
| `Esc` | Cancel current request |
| `Ctrl+M` | Toggle mouse capture (off = copy mode) |
| `Ctrl+C` / `Ctrl+Q` | Quit |

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

### Reviewing a branch (KAS)

`/review`, run from the repository root, reviews the branch's changes with
Kiro's workflow engine. The form shows the target (`auto`: HEAD against its
upstream, `main` or `master`, plus uncommitted changes), the scope, the
number of files, the expected cost (about 50–65 model sessions and 40–50
minutes) and what the run may do. `Esc` backs out without writing anything;
`Enter` starts it. An empty diff stops there with "nothing to review".

Starting installs the `cyril-review` recipe and its four agents into
`~/.kiro/workflows` and `~/.kiro/agents`, rewriting only files that differ.
It refuses if the workspace defines an agent with one of those names. Results
go to a new `.code-review/<YYYYMMDD-HHMMSS>-<hex>/` directory, whose parent
ignores itself in git. While the review runs, cyril answers its step
sessions' permission requests itself:

- reads only inside the workspace;
- writes only inside the run directory;
- shell only for the run's own `cyril crtool` step calls.

Everything else is denied, without a prompt, and listed in `denied.log`. The
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
| `/memory status` | Show local memory runtime health, store versions, and project binding |
| `/memory teach <text>` | Store an explicit lesson for the bound project |
| `/memory teach --replace <id> <text>` | Supersede one lesson with new text |
| `/memory list` | List active project lessons (newest first) |
| `/memory inspect <id>` | Show one lesson, active or replaced |
| `/review` | Review this branch's changes with a multi-agent workflow (KAS) |
| `/quit` | Quit |

**Agent commands** (forwarded to Kiro via ACP):

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
