//! The review recipe's inputs, validated before a workflow exists: every value
//! the recipe places inside a shell's double quotes must be safe there.

use super::CrtoolPrefix;
use super::policy::input_problem;
use serde_json::{Map, Value};
use std::path::Path;

/// What the verifiers are told when the repository names no authorities.
pub const CONTEXT_FALLBACK: &str =
    "No extra context was provided; rely on manifest.json `change_docs`.";

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
#[error("cannot start the review: {0}")]
pub struct InputProblem(String);

/// `{rundir, target, scope, context, crtool}` for `_kiro/workflow/new`.
/// `rundir` is absolute with forward slashes; `scope` is the pathspecs joined
/// with spaces; an empty `context` becomes [`CONTEXT_FALLBACK`].
pub fn recipe_inputs(
    run_dir: &Path,
    target: &str,
    scope: &[String],
    context: Option<&str>,
    crtool: &CrtoolPrefix,
) -> Result<Map<String, Value>, InputProblem> {
    let rundir = forward_slashes(run_dir);
    let scope = scope.join(" ");
    for (name, value) in [
        ("rundir", rundir.as_str()),
        ("target", target),
        ("scope", scope.as_str()),
    ] {
        if let Some(problem) = input_problem(name, value) {
            return Err(InputProblem(problem));
        }
    }
    let context = context
        .map(str::trim)
        .filter(|context| !context.is_empty())
        .unwrap_or(CONTEXT_FALLBACK);
    let mut inputs = Map::new();
    inputs.insert("rundir".to_owned(), Value::from(rundir));
    inputs.insert("target".to_owned(), Value::from(target));
    inputs.insert("scope".to_owned(), Value::from(scope));
    inputs.insert("context".to_owned(), Value::from(context));
    inputs.insert("crtool".to_owned(), Value::from(crtool.as_str()));
    Ok(inputs)
}

/// The path with `/` separators: bash, pwsh, cmd and node all accept `C:/x`.
/// Only Windows separators are converted; on Unix a backslash is part of a
/// name (and then refused by [`input_problem`]).
pub fn forward_slashes(path: &Path) -> String {
    let path = path.to_string_lossy();
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::ShellDialect;

    fn prefix() -> CrtoolPrefix {
        let exe = if cfg!(windows) {
            "C:/bin/cyril.exe"
        } else {
            "/bin/cyril"
        };
        CrtoolPrefix::from_executable(Path::new(exe), ShellDialect::Posix).expect("a valid prefix")
    }

    #[test]
    fn inputs_carry_every_recipe_field_with_the_context_fallback() {
        let inputs = recipe_inputs(
            Path::new("/w/.code-review/r"),
            "auto",
            &["src".to_owned(), "docs".to_owned()],
            Some("  "),
            &prefix(),
        )
        .expect("valid inputs");
        assert_eq!(inputs["rundir"], "/w/.code-review/r");
        assert_eq!(inputs["target"], "auto");
        assert_eq!(inputs["scope"], "src docs");
        assert_eq!(inputs["context"], CONTEXT_FALLBACK);
        assert_eq!(inputs["crtool"], prefix().as_str());
    }

    #[test]
    fn unsafe_values_are_refused_before_any_workflow_exists() {
        let refused = recipe_inputs(Path::new("/w/r"), "a$b", &[".".to_owned()], None, &prefix());
        assert!(
            matches!(refused, Err(InputProblem(problem)) if problem.starts_with("target contains"))
        );
    }
}
