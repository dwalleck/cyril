use crate::{Result, ReviewError, io_error};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// The running crtool's version, stamped into every manifest it gathers.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Characters per text page: one tool read is capped at about 30,000.
pub(crate) const PAGE_BUDGET: usize = 20_000;

/// One review run: the repository it reviews and the directory its files live in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewRun {
    workspace: PathBuf,
    dir: PathBuf,
}

impl ReviewRun {
    /// `workspace` is the repository root; `dir` is the run directory. Both
    /// are made absolute against the process cwd.
    pub fn new(workspace: impl AsRef<Path>, dir: impl AsRef<Path>) -> Result<Self> {
        let absolute = |path: &Path| {
            std::path::absolute(path).map_err(|source| io_error("resolve path", path, source))
        };
        Ok(Self {
            workspace: absolute(workspace.as_ref())?,
            dir: absolute(dir.as_ref())?,
        })
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn path(&self, relative: &str) -> PathBuf {
        self.dir.join(relative)
    }

    /// The run directory relative to the repository root, with forward
    /// slashes and a trailing `/`, when it lies inside the repository. Search
    /// hits under it are review artifacts, not code.
    pub(crate) fn repo_prefix(&self) -> Option<String> {
        let relative = self.dir.strip_prefix(&self.workspace).ok()?;
        let mut prefix = relative.to_string_lossy().replace('\\', "/");
        if prefix.is_empty() {
            return None;
        }
        prefix.push('/');
        Some(prefix)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Manifest {
    pub(crate) requested_target: String,
    pub(crate) target: String,
    pub(crate) scope: Vec<String>,
    pub(crate) head: String,
    pub(crate) worktree_matches_diff_head: bool,
    pub(crate) gathered_at: String,
    #[serde(default)]
    pub(crate) crtool_version: Option<String>,
    pub(crate) total_files: u64,
    pub(crate) total_patch_bytes: u64,
    pub(crate) warnings: Vec<String>,
    pub(crate) files: Vec<ManifestFile>,
    /// `change_docs`, `facts` and anything later steps add.
    #[serde(flatten)]
    pub(crate) rest: Map<String, Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ManifestFile {
    pub(crate) index: u64,
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) insertions: Option<u64>,
    pub(crate) deletions: Option<u64>,
    pub(crate) binary: bool,
    pub(crate) patch: String,
    pub(crate) patch_bytes: u64,
}

impl Manifest {
    /// Change the `facts` object, creating it when absent or null.
    pub(crate) fn update_facts(&mut self, change: impl FnOnce(&mut Map<String, Value>)) {
        let mut facts = match self.rest.remove("facts") {
            Some(Value::Object(facts)) => facts,
            None | Some(Value::Null) => Map::new(),
            Some(other) => {
                tracing::warn!(%other, "manifest `facts` was not an object; replacing it");
                Map::new()
            }
        };
        change(&mut facts);
        self.rest.insert("facts".to_owned(), Value::Object(facts));
    }
}

/// Read `manifest.json` and refuse a run gathered by another crtool version.
pub(crate) fn read_manifest(run: &ReviewRun) -> Result<Manifest> {
    let path = run.path("manifest.json");
    let manifest: Manifest = read_json(&path)?;
    match manifest.crtool_version.as_deref() {
        Some(VERSION) => Ok(manifest),
        found => Err(ReviewError::StampMismatch {
            path,
            found: found.unwrap_or("(unstamped)").to_owned(),
            current: VERSION,
        }),
    }
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).map_err(|source| io_error("read", path, source))?;
    serde_json::from_slice(&bytes).map_err(|source| ReviewError::Json {
        path: path.to_path_buf(),
        source,
    })
}

/// Write pretty JSON plus a newline, atomically (temp file + rename).
pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut text = serde_json::to_string_pretty(value).map_err(|source| ReviewError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    text.push('\n');
    write(path, text.as_bytes())
}

/// Write a run file atomically (temp file + rename), so a crash never leaves
/// a half-written one.
pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| io_error("create directory", parent, source))?;
    }
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = Path::new(&temporary);
    fs::write(temporary, bytes).map_err(|source| io_error("write", temporary, source))?;
    fs::rename(temporary, path).map_err(|source| io_error("replace", path, source))
}

/// Write `queues/<file>.json` for the ids, with its own absolute verdict directory.
pub(crate) fn write_queue(
    run: &ReviewRun,
    file: &str,
    verdict_dir: &str,
    ids: &[String],
) -> Result<()> {
    let directory = run.path(&format!("verdicts/{verdict_dir}"));
    fs::create_dir_all(&directory)
        .map_err(|source| io_error("create directory", &directory, source))?;
    write_json(
        &run.path(&format!("queues/{file}.json")),
        &json!({
            "done": ids.is_empty(),
            "ids": ids,
            "verdict_dir": forward_slashes(&directory),
        }),
    )
}

/// A path with `/` separators, as queue files spell it. Only Windows
/// separators are converted: on Unix a backslash belongs to the name.
fn forward_slashes(path: &std::path::Path) -> String {
    let path = path.to_string_lossy();
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.into_owned()
    }
}

/// Write `lines` as `<stem>-N.txt` pages that each fit one tool read, after
/// removing the stem's old pages. Returns the pages' run-relative paths.
pub(crate) fn write_pages(
    run: &ReviewRun,
    stem: &str,
    header: &str,
    lines: &[String],
) -> Result<Vec<String>> {
    let stem_path = run.path(stem);
    let (directory, base) = match (stem_path.parent(), stem_path.file_name()) {
        (Some(directory), Some(base)) => {
            (directory.to_path_buf(), base.to_string_lossy().into_owned())
        }
        _ => (run.dir().to_path_buf(), stem.to_owned()),
    };
    if let Ok(entries) = fs::read_dir(&directory) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_page_of(&name, &base) {
                let path = entry.path();
                fs::remove_file(&path)
                    .map_err(|source| io_error("remove old page", path, source))?;
            }
        }
    }
    let pages = paginate(lines);
    let mut relative = Vec::with_capacity(pages.len());
    for (number, body) in pages.iter().enumerate() {
        let name = format!("{stem}-{}.txt", number + 1);
        let text = format!(
            "# {header} -- page {} of {}\n{}\n",
            number + 1,
            pages.len(),
            body.join("\n")
        );
        write(&run.path(&name), text.as_bytes())?;
        relative.push(name);
    }
    Ok(relative)
}

fn is_page_of(name: &str, base: &str) -> bool {
    name.strip_prefix(base)
        .and_then(|rest| rest.strip_prefix('-'))
        .and_then(|rest| rest.strip_suffix(".txt"))
        .is_some_and(|number| {
            !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
        })
}

/// Split lines into pages of at most `PAGE_BUDGET` characters (each line
/// counts its newline). There is always at least one page.
pub(crate) fn paginate(lines: &[String]) -> Vec<Vec<&str>> {
    let mut pages = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut size = 0;
    for line in lines {
        let length = line.chars().count() + 1;
        if !current.is_empty() && size + length > PAGE_BUDGET {
            pages.push(std::mem::take(&mut current));
            size = 0;
        }
        current.push(line);
        size += length;
    }
    pages.push(current);
    pages
}

/// Character count of `lines` as written, one newline each.
pub(crate) fn text_size(lines: &[String]) -> usize {
    lines.iter().map(|line| line.chars().count() + 1).sum()
}

/// A run directory with a minimal stamped manifest, for step tests that do
/// not need a gathered repository.
#[cfg(test)]
pub(crate) fn stamped_run() -> Result<(tempfile::TempDir, ReviewRun)> {
    let tree = tempfile::tempdir().map_err(|source| io_error("create temp dir", ".", source))?;
    let run = ReviewRun::new(tree.path(), tree.path().join("run"))?;
    write_json(
        &run.path("manifest.json"),
        &serde_json::json!({
            "requested_target": "HEAD", "target": "HEAD", "scope": ["."], "head": "0".repeat(40),
            "worktree_matches_diff_head": true, "gathered_at": "2026-01-01T00:00:00+00:00",
            "crtool_version": VERSION, "total_files": 0, "total_patch_bytes": 0,
            "warnings": [], "files": [],
        }),
    )?;
    Ok((tree, run))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_unix_backslash_stays_in_the_verdict_dir() {
        assert_eq!(
            forward_slashes(std::path::Path::new("/r/a\\b/verdicts/q1")),
            "/r/a\\b/verdicts/q1"
        );
    }

    #[test]
    fn pages_split_at_the_budget_and_never_split_a_line() {
        let line = "x".repeat(PAGE_BUDGET / 2 - 1); // exactly half a page with its newline
        let lines = vec![line.clone(), line.clone(), line.clone()];
        let pages = paginate(&lines);
        assert_eq!(pages.iter().map(Vec::len).collect::<Vec<_>>(), [2, 1]);
        let oversized = vec!["y".repeat(PAGE_BUDGET * 2)];
        assert_eq!(paginate(&oversized).len(), 1);
        assert_eq!(paginate(&[]), vec![Vec::<&str>::new()]);
    }

    #[test]
    fn page_names_match_only_numbered_pages_of_the_stem() {
        assert!(is_page_of("usages-1.txt", "usages"));
        assert!(is_page_of("usages-12.txt", "usages"));
        assert!(!is_page_of("usages-.txt", "usages"));
        assert!(!is_page_of("usages-1a.txt", "usages"));
        assert!(!is_page_of("other-1.txt", "usages"));
    }
}
