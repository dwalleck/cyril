//! What an armed review run's step sessions may do: read inside the
//! workspace, write under the run directory, and run this run's own crtool
//! steps. Everything else is denied, never prompted. A port, by construction,
//! of `experiments/code-review-workflow/review_policy.py`; both run the shared
//! vectors in `tests/fixtures/review_policy_vectors.json`.

use super::consent::{Capability, PermissionConsent};
use std::path::{Component, Path, PathBuf};

/// The crtool subcommands a workflow step may run. The check command runs an
/// arbitrary program, so it is never a step.
pub const STEP_SUBCOMMANDS: [&str; 8] = [
    "gather", "merge", "shard", "facts", "ballots", "collate", "finalize", "comments",
];

/// Quotes PowerShell reads as `"`: a value holding one ends its argument early.
const SMART_QUOTES: [char; 3] = ['\u{201c}', '\u{201d}', '\u{201e}'];

/// What one armed run allows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyScope {
    workspace_root: PathBuf,
    run_dir: PathBuf,
    crtool_prefix: String,
}

impl PolicyScope {
    pub fn new(workspace_root: PathBuf, run_dir: PathBuf, crtool_prefix: String) -> Self {
        Self {
            workspace_root,
            run_dir,
            crtool_prefix,
        }
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    pub fn crtool_prefix(&self) -> &str {
        &self.crtool_prefix
    }
}

/// Allow or deny, and why (the reason a denial is logged with).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decision {
    allow: bool,
    reason: String,
}

impl Decision {
    fn new(allow: bool, reason: String) -> Self {
        Self { allow, reason }
    }

    pub(crate) fn denied(reason: String) -> Self {
        Self::new(false, reason)
    }

    pub fn allowed(&self) -> bool {
        self.allow
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// Decide one permission request of an armed run's step session.
pub fn decide(consent: &PermissionConsent, scope: &PolicyScope) -> Decision {
    let resource = consent.resource().unwrap_or_default();
    let base = consent.workspace_root().unwrap_or(&scope.workspace_root);
    let full = || base.join(resource);
    let tool = consent.tool_id().unwrap_or_default();
    match consent.capability() {
        // KAS asks an implicit fs_read for directory listings and for a shell
        // command's cwd; reading the workspace is the whole job.
        Capability::FsRead => Decision::new(
            under(&full(), &scope.workspace_root),
            format!("read {resource}"),
        ),
        Capability::FsWrite | Capability::StrReplace => {
            Decision::new(under(&full(), &scope.run_dir), format!("write {resource}"))
        }
        _ if matches!(tool, "fs_write" | "str_replace") => {
            Decision::new(under(&full(), &scope.run_dir), format!("write {resource}"))
        }
        Capability::Shell => shell(consent, scope),
        _ if tool == "execute_bash" => shell(consent, scope),
        Capability::Unrecognized(name) => Decision::new(
            false,
            format!("unhandled capability {name:?} {}", truncate(resource, 120)),
        ),
    }
}

/// EVERY place the request carries a command must hold an allowed call:
/// whichever one KAS actually runs is then allowed. A title is display text,
/// never checked.
fn shell(consent: &PermissionConsent, scope: &PolicyScope) -> Decision {
    let commands: Vec<&str> = [consent.resource(), consent.command()]
        .into_iter()
        .flatten()
        .chain(consent.raw_commands().iter().map(String::as_str))
        .filter(|command| !command.trim().is_empty())
        .collect();
    let Some(first) = commands.first() else {
        return Decision::new(
            false,
            "shell request carries no command to check".to_owned(),
        );
    };
    for command in &commands {
        if let Some(problem) = crtool_call_problem(command, scope) {
            return Decision::new(
                false,
                format!("shell {} ({problem})", truncate(command, 160)),
            );
        }
    }
    Decision::new(true, format!("shell {}", truncate(first, 160)))
}

/// Why a shell command is not exactly a workflow step's crtool call, or None.
///
/// Allowed: an optional `cd <workspace> && `, then this run's crtool prefix,
/// then a step subcommand whose first argument is the run directory, then
/// plain or safely double-quoted words. Checked by construction (what runs),
/// not by looking for dangerous characters, so no shell syntax slips past.
pub fn crtool_call_problem(command: &str, scope: &PolicyScope) -> Option<String> {
    let mut body = command.trim_matches([' ', '\t']);
    if let Some((target, rest)) = cd_prefix(body) {
        let lands_in_workspace = split_args(target)
            .filter(|words| words.len() == 1)
            .is_some_and(|words| {
                same_path(&scope.workspace_root.join(&words[0]), &scope.workspace_root)
            });
        if !lands_in_workspace {
            return Some(format!(
                "cd to somewhere other than the workspace: {}",
                truncate(target, 80)
            ));
        }
        body = rest;
    }
    let Some(arguments) = body
        .strip_prefix(scope.crtool_prefix.as_str())
        .and_then(|rest| rest.strip_prefix(' '))
    else {
        return Some(format!(
            "not the crtool command this run was started with ({})",
            scope.crtool_prefix
        ));
    };
    let Some(words) = split_args(arguments) else {
        return Some("an argument is not a plain word or a safely double-quoted string".to_owned());
    };
    match words.first() {
        Some(step) if STEP_SUBCOMMANDS.contains(&step.as_str()) => {}
        other => {
            return Some(format!(
                "crtool subcommand {:?} is not one a step runs",
                other.map_or("(none)", String::as_str)
            ));
        }
    }
    let in_run = words
        .get(1)
        .is_some_and(|dir| same_path(&scope.workspace_root.join(dir), &scope.run_dir));
    (!in_run).then(|| "the first argument is not this run's directory".to_owned())
}

/// `cd <target> && ` at the start: (target text, rest of the command).
fn cd_prefix(body: &str) -> Option<(&str, &str)> {
    let rest = body.strip_prefix("cd")?;
    let after_gap = rest.trim_start_matches([' ', '\t']);
    if after_gap.len() == rest.len() {
        return None;
    }
    let target_len = if let Some(quoted) = after_gap.strip_prefix('"') {
        quoted.find('"')? + 2
    } else {
        after_gap.find([' ', '\t']).unwrap_or(after_gap.len())
    };
    let (target, rest) = after_gap.split_at(target_len);
    let rest_trimmed = rest.trim_start_matches([' ', '\t']);
    if rest_trimmed.len() == rest.len() {
        return None;
    }
    let rest = rest_trimmed.strip_prefix("&&")?;
    let command = rest.trim_start_matches([' ', '\t']);
    (command.len() < rest.len()).then_some((target, command))
}

/// The argument list a shell would build from `text`, or None when any part
/// is not a plain word or a safely double-quoted string. Nothing that could
/// redirect, chain, substitute or continue onto another line gets through.
pub fn split_args(text: &str) -> Option<Vec<String>> {
    let text = text.trim_matches([' ', '\t']);
    let chars: Vec<char> = text.chars().collect();
    let mut words = Vec::new();
    let mut position = 0;
    while position < chars.len() {
        let (word, end) = if chars[position] == '"' {
            quoted(&chars, position)?
        } else {
            bare(&chars, position)?
        };
        words.push(word);
        position = end;
        if position < chars.len() {
            // `"a"b` or `a"b"`: one word to the shell, two here.
            if !matches!(chars[position], ' ' | '\t') {
                return None;
            }
            while position < chars.len() && matches!(chars[position], ' ' | '\t') {
                position += 1;
            }
        }
    }
    Some(words)
}

/// A double-quoted string: no quote of any kind, backtick or control
/// character; a backslash only when not right before the closing quote (kept
/// for Windows paths); `$` only before `/`, as in `//wsl$/Ubuntu/...`.
fn quoted(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut word = String::new();
    let mut position = start + 1;
    loop {
        let character = *chars.get(position)?;
        match character {
            '"' => return Some((word, position + 1)),
            '`' => return None,
            '\\' if chars.get(position + 1) == Some(&'"') => return None,
            '$' if chars.get(position + 1) != Some(&'/') => return None,
            c if (c as u32) < 0x20 || SMART_QUOTES.contains(&c) => return None,
            c => word.push(c),
        }
        position += 1;
    }
}

/// A bare word: letters, digits and `_./:=+-`.
fn bare(chars: &[char], start: usize) -> Option<(String, usize)> {
    let end = chars[start..]
        .iter()
        .position(|c| !(c.is_ascii_alphanumeric() || "_./:=+-".contains(*c)))
        .map_or(chars.len(), |offset| start + offset);
    (end > start).then(|| (chars[start..end].iter().collect(), end))
}

/// Why a value cannot be placed inside the recipe's double quotes, or None.
/// Safe in bash and in both PowerShells: no quote of any kind, no backtick,
/// no backslash (bash escapes with it; Windows PowerShell turns a trailing
/// `\"` into an escaped quote), no control character, and `$` only before `/`.
pub fn input_problem(name: &str, value: &str) -> Option<String> {
    if value.is_empty() {
        return Some(format!(
            "{name} is empty (Windows PowerShell 5.1 drops an empty quoted argument)"
        ));
    }
    let chars: Vec<char> = value.chars().collect();
    let unsafe_at = chars.iter().enumerate().any(|(index, c)| match c {
        '"' | '`' | '\\' => true,
        '$' => chars.get(index + 1) != Some(&'/'),
        c => (*c as u32) < 0x20 || SMART_QUOTES.contains(c),
    });
    unsafe_at.then(|| {
        format!(
            "{name} contains a quote, a backtick, a backslash, a control character or a `$` that a shell would expand (use forward slashes): {value:?}"
        )
    })
}

/// Whether `path` lies inside `root` (or is it).
pub fn under(path: &Path, root: &Path) -> bool {
    match (resolve(path), resolve(root)) {
        (Some(path), Some(root)) => path.starts_with(root),
        _ => false,
    }
}

fn same_path(path: &Path, target: &Path) -> bool {
    matches!((resolve(path), resolve(target)), (Some(a), Some(b)) if a == b)
}

/// The path with its nearest existing ancestor canonicalized (symlinks
/// resolved) and the rest appended, compared case-insensitively on Windows.
/// A `..` that would still be left over makes the path unresolvable: it is
/// never treated as inside anything.
fn resolve(path: &Path) -> Option<PathBuf> {
    let mut existing = path.to_path_buf();
    let mut rest = Vec::new();
    let canonical = loop {
        match existing.canonicalize() {
            Ok(canonical) => break canonical,
            Err(_) => {
                rest.push(existing.file_name()?.to_owned());
                existing = existing.parent()?.to_path_buf();
            }
        }
    };
    let mut resolved = canonical;
    for part in rest.iter().rev() {
        match Path::new(part).components().next() {
            Some(Component::Normal(_)) => resolved.push(part),
            _ => return None,
        }
    }
    Some(normalize_case(resolved))
}

#[cfg(windows)]
fn normalize_case(path: PathBuf) -> PathBuf {
    PathBuf::from(path.to_string_lossy().to_lowercase())
}

#[cfg(not(windows))]
fn normalize_case(path: PathBuf) -> PathBuf {
    path
}

fn truncate(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_args_accepts_plain_and_safely_quoted_words_only() {
        assert_eq!(
            split_args(r#"merge "/r/x" --expect "a,b""#),
            Some(vec![
                "merge".into(),
                "/r/x".into(),
                "--expect".into(),
                "a,b".into()
            ])
        );
        assert_eq!(split_args(r#""C:\w\r""#), Some(vec![r"C:\w\r".into()]));
        assert_eq!(
            split_args(r#""//wsl$/Ubuntu/r""#),
            Some(vec!["//wsl$/Ubuntu/r".into()])
        );
        for bad in [
            r#""a"b"#,
            r#"a"b""#,
            r#""a\""#,
            r#""$(x)""#,
            "a;b",
            "a > b",
            "\"a\nb\"",
            "\"a`b\"",
            "a|b",
        ] {
            assert_eq!(split_args(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn input_problems_match_the_driver() {
        assert_eq!(input_problem("rundir", "C:/w/r"), None);
        assert_eq!(input_problem("rundir", "//wsl$/Ubuntu/home/u/r"), None);
        for bad in [
            "",
            "a$b",
            "a$",
            "a`b",
            "a\"b",
            "a\u{201d}b",
            "C:\\w",
            "a\nb",
        ] {
            assert!(input_problem("x", bad).is_some(), "{bad:?}");
        }
    }

    #[test]
    fn cd_prefix_needs_spaces_and_the_and_operator() {
        assert_eq!(cd_prefix(r#"cd "/w x" && run"#), Some((r#""/w x""#, "run")));
        assert_eq!(cd_prefix("cd /w && run"), Some(("/w", "run")));
        assert_eq!(cd_prefix("cd /w; run"), None);
        assert_eq!(cd_prefix("cd/w && run"), None);
    }
}
