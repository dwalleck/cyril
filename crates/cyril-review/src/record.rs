//! Agent-written records (candidates, decisions, verdicts) are untrusted JSON.
//! Strings and integers read as agents are told to write them; any other
//! value is printed as its JSON text (docs/crtool-contract.md, deviations).

use crate::run::read_json;
use serde_json::{Map, Value};
use std::path::Path;

pub(crate) type Record = Map<String, Value>;

/// Candidates per finder angle (and in the gap sweep) that are kept.
pub(crate) const MAX_PER_ANGLE: usize = 8;

/// Absent, null or an empty string.
pub(crate) fn blank(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(text)) => text.is_empty(),
        Some(_) => false,
    }
}

/// A value as text: a string as itself, anything else as its JSON text.
pub(crate) fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The value's text with whitespace runs collapsed and `|` (the digest column
/// separator) replaced, cut to `limit` characters with an ellipsis.
pub(crate) fn one_line(value: Option<&Value>, limit: usize) -> String {
    let full = match value {
        Some(value) if !blank(Some(value)) => text(value),
        _ => String::new(),
    };
    let joined = full
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "/");
    if joined.chars().count() <= limit {
        return joined;
    }
    let cut: String = joined.chars().take(limit.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

/// `file:line` as the digests print it, `?` for a part that is missing.
pub(crate) fn location(record: &Record) -> String {
    let part = |key| match record.get(key) {
        None | Some(Value::Null) => "?".to_owned(),
        Some(value) => text(value),
    };
    format!("{}:{}", part("file"), part("line"))
}

/// A line number: an integer, or a string holding one; otherwise null.
pub(crate) fn to_line(value: Option<&Value>) -> Value {
    let line = match value {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.trim().parse().ok(),
        _ => None,
    };
    line.map_or(Value::Null, Value::from)
}

/// The record's field, or `""` when it is absent.
pub(crate) fn field_or_empty(record: &Record, key: &str) -> Value {
    record
        .get(key)
        .cloned()
        .unwrap_or_else(|| Value::String(String::new()))
}

/// The `candidates` crtool itself wrote into `path`: anything but an array of
/// objects means the run file is corrupt, never "no candidates".
pub(crate) fn required_records(file: &Record, path: &Path) -> crate::Result<Vec<Record>> {
    let corrupt = || crate::ReviewError::CorruptRunFile {
        path: path.to_path_buf(),
        message: "`candidates` is not an array of objects",
    };
    let items = file
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or_else(corrupt)?;
    items
        .iter()
        .map(|item| item.as_object().cloned().ok_or_else(corrupt))
        .collect()
}

/// An id that is safe as a file name: letters, digits, `.`, `_` and `-`,
/// never `..`. Candidate ids name ballot and verdict files.
pub(crate) fn plain_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains("..")
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// A finder's candidates file: `{"candidates": [...]}` or a bare list. Returns
/// the object entries and a problem description when something was wrong.
pub(crate) fn load_candidates(path: &Path) -> (Vec<Record>, Option<String>) {
    let data: Value = match read_json(path) {
        Ok(data) => data,
        Err(error) => {
            return (
                Vec::new(),
                Some(format!("unreadable: {}", unreadable(&error))),
            );
        }
    };
    let items = match &data {
        Value::Object(fields) => fields.get("candidates"),
        other => Some(other),
    };
    let Some(Value::Array(items)) = items else {
        return (Vec::new(), Some("no `candidates` array".to_owned()));
    };
    let good: Vec<Record> = items
        .iter()
        .filter_map(|item| item.as_object().cloned())
        .collect();
    let problem = (good.len() != items.len()).then(|| "non-object entries skipped".to_owned());
    (good, problem)
}

/// The cause of a read or parse failure, without the path (the caller names
/// the file).
pub(crate) fn unreadable(error: &crate::ReviewError) -> String {
    match error {
        crate::ReviewError::Io { source, .. } => source.to_string(),
        crate::ReviewError::Json { source, .. } => source.to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn one_line_collapses_whitespace_pipes_and_cuts_with_an_ellipsis() {
        assert_eq!(one_line(Some(&json!(" a \n b | c ")), 20), "a b / c");
        assert_eq!(one_line(Some(&json!("abcdefgh")), 5), "abcd…");
        assert_eq!(one_line(Some(&json!("abc  defgh")), 5), "abc…");
        assert_eq!(one_line(Some(&json!(null)), 5), "");
        assert_eq!(one_line(None, 5), "");
    }

    #[test]
    fn lines_are_integers_or_integer_strings() {
        assert_eq!(to_line(Some(&json!(12))), json!(12));
        assert_eq!(to_line(Some(&json!(" 7 "))), json!(7));
        assert_eq!(to_line(Some(&json!("x"))), json!(null));
        assert_eq!(to_line(Some(&json!(null))), json!(null));
        assert_eq!(to_line(None), json!(null));
    }

    #[test]
    fn only_plain_ids_name_files() {
        for id in ["C01", "S02", "C03.v2", "a-b_c"] {
            assert!(plain_id(id), "{id}");
        }
        for id in ["", "..", "../x", "a/b", "a\\b", "/abs", "C01..v2", "S 1"] {
            assert!(!plain_id(id), "{id}");
        }
    }

    #[test]
    fn locations_mark_missing_parts() {
        let record = |value: Value| value.as_object().cloned().unwrap_or_default();
        assert_eq!(
            location(&record(json!({"file": "a.rs", "line": 3}))),
            "a.rs:3"
        );
        assert_eq!(
            location(&record(json!({"file": "a.rs", "line": null}))),
            "a.rs:?"
        );
        assert_eq!(location(&record(json!({}))), "?:?");
    }
}
