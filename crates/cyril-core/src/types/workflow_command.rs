//! Command-plane types for cyril's native `/workflow` family.
//!
//! ADR-0011: cyril drives `_kiro/workflow/*` directly with the workflow gate
//! OFF and owns the control plane natively — the model never gains
//! `run_workflow`. These types carry one user-issued operation from the
//! `/workflow` command through the bridge and its outcome back to the UI.
//!
//! Wire facts (live-verified 2.16.2 and 2.18.0, see `.cyril-0qe6/findings.md`):
//! `cancel` replies `{ok, previousStatus}` while `invoke`/`resume` reply
//! `{workflowId, status}`; `list` entries omit `startedAt` until a run is
//! invoked and `endedAt` until it is terminal.

use std::path::{Path, PathBuf};

use super::session::SessionId;
use super::workflow::{WorkflowId, WorkflowRunStatus, WorkflowSnapshot};

/// One operation of the client-owned workflow control plane, as resolved by
/// the `/workflow` command and executed by the bridge.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowOp {
    /// `/workflow recipes` → `_kiro/workflow/listRecipes`.
    ListRecipes,
    /// `/workflow list` → `_kiro/workflow/list`.
    ListRuns,
    /// `/workflow run <ref> [k=v …]` → `_kiro/workflow/new` then, only on
    /// success, `_kiro/workflow/invoke`.
    Run {
        /// Where the engine finds the recipe.
        target: WorkflowRunTarget,
        /// Run inputs; string-valued in v1 (typed values: cyril-2ibk).
        inputs: serde_json::Map<String, serde_json::Value>,
    },
    /// `/workflow attach <id>` → `_kiro/workflow/inspect` (read-only; the
    /// ownership-taking act is [`WorkflowOp::Resume`]).
    Attach {
        /// The persisted run to read.
        id: WorkflowId,
    },
    /// `/workflow status <id>` → `_kiro/workflow/inspect`. The no-argument
    /// form never reaches the bridge — it renders from the tracker.
    Status {
        /// The persisted run to read.
        id: WorkflowId,
    },
    /// `/workflow cancel <id>` → `_kiro/workflow/cancel`.
    Cancel {
        /// The run to cancel.
        id: WorkflowId,
    },
    /// `/workflow resume <id>` → `_kiro/workflow/resume`. A refusal from a
    /// live foreign owner surfaces verbatim (run-ownership contract).
    Resume {
        /// The run to resume.
        id: WorkflowId,
    },
}

impl WorkflowOp {
    /// Stable operation label used for `BridgeError { operation }` and log
    /// context, so a failure names the user-facing command that caused it.
    pub fn label(&self) -> &'static str {
        match self {
            Self::ListRecipes => "workflow recipes",
            Self::ListRuns => "workflow list",
            Self::Run { .. } => "workflow run",
            Self::Attach { .. } => "workflow attach",
            Self::Status { .. } => "workflow status",
            Self::Cancel { .. } => "workflow cancel",
            Self::Resume { .. } => "workflow resume",
        }
    }
}

/// Where `/workflow run` points the engine, resolved from the user's ref
/// token by [`parse_run_target`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowRunTarget {
    /// A `bundled://` or `generated://` reference, passed verbatim.
    Reference(String),
    /// A filesystem recipe (`.workflow.json`), absolute by construction.
    RecipeFile(PathBuf),
}

impl WorkflowRunTarget {
    /// The string the wire's `workflowPath` parameter carries.
    pub fn as_workflow_path(&self) -> String {
        match self {
            Self::Reference(reference) => reference.clone(),
            // Lossless in practice: the path was built from the user's UTF-8
            // command text in `parse_run_target`.
            Self::RecipeFile(path) => path.to_string_lossy().into_owned(),
        }
    }
}

/// Resolves a `/workflow run` ref token.
///
/// - `bundled://…` / `generated://…` pass verbatim;
/// - a token containing a path separator or ending in `.workflow.json` is a
///   recipe file, absolutized against `workspace_root` when relative;
/// - any other bare word names a bundled recipe (`bundled://<word>`).
///
/// The ref is always the first token after `run` — a `key=value`-shaped
/// first token is treated as a ref, never as an input.
pub fn parse_run_target(workspace_root: &Path, reference: &str) -> WorkflowRunTarget {
    if reference.starts_with("bundled://") || reference.starts_with("generated://") {
        return WorkflowRunTarget::Reference(reference.to_owned());
    }
    if reference.contains('/') || reference.ends_with(".workflow.json") {
        let path = Path::new(reference);
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            workspace_root.join(path)
        };
        return WorkflowRunTarget::RecipeFile(absolute);
    }
    WorkflowRunTarget::Reference(format!("bundled://{reference}"))
}

/// Error for a `/workflow run` input token that is not `key=value`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkflowInputError {
    /// The token has no `=` at all.
    #[error("input `{token}` is not key=value")]
    NotKeyValue {
        /// The offending token, verbatim.
        token: String,
    },
    /// The token starts with `=` (empty key).
    #[error("input `={value}` has an empty key")]
    EmptyKey {
        /// The value that followed the empty key.
        value: String,
    },
}

/// Parses `key=value` input tokens into a JSON inputs map.
///
/// Values are JSON strings in v1 (typed values: cyril-2ibk). The first `=`
/// splits key from value, so values may contain `=`. Duplicate keys
/// last-wins. An empty token list is a valid empty map.
pub fn parse_run_inputs<'token>(
    tokens: impl IntoIterator<Item = &'token str>,
) -> Result<serde_json::Map<String, serde_json::Value>, WorkflowInputError> {
    let mut inputs = serde_json::Map::new();
    for token in tokens {
        let Some((key, value)) = token.split_once('=') else {
            return Err(WorkflowInputError::NotKeyValue {
                token: token.to_owned(),
            });
        };
        if key.is_empty() {
            return Err(WorkflowInputError::EmptyKey {
                value: value.to_owned(),
            });
        }
        inputs.insert(key.to_owned(), serde_json::Value::String(value.to_owned()));
    }
    Ok(inputs)
}

/// One recipe row from `_kiro/workflow/listRecipes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRecipe {
    /// Recipe name (e.g. `ralph`, `autoresearch`).
    pub name: String,
    /// One-line description shipped with the recipe.
    pub description: String,
    /// Where the recipe lives: absent for bundled recipes, the absolute
    /// `.workflow.json` path for workspace recipes.
    pub source: Option<String>,
}

/// One run row from `_kiro/workflow/list`.
///
/// Timestamps are `Option` because the wire genuinely omits them: a
/// never-invoked run has no `startedAt`, a non-terminal run has no
/// `endedAt` (live-observed; no sentinel defaults).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRunSummary {
    /// The run's stable identifier.
    pub workflow_id: WorkflowId,
    /// The workflow name the run was created from.
    pub name: String,
    /// Current run status.
    pub status: WorkflowRunStatus,
    /// RFC 3339 creation time.
    pub created_at: Option<String>,
    /// RFC 3339 last-update time.
    pub updated_at: Option<String>,
    /// RFC 3339 first-invoke time; absent until the run is invoked.
    pub started_at: Option<String>,
    /// RFC 3339 completion time; absent until the run is terminal.
    pub ended_at: Option<String>,
    /// The session that created the run.
    pub parent_session_id: Option<SessionId>,
}

/// Which user verb fetched a run's state — the two verbs share the wire
/// call (`inspect`) and differ only in how the outcome is presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowFetchVerb {
    /// `/workflow attach <id>` — "follow this run from now on".
    Attach,
    /// `/workflow status <id>` — "show me this run".
    Status,
}

/// Outcome of one [`WorkflowOp`], carried by
/// `Notification::WorkflowCommand` for the UI to render.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowCommandOutcome {
    /// `listRecipes` succeeded.
    Recipes(Vec<WorkflowRecipe>),
    /// `list` succeeded.
    Runs(Vec<WorkflowRunSummary>),
    /// `inspect` succeeded; the same snapshot separately seeds the tracker
    /// via `Notification::WorkflowSnapshot` (sent first).
    Fetched {
        /// Which user verb asked.
        verb: WorkflowFetchVerb,
        /// The run state, for display.
        snapshot: Box<WorkflowSnapshot>,
    },
    /// `new` + `invoke` both succeeded.
    Launched {
        /// The freshly minted run.
        workflow_id: WorkflowId,
        /// The workflow name the engine reported.
        name: String,
    },
    /// `cancel` succeeded (`{ok, previousStatus}` reply shape).
    Cancelled {
        /// The cancelled run.
        workflow_id: WorkflowId,
        /// Status the run held before cancellation, when reported.
        previous_status: Option<WorkflowRunStatus>,
    },
    /// `resume` succeeded (`{workflowId, status}` reply shape).
    Resumed {
        /// The resumed run.
        workflow_id: WorkflowId,
        /// Status the engine reported after resuming, when parseable.
        status: Option<WorkflowRunStatus>,
    },
    /// Any operation failed — agent error, transport error, or a reply
    /// cyril could not parse. Always produced instead of silence.
    Failed {
        /// [`WorkflowOp::label`] of the failed operation.
        operation: String,
        /// JSON-RPC error code, when the failure was an agent error.
        code: Option<i64>,
        /// Human-actionable text: the agent's `error.data.details` when
        /// present (verbatim — the run-ownership refusal lives there),
        /// otherwise the best message available.
        details: String,
    },
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::test_support::must_succeed;

    #[test]
    fn scheme_refs_pass_verbatim() {
        let root = Path::new("/ws");
        assert_eq!(
            parse_run_target(root, "bundled://ralph"),
            WorkflowRunTarget::Reference("bundled://ralph".into())
        );
        assert_eq!(
            parse_run_target(root, "generated://gen_a1b2"),
            WorkflowRunTarget::Reference("generated://gen_a1b2".into())
        );
    }

    #[test]
    fn bare_word_becomes_bundled_ref() {
        assert_eq!(
            parse_run_target(Path::new("/ws"), "ralph"),
            WorkflowRunTarget::Reference("bundled://ralph".into())
        );
    }

    #[test]
    fn unicode_bare_word_becomes_bundled_ref() {
        assert_eq!(
            parse_run_target(Path::new("/ws"), "änderung"),
            WorkflowRunTarget::Reference("bundled://änderung".into())
        );
    }

    #[test]
    fn relative_paths_absolutize_against_workspace_root() {
        assert_eq!(
            parse_run_target(Path::new("/ws"), "wf/a.workflow.json"),
            WorkflowRunTarget::RecipeFile("/ws/wf/a.workflow.json".into())
        );
        assert_eq!(
            parse_run_target(Path::new("/ws"), "./wf/a.workflow.json"),
            WorkflowRunTarget::RecipeFile("/ws/./wf/a.workflow.json".into())
        );
    }

    #[test]
    fn workflow_json_suffix_is_a_file_even_without_separator() {
        assert_eq!(
            parse_run_target(Path::new("/ws"), "a.workflow.json"),
            WorkflowRunTarget::RecipeFile("/ws/a.workflow.json".into())
        );
    }

    #[test]
    fn absolute_paths_are_kept() {
        assert_eq!(
            parse_run_target(Path::new("/ws"), "/abs/b.workflow.json"),
            WorkflowRunTarget::RecipeFile("/abs/b.workflow.json".into())
        );
    }

    #[test]
    fn key_value_shaped_ref_is_still_a_ref() {
        // The ref is positional; `a=b` first is a (strange) bundled name,
        // never an input.
        assert_eq!(
            parse_run_target(Path::new("/ws"), "a=b"),
            WorkflowRunTarget::Reference("bundled://a=b".into())
        );
    }

    #[test]
    fn inputs_split_on_first_equals_and_last_wins() {
        let inputs = must_succeed(
            parse_run_inputs(["k=v", "url=https://x/?a=1", "k=w"]),
            "valid inputs",
        );
        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs["k"], serde_json::Value::String("w".into()));
        assert_eq!(
            inputs["url"],
            serde_json::Value::String("https://x/?a=1".into())
        );
    }

    #[test]
    fn empty_token_list_is_an_empty_map() {
        assert_eq!(must_succeed(parse_run_inputs([]), "empty ok").len(), 0);
    }

    #[test]
    fn non_key_value_token_errors() {
        assert_eq!(
            parse_run_inputs(["oops"]),
            Err(WorkflowInputError::NotKeyValue {
                token: "oops".into()
            })
        );
    }

    #[test]
    fn empty_key_errors() {
        assert_eq!(
            parse_run_inputs(["=v"]),
            Err(WorkflowInputError::EmptyKey { value: "v".into() })
        );
    }

    #[test]
    fn op_labels_name_the_user_command() {
        let id = must_succeed(WorkflowId::try_from("wf_1".to_owned()), "non-empty id");
        assert_eq!(WorkflowOp::ListRecipes.label(), "workflow recipes");
        assert_eq!(WorkflowOp::Resume { id }.label(), "workflow resume");
    }

    #[test]
    fn run_target_workflow_path_round_trips() {
        assert_eq!(
            WorkflowRunTarget::Reference("bundled://ralph".into()).as_workflow_path(),
            "bundled://ralph"
        );
        assert_eq!(
            WorkflowRunTarget::RecipeFile("/ws/a.workflow.json".into()).as_workflow_path(),
            "/ws/a.workflow.json"
        );
    }
}
