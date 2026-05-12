# Kiro IDE 0.12.155 — Built-In Agent Extension (KAS, in-bundle)

> **Source:** extracted from `kiro-ide-0.12.155-stable-linux-x64.tar.gz` (267 MB compressed) on 2026-05-12, hosted at `prod.download.desktop.kiro.dev/releases/stable/linux-x64/signed/0.12.155/tar/`. Analysis focuses on `Kiro/resources/app/extensions/kiro.kiro-agent/`. Companion to [`docs/kiro-2.3.0-wire-audit.md`](kiro-2.3.0-wire-audit.md), which covers the CLI side.

## The big picture

Kiro IDE 0.12.155 ships a VS Code extension at `kiro.kiro-agent` (publisher `kiro`, extension version `0.3.323`). It's a Continue.dev fork wrapped in Kiro-specific packaging, monorepo-style with workspaces for `continuedev/{core,extension,gui}`, `@amzn/codewhisperer-runtime`, `kiro-ui-powers`, `kiro-ui-agent-chat`, `hook-editor`, and several others.

Inside this extension's `kiro-shared` package, `package.json` declares three peerDependencies:

```json
"peerDependencies": {
  "@kiro/acp-type-covenant": "0.3.17-hotfix.1",
  "@kiro/agent": "0.3.17-hotfix.1",
  "@kiro/client": "0.3.17-hotfix.1"
}
```

This triad is what the extension's `RELEASE_STAGING_INFO.md` refers to as the **"KAS packages"** — same name the CLI plans to use for `--agent-engine kas`. **Same code, different distribution model.** The IDE bundles KAS inline via webpack into a single 36 MB `extension.js`; the CLI will eventually extract these packages to disk and spawn node as a child process speaking ACP over stdio.

### Inlining verified at byte level

`find` returns no `@kiro/agent/dist/*.js` files anywhere on disk in the extracted IDE — but `grep -aoE '@kiro/agent/dist/[a-z-]+' dist/extension.js` returns 102 distinct paths. Inspecting the raw bytes around one such reference (offset 16,699,591):

```javascript
// node_modules/@kiro/agent/dist/acp-CZtNeFj1.js
function Mt5(e91) {
  let t59 = jt5[e91] ?? [e91], n63 = [], r62 = [];
  for (let e92 of t59) At5.includes(e92) ? n63.push(e92) : r62.push(e92);
  return { fsCaps: n63, nonFsCaps: r62 };
}
```

Classic webpack/esbuild concatenation output: each source file's contents inlined with a `// provenance` comment, then minified. The 102 paths are provenance breadcrumbs scattered through the bundle, not external imports.

## Wire-format differences from `kiro-cli acp` (rust engine)

The IDE's `@kiro/agent` uses a **different ACP extension namespace** from the CLI today, and a fundamentally different capability set.

### Extension namespace

| CLI today (rust engine) | IDE (`@kiro/agent`, future KAS-CLI) |
|---|---|
| `_kiro.dev/commands/execute` | `kiro/createSession`, `kiro/executionLog/*` |
| `_kiro.dev/metadata` | `_meta.kiro.*` on standard ACP messages |
| `_kiro.dev/subagent/list_update` | (no direct equivalent — execution stream replaces it) |

The `_kiro.dev/` prefix is **not used** by `@kiro/agent`. KAS uses `_kiro/` (single segment) and `kiro/` (no underscore) prefixes instead.

### Filesystem callbacks (KAS uses them; today's CLI doesn't)

Today's `kiro-cli acp` sets `fs: {}` and never calls fs callbacks. **That is NOT true for `@kiro/agent`.** Extracted method strings:

```
_kiro/fs/read_file
_kiro/fs/write_file
_kiro/fs/delete
_kiro/fs/stat
_kiro/fs/read_directory
```

When KAS-equipped Kiro CLI ships, ACP clients like cyril will need to implement file-callback responders. The agent stops doing fs I/O in-process and starts delegating to the client.

### Session lifecycle (KAS has the "unstable" methods)

CLI today rejects most of these as unstable. `@kiro/agent` implements the full set:

```
session/cancel    session/close   session/fork    session/list
session/load      session/new     session/prompt  session/resume
session/update    initialize      authenticate
```

`session/fork`, `session/list`, `session/load`, `session/resume`, `session/close` — all real. The CLI's current `sessionCapabilities: {}` returns false on these; KAS will likely set them true.

### Execution log stream (fine-grained replacement for session/update)

```
kiro/executionLog/pushExecution        kiro/executionLog/operationUpdate
kiro/executionLog/executionAssignments kiro/executionLog/executionLoad
kiro/executionLog/executionUpdate      kiro/executionLog/userApprovalResponse
kiro/executionLog/userInputResponse    kiro/executionLog/userMessageCleared
kiro/executionLog/userMessageQueued    kiro/executionLog/userResponse
kiro/executionLog/uiControl
```

Much more granular than the rust engine's coarse `session/update` chunking. The IDE uses these to drive its real-time progress UI.

### Other extension methods

```
_kiro/checkpoint/revert    _kiro/checkpoint/revertMultiple
_kiro/code_references
```

Checkpoints are TUI-only in the CLI today (cyril doesn't see them via ACP). KAS exposes them as first-class ACP methods.

## Spec workflow is real and concrete

Document name occurrences in `extension.js`:

```
requirements.md  (72 refs)
design.md        (77 refs)
tasks.md         (61 refs)
kiro-spec://     (35 refs — registered FS provider scheme)
specMode         (3 refs)
```

The three-document spec workflow (requirements → design → tasks) is baked into `@kiro/agent`. The extension registers a `kiro-spec://` URI scheme as an FS provider via `onFileSystem:kiro-spec` activation event, alongside `kiro-diff://` and `kiro-meta://`.

## Autonomy modes

```
"autonomous"  (9 refs)
"interactive" (5 refs)
"autopilot"   (2 refs)
```

Three modes. The v2 TUI bundle's KAS factory class auto-sets `autopilot=on` on session start via `setSessionConfigOption({configId: "autopilot", value: "on"})` — that's the default for KAS sessions.

## Sandboxing

`bubblewrap-sandbox-Cm_q1A6O.js` is one of the inlined `@kiro/agent` modules. Bubblewrap (Flatpak's `bwrap`) is used to sandbox tool execution on Linux. KAS supports namespace-isolated tool runs out of the box — a feature the rust-engine CLI doesn't have today.

## Tools advertised

Same tool names as the rust engine: `fs_read`, `fs_write`, `grep`, `read_file`, `write_file`, `run_command`, `execute`, `web_fetch`, `web_search`. Input schemas may differ — outstanding question for if/when KAS is exercised through cyril.

## Submodule inventory (102 modules)

Selected `@kiro/agent/dist/<file>` paths visible as provenance comments inside `extension.js`:

```
acp                     actions                 agent-context
agents                  annotation-parser       api
async-delivered-object  async-stream            autonomous-agent
autonomy-mode           bubblewrap-sandbox      bundled-agents
cancellation            checkpoint-revert       chunk
code                    command-approval        common-network-rules
compaction              ...
```

Full list: `grep -aoE '@kiro/agent/dist/[a-z-]+' dist/extension.js | sort -u`.

## Distribution model contrast

| Aspect | IDE 0.12.155 | CLI 2.3.0 (`kiro-cli acp --agent-engine kas`) |
|---|---|---|
| Where `@kiro/agent` lives | webpack-inlined into `kiro.kiro-agent/dist/extension.js` (36 MB) | extracted to disk by `extract_kas_assets_if_needed` (not yet shipped) |
| How it runs | in the VS Code extension host (same process) | spawned as a child node process, ACP over stdio |
| Resolution path | bundled at build time | `KIRO_KAS_SERVER_PATH` env → embedded extraction → `node_modules/@kiro/agent` package walk |
| Status today | shipping in IDE | scaffolding only (assets not embedded) |

## Implications for cyril

1. **`convert/kiro.rs` will need a parallel KAS dialect handler.** Today's code is keyed off `_kiro.dev/*`; KAS uses `_kiro/*` and `kiro/*`. When users adopt `--agent-engine kas` (or when AWS flips the default), cyril sees a different namespace on the wire.

2. **Filesystem capability advertisement must change.** Today cyril sends `fs: {}` because Kiro never calls back. KAS WILL call back. cyril needs to advertise fs capabilities and implement `_kiro/fs/{read_file, write_file, delete, stat, read_directory}` responders. **This is a real implementation task, not config.**

3. **Session lifecycle gets richer.** `fork`, `list`, `load`, `resume`, `close` become real. cyril could ship session-history UI, branching, etc. — capability surface that doesn't exist today.

4. **Execution-log stream is high-value UI surface.** Fine-grained operation events would let cyril show better progress, queued user messages, approval requests — much more than the toolbar can express today.

5. **Spec mode is agent-side, not client-side.** cyril doesn't need to implement spec workflow logic — but UX-wise, surfacing the three-document state could be valuable for KAS users.

6. **Plumbing favors NPM distribution.** Three independently-versioned packages (`@kiro/acp-type-covenant`, `@kiro/agent`, `@kiro/client`) with semver — that's an NPM-shaped distribution, not a monolithic asset blob. Strengthens the public-NPM-eventually hypothesis from [`docs/kiro-2.3.0-wire-audit.md`](kiro-2.3.0-wire-audit.md).

## Reproducing the analysis

```bash
mkdir -p /tmp/kiro-ide-0.12.155 && cd /tmp/kiro-ide-0.12.155
curl -sSL -O "https://prod.download.desktop.kiro.dev/releases/stable/linux-x64/signed/0.12.155/tar/kiro-ide-0.12.155-stable-linux-x64.tar.gz"
tar -xzf kiro-ide-*.tar.gz
cd Kiro/resources/app/extensions/kiro.kiro-agent

# Confirm KAS triad
cat packages/kiro-shared/package.json | python3 -m json.tool | grep -A 5 peerDependencies

# Enumerate @kiro/agent submodules
grep -aoE '@kiro/agent/dist/[a-z-]+' dist/extension.js | sort -u | wc -l

# Extension methods on the wire
grep -aoE '"_?kiro\.?dev?/[a-z_/]+"|"session/[a-z]+"' dist/extension.js | sort -u
```

For deeper investigation, beautify specific submodules with `js-beautify` or `prettier`. Cross-reference against Continue.dev's open-source code at `github.com/continuedev/continue` since the IDE forks it.
