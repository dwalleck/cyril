use std::collections::HashSet;
use std::path::{Path, PathBuf};

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

/// Owns the cached file list and fuzzy matcher for `@file` autocomplete.
///
/// Load files from `git ls-files` via [`FileCompleter::load`], then use
/// [`FileCompleter::suggest`] to get fuzzy-matched completions.
pub struct FileCompleter {
    root: PathBuf,
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
                Self::from_files_with_root(cwd.to_path_buf(), file_list)
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
            root: PathBuf::new(),
            files: HashSet::new(),
            file_list: Vec::new(),
        }
    }

    /// Create a completer from an explicit list of file paths (useful for testing).
    pub fn from_files(file_list: Vec<String>) -> Self {
        Self::from_files_with_root(PathBuf::new(), file_list)
    }

    /// Create a completer from a root path and an explicit list of file paths.
    pub fn from_files_with_root(root: PathBuf, file_list: Vec<String>) -> Self {
        let files: HashSet<String> = file_list.iter().cloned().collect();
        Self {
            root,
            files,
            file_list,
        }
    }

    /// The root directory that file paths are relative to.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The set of known file paths in the project.
    pub fn known_files(&self) -> &HashSet<String> {
        &self.files
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
            return Err(std::io::Error::other(format!(
                "git ls-files exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();

        Ok(files)
    }
}

/// Scan prompt text for `@filepath` tokens.
///
/// The `@` must appear at the start of a line or be preceded by whitespace.
/// Only paths that exist in `known_files` are returned. Results are deduplicated
/// and returned in the order they first appear.
pub fn parse_file_references(text: &str, known_files: &HashSet<String>) -> Vec<String> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();
    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '@' {
                let valid_start = i == 0 || chars[i - 1].is_whitespace();
                if valid_start {
                    let start = i + 1;
                    let mut end = start;
                    while end < chars.len() && !chars[end].is_whitespace() {
                        end += 1;
                    }
                    if end > start {
                        let path: String = chars[start..end].iter().collect();
                        if known_files.contains(&path) && seen.insert(path.clone()) {
                            refs.push(path);
                        }
                    }
                    i = end;
                    continue;
                }
            }
            i += 1;
        }
    }
    refs
}

/// Read a file relative to a root path, capping content at 100 KB.
///
/// Returns the file contents as a string. If the file exceeds 100 KB, the content
/// is truncated at a valid UTF-8 boundary and a truncation notice is appended.
pub fn read_file(root: &Path, relative_path: &str) -> std::io::Result<String> {
    let full_path = root.join(relative_path);
    let metadata = std::fs::metadata(&full_path)?;
    const MAX_SIZE: u64 = 100 * 1024;
    let mut contents = std::fs::read_to_string(&full_path)?;
    if metadata.len() > MAX_SIZE {
        contents.truncate(MAX_SIZE as usize);
        while !contents.is_char_boundary(contents.len()) {
            contents.pop();
        }
        contents.push_str("\n... [truncated at 100KB]");
    }
    Ok(contents)
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

    #[test]
    fn root_returns_stored_root() {
        let completer =
            FileCompleter::from_files_with_root(PathBuf::from("/project"), vec!["a.rs".into()]);
        assert_eq!(completer.root(), Path::new("/project"));
    }

    #[test]
    fn known_files_returns_file_set() {
        let completer = FileCompleter::from_files(vec!["src/main.rs".into(), "lib.rs".into()]);
        let known = completer.known_files();
        assert!(known.contains("src/main.rs"));
        assert!(known.contains("lib.rs"));
        assert!(!known.contains("nope.rs"));
    }

    // --- parse_file_references tests ---

    #[test]
    fn parse_refs_basic() {
        let known: HashSet<String> = ["src/main.rs".into()].into_iter().collect();
        let refs = parse_file_references("look at @src/main.rs please", &known);
        assert_eq!(refs, vec!["src/main.rs"]);
    }

    #[test]
    fn parse_refs_at_start_of_line() {
        let known: HashSet<String> = ["src/main.rs".into()].into_iter().collect();
        let refs = parse_file_references("@src/main.rs is important", &known);
        assert_eq!(refs, vec!["src/main.rs"]);
    }

    #[test]
    fn parse_refs_ignores_mid_word_at() {
        let known: HashSet<String> = ["file.rs".into()].into_iter().collect();
        let refs = parse_file_references("user@file.rs", &known);
        assert!(refs.is_empty(), "@ preceded by non-whitespace should be ignored");
    }

    #[test]
    fn parse_refs_deduplicates() {
        let known: HashSet<String> = ["a.rs".into()].into_iter().collect();
        let refs = parse_file_references("@a.rs and @a.rs again", &known);
        assert_eq!(refs, vec!["a.rs"]);
    }

    #[test]
    fn parse_refs_unknown_file_ignored() {
        let known: HashSet<String> = ["known.rs".into()].into_iter().collect();
        let refs = parse_file_references("@unknown.rs @known.rs", &known);
        assert_eq!(refs, vec!["known.rs"]);
    }

    #[test]
    fn parse_refs_multiple_files() {
        let known: HashSet<String> = ["a.rs".into(), "b.rs".into()].into_iter().collect();
        let refs = parse_file_references("@a.rs and @b.rs", &known);
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&"a.rs".to_string()));
        assert!(refs.contains(&"b.rs".to_string()));
    }

    #[test]
    fn parse_refs_multiline() {
        let known: HashSet<String> = ["a.rs".into(), "b.rs".into()].into_iter().collect();
        let refs = parse_file_references("@a.rs\n@b.rs", &known);
        assert_eq!(refs, vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn parse_refs_empty_text() {
        let known: HashSet<String> = ["a.rs".into()].into_iter().collect();
        let refs = parse_file_references("", &known);
        assert!(refs.is_empty());
    }

    #[test]
    fn parse_refs_bare_at_sign() {
        let known: HashSet<String> = HashSet::new();
        let refs = parse_file_references("@ alone", &known);
        assert!(refs.is_empty());
    }

    // --- read_file tests ---

    #[test]
    fn read_file_basic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello world").expect("write");
        let contents = read_file(dir.path(), "test.txt").expect("read");
        assert_eq!(contents, "hello world");
    }

    #[test]
    fn read_file_not_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = read_file(dir.path(), "nonexistent.txt");
        assert!(result.is_err());
    }
}
