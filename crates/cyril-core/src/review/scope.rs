//! The Review scope the form offers: the top-level directories (and root
//! files) a target's diff touches, as checkboxes. A configured or typed scope
//! sets the preselection; the checked paths are the pathspecs the probe,
//! gather, the workflow and `run.json` all receive.

use std::collections::BTreeMap;

/// One checkbox: a pathspec and how many touched files it covers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathChoice {
    pub path: String,
    pub files: usize,
    pub checked: bool,
}

/// Whether pathspec `path` covers repository-relative `file`.
pub fn covers(path: &str, file: &str) -> bool {
    path == "."
        || file == path
        || file
            .strip_prefix(path)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The form's choices, and the touched paths that cannot be offered.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Choices {
    pub paths: Vec<PathChoice>,
    /// Touched groups no pathspec can name safely (spaces, shell characters):
    /// left out of the review, and said so.
    pub unusable: Vec<String>,
}

/// The choices for `touched` files, recomputed from scratch on every change
/// so the order is stable: preset paths first, then the touched groups in
/// name order.
///
/// - Without a preset every touched group (a top-level directory, or a root
///   file) is checked. With one (`[review] scope`, `/review -- <paths>`) its
///   paths are checked — kept even when they cover nothing now — and the
///   other touched groups are offered unchecked; a group a preset path lies
///   inside is offered one level down, so its other files stay selectable.
/// - `overrides` are the operator's own toggles and win over both; a path
///   checked by the operator that the target no longer touches stays shown.
/// - A group `usable` rejects is not offered.
pub fn choices(
    touched: &[String],
    preset: Option<&[String]>,
    overrides: &BTreeMap<String, bool>,
    usable: impl Fn(&str) -> bool,
) -> Choices {
    let preset = preset.unwrap_or_default();
    let mut groups: Vec<String> = Vec::new();
    for file in touched {
        if preset.iter().any(|path| covers(path, file)) {
            continue;
        }
        let mut parts = file.splitn(3, '/');
        let top = parts.next().unwrap_or(file.as_str());
        let group = match (parts.next(), parts.next()) {
            (None, _) => file.clone(),
            (Some(second), rest) if preset.iter().any(|path| covers(top, path)) => match rest {
                Some(_) => format!("{top}/{second}"),
                None => file.clone(),
            },
            _ => top.to_owned(),
        };
        if !groups.contains(&group) {
            groups.push(group);
        }
    }
    groups.sort();
    let count = |path: &str| touched.iter().filter(|file| covers(path, file)).count();
    let default_checked = preset.is_empty();
    let mut paths: Vec<PathChoice> = preset
        .iter()
        .map(|path| (path.clone(), true))
        .chain(groups.into_iter().map(|group| (group, default_checked)))
        .map(|(path, checked)| PathChoice {
            files: count(&path),
            checked: overrides.get(&path).copied().unwrap_or(checked),
            path,
        })
        .collect();
    for (path, checked) in overrides {
        if *checked && !paths.iter().any(|choice| &choice.path == path) {
            paths.push(PathChoice {
                files: count(path),
                path: path.clone(),
                checked: true,
            });
        }
    }
    let (paths, unusable): (Vec<_>, Vec<_>) =
        paths.into_iter().partition(|choice| usable(&choice.path));
    Choices {
        paths,
        unusable: unusable.into_iter().map(|choice| choice.path).collect(),
    }
}

/// The checked pathspecs.
pub fn selected(choices: &[PathChoice]) -> Vec<String> {
    choices
        .iter()
        .filter(|choice| choice.checked)
        .map(|choice| choice.path.clone())
        .collect()
}

/// How many touched files the checked choices cover.
pub fn count(choices: &[PathChoice], touched: &[String]) -> usize {
    touched
        .iter()
        .filter(|file| {
            choices
                .iter()
                .any(|choice| choice.checked && covers(&choice.path, file))
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    fn shown(choices: &Choices) -> Vec<(&str, usize, bool)> {
        choices
            .paths
            .iter()
            .map(|choice| (choice.path.as_str(), choice.files, choice.checked))
            .collect()
    }

    fn plain(touched: &[String], preset: Option<&[String]>) -> Choices {
        choices(touched, preset, &BTreeMap::new(), |_| true)
    }

    #[test]
    fn touched_groups_are_all_checked_and_root_files_kept() {
        let touched = files(&["crates/a.rs", "crates/b/c.rs", "docs/x.md", "README.md"]);
        let offered = plain(&touched, None);
        assert_eq!(
            shown(&offered),
            [
                ("README.md", 1, true),
                ("crates", 2, true),
                ("docs", 1, true)
            ]
        );
        assert_eq!(count(&offered.paths, &touched), 4);
    }

    #[test]
    fn a_preset_checks_its_paths_and_offers_the_rest_one_level_down() {
        let touched = files(&[
            "crates/b/c.rs",
            "crates/d/e.rs",
            "crates/top.rs",
            "docs/x.md",
            "README.md",
        ]);
        let preset = files(&["crates/b", "ops"]);
        let offered = plain(&touched, Some(&preset));
        assert_eq!(
            shown(&offered),
            [
                ("crates/b", 1, true),
                ("ops", 0, true),
                ("README.md", 1, false),
                ("crates/d", 1, false),
                ("crates/top.rs", 1, false),
                ("docs", 1, false),
            ]
        );
        assert_eq!(selected(&offered.paths), ["crates/b", "ops"]);
    }

    #[test]
    fn the_operators_toggles_survive_target_changes() {
        let mut toggles = BTreeMap::new();
        toggles.insert("docs".to_owned(), false);
        // A target that does not touch docs, then one that does again.
        let first = choices(&files(&["crates/a.rs"]), None, &toggles, |_| true);
        assert_eq!(shown(&first), [("crates", 1, true)]);
        let back = choices(
            &files(&["crates/a.rs", "docs/y.md"]),
            None,
            &toggles,
            |_| true,
        );
        assert_eq!(shown(&back), [("crates", 1, true), ("docs", 1, false)]);
    }

    #[test]
    fn unusable_paths_are_named_not_offered() {
        let touched = files(&["Release Notes.md", "src/a.rs"]);
        let offered = choices(&touched, None, &BTreeMap::new(), |path| !path.contains(' '));
        assert_eq!(shown(&offered), [("src", 1, true)]);
        assert_eq!(offered.unusable, ["Release Notes.md"]);
    }

    #[test]
    fn covers_matches_whole_components_only() {
        assert!(covers("crates", "crates/a.rs"));
        assert!(covers("README.md", "README.md"));
        assert!(!covers("crate", "crates/a.rs"));
        assert!(covers(".", "anything"));
    }
}
