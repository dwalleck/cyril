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
  cyril-review/   # Native review evidence: run artifacts, gather, facts, diagnostics
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

The internal `cyril crtool gather <rundir> <target> [scope]` and
`cyril crtool facts <rundir>` commands run without Python, an agent, or TUI startup.
Run them from the repository root; `auto` selects the gather target, and scope
is a whitespace-separated list of paths (not shell-quoted path syntax).
Gather writes the diff, per-file patches, manifest, and symbol/usage facts.
An existing matching run is reused; a different target/scope or missing/stale
version stamp is refused. Required manifest fields are validated before reuse or
facts rebuilding. Git search errors are reported rather than treated as no usages.
Manifest JSON must be valid UTF-8. Before changing evidence, facts and diagnostics
reject non-object `facts` metadata; a missing or null value is initialized as an
object, and unknown object fields are preserved.
Raw Git names retain their identity for status, patches, persisted symbol/usage
facts, and document lookup, even when display labels need replacement characters.
Colons and line feeds inside filenames are not treated as Git record delimiters.
If the host cannot represent a Git filename, gather refuses it rather than
selecting another file.
An empty diff exits 3; other operation errors exit 2.
The Python tool is a functional reference for verification, not a byte-format
contract: JSON formatting, diagnostic wording, and text newlines may differ.

Library callers can run a configured check with `cyril_review::diagnostics`,
`DiagnosticsOptions`, `Cancellation`, and `SystemReviewClock`. This is a blocking
operation; async callers must use a worker. There is no diagnostics CLI verb.
The default timeout is 1,800 seconds. Results distinguish clean, failed (with the
native exit code), timed-out, and cancelled checks. Pre-launch errors and
cancellation before launch leave diagnostics artifacts unchanged.

Started checks save lossless stdout/stderr in `facts/diagnostics-raw.txt`, plus a
report containing the first 200 changed-path matches and last 15 nonempty lines.
Cancellation and timeout terminate only the directly spawned child, with a
one-second termination/reap deadline; descendants are not targeted. Fixed-length
file snapshots avoid waiting for inherited output handles to reach EOF. That
deadline is not a filesystem I/O latency guarantee.
POSIX commands use shell-style argument parsing without an implicit shell.
Windows uses native executable/argument parsing; explicitly selected `.cmd` and
`.bat` files follow native batch dispatch, without wrapping arbitrary command
text in a shell.

## License

[MIT](LICENSE)
