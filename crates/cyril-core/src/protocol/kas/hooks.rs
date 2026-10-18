//! Hook registry for the KAS hooks host (cyril-jiyn, KAS-7).
//!
//! With `[agent] kas_hooks = "host"` cyril owns the hooks: this module loads
//! the user's on-disk hook files and serves them to KAS's `_kiro/hooks/list`
//! queries. Execution lives in the executor half of this module (slices
//! 5a/5b); wire dispatch in `client.rs` (slice 7).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use agent_client_protocol::schema::v1 as acp;

use super::json_ext_response;

/// The acp-stripped method name for `_kiro/hooks/list` (the acp library strips
/// the leading underscore, per the `SHELL_TYPE_METHOD` precedent).
///
/// **This method is bidirectional, and which direction is live depends on the
/// hook generation the client advertised** (cyril-gk17). Under
/// `kas_hooks = "host"` the *agent* calls it and [`HookRegistry::respond_list`]
/// answers. Under `kas_hooks = "kas"` the agent owns a file-watched registry
/// and cyril calls it — same name, opposite direction, different registry.
pub(crate) const LIST_METHOD: &str = "kiro/hooks/list";

/// The acp-stripped method name for `_kiro/hooks/setEnabled`
/// (client→agent only; it rewrites the `enabled` flag in the backing file).
pub(crate) const SET_ENABLED_METHOD: &str = "kiro/hooks/setEnabled";

/// The acp-stripped method name for `_kiro/hooks/executeHook`.
pub(crate) const EXECUTE_METHOD: &str = "kiro/hooks/executeHook";

/// The acp-stripped method name for the `_kiro/hooks/cancel` notification.
pub(crate) const CANCEL_METHOD: &str = "kiro/hooks/cancel";

/// The acp-stripped method name for the `_kiro/hooks/didChange` notification
/// (on-disk hook edits; v1 logs it — hot-reload is cyril-2adk).
pub(crate) const DID_CHANGE_METHOD: &str = "kiro/hooks/didChange";

/// The acp-stripped method name for `_kiro/hooks/sessionStart`.
pub(crate) const SESSION_START_METHOD: &str = "kiro/hooks/sessionStart";

/// The wire spelling of a shell-command hook action. The hook FILE says
/// `"command"`; the wire says `"runCommand"` — one of the three file/wire
/// divergences [`parse_wire_hooks`] translates, and the only action type with
/// anything for the panel to display.
const WIRE_RUN_COMMAND: &str = "runCommand";

/// Why a `_kiro/hooks/setEnabled` reply did not confirm the change.
///
/// The two arms must not collapse: a `{success: false}` is the agent *telling*
/// cyril it declined, while an unparseable body means cyril has no idea what
/// happened. Both end the operation, but only the first can be reported to the
/// user as the agent's answer (CLAUDE.md, "Distinguish 'missing' from
/// 'corrupt'").
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum SetEnabledError {
    /// The reply was not JSON at all — wire drift, not an answer.
    #[error("setEnabled reply is not JSON: {0}")]
    Corrupt(String),
    /// The agent explicitly declined to rewrite the backing file.
    #[error("{0}")]
    Refused(String),
}

/// Decide whether a `_kiro/hooks/setEnabled` reply confirms the change.
///
/// A transport-level `Ok` is not an agent-level yes: `setEnabled` rewrites the
/// `enabled` flag in a file on the user's disk, so "it returned" and "it did
/// it" are different claims.
///
/// - **Absent `success` is success.** The field is optional on the covenant's
///   `BaseCapabilityResponse`, so absence is "not reported", not "refused".
/// - **`success: false` is a refusal**, carrying `message` when the agent
///   supplied one.
/// - **An unparseable body is neither** — reporting it as success would tell
///   the user a disk mutation happened that cyril never confirmed.
pub(crate) fn interpret_set_enabled_reply(raw: &str) -> Result<(), SetEnabledError> {
    let body: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| SetEnabledError::Corrupt(e.to_string()))?;
    if body.get("success").and_then(serde_json::Value::as_bool) == Some(false) {
        return Err(SetEnabledError::Refused(
            body.get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("agent declined the change")
                .to_string(),
        ));
    }
    Ok(())
}

/// Translate KAS's v2 hook registry — as carried by `_kiro/hooks/list`'s
/// `{hooks: [...]}` reply and by the `_kiro/hooks/didChange` notification —
/// into cyril's display [`HookInfo`](crate::types::HookInfo).
///
/// The wire shape differs from the on-disk hook file in three ways cyril must
/// translate (live-verified 2026-07-31, `kas-v2hooks-2.16.0.jsonl`):
///
/// | | file | wire |
/// |---|---|---|
/// | identity | bare `name` | composite `"<filePath>#hook-<n>"` in `id` |
/// | action tag | `"command"` | `"runCommand"` |
/// | trigger case | `preToolUse` | `PreToolUse`, and under `_meta` |
///
/// Returns `None` when `hooks` is absent or is not an array — the caller then
/// surfaces the raw payload rather than an empty panel, matching how the v2
/// `/hooks` path already degrades. Individual entries that are malformed or
/// carry a non-`runCommand` action are **skipped with a warning** rather than
/// failing the batch: a `prompt`- or `agent`-action hook is legitimate on disk
/// and simply has no command to display, so dropping the whole registry over
/// one would hide every other hook the user has.
pub(crate) fn parse_wire_hooks(params: &serde_json::Value) -> Option<Vec<crate::types::HookInfo>> {
    // Missing and corrupt are different failure modes and must not collapse:
    // an ABSENT `hooks` key is normal (a host-mode didChange carries none),
    // while a PRESENT key of the wrong type is wire drift worth a warning.
    let array = match params.get("hooks") {
        None => {
            tracing::debug!("KAS hooks payload carries no `hooks` key");
            return None;
        }
        Some(serde_json::Value::Array(a)) => a,
        Some(other) => {
            tracing::warn!(
                kind = ?std::mem::discriminant(other),
                "KAS `hooks` is present but not an array — wire drift"
            );
            return None;
        }
    };
    let mut out = Vec::with_capacity(array.len());
    for entry in array {
        let meta = entry.get("_meta");
        let action_type = entry.pointer("/action/type").and_then(|t| t.as_str());
        // The tag is CHECKED, not merely logged. `runCommand` is the wire
        // spelling of the file's `command`, and that rename is one of the three
        // divergences this function exists to translate — so accepting any
        // action that merely happens to carry a `command` string would let a
        // `{"type": "prompt", ...}` entry through as if it were a shell hook,
        // and the divergence would have no real test behind it.
        if action_type != Some(WIRE_RUN_COMMAND) {
            tracing::warn!(
                action_type,
                id = entry.get("id").and_then(|i| i.as_str()),
                "KAS hook action is not `runCommand`; skipped (nothing to display)"
            );
            continue;
        }
        let Some(command) = entry.pointer("/action/command").and_then(|c| c.as_str()) else {
            tracing::warn!(
                action_type,
                id = entry.get("id").and_then(|i| i.as_str()),
                "KAS hook has no runCommand command; skipped"
            );
            continue;
        };
        // `_meta.trigger` is PascalCase on the wire, which is already the case
        // the v2 panel displays — so it passes through verbatim rather than
        // being normalized and re-cased.
        let Some(trigger) = meta.and_then(|m| m.get("trigger")).and_then(|t| t.as_str()) else {
            tracing::warn!(
                id = entry.get("id").and_then(|i| i.as_str()),
                "KAS hook has no _meta.trigger; skipped"
            );
            continue;
        };
        out.push(crate::types::HookInfo {
            trigger: trigger.to_string(),
            command: command.to_string(),
            // The v2 `matcher` column has no KAS counterpart: KAS matches with
            // the hook file's own `matcher` field, which the wire projection
            // does not carry. `None` is the honest answer, not "".
            matcher: None,
            id: entry
                .get("id")
                .and_then(|i| i.as_str())
                .map(crate::types::hook::HookId::new),
            name: entry
                .get("name")
                .and_then(|n| n.as_str())
                .map(str::to_string),
            enabled: meta
                .and_then(|m| m.get("enabled"))
                .and_then(|e| e.as_bool()),
        });
    }
    Some(out)
}

/// Answer `_kiro/hooks/sessionStart` by executing the registry's
/// SessionStart hooks and packaging their output as
/// `AcpPrecomputedHookResult[]` — the carved shape (2.13.0/2.14.1 bundles,
/// live-verified 2026-07-23, `.cyril-tpfd/findings.md`). KAS wraps each
/// element's `content` in a `<HOOK_INSTRUCTION>` block appended to the
/// session's first user prompt. Hooks run sequentially in registry order,
/// each under its own file-declared timeout (default 60s), with
/// `USER_PROMPT` present-but-empty (no prompt exists at session start).
/// Zero hooks → `{results: []}`, the same wire-safe acknowledgment the
/// pre-execution stub sent.
pub(crate) async fn respond_session_start(
    registry: &HookRegistry,
    cwd: &Path,
) -> acp::Result<acp::ExtResponse> {
    let mut runs = Vec::new();
    for def in registry.session_start_hooks() {
        let outcome = run_hook_command(&def.command, "", cwd, def.effective_timeout()).await;
        runs.push((def, outcome));
    }
    let results = package_session_start_results(runs);
    json_ext_response(&serde_json::json!({ "results": results }))
}

/// Package hook run outcomes as `AcpPrecomputedHookResult` elements, per the
/// carved KAS producer: include iff the run completed with non-empty output;
/// `content` is stdout when non-empty, else stderr — never combined (this is
/// deliberately NOT `executeHook`'s combined-output convention). Non-zero
/// exits with output are still included (D1: the producer has no exit-code
/// filter). `originalType` must stay the literal `"runCommand"` — an unknown
/// value throws `assertNever` inside the agent's telemetry path.
fn package_session_start_results(runs: Vec<(&HookDef, HookRunOutcome)>) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for (def, outcome) in runs {
        match outcome {
            HookRunOutcome::Completed {
                stdout,
                stderr,
                exit_code,
            } => {
                let content = if stdout.is_empty() { stderr } else { stdout };
                if content.is_empty() {
                    tracing::warn!(hook = %def.id, "sessionStart hook produced no output; skipped");
                    continue;
                }
                if exit_code != 0 {
                    tracing::debug!(
                        hook = %def.id, exit_code,
                        "sessionStart hook exited non-zero; output injected anyway (KAS parity)"
                    );
                }
                out.push(serde_json::json!({
                    "id": def.id,
                    "name": def.name,
                    "hookId": def.id,
                    "originalType": "runCommand",
                    "content": content,
                }));
            }
            HookRunOutcome::SpawnFailed { message } => {
                tracing::warn!(hook = %def.id, message, "sessionStart hook failed to spawn; skipped");
            }
            HookRunOutcome::TimedOut => {
                tracing::warn!(hook = %def.id, "sessionStart hook timed out; skipped");
            }
        }
    }
    out
}

/// In-flight hook executions, keyed by `operationId`, each holding a
/// cancel trigger. Shared (single `LocalSet` thread, so `RefCell`, mirroring
/// the terminal registry) between the executeHook responder and the cancel
/// notification handler.
#[derive(Debug, Default, Clone)]
pub(crate) struct HookOps {
    inner: Rc<RefCell<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
}

impl HookOps {
    fn register(&self, op_id: String) -> tokio::sync::oneshot::Receiver<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.inner.borrow_mut().insert(op_id, tx);
        rx
    }

    fn finish(&self, op_id: &str) {
        self.inner.borrow_mut().remove(op_id);
    }

    /// Trigger cancellation of the named operation. A no-op `warn` if the id
    /// is unknown (already finished, or a stale cancel — the lw67 class:
    /// never a silent nothing, never a panic).
    pub(crate) fn cancel(&self, op_id: &str) {
        match self.inner.borrow_mut().remove(op_id) {
            Some(tx) => {
                if tx.send(()).is_err() {
                    tracing::debug!(op_id, "hook operation finished before cancel landed");
                }
            }
            None => tracing::warn!(op_id, "cancel for an unknown hook operation; ignored"),
        }
    }
}

/// Default per-hook execution timeout when the agent sends no `timeout`
/// (covenant `HookExecuteParams.timeout?`). Bounds a runaway user command.
const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Typed result of one host-driven hook command run — the shared core under
/// both wire shapes: `executeHook` combines stdout+stderr into one `output`,
/// while sessionStart packaging picks stdout-else-stderr (the carved KAS
/// producer semantics). Keeping the streams separate here lets each caller
/// apply its own convention without string-matching on error text.
enum HookRunOutcome {
    Completed {
        stdout: String,
        stderr: String,
        exit_code: i32,
    },
    SpawnFailed {
        message: String,
    },
    TimedOut,
}

/// Run a hook command via the platform shell (`/bin/sh -c` on Unix, `cmd /C`
/// on Windows — hooks execute natively on the host, like agent terminal
/// commands) with `USER_PROMPT` in the environment and `cwd` as the working
/// directory. On timeout the child is killed (`kill_on_drop`).
async fn run_hook_command(
    command: &str,
    user_prompt: &str,
    cwd: &Path,
    timeout: std::time::Duration,
) -> HookRunOutcome {
    #[cfg(unix)]
    let (shell, flag) = ("/bin/sh", "-c");
    #[cfg(windows)]
    let (shell, flag) = ("cmd", "/C");
    let mut cmd = tokio::process::Command::new(shell);
    cmd.arg(flag)
        .arg(command)
        .env("USER_PROMPT", user_prompt)
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    match tokio::time::timeout(timeout, cmd.output()).await {
        Ok(Ok(out)) => {
            // `.code()` is None only on signal death; surface that as a
            // non-zero rather than a plausible-looking 0 (errors-are-not-
            // defaults). 137 = 128 + SIGKILL, a conventional shell mapping.
            let exit_code = out.status.code().unwrap_or_else(|| {
                tracing::warn!(command, "hook killed by signal; reporting 137");
                137
            });
            HookRunOutcome::Completed {
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                exit_code,
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(command, error = %e, "hook command failed to spawn");
            HookRunOutcome::SpawnFailed {
                message: e.to_string(),
            }
        }
        Err(_elapsed) => {
            tracing::warn!(command, ?timeout, "hook timed out; child killed");
            HookRunOutcome::TimedOut
        }
    }
}

/// Run a `runCommand` hook and shape the covenant `executeHook` reply
/// `{output?, exitCode, cancelled}`. Output is stdout+stderr combined;
/// `exitCode` is the real code (an exit-2 `preToolUse` hook is how KAS
/// blocks a tool — passed through verbatim). On timeout the reply is
/// `{cancelled:true}`.
pub(crate) async fn execute_hook(
    command: &str,
    user_prompt: &str,
    cwd: &Path,
    timeout: std::time::Duration,
) -> serde_json::Value {
    match run_hook_command(command, user_prompt, cwd, timeout).await {
        HookRunOutcome::Completed {
            stdout,
            stderr,
            exit_code,
        } => {
            let mut output = stdout;
            output.push_str(&stderr);
            serde_json::json!({"output": output, "exitCode": exit_code, "cancelled": false})
        }
        HookRunOutcome::SpawnFailed { message } => serde_json::json!({
            "output": format!("hook failed to spawn: {message}"),
            "exitCode": 127,
            "cancelled": false
        }),
        HookRunOutcome::TimedOut => {
            serde_json::json!({"cancelled": true, "exitCode": 124})
        }
    }
}

/// A loaded, servable hook: one `runCommand` entry from a `.kiro/hooks/*.json`
/// file, keyed by the wire-side (camelCase) trigger it answers to.
#[derive(Debug, Clone)]
pub(crate) struct HookDef {
    /// Namespaced `<file-stem>:<name>` — duplicate names across files stay
    /// distinct and traceable to their source file.
    pub id: String,
    pub name: String,
    /// The wire trigger this hook answers (`promptSubmit`, `preToolUse`,
    /// `postToolUse`, `agentStop`, `sessionStart`) — mapped from the file's
    /// PascalCase (`.cyril-jiyn/findings.md` Q2: the two vocabularies differ).
    pub wire_trigger: &'static str,
    /// The compiled matcher. `None` (absent or empty) is unconditional; an
    /// invalid matcher is retained as [`HookMatcher::Invalid`].
    matcher: Option<HookMatcher>,
    pub command: String,
    /// The file's `action.timeout` in seconds, if declared.
    pub timeout: Option<u64>,
}

impl HookDef {
    /// Execution bound for host-driven runs: the file's `action.timeout`
    /// seconds, else the 60s default. A declared `0` is honored verbatim
    /// (degenerate: the hook always times out, visible via the skip warn).
    pub(crate) fn effective_timeout(&self) -> std::time::Duration {
        self.timeout
            .map_or(DEFAULT_TIMEOUT, std::time::Duration::from_secs)
    }
}

/// Whether a wire trigger carries a tool to match (KAS's `toolName` trigger
/// kind). Only these triggers compile raw tool selectors or test a matcher
/// against a tool; on every other trigger a valid matcher selects every
/// invocation, because there is nothing to test it against.
fn is_tool_trigger(wire_trigger: &str) -> bool {
    matches!(wire_trigger, "preToolUse" | "postToolUse")
}

/// Characters that make a tool-trigger matcher regex-only rather than a raw
/// tool selector.
const REGEX_ONLY_CHARS: [char; 11] = ['\\', '^', '$', '(', ')', '[', ']', '{', '}', '|', '+'];

/// A configured, non-empty matcher compiled at load time. Mirrors KAS
/// 0.66.26+'s hook matcher (find it in the bundle by its `toolPattern`
/// field): on tool triggers a matcher free of [`REGEX_ONLY_CHARS`] is also a
/// raw tool selector, and independently a regex tested against the tool ID.
#[derive(Debug, Clone)]
enum HookMatcher {
    /// Neither form compiled. Retained (warned about at load) so the hook
    /// never matches rather than becoming unconditional.
    Invalid,
    /// A selector that is not a valid regex, such as bare `*` or `?`.
    Selector(ToolSelector),
    /// A regex only: regex syntax, or any matcher on a non-tool trigger.
    Regex(regex::Regex),
    /// Both forms; either one selects the tool.
    SelectorOrRegex(ToolSelector, regex::Regex),
}

impl HookMatcher {
    fn compile(source: &str, wire_trigger: &str, path: &Path, hook: &str) -> Self {
        let selector = if is_tool_trigger(wire_trigger) && !source.contains(REGEX_ONLY_CHARS) {
            ToolSelector::compile(source, path, hook)
        } else {
            None
        };
        match (selector, regex::Regex::new(source)) {
            (Some(selector), Ok(regex)) => Self::SelectorOrRegex(selector, regex),
            (None, Ok(regex)) => Self::Regex(regex),
            // KAS keeps a selector whose regex failed only when it is a glob;
            // a literal selector has no character that can fail a regex.
            (Some(selector @ ToolSelector::Glob(_)), Err(_)) => Self::Selector(selector),
            (_, Err(error)) => {
                tracing::warn!(
                    file = %path.display(), hook, matcher = %source, %error,
                    "invalid matcher (Rust regex syntax: no lookaround or backreferences); \
                     hook retained but will not match"
                );
                Self::Invalid
            }
        }
    }

    fn is_match(&self, wire_trigger: &str, tool_id: Option<&str>, tool_tags: &[String]) -> bool {
        if !is_tool_trigger(wire_trigger) {
            return !matches!(self, Self::Invalid);
        }
        let Some(tool_id) = tool_id else {
            return false;
        };
        match self {
            Self::Invalid => false,
            Self::Selector(selector) => selector.is_match(tool_id, tool_tags),
            Self::Regex(regex) => regex.is_match(tool_id),
            Self::SelectorOrRegex(selector, regex) => {
                selector.is_match(tool_id, tool_tags) || regex.is_match(tool_id)
            }
        }
    }
}

/// A raw tool selector, tested against the tool ID and each supplied tag.
#[derive(Debug, Clone)]
enum ToolSelector {
    /// An exact tool ID (or its bash/pwsh alias) or tag.
    Literal(String),
    /// A `*`/`?` glob anchored to the whole ID or tag.
    Glob(regex::Regex),
}

impl ToolSelector {
    fn compile(source: &str, path: &Path, hook: &str) -> Option<Self> {
        if !source.contains(['*', '?']) {
            return Some(Self::Literal(source.to_owned()));
        }
        let mut pattern = String::from(r"\A(?:");
        let mut utf8 = [0; 4];
        for ch in source.chars() {
            match ch {
                '*' => pattern.push_str(".*"),
                '?' => pattern.push('.'),
                _ => pattern.push_str(&regex::escape(ch.encode_utf8(&mut utf8))),
            }
        }
        pattern.push_str(r")\z");
        match regex::Regex::new(&pattern) {
            Ok(regex) => Some(Self::Glob(regex)),
            Err(error) => {
                tracing::warn!(
                    file = %path.display(), hook, matcher = %source, %error,
                    "tool selector could not compile; ID regex fallback only"
                );
                None
            }
        }
    }

    fn is_match(&self, tool_id: &str, tool_tags: &[String]) -> bool {
        match self {
            Self::Literal(literal) => {
                literal == tool_id
                    || matches!(
                        (literal.as_str(), tool_id),
                        ("execute_bash", "execute_pwsh")
                            | ("execute_pwsh", "execute_bash")
                            | ("control_bash_process", "control_pwsh_process")
                            | ("control_pwsh_process", "control_bash_process")
                    )
                    || tool_tags.iter().any(|tag| tag == literal)
            }
            Self::Glob(regex) => {
                regex.is_match(tool_id) || tool_tags.iter().any(|tag| regex.is_match(tag))
            }
        }
    }
}

/// `_kiro/hooks/list {trigger, toolId?, toolTags?}`, parsed once at the wire
/// seam. Parsing never fails: KAS treats a failed list call as "no hooks" for
/// the tool, so refusing a malformed frame would skip every hook, blocking
/// policy hooks included. A non-array `toolTags` is warned about and treated
/// as absent; an array's non-string members are warned about and dropped while
/// its string members are kept, since a tag can only add hooks to the reply.
#[derive(Debug)]
pub(crate) struct HooksListArgs {
    trigger: Option<String>,
    tool_id: Option<String>,
    /// Absent and empty tags are the same to KAS's selector (`tags` is
    /// optional on its tool descriptor).
    tool_tags: Vec<String>,
}

impl HooksListArgs {
    pub(crate) fn parse(params: &serde_json::Value) -> Self {
        let text = |key: &str| {
            params
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        Self {
            trigger: text("trigger"),
            tool_id: text("toolId"),
            tool_tags: parse_tool_tags(params.get("toolTags")),
        }
    }
}

fn parse_tool_tags(value: Option<&serde_json::Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Some(members) = value.as_array() else {
        tracing::warn!(tool_tags = %value, "hooks/list toolTags is not an array; matching by tool ID only");
        return Vec::new();
    };
    members
        .iter()
        .enumerate()
        .filter_map(|(index, member)| {
            let tag = member.as_str();
            if tag.is_none() {
                tracing::warn!(index, %member, "hooks/list toolTags member is not a string; dropped");
            }
            tag.map(str::to_owned)
        })
        .collect()
}

/// The on-disk file schema (kasHookFileSchema shape; hooksBlock carve in
/// `.cyril-0wyn/`): `{version: "v1", hooks: [{name, trigger, matcher?,
/// action: {type, command?}}]}`.
#[derive(Debug, serde::Deserialize)]
struct HookFile {
    version: String,
    hooks: Vec<HookFileEntry>,
}

#[derive(Debug, serde::Deserialize)]
struct HookFileEntry {
    name: String,
    trigger: String,
    #[serde(default)]
    matcher: Option<String>,
    action: HookAction,
}

#[derive(Debug, serde::Deserialize)]
struct HookAction {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    command: Option<String>,
    /// Per-hook execution bound in SECONDS (kasHookFileSchema: "timeout
    /// must be >= 0 seconds"). Used by host-driven sessionStart execution;
    /// `executeHook` keeps taking its timeout from the wire instead.
    #[serde(default)]
    timeout: Option<u64>,
}

/// PascalCase file trigger → camelCase wire trigger. `None` for triggers the
/// host wire model cannot serve (IDE file events, task events) or unknowns.
fn wire_trigger(file_trigger: &str) -> Option<&'static str> {
    match file_trigger {
        "UserPromptSubmit" => Some("promptSubmit"),
        "Stop" => Some("agentStop"),
        "PreToolUse" => Some("preToolUse"),
        "PostToolUse" => Some("postToolUse"),
        "SessionStart" => Some("sessionStart"),
        _ => None,
    }
}

/// The loaded hook set for one bridge lifetime (no hot-reload: cyril-2adk).
#[derive(Debug, Default)]
pub(crate) struct HookRegistry {
    hooks: Vec<HookDef>,
}

impl HookRegistry {
    /// Load hooks from the workspace root's `.kiro/hooks/` and the global
    /// `~/.kiro/hooks/`. Unservable entries warn and skip; invalid matchers
    /// warn and remain nonmatching. One bad file never takes down the rest
    /// (the load runs at bridge startup on user-authored content).
    pub(crate) fn load(workspace_root: &Path, global_kiro_home: Option<&Path>) -> Self {
        let mut hooks = Vec::new();
        let mut dirs = vec![workspace_root.join(".kiro").join("hooks")];
        if let Some(home) = global_kiro_home {
            dirs.push(home.join("hooks"));
        }
        for dir in dirs {
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    tracing::warn!(dir = %dir.display(), error = %e, "hooks dir unreadable; skipping");
                    continue;
                }
            };
            for entry in entries {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!(dir = %dir.display(), error = %e, "hooks dir entry unreadable; skipped");
                        continue;
                    }
                };
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                Self::load_file(&path, &mut hooks);
            }
        }
        tracing::info!(count = hooks.len(), "KAS hooks host: registry loaded");
        Self { hooks }
    }

    fn load_file(path: &Path, out: &mut Vec<HookDef>) {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("hooks")
            .to_string();
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(file = %path.display(), error = %e, "hook file unreadable; skipped");
                return;
            }
        };
        let file: HookFile = match serde_json::from_str(&text) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(file = %path.display(), error = %e, "hook file is not valid hook JSON; skipped");
                return;
            }
        };
        if file.version != "v1" {
            tracing::warn!(file = %path.display(), version = %file.version, "unknown hook file version; skipped");
            return;
        }
        for entry in file.hooks {
            let Some(trigger) = wire_trigger(&entry.trigger) else {
                tracing::warn!(
                    file = %path.display(), hook = %entry.name, trigger = %entry.trigger,
                    "trigger not servable in host mode; hook skipped"
                );
                continue;
            };
            if entry.action.kind != "command" {
                // agent-type actions need a prompt-injection vehicle: cyril-n03f.
                tracing::warn!(
                    file = %path.display(), hook = %entry.name, kind = %entry.action.kind,
                    "non-command hook action not executed in host mode; hook skipped"
                );
                continue;
            }
            let Some(command) = entry.action.command.filter(|c| !c.is_empty()) else {
                tracing::warn!(file = %path.display(), hook = %entry.name, "command action without a command; skipped");
                continue;
            };
            let matcher = entry
                .matcher
                .filter(|source| !source.is_empty())
                .map(|source| HookMatcher::compile(&source, trigger, path, &entry.name));
            out.push(HookDef {
                id: format!("{stem}:{}", entry.name),
                name: entry.name,
                wire_trigger: trigger,
                matcher,
                command,
                timeout: entry.action.timeout,
            });
        }
    }

    /// Answer `_kiro/hooks/list`, replying `{hooks: [...]}`. A missing
    /// trigger yields an empty list (the agent always sends one; a malformed
    /// frame should not error the turn).
    pub(crate) fn respond_list(&self, args: &HooksListArgs) -> acp::Result<acp::ExtResponse> {
        let Some(trigger) = args.trigger.as_deref() else {
            tracing::warn!("hooks/list without a trigger; replying empty");
            return json_ext_response(&serde_json::json!({ "hooks": [] }));
        };
        let hooks = self.list(trigger, args.tool_id.as_deref(), &args.tool_tags);
        json_ext_response(&serde_json::json!({ "hooks": hooks }))
    }

    /// The hooks whose wire trigger equals `trigger` and whose matcher (if
    /// any) selects the call. On tool triggers a matcher requires `tool_id`,
    /// even when tags are supplied; on other triggers a valid matcher selects
    /// every call. Unknown triggers yield an empty list, never an error.
    pub(crate) fn list(
        &self,
        trigger: &str,
        tool_id: Option<&str>,
        tool_tags: &[String],
    ) -> Vec<serde_json::Value> {
        self.matching(trigger, tool_id, tool_tags)
            .map(|h| {
                serde_json::json!({
                    "id": h.id,
                    "name": h.name,
                    "action": {"type": "runCommand", "command": h.command},
                    "approved": true,
                })
            })
            .collect()
    }

    /// The single membership predicate under both the wire `list` reply and
    /// host-driven sessionStart execution: wire trigger equals `trigger`,
    /// and the optional matcher selects the supplied tool ID/tags (see
    /// [`HookMatcher::is_match`]).
    fn matching<'a>(
        &'a self,
        trigger: &'a str,
        tool_id: Option<&'a str>,
        tool_tags: &'a [String],
    ) -> impl Iterator<Item = &'a HookDef> + 'a {
        self.hooks
            .iter()
            .filter(move |h| h.wire_trigger == trigger)
            .filter(move |h| {
                h.matcher
                    .as_ref()
                    .is_none_or(|matcher| matcher.is_match(h.wire_trigger, tool_id, tool_tags))
            })
    }

    /// The hooks host-driven sessionStart execution serves — the same
    /// `matching` predicate as `list("sessionStart", None, &[])`, so the
    /// accessor and the wire list cannot disagree on membership (structurally,
    /// not by parallel filters kept in sync by hand). As in KAS, a valid
    /// matcher selects at session start (there is no tool to test) and an
    /// invalid one, already warned about at load, never does.
    fn session_start_hooks(&self) -> impl Iterator<Item = &HookDef> {
        self.matching("sessionStart", None, &[])
    }
}

/// Answer the `_kiro/hooks/executeHook` ext request: run the params' command
/// and reply `{output?, exitCode, cancelled}`. The command is the one cyril
/// handed the agent in its `list` response (echoed back per the covenant);
/// `cwd` is the session workspace. A missing `command` is a warn + a
/// non-executing `{exitCode:127}` reply rather than an errored turn.
pub(crate) async fn respond_execute(
    params: &serde_json::Value,
    cwd: &Path,
    ops: &HookOps,
) -> acp::Result<acp::ExtResponse> {
    let command = params.get("command").and_then(|c| c.as_str());
    let user_prompt = params
        .get("userPrompt")
        .and_then(|u| u.as_str())
        .unwrap_or_else(|| {
            tracing::warn!("executeHook without userPrompt; USER_PROMPT set empty");
            ""
        });
    // The wire `timeout` is in SECONDS — the hook file schema declares
    // "timeout must be >= 0 seconds" and the host-callback forwards
    // `action.timeout` verbatim (2.13.0 bundle carve). Treating it as millis
    // would make every timeout 1000x too short (schema-vs-runtime, verified
    // against the bundle not assumed).
    let timeout = params
        .get("timeout")
        .and_then(serde_json::Value::as_u64)
        .map_or(DEFAULT_TIMEOUT, std::time::Duration::from_secs);
    let op_id = params
        .get("operationId")
        .and_then(|o| o.as_str())
        .map(str::to_owned);

    let reply = match command {
        Some(cmd) => match &op_id {
            // Cancellable: race the command against the cancel trigger. If
            // cancel wins, the `execute_hook` future is dropped mid-await and
            // `kill_on_drop` reaps the child (the lw67 no-orphan invariant).
            Some(id) => {
                let cancel = ops.register(id.clone());
                let result = tokio::select! {
                    biased;
                    out = execute_hook(cmd, user_prompt, cwd, timeout) => out,
                    _ = cancel => serde_json::json!({"cancelled": true, "exitCode": 130}),
                };
                ops.finish(id);
                result
            }
            None => execute_hook(cmd, user_prompt, cwd, timeout).await,
        },
        None => {
            tracing::warn!("executeHook without a command; not executed");
            serde_json::json!({"output": "no command", "exitCode": 127, "cancelled": false})
        }
    };
    json_ext_response(&reply)
}

#[cfg(test)]
mod wire_hook_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::types::hook::HookId;

    /// The `_kiro/hooks/didChange` payload captured live on kiro-cli 2.16.0 /
    /// KAS 0.27.8 (`experiments/conductor-spike/kas-v2hooks-2.16.0.jsonl`),
    /// verbatim. The `list` reply carries the same `{hooks: [...]}` element
    /// shape, so one fixture fences both directions.
    fn live_capture() -> serde_json::Value {
        serde_json::json!({
          "sessionId": "sess_80d7265a-c751-4754-a738-767f51c076ca",
          "hooks": [
            {
              "id": "/tmp/kas-v2h/.kiro/hooks/audit.json#hook-0",
              "name": "cyril-audit-pre",
              "action": { "type": "runCommand", "command": "echo pre >> /tmp/evidence.txt" },
              "_meta": {
                "trigger": "PreToolUse",
                "source": "standalone-file",
                "filePath": "/tmp/kas-v2h/.kiro/hooks/audit.json",
                "enabled": true
              }
            },
            {
              "id": "/tmp/kas-v2h/.kiro/hooks/audit.json#hook-1",
              "name": "cyril-audit-start",
              "action": { "type": "runCommand", "command": "echo start >> /tmp/evidence.txt" },
              "_meta": {
                "trigger": "SessionStart",
                "source": "standalone-file",
                "filePath": "/tmp/kas-v2h/.kiro/hooks/audit.json",
                "enabled": false
              }
            }
          ]
        })
    }

    #[test]
    fn translates_the_live_capture() {
        let hooks = parse_wire_hooks(&live_capture()).expect("hooks array present");
        assert_eq!(hooks.len(), 2);

        // The three documented file-vs-wire divergences, asserted as VALUES so
        // a translation that silently passed the file shape through would fail:
        // the id is the COMPOSITE (not the bare name), the command comes from
        // the `runCommand`-tagged action, and the trigger is the PascalCase one
        // nested under `_meta` (not a top-level camelCase field, which does not
        // exist on the wire at all).
        assert_eq!(
            hooks[0].id.as_ref().map(HookId::as_str),
            Some("/tmp/kas-v2h/.kiro/hooks/audit.json#hook-0")
        );
        assert_ne!(
            hooks[0].id.as_ref().map(HookId::as_str),
            Some("cyril-audit-pre"),
            "id is not the name"
        );
        assert_eq!(hooks[0].name.as_deref(), Some("cyril-audit-pre"));
        assert_eq!(hooks[0].trigger, "PreToolUse");
        assert_eq!(hooks[0].command, "echo pre >> /tmp/evidence.txt");
        assert_eq!(hooks[0].enabled, Some(true));

        // includeDisabled means a disabled hook is PRESENT and marked, not
        // absent — the panel dims it on exactly this value.
        assert_eq!(hooks[1].enabled, Some(false));
        assert_eq!(hooks[1].trigger, "SessionStart");

        // KAS carries no matcher in this projection; None, never "".
        assert!(hooks.iter().all(|h| h.matcher.is_none()));
    }

    #[test]
    fn missing_hooks_array_is_none_not_empty() {
        // The distinction the caller depends on: `None` surfaces a BridgeError
        // naming the drift, while `Some(vec![])` would render as "you have no
        // hooks". A host-mode didChange (no `hooks` key) takes this path too.
        assert!(parse_wire_hooks(&serde_json::json!({"sessionId": "s"})).is_none());
        assert!(parse_wire_hooks(&serde_json::json!({"hooks": "not-an-array"})).is_none());
        let empty = parse_wire_hooks(&serde_json::json!({"hooks": []})).expect("empty is Some");
        assert!(
            empty.is_empty(),
            "a genuinely empty registry is Some(vec![])"
        );
    }

    #[test]
    fn malformed_entries_are_skipped_not_fatal() {
        // A prompt-action hook is legitimate on disk and simply has no command
        // to display. Dropping the whole registry over it would hide every
        // other hook the user has, so the batch survives and the good entry
        // still arrives.
        let payload = serde_json::json!({"hooks": [
            { "id": "a#hook-0", "name": "promptish",
              "action": { "type": "prompt", "prompt": "think harder" },
              "_meta": { "trigger": "Stop", "enabled": true } },
            { "id": "b#hook-0", "name": "no-trigger",
              "action": { "type": "runCommand", "command": "echo hi" },
              "_meta": { "enabled": true } },
            { "id": "c#hook-0", "name": "good",
              "action": { "type": "runCommand", "command": "echo ok" },
              "_meta": { "trigger": "PreToolUse", "enabled": true } }
        ]});
        let hooks = parse_wire_hooks(&payload).expect("array present");
        assert_eq!(hooks.len(), 1, "only the well-formed entry survives");
        assert_eq!(hooks[0].name.as_deref(), Some("good"));
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    fn write(dir: &Path, name: &str, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), body).unwrap();
    }

    /// The `id`s of listed hook entries, in reply order.
    fn hook_ids(hooks: &[serde_json::Value]) -> Vec<String> {
        hooks
            .iter()
            .map(|hook| hook["id"].as_str().unwrap().to_owned())
            .collect()
    }

    /// Serve wire `params` through the parser and responder, returning the
    /// listed hook IDs.
    fn listed_ids(reg: &HookRegistry, params: &serde_json::Value) -> Vec<String> {
        let resp = reg.respond_list(&HooksListArgs::parse(params)).unwrap();
        let reply: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        hook_ids(reply["hooks"].as_array().unwrap())
    }

    // cyril-gk17 review fence: the file-vs-wire action rename is a real check,
    // not a log line. Before this, `action_type` was read and only printed, so a
    // `prompt`-action entry that happened to carry a `command` string was
    // accepted as a shell hook — and the fence for this divergence would pass
    // identically against the file spelling, testing nothing.
    #[test]
    fn only_run_command_actions_are_displayed() {
        let wire = serde_json::json!({"hooks": [
            // The wire spelling: accepted.
            {"id": "f#hook-0", "name": "ok",
             "action": {"type": "runCommand", "command": "echo hi"},
             "_meta": {"trigger": "PreToolUse", "enabled": true}},
            // The FILE spelling on the wire: this is the divergence, and it must
            // NOT be accepted just because a `command` key is present.
            {"id": "f#hook-1", "name": "file-spelling",
             "action": {"type": "command", "command": "echo no"},
             "_meta": {"trigger": "PreToolUse", "enabled": true}},
            // A legitimate non-shell action with a command-shaped payload.
            {"id": "f#hook-2", "name": "prompt-action",
             "action": {"type": "prompt", "command": "echo no"},
             "_meta": {"trigger": "PreToolUse", "enabled": true}},
        ]});
        let hooks = parse_wire_hooks(&wire).expect("array present");
        let names: Vec<&str> = hooks.iter().filter_map(|h| h.name.as_deref()).collect();
        assert_eq!(
            names,
            vec!["ok"],
            "only the runCommand action is displayable; the others are skipped, \
             not silently rendered as shell hooks"
        );
    }

    // cyril-gk17 review fence: `setEnabled` rewrites a file on the user's disk,
    // so the three readings of its reply must stay distinct. The regression
    // this guards is the middle case — a body that does not parse being taken
    // as assent, which reported a mutation cyril never confirmed.
    #[test]
    fn set_enabled_reply_separates_confirmed_refused_and_corrupt() {
        // Confirmed: explicit success, and absent `success` (optional on the
        // covenant's BaseCapabilityResponse — "not reported", not "refused").
        for ok in [r#"{"success":true}"#, "{}", r#"{"hooks":[]}"#] {
            assert_eq!(
                interpret_set_enabled_reply(ok),
                Ok(()),
                "{ok} must read as confirmed"
            );
        }

        // Refused: the agent's own message is surfaced when it supplies one,
        // and stands in for it when it does not.
        assert_eq!(
            interpret_set_enabled_reply(r#"{"success":false,"message":"file is read-only"}"#),
            Err(SetEnabledError::Refused("file is read-only".to_string()))
        );
        assert_eq!(
            interpret_set_enabled_reply(r#"{"success":false}"#),
            Err(SetEnabledError::Refused(
                "agent declined the change".to_string()
            ))
        );

        // Corrupt: NOT success. An unparseable body means cyril has no idea
        // whether the flag moved; saying "done" would be a false report.
        for bad in ["", "not json", r#"{"success":"#] {
            assert!(
                matches!(
                    interpret_set_enabled_reply(bad),
                    Err(SetEnabledError::Corrupt(_))
                ),
                "{bad:?} must not read as confirmed"
            );
        }
    }

    // cyril-jiyn claim 4 fence (mapping half): a valid load maps PascalCase
    // file triggers to the wire vocabulary and namespaces ids by file stem.
    #[test]
    fn hook_registry_loads_and_maps() {
        let ws = tempfile::tempdir().unwrap();
        let hooks_dir = ws.path().join(".kiro/hooks");
        write(
            &hooks_dir,
            "team.json",
            r#"{"version":"v1","hooks":[
                {"name":"lint","trigger":"PreToolUse","matcher":"fs_.*",
                 "action":{"type":"command","command":"echo lint"}},
                {"name":"greet","trigger":"UserPromptSubmit",
                 "action":{"type":"command","command":"echo hi"}}
            ]}"#,
        );
        // A global root with one Stop hook — both sources merge.
        let global = tempfile::tempdir().unwrap();
        write(
            &global.path().join("hooks"),
            "personal.json",
            r#"{"version":"v1","hooks":[
                {"name":"bye","trigger":"Stop","action":{"type":"command","command":"echo bye"}}
            ]}"#,
        );

        let reg = HookRegistry::load(ws.path(), Some(global.path()));
        let mut got: Vec<(&str, &str)> = reg
            .hooks
            .iter()
            .map(|h| (h.id.as_str(), h.wire_trigger))
            .collect();
        got.sort_unstable();
        assert_eq!(
            got,
            vec![
                ("personal:bye", "agentStop"),
                ("team:greet", "promptSubmit"),
                ("team:lint", "preToolUse"),
            ]
        );
        assert!(
            reg.hooks.iter().any(|h| h.matcher.is_some()),
            "the matcher survived the load"
        );
    }

    // cyril-jiyn claim 4 fence (isolation half): the stress-fixture load — one
    // dir containing an invalid-JSON file, an unknown trigger, an agent action,
    // an invalid matcher regex, and a duplicate name in a second file.
    // Expected: exactly the servable hooks are listed (the invalid-matcher hook
    // is retained but never matches); nothing aborts.
    #[test]
    fn hook_registry_serves_valid_hooks_despite_invalid_entries() {
        let ws = tempfile::tempdir().unwrap();
        let dir = ws.path().join(".kiro/hooks");
        write(&dir, "broken.json", "{ not json !!!");
        write(
            &dir,
            "mixed.json",
            r#"{"version":"v1","hooks":[
                {"name":"good","trigger":"PostToolUse","action":{"type":"command","command":"echo ok"}},
                {"name":"filey","trigger":"PostFileSave","action":{"type":"command","command":"echo nope"}},
                {"name":"agenty","trigger":"Stop","action":{"type":"agent"}},
                {"name":"badrx","trigger":"PreToolUse","matcher":"fs_(","action":{"type":"command","command":"echo nope"}}
            ]}"#,
        );
        write(
            &dir,
            "second.json",
            r#"{"version":"v1","hooks":[
                {"name":"good","trigger":"UserPromptSubmit","action":{"type":"command","command":"echo dup"}}
            ]}"#,
        );
        write(
            &dir,
            "oldver.json",
            r#"{"version":"v2","hooks":[
                {"name":"future","trigger":"Stop","action":{"type":"command","command":"echo nope"}}
            ]}"#,
        );

        let reg = HookRegistry::load(ws.path(), None);
        let mut ids: Vec<String> = ["postToolUse", "promptSubmit", "preToolUse", "agentStop"]
            .into_iter()
            .flat_map(|trigger| hook_ids(&reg.list(trigger, Some("fs_write"), &[])))
            .collect();
        ids.sort_unstable();
        assert_eq!(
            ids,
            vec!["mixed:good", "second:good"],
            "invalid entries cannot become unconditional; duplicate names retain distinct IDs"
        );
    }

    // cyril-jiyn claim 5 fence: list honors trigger + matcher-vs-toolId, and an
    // unknown trigger is empty (not an error). The matcher hook and the
    // no-matcher hook share a trigger so the two toolId cases differ only by
    // the matcher — a matcher-ignoring impl makes the first two asserts equal.
    #[test]
    fn hooks_list_filtering() {
        let ws = tempfile::tempdir().unwrap();
        write(
            &ws.path().join(".kiro/hooks"),
            "h.json",
            r#"{"version":"v1","hooks":[
                {"name":"fsonly","trigger":"PreToolUse","matcher":"fs_.*",
                 "action":{"type":"command","command":"echo fs"}},
                {"name":"always","trigger":"PreToolUse",
                 "action":{"type":"command","command":"echo any"}}
            ]}"#,
        );
        let reg = HookRegistry::load(ws.path(), None);

        let names = |v: &[serde_json::Value]| -> Vec<String> {
            let mut n = hook_ids(v);
            n.sort();
            n
        };

        // fs_write matches the matcher → both hooks.
        assert_eq!(
            names(&reg.list("preToolUse", Some("fs_write"), &[])),
            vec!["h:always", "h:fsonly"]
        );
        // execute_bash misses the matcher → only the no-matcher hook.
        assert_eq!(
            names(&reg.list("preToolUse", Some("execute_bash"), &[])),
            vec!["h:always"]
        );
        // No toolId → matcher hooks can't match → only the no-matcher hook.
        assert_eq!(names(&reg.list("preToolUse", None, &[])), vec!["h:always"]);
        // Unknown trigger → empty, not an error.
        assert!(reg.list("bogusTrigger", Some("fs_write"), &[]).is_empty());
    }

    #[test]
    fn respond_list_selects_shell_tag_hook() {
        let ws = tempfile::tempdir().unwrap();
        write(
            &ws.path().join(".kiro/hooks"),
            "tags.json",
            r#"{"version":"v1","hooks":[
                {"name":"shell-tag","trigger":"PreToolUse","matcher":"shell",
                 "action":{"type":"command","command":"echo hook"}}
            ]}"#,
        );
        let reg = HookRegistry::load(ws.path(), None);
        let resp = reg
            .respond_list(&HooksListArgs::parse(&serde_json::json!({
                "trigger": "preToolUse",
                "toolId": "execute_bash",
                "toolTags": ["shell", "@builtin"]
            })))
            .unwrap();
        let reply: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        assert_eq!(
            reply["hooks"],
            serde_json::json!([{
                "id": "tags:shell-tag",
                "name": "shell-tag",
                "action": {"type": "runCommand", "command": "echo hook"},
                "approved": true
            }])
        );
    }

    #[test]
    fn respond_list_matches_kas_tool_selector_matrix() {
        // Matcher source, optional tool ID/tags, and expected selection.
        type ToolSelectorCase<'a> = (&'a str, Option<&'a str>, Option<&'a [&'a str]>, bool);
        let cases: &[ToolSelectorCase<'_>] = &[
            (
                "shell",
                Some("execute_bash"),
                Some(&["shell", "@builtin"]),
                true,
            ),
            ("read", Some("read_file"), Some(&["read", "@builtin"]), true),
            (
                "@builtin",
                Some("execute_bash"),
                Some(&["shell", "@builtin"]),
                true,
            ),
            ("@mcp", Some("mcp_git_status"), Some(&["@mcp"]), true),
            ("@mcp", Some("execute_bash"), Some(&["@mcp"]), true),
            ("shell", Some("execute_bash"), None, false),
            ("shell", Some("execute_bash"), Some(&[]), false),
            ("shell", Some("execute_bash"), Some(&["shell-tools"]), false),
            ("shell", Some("execute_bash"), Some(&["@shell"]), false),
            ("shell", None, Some(&["shell", "@builtin"]), false),
            ("@mcp", Some("mcp_git_status"), None, false),
            ("@builtin", Some("builtin_tool"), None, false),
            ("*", Some("execute_bash"), None, true),
            ("*", Some(""), None, true),
            ("*", None, Some(&["shell"]), false),
            ("?", Some("x"), None, true),
            ("?", Some("xy"), None, false),
            ("?", Some("execute_bash"), Some(&["x"]), true),
            ("mcp_*", Some("mcp_git_status"), None, true),
            ("mcp_*", Some("execute_bash"), Some(&["mcp_git"]), true),
            (
                "mcp_*",
                Some("execute_bash"),
                Some(&["prefix_mcp_git"]),
                false,
            ),
            ("sh?ll", Some("execute_bash"), Some(&["shell"]), true),
            ("sh?ll", Some("execute_bash"), Some(&["shll"]), false),
            ("sh?ll", Some("execute_bash"), Some(&["shell_extra"]), false),
            ("@mcp.*", Some("execute_bash"), Some(&["@mcp.git"]), true),
            ("@mcp.*", Some("execute_bash"), Some(&["@mcpXgit"]), false),
            ("é?lair*", Some("x"), Some(&["éclair_v2"]), true),
            ("é?lair*", Some("x"), Some(&["eclair"]), false),
            ("a-b #c*", Some("a-b #cd"), None, true),
            ("a-b #c*", Some("a-b  #cd"), None, false),
            ("execute_bash", Some("execute_pwsh"), None, true),
            ("execute_pwsh", Some("execute_bash"), None, true),
            (
                "control_bash_process",
                Some("control_pwsh_process"),
                None,
                true,
            ),
            (
                "control_pwsh_process",
                Some("control_bash_process"),
                None,
                true,
            ),
            ("execute_ba*", Some("execute_pwsh"), None, false),
            ("fs_.*", Some("fs_write"), None, true),
            ("fs_.*", Some("execute_bash"), Some(&["fs_write"]), false),
            ("shell", Some("shell_tool"), None, true),
            ("^mcp_.*$", Some("mcp_git_status"), None, true),
            (
                "^mcp_.*$",
                Some("execute_bash"),
                Some(&["mcp_git_status"]),
                false,
            ),
            ("shell|read", Some("execute_bash"), Some(&["shell"]), false),
            (r"\bshell\b", Some("execute_bash"), Some(&["shell"]), false),
            ("shell$", Some("execute_bash"), Some(&["shell"]), false),
            ("sh[ae]ll", Some("execute_bash"), Some(&["shell"]), false),
            ("s(hell)", Some("execute_bash"), Some(&["shell"]), false),
            ("sh{2}ell", Some("execute_bash"), Some(&["shhell"]), false),
            ("shell+", Some("execute_bash"), Some(&["shell"]), false),
            (
                "[",
                Some("execute_bash"),
                Some(&["shell", "@builtin"]),
                false,
            ),
            ("fs_(", Some("fs_write"), None, false),
            ("", None, None, true),
        ];
        let ws = tempfile::tempdir().unwrap();
        let dir = ws.path().join(".kiro/hooks");
        for (file_trigger, trigger) in
            [("PreToolUse", "preToolUse"), ("PostToolUse", "postToolUse")]
        {
            for &(matcher, tool_id, tool_tags, expected) in cases {
                let file = serde_json::json!({
                    "version": "v1",
                    "hooks": [{
                        "name": "selected",
                        "trigger": file_trigger,
                        "matcher": matcher,
                        "action": {"type": "command", "command": "echo hook"}
                    }]
                });
                write(&dir, "matrix.json", &file.to_string());
                let reg = HookRegistry::load(ws.path(), None);
                let mut params = serde_json::json!({"trigger": trigger});
                if let Some(id) = tool_id {
                    params["toolId"] = id.into();
                }
                if let Some(tags) = tool_tags {
                    params["toolTags"] = serde_json::json!(tags);
                }
                assert_eq!(
                    listed_ids(&reg, &params),
                    if expected {
                        vec!["matrix:selected"]
                    } else {
                        vec![]
                    },
                    "{trigger}: matcher={matcher:?}, id={tool_id:?}, tags={tool_tags:?}"
                );
            }
        }
    }

    // KAS's `none` trigger kind: on a trigger with no tool, a valid matcher
    // has nothing to test and selects every call, whatever toolId/toolTags
    // arrive; an invalid one (bare `*` is not a regex and selectors exist only
    // on tool triggers) never does. Host-driven sessionStart shares the rule.
    #[test]
    fn respond_list_non_tool_trigger_matchers_follow_kas() {
        let ws = tempfile::tempdir().unwrap();
        let dir = ws.path().join(".kiro/hooks");
        for (file_trigger, trigger) in [
            ("SessionStart", "sessionStart"),
            ("UserPromptSubmit", "promptSubmit"),
            ("Stop", "agentStop"),
        ] {
            let file = serde_json::json!({
                "version": "v1",
                "hooks": [
                    {"name": "always", "trigger": file_trigger,
                     "action": {"type": "command", "command": "echo always"}},
                    {"name": "empty", "trigger": file_trigger, "matcher": "",
                     "action": {"type": "command", "command": "echo empty"}},
                    {"name": "regex", "trigger": file_trigger, "matcher": "execute_bash",
                     "action": {"type": "command", "command": "echo regex"}},
                    {"name": "invalid", "trigger": file_trigger, "matcher": "*",
                     "action": {"type": "command", "command": "echo invalid"}}
                ]
            });
            write(&dir, "other.json", &file.to_string());
            let reg = HookRegistry::load(ws.path(), None);
            let expected = vec!["other:always", "other:empty", "other:regex"];
            for tool_id in [None, Some("execute_pwsh"), Some("execute_bash")] {
                let mut params = serde_json::json!({
                    "trigger": trigger,
                    "toolTags": ["execute_bash"]
                });
                if let Some(id) = tool_id {
                    params["toolId"] = id.into();
                }
                assert_eq!(
                    listed_ids(&reg, &params),
                    expected,
                    "{trigger}: toolId={tool_id:?}"
                );
            }
            if trigger == "sessionStart" {
                let session_start: Vec<&str> =
                    reg.session_start_hooks().map(|h| h.id.as_str()).collect();
                assert_eq!(session_start, expected);
            }
        }
    }

    // KAS answers a failed list call by running none of the tool's hooks, so a
    // malformed toolTags must degrade rather than refuse: a non-array is
    // ID-only matching, an array keeps its string members. The `byid` and
    // `always` hooks show unrelated hooks still run; `shell` shows kept tags.
    #[test]
    fn respond_list_degrades_malformed_tool_tags() {
        let ws = tempfile::tempdir().unwrap();
        write(
            &ws.path().join(".kiro/hooks"),
            "tags.json",
            r#"{"version":"v1","hooks":[
                {"name":"always","trigger":"PreToolUse",
                 "action":{"type":"command","command":"echo always"}},
                {"name":"byid","trigger":"PreToolUse","matcher":"execute_bash",
                 "action":{"type":"command","command":"echo byid"}},
                {"name":"shell","trigger":"PreToolUse","matcher":"shell",
                 "action":{"type":"command","command":"echo shell"}}
            ]}"#,
        );
        let reg = HookRegistry::load(ws.path(), None);
        let id_only = vec!["tags:always", "tags:byid"];
        let with_shell = vec!["tags:always", "tags:byid", "tags:shell"];
        for (tags, expected) in [
            (serde_json::json!(null), &id_only),
            (serde_json::json!("shell"), &id_only),
            (serde_json::json!(42), &id_only),
            (serde_json::json!(false), &id_only),
            (serde_json::json!({"tag": "shell"}), &id_only),
            (serde_json::json!([null]), &id_only),
            (serde_json::json!(["shell", null]), &with_shell),
            (serde_json::json!([42, "shell"]), &with_shell),
            (serde_json::json!(["shell", false]), &with_shell),
            (serde_json::json!(["shell", ["@builtin"]]), &with_shell),
            (
                serde_json::json!(["shell", {"tag": "@builtin"}]),
                &with_shell,
            ),
        ] {
            let params = serde_json::json!({
                "trigger": "preToolUse",
                "toolId": "execute_bash",
                "toolTags": tags
            });
            assert_eq!(&listed_ids(&reg, &params), expected, "toolTags={tags}");
        }
        let untriggered = serde_json::json!({"toolId": "execute_bash", "toolTags": [42]});
        assert!(listed_ids(&reg, &untriggered).is_empty());
    }

    // A malformed list frame (no `trigger`) is a warn + empty reply, never an
    // errored turn — fences the logged early-return against a future panic
    // or error-reply rewrite.
    #[test]
    fn respond_list_missing_trigger_replies_empty() {
        let reg = HookRegistry { hooks: Vec::new() };
        let resp = reg
            .respond_list(&HooksListArgs::parse(&serde_json::json!({})))
            .unwrap();
        let reply: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        assert_eq!(reply["hooks"], serde_json::json!([]));
    }

    // cyril-jiyn claim 6 fence: the command runs with USER_PROMPT in env and
    // the workspace as cwd — a command echoing both proves the wiring. POSIX
    // command syntax (printf/pwd/$VAR) — meaningful only on Unix.
    #[cfg(unix)]
    #[tokio::test]
    async fn execute_hook_env_and_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let out = execute_hook(
            r#"printf '%s' "$USER_PROMPT"; printf ' @ '; pwd"#,
            "the-prompt",
            dir.path(),
            std::time::Duration::from_secs(10),
        )
        .await;
        let text = out["output"].as_str().unwrap();
        assert!(text.starts_with("the-prompt @ "), "USER_PROMPT env: {text}");
        // The tempdir may be a symlink (macOS /tmp); compare the trailing name.
        let leaf = dir.path().file_name().unwrap().to_str().unwrap();
        assert!(text.contains(leaf), "cwd is the workspace: {text}");
        assert_eq!(out["exitCode"], 0);
        assert_eq!(out["cancelled"], false);
    }

    // cyril-jiyn claim 7 fence: combined stdout+stderr and the REAL exit code
    // for 0, 1, and 2 — a bool-success mapping or a dropped stderr fails this.
    // POSIX syntax (`;`, `>&2`) — the Windows counterpart is below.
    #[cfg(unix)]
    #[tokio::test]
    async fn execute_hook_real_exit_codes() {
        let dir = tempfile::tempdir().unwrap();
        let t = std::time::Duration::from_secs(10);

        let zero = execute_hook("echo out", "", dir.path(), t).await;
        assert_eq!(zero["exitCode"], 0);
        assert_eq!(zero["output"], "out\n");

        let one = execute_hook("echo o; echo e >&2; exit 1", "", dir.path(), t).await;
        assert_eq!(one["exitCode"], 1);
        let combined = one["output"].as_str().unwrap();
        assert!(
            combined.contains("o") && combined.contains("e"),
            "stdout+stderr combined: {combined:?}"
        );

        // Claim 8 (the AC's named block contract): exit 2 passes through
        // verbatim as {output, exitCode:2, cancelled:false} — the preToolUse
        // block signal.
        let two = execute_hook("echo DENY; exit 2", "", dir.path(), t).await;
        assert_eq!(two["exitCode"], 2, "exit 2 is the preToolUse block");
        assert_eq!(two["cancelled"], false);
        assert_eq!(two["output"], "DENY\n");
    }

    // Windows counterpart of the claim 7/8 fences: the hook spawns via
    // `cmd /C` and the real exit code + output cross the reply. Fences the
    // hardcoded-/bin/sh regression, where every hook died as spawn-fail 127.
    #[cfg(windows)]
    #[tokio::test]
    async fn execute_hook_real_exit_codes_windows() {
        let dir = tempfile::tempdir().unwrap();
        let t = std::time::Duration::from_secs(10);

        let zero = execute_hook("echo out", "", dir.path(), t).await;
        assert_eq!(zero["exitCode"], 0);
        assert_eq!(zero["output"], "out\r\n");

        let two = execute_hook("echo DENY& exit /b 2", "", dir.path(), t).await;
        assert_eq!(two["exitCode"], 2, "exit 2 is the preToolUse block");
        assert_eq!(two["cancelled"], false);
        assert_eq!(two["output"], "DENY\r\n");
    }

    // cyril-jiyn claim 8 (block contract at the responder level): the
    // executeHook reply for an exit-2 command is exactly
    // {output, exitCode:2, cancelled:false} through respond_execute.
    #[tokio::test]
    async fn pre_tool_use_exit2_block_contract() {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        let (command, expected) = ("echo blocked; exit 2", "blocked\n");
        #[cfg(windows)]
        let (command, expected) = ("echo blocked& exit /b 2", "blocked\r\n");
        let params = serde_json::json!({
            "hookId": "h", "hookName": "policy", "command": command,
            "sessionId": "s", "userPrompt": "{}"
        });
        let resp = respond_execute(&params, dir.path(), &HookOps::default())
            .await
            .unwrap();
        let reply: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        assert_eq!(reply["exitCode"], 2);
        assert_eq!(reply["cancelled"], false);
        assert_eq!(reply["output"], expected);
    }

    // A ~30s sleeper for the timeout/cancel fences. `ping -n` is the cmd-shell
    // idiom: `timeout /t` errors out when stdin is redirected (it is — null).
    #[cfg(unix)]
    const SLEEP_30: &str = "sleep 30";
    #[cfg(windows)]
    const SLEEP_30: &str = "ping -n 31 127.0.0.1 >nul";

    // cyril-jiyn claim 9 fence: a hook exceeding its timeout is killed and the
    // reply says cancelled — a timer-without-kill leaves the child alive and
    // the reply would (wrongly) carry command output. 300ms timeout on a
    // 30s sleep; must return in ~timeout, not ~30s.
    #[tokio::test]
    async fn execute_hook_timeout_kills() {
        let dir = tempfile::tempdir().unwrap();
        let start = std::time::Instant::now();
        let out = execute_hook(
            SLEEP_30,
            "",
            dir.path(),
            std::time::Duration::from_millis(300),
        )
        .await;
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "returned on timeout, not after the full sleep"
        );
        assert_eq!(out["cancelled"], true);
        assert!(out.get("output").is_none(), "no command output on timeout");
    }

    // cyril-jiyn claim 10 fence: cancel by operationId aborts a running hook —
    // reply cancelled, and the select-drop reaps the child (lw67 class: cancel
    // during a pending execution is never a silent no-op). Also: an unknown
    // operationId cancel is a warn no-op that does not disturb the running op.
    #[tokio::test]
    async fn execute_hook_cancel_reaps() {
        let dir = tempfile::tempdir().unwrap();
        let ops = HookOps::default();
        let params = serde_json::json!({
            "hookId": "h", "hookName": "slow", "command": SLEEP_30,
            "sessionId": "s", "userPrompt": "", "operationId": "op-1"
        });
        // Both futures run on this one task via join! (no LocalSet needed):
        // the canceller sleeps past the child spawn, fires an unknown cancel
        // (warn no-op), then the real one; respond_execute's internal select
        // wakes on the oneshot and drops the child (kill_on_drop reaps).
        let start = std::time::Instant::now();
        let (resp, ()) = tokio::join!(respond_execute(&params, dir.path(), &ops), async {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            ops.cancel("does-not-exist");
            ops.cancel("op-1");
        });
        assert!(
            start.elapsed() < std::time::Duration::from_secs(5),
            "cancel returns promptly, not after the 30s sleep"
        );
        let reply: serde_json::Value = serde_json::from_str(resp.unwrap().0.get()).unwrap();
        assert_eq!(reply["cancelled"], true);
    }

    // cyril-tpfd claim 8: zero sessionStart hooks → `{results: []}`, byte-
    // compatible with the pre-execution stub. A non-array or an error reply
    // would break the turn's sessionStart phase.
    #[tokio::test]
    async fn session_start_acknowledges_empty_results() {
        let dir = tempfile::tempdir().unwrap();
        let reg = HookRegistry { hooks: Vec::new() };
        let resp = respond_session_start(&reg, dir.path()).await.unwrap();
        let reply: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        assert_eq!(reply["results"], serde_json::json!([]));
    }

    fn session_start_def(id: &str, command: &str) -> HookDef {
        HookDef {
            id: id.to_string(),
            name: format!("{id}-name"),
            wire_trigger: "sessionStart",
            matcher: None,
            command: command.to_string(),
            timeout: None,
        }
    }

    fn completed(stdout: &str, stderr: &str, exit_code: i32) -> HookRunOutcome {
        HookRunOutcome::Completed {
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            exit_code,
        }
    }

    // cyril-tpfd claim 3: element key-set and identity fields match the
    // carved AcpPrecomputedHookResult exactly (the live-accepted element,
    // .cyril-tpfd/live-results/result-shaped.json); originalType is the
    // literal "runCommand" — anything else throws assertNever agent-side.
    #[test]
    fn session_start_element_shape_matches_carve() {
        let d = session_start_def("f:greet", "unused");
        let els = package_session_start_results(vec![(&d, completed("hi\n", "", 0))]);
        assert_eq!(els.len(), 1);
        let obj = els[0].as_object().unwrap();
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["content", "hookId", "id", "name", "originalType"]);
        assert_eq!(els[0]["id"], "f:greet");
        assert_eq!(els[0]["hookId"], "f:greet");
        assert_eq!(els[0]["name"], "f:greet-name");
        assert_eq!(els[0]["originalType"], "runCommand");
        assert_eq!(els[0]["content"], "hi\n");
    }

    // cyril-tpfd claim 4: content is stdout-ELSE-stderr, never combined —
    // an executeHook-style combined packaging fails this.
    #[test]
    fn session_start_content_stdout_precedence() {
        let d = session_start_def("f:both", "unused");
        let els = package_session_start_results(vec![(&d, completed("out\n", "err\n", 0))]);
        assert_eq!(els[0]["content"], "out\n");
    }

    // cyril-tpfd claim 5 (D1 parity): a non-zero exit with output is still
    // included — an exit-code filter fails this.
    #[test]
    fn session_start_nonzero_exit_still_included() {
        let d = session_start_def("f:warn", "unused");
        let els = package_session_start_results(vec![(&d, completed("", "boom\n", 3))]);
        assert_eq!(els.len(), 1);
        assert_eq!(els[0]["content"], "boom\n");
    }

    // cyril-tpfd claims 2+6 (pure half): mixed outcomes — empty output,
    // spawn-fail, and timeout are skipped; survivors appear in run order.
    #[test]
    fn session_start_packaging_skips_and_orders() {
        let a = session_start_def("f:a", "unused");
        let b = session_start_def("f:b", "unused");
        let c = session_start_def("f:c", "unused");
        let d = session_start_def("f:d", "unused");
        let e = session_start_def("f:e", "unused");
        let els = package_session_start_results(vec![
            (&a, completed("first\n", "ignored\n", 0)),
            (&b, completed("", "", 0)),
            (
                &c,
                HookRunOutcome::SpawnFailed {
                    message: "no shell".into(),
                },
            ),
            (&d, HookRunOutcome::TimedOut),
            (&e, completed("", "last\n", 1)),
        ]);
        let contents: Vec<&str> = els.iter().map(|x| x["content"].as_str().unwrap()).collect();
        assert_eq!(contents, ["first\n", "last\n"]);
        let ids: Vec<&str> = els.iter().map(|x| x["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["f:a", "f:e"]);
    }

    // cyril-tpfd claim 2 (executed half): two REAL hooks run and their reply
    // order equals within-file registry order — a concurrent-join or
    // collection reorder fails this. (The pure-packer half of claim 2 is
    // session_start_packaging_skips_and_orders.)
    #[cfg(unix)]
    #[tokio::test]
    async fn session_start_results_in_registry_order() {
        let ws = tempfile::tempdir().unwrap();
        write(
            &ws.path().join(".kiro/hooks"),
            "ord.json",
            r#"{"version":"v1","hooks":[
                {"name":"first","trigger":"SessionStart",
                 "action":{"type":"command","command":"echo A"}},
                {"name":"second","trigger":"SessionStart",
                 "action":{"type":"command","command":"echo B"}}
            ]}"#,
        );
        let reg = HookRegistry::load(ws.path(), None);
        let resp = respond_session_start(&reg, ws.path()).await.unwrap();
        let reply: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        let contents: Vec<&str> = reply["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["content"].as_str().unwrap())
            .collect();
        assert_eq!(contents, ["A\n", "B\n"]);
    }

    // cyril-tpfd claim 1: only sessionStart-trigger hooks run — a hook
    // registered for another trigger leaves no marker and no element. POSIX
    // commands; meaningful only on Unix (Windows executor path is fenced by
    // execute_hook_real_exit_codes_windows + session_start_executes_on_windows).
    #[cfg(unix)]
    #[tokio::test]
    async fn session_start_runs_only_session_start_hooks() {
        let ws = tempfile::tempdir().unwrap();
        let ss = ws.path().join("ss-marker");
        let pre = ws.path().join("pre-marker");
        write(
            &ws.path().join(".kiro/hooks"),
            "m.json",
            &format!(
                r#"{{"version":"v1","hooks":[
                    {{"name":"ss","trigger":"SessionStart",
                     "action":{{"type":"command","command":"touch {} && echo ss-ran"}}}},
                    {{"name":"pre","trigger":"PreToolUse",
                     "action":{{"type":"command","command":"touch {} && echo pre-ran"}}}}
                ]}}"#,
                ss.display(),
                pre.display()
            ),
        );
        let reg = HookRegistry::load(ws.path(), None);
        let resp = respond_session_start(&reg, ws.path()).await.unwrap();
        let reply: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        assert!(ss.exists(), "sessionStart hook ran");
        assert!(
            !pre.exists(),
            "preToolUse hook must not run at sessionStart"
        );
        assert_eq!(reply["results"].as_array().unwrap().len(), 1);
        assert_eq!(reply["results"][0]["content"], "ss-ran\n");
    }

    // cyril-tpfd claims 6+7 (integration): a 1s-timeout sleeper is skipped
    // fast (seconds, not millis — a millis misread would also cut the 2s
    // `sleep 1 && echo ok` hook), empty output is skipped, and the healthy
    // hooks still land in a well-formed reply.
    #[cfg(unix)]
    #[tokio::test]
    async fn session_start_skips_empty_and_timeout() {
        let ws = tempfile::tempdir().unwrap();
        write(
            &ws.path().join(".kiro/hooks"),
            "mix.json",
            r#"{"version":"v1","hooks":[
                {"name":"silent","trigger":"SessionStart",
                 "action":{"type":"command","command":"true"}},
                {"name":"stuck","trigger":"SessionStart",
                 "action":{"type":"command","command":"sleep 30","timeout":1}},
                {"name":"slowok","trigger":"SessionStart",
                 "action":{"type":"command","command":"sleep 1 && echo ok","timeout":2}}
            ]}"#,
        );
        let reg = HookRegistry::load(ws.path(), None);
        let start = std::time::Instant::now();
        let resp = respond_session_start(&reg, ws.path()).await.unwrap();
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "stuck hook was killed at its 1s timeout, not awaited for 30s"
        );
        let reply: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        let results = reply["results"].as_array().unwrap();
        assert_eq!(results.len(), 1, "silent+stuck skipped, slowok included");
        assert_eq!(results[0]["content"], "ok\n");
        assert_eq!(results[0]["name"], "slowok");
    }

    // cyril-tpfd claim 9: USER_PROMPT is present-but-empty. `printenv VAR`
    // exits 1 with no output when VAR is UNSET (element absent) and prints
    // an empty line when set-empty — so this fence distinguishes the two.
    #[cfg(unix)]
    #[tokio::test]
    async fn session_start_user_prompt_env_empty() {
        let ws = tempfile::tempdir().unwrap();
        write(
            &ws.path().join(".kiro/hooks"),
            "env.json",
            r#"{"version":"v1","hooks":[
                {"name":"envcheck","trigger":"SessionStart",
                 "action":{"type":"command","command":"printenv USER_PROMPT && echo SET"}}
            ]}"#,
        );
        let reg = HookRegistry::load(ws.path(), None);
        let resp = respond_session_start(&reg, ws.path()).await.unwrap();
        let reply: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        let results = reply["results"].as_array().unwrap();
        assert_eq!(
            results.len(),
            1,
            "USER_PROMPT must be set (empty), not unset"
        );
        assert_eq!(results[0]["content"], "\nSET\n");
    }

    // cyril-tpfd claim 10: sessionStart execution is a non-blocking future —
    // a concurrent future on the same task resolves while a 3s hook runs. A
    // blocking executor (std::process inside the async fn) starves the task
    // and fails this. Responder-level with an explicit registry for
    // determinism (KiroClient::new would merge ~/.kiro/hooks — a dev
    // machine's real hooks must not run in tests); ext_method-level
    // concurrency for hook responders is already fenced by
    // client::slow_hook_does_not_block_loop.
    #[cfg(unix)]
    #[tokio::test]
    async fn slow_session_start_does_not_block_loop() {
        let ws = tempfile::tempdir().unwrap();
        write(
            &ws.path().join(".kiro/hooks"),
            "slow.json",
            r#"{"version":"v1","hooks":[
                {"name":"slow","trigger":"SessionStart",
                 "action":{"type":"command","command":"sleep 3 && echo done"}}
            ]}"#,
        );
        let reg = HookRegistry::load(ws.path(), None);
        let start = std::time::Instant::now();
        // Timing captured at RESOLUTION of the cheap future — measured after
        // join! it would always include the hook's 3s (the jiyn P2 bug class).
        let (resp, cheap_elapsed) = tokio::join!(respond_session_start(&reg, ws.path()), async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            start.elapsed()
        });
        assert!(resp.is_ok());
        assert!(
            cheap_elapsed < std::time::Duration::from_secs(2),
            "concurrent future must resolve while the 3s sessionStart hook runs: {cheap_elapsed:?}"
        );
    }

    // Windows counterpart: sessionStart execution works through cmd /C and
    // packages output (the platform-shell regression class).
    #[cfg(windows)]
    #[tokio::test]
    async fn session_start_executes_on_windows() {
        let ws = tempfile::tempdir().unwrap();
        write(
            &ws.path().join(".kiro/hooks"),
            "w.json",
            r#"{"version":"v1","hooks":[
                {"name":"hello","trigger":"SessionStart",
                 "action":{"type":"command","command":"echo ok"}}
            ]}"#,
        );
        let reg = HookRegistry::load(ws.path(), None);
        let resp = respond_session_start(&reg, ws.path()).await.unwrap();
        let reply: serde_json::Value = serde_json::from_str(resp.0.get()).unwrap();
        let results = reply["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["content"], "ok\r\n");
    }
}
