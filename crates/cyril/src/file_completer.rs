use std::path::PathBuf;

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher};

/// A fuzzy file path suggestion with match score.
pub struct FileSuggestion {
    pub path: String,
    pub score: u32,
}

/// Describes an active `@` trigger in the textarea.
pub struct AtContext {
    pub row: usize,
    pub at_col: usize,
    pub cursor_col: usize,
    pub query: String,
}

/// Owns the cached file list and fuzzy matcher for `@file` autocomplete.
pub struct FileCompleter {
    project_root: PathBuf,
    files: Vec<String>,
    matcher: Matcher,
}

impl FileCompleter {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            files: Vec::new(),
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
        }
    }

    /// Populate the file cache by running `git ls-files` in the project root.
    pub fn load_files(&mut self) -> anyhow::Result<()> {
        let output = std::process::Command::new("git")
            .args(["ls-files"])
            .current_dir(&self.project_root)
            .output()?;

        if !output.status.success() {
            anyhow::bail!("git ls-files failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        self.files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();

        tracing::info!("Loaded {} project files for @-completion", self.files.len());
        Ok(())
    }

    /// Fuzzy-match `query` against cached files, returning up to `max` results sorted by score.
    pub fn suggestions(&mut self, query: &str, max: usize) -> Vec<FileSuggestion> {
        if query.is_empty() || self.files.is_empty() {
            return Vec::new();
        }

        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
        let matches = pattern.match_list(&self.files, &mut self.matcher);

        matches
            .into_iter()
            .take(max)
            .map(|(path, score)| FileSuggestion {
                path: path.to_string(),
                score,
            })
            .collect()
    }

    /// Check if a path exists in the cached file list.
    pub fn file_exists(&self, path: &str) -> bool {
        self.files.iter().any(|f| f == path)
    }

    /// Read a file relative to the project root. Caps content at 100KB.
    pub fn read_file(&self, path: &str) -> anyhow::Result<String> {
        let full_path = self.project_root.join(path);
        let metadata = std::fs::metadata(&full_path)?;
        const MAX_SIZE: u64 = 100 * 1024;
        if metadata.len() > MAX_SIZE {
            let mut contents = std::fs::read_to_string(&full_path)?;
            let end = contents.floor_char_boundary(MAX_SIZE as usize);
            contents.truncate(end);
            contents.push_str("\n... [truncated at 100KB]");
            Ok(contents)
        } else {
            Ok(std::fs::read_to_string(&full_path)?)
        }
    }
}

/// Scan backward from cursor on the current line looking for an `@` trigger.
///
/// Rules:
/// - `@` must be at column 0 or preceded by whitespace
/// - Query text between `@` and cursor must not contain whitespace
pub fn find_at_trigger(lines: &[String], cursor_row: usize, cursor_col: usize) -> Option<AtContext> {
    let line = lines.get(cursor_row)?;
    if cursor_col == 0 {
        return None;
    }

    let chars: Vec<char> = line.chars().collect();
    let end = cursor_col.min(chars.len());

    for i in (0..end).rev() {
        let ch = chars[i];
        if ch == '@' {
            // '@' must be at position 0 or preceded by whitespace
            if i > 0 && !chars[i - 1].is_whitespace() {
                return None;
            }
            let query: String = chars[i + 1..end].iter().collect();
            // Query must not contain whitespace
            if query.contains(char::is_whitespace) {
                return None;
            }
            return Some(AtContext {
                row: cursor_row,
                at_col: i,
                cursor_col: end,
                query,
            });
        }
        // If we hit whitespace before finding '@', no trigger
        if ch.is_whitespace() {
            return None;
        }
    }

    None
}

/// Scan the full prompt text for `@filepath` tokens, validate each exists, return valid paths.
pub fn parse_file_references(text: &str, completer: &FileCompleter) -> Vec<String> {
    let mut refs = Vec::new();

    for line in text.lines() {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '@' {
                // '@' must be at start of line or preceded by whitespace
                let valid_start = i == 0 || chars[i - 1].is_whitespace();
                if valid_start {
                    // Collect the token after '@'
                    let start = i + 1;
                    let mut end = start;
                    while end < chars.len() && !chars[end].is_whitespace() {
                        end += 1;
                    }
                    if end > start {
                        let path: String = chars[start..end].iter().collect();
                        if completer.file_exists(&path) && !refs.contains(&path) {
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
