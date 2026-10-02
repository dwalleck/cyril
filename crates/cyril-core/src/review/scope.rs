//! The Review scope the form offers: the top-level directories (and root
//! files) a target's diff touches, as checkboxes. A configured or typed scope
//! sets the preselection; the checked paths are the pathspecs the probe,
//! gather, the workflow and `run.json` all receive.

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

/// The choices for `touched` files. Without a preset every touched group is
/// checked. With one (`[review] scope` or `/review -- <paths>`) its paths are
/// checked — kept even when they cover nothing now — and the touched groups
/// they leave out are offered unchecked. Root files are their own choices,
/// never dropped.
pub fn choices(touched: &[String], preset: Option<&[String]>) -> Vec<PathChoice> {
    let mut groups: Vec<String> = Vec::new();
    for file in touched {
        let group = file.split_once('/').map_or(file.as_str(), |(top, _)| top);
        if !groups.iter().any(|known| known == group) {
            groups.push(group.to_owned());
        }
    }
    groups.sort();
    let count = |path: &str| touched.iter().filter(|file| covers(path, file)).count();
    match preset {
        None => groups
            .into_iter()
            .map(|path| PathChoice {
                files: count(&path),
                path,
                checked: true,
            })
            .collect(),
        Some(preset) => {
            let mut choices: Vec<PathChoice> = preset
                .iter()
                .map(|path| PathChoice {
                    files: count(path),
                    path: path.clone(),
                    checked: true,
                })
                .collect();
            choices.extend(
                groups
                    .into_iter()
                    .filter(|group| {
                        !preset
                            .iter()
                            .any(|path| covers(path, group) || covers(group, path))
                    })
                    .map(|path| PathChoice {
                        files: count(&path),
                        path,
                        checked: false,
                    }),
            );
            choices
        }
    }
}

/// The choices for a new target, keeping every path's checked state; paths
/// that are new are checked only when no preset governs the form.
pub fn rebuild(
    previous: &[PathChoice],
    touched: &[String],
    preset: Option<&[String]>,
) -> Vec<PathChoice> {
    let kept: Vec<String> = previous
        .iter()
        .filter(|choice| choice.checked)
        .map(|choice| choice.path.clone())
        .collect();
    let mut rebuilt = choices(touched, Some(&kept));
    for choice in &mut rebuilt {
        let was = previous.iter().find(|old| old.path == choice.path);
        choice.checked = match was {
            Some(old) => old.checked,
            None => preset.is_none(),
        };
    }
    // A checked path the new target no longer touches stays offered (and
    // counts 0) so the operator sees it; an unchecked one is dropped.
    rebuilt.retain(|choice| choice.checked || choice.files > 0);
    rebuilt
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

    fn shown(choices: &[PathChoice]) -> Vec<(&str, usize, bool)> {
        choices
            .iter()
            .map(|choice| (choice.path.as_str(), choice.files, choice.checked))
            .collect()
    }

    #[test]
    fn touched_groups_are_all_checked_and_root_files_kept() {
        let touched = files(&["crates/a.rs", "crates/b/c.rs", "docs/x.md", "README.md"]);
        let choices = choices(&touched, None);
        assert_eq!(
            shown(&choices),
            [
                ("README.md", 1, true),
                ("crates", 2, true),
                ("docs", 1, true),
            ]
        );
        assert_eq!(count(&choices, &touched), 4);
    }

    #[test]
    fn a_preset_checks_its_paths_and_offers_the_rest_unchecked() {
        let touched = files(&["crates/a.rs", "crates/b/c.rs", "docs/x.md", "README.md"]);
        let preset = files(&["crates/b", "ops"]);
        let choices = choices(&touched, Some(&preset));
        assert_eq!(
            shown(&choices),
            [
                ("crates/b", 1, true),
                ("ops", 0, true),
                ("README.md", 1, false),
                ("docs", 1, false),
            ]
        );
        assert_eq!(selected(&choices), ["crates/b", "ops"]);
        assert_eq!(count(&choices, &touched), 1);
    }

    #[test]
    fn rebuilding_keeps_what_the_operator_chose() {
        let first = files(&["crates/a.rs", "docs/x.md"]);
        let mut choices = choices(&first, None);
        choices[1].checked = false; // docs
        let second = files(&["crates/a.rs", "docs/y.md", "ops/z.yml"]);
        let rebuilt = rebuild(&choices, &second, None);
        assert_eq!(
            shown(&rebuilt),
            [("crates", 1, true), ("docs", 1, false), ("ops", 1, true)]
        );
    }

    #[test]
    fn covers_matches_whole_components_only() {
        assert!(covers("crates", "crates/a.rs"));
        assert!(covers("README.md", "README.md"));
        assert!(!covers("crate", "crates/a.rs"));
        assert!(covers(".", "anything"));
    }
}
