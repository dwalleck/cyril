use std::collections::HashSet;
use std::path::Path;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

/// Owns the cached file list and fuzzy matcher for `@file` autocomplete.
///
/// Load files from `git ls-files` via [`FileCompleter::load`], then use
/// [`FileCompleter::suggest`] to get fuzzy-matched completions.
pub struct FileCompleter {
    files: HashSet<String>,
    file_list: Vec<String>,
}

impl FileCompleter {
    /// Load files from git in the given working directory.
    ///
    /// Returns an empty completer if git is not available or the command fails.
    pub async fn load(cwd: &Path) -> Self {
        match Self::run_git_ls_files(cwd).await {
            Ok(file_list) => {
                tracing::info!(
                    "Loaded {} project files for @-completion",
                    file_list.len()
                );
                Self::from_files(file_list)
            }
            Err(err) => {
                tracing::warn!("Failed to load git files for completion: {err}");
                Self::empty()
            }
        }
    }

    /// Create an empty completer with no files.
    pub fn empty() -> Self {
        Self {
            files: HashSet::new(),
            file_list: Vec::new(),
        }
    }

    /// Create a completer from an explicit list of file paths (useful for testing).
    pub fn from_files(file_list: Vec<String>) -> Self {
        let files: HashSet<String> = file_list.iter().cloned().collect();
        Self { files, file_list }
    }

    /// Get fuzzy-matched suggestions for the given query, returning up to `limit` results.
    ///
    /// Results are sorted by match score (best first).
    pub fn suggest(&self, query: &str, limit: usize) -> Vec<String> {
        if query.is_empty() || self.file_list.is_empty() {
            return Vec::new();
        }

        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let matches = pattern.match_list(&self.file_list, &mut matcher);

        matches
            .into_iter()
            .take(limit)
            .map(|(path, _score)| path.to_string())
            .collect()
    }

    /// Check if a file exists in the project.
    pub fn contains(&self, path: &str) -> bool {
        self.files.contains(path)
    }

    /// Run `git ls-files` and parse the output into a list of file paths.
    async fn run_git_ls_files(cwd: &Path) -> Result<Vec<String>, std::io::Error> {
        let output = tokio::process::Command::new("git")
            .args(["ls-files"])
            .current_dir(cwd)
            .output()
            .await?;

        if !output.status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "git ls-files exited with {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                ),
            ));
        }

        let files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_completer() {
        let completer = FileCompleter::empty();
        assert!(completer.suggest("main", 5).is_empty());
        assert!(!completer.contains("anything"));
    }

    #[test]
    fn suggest_filters_by_query() {
        let completer = FileCompleter::from_files(vec![
            "src/main.rs".into(),
            "src/lib.rs".into(),
            "Cargo.toml".into(),
        ]);
        let results = completer.suggest("main", 5);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.contains("main")));
    }

    #[test]
    fn contains_checks_exact_path() {
        let completer = FileCompleter::from_files(vec!["src/main.rs".into()]);
        assert!(completer.contains("src/main.rs"));
        assert!(!completer.contains("src/lib.rs"));
    }

    #[test]
    fn suggest_respects_limit() {
        let files: Vec<String> = (0..100).map(|i| format!("file_{i}.rs")).collect();
        let completer = FileCompleter::from_files(files);
        let results = completer.suggest("file", 5);
        assert!(results.len() <= 5);
    }
}
