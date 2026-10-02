//! Code intelligence computed once per review: every symbol the diff adds or
//! changes, with its textual usages (`git grep -w`, same language).

use crate::git::{self, DIFF};
use crate::run::{
    Manifest, ManifestFile, ReviewRun, read_manifest, text_size, write_json, write_pages,
};
use crate::{Result, io_error};
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fs;

const DOC_EXTENSIONS: [&str; 4] = [".md", ".rst", ".adoc", ".txt"];
const MAX_CHANGE_DOCS: usize = 40;
const USAGE_TEXT_CHARS: usize = 140;
const USAGE_CAPS: [usize; 4] = [25, 12, 6, 3];
/// The usage map shrinks its per-symbol cap until it fits in this many pages.
const USAGE_PAGES: usize = 4;
/// Patterns per `git grep` call, which keeps every command line short.
const GREP_BATCH_CHARS: usize = 8_000;

/// Names that match half the codebase: their usages are listed short and flagged.
const COMMON: [&str; 25] = [
    "new", "default", "fmt", "from", "into", "main", "run", "get", "set", "render", "update",
    "init", "len", "is_empty", "clone", "drop", "build", "name", "id", "parse", "apply", "handle",
    "tests", "test", "value",
];

/// Definition patterns per language: group 1 is the kind, group 2 the name.
const LANGUAGES: [(&[&str], &str); 4] = [
    (
        &[".rs"],
        r#"(?:pub(?:\([^)]*\))?\s+)?(?:(?:async|const|unsafe|default)\s+)*(?:extern\s+"[^"]*"\s+)?(fn|struct|enum|trait|type|const|static|mod|union)\s+([A-Za-z_][A-Za-z0-9_]*)"#,
    ),
    (&[".py"], r"(?:async\s+)?(def|class)\s+([A-Za-z_]\w*)"),
    (
        &[".js", ".jsx", ".ts", ".tsx", ".mjs"],
        r"(?:export\s+)?(?:default\s+)?(?:async\s+)?(function|class|interface|type|enum|const)\s+([A-Za-z_$][\w$]*)",
    ),
    (&[".go"], r"(func|type)\s+(?:\([^)]*\)\s*)?([A-Za-z_]\w*)"),
];
const TEST_ATTRIBUTE: &str = r"#\[(?:[a-z_:]+::)?test\b|@pytest|\bit\(|\btest\(";
const HUNK: &str = r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@ ?(.*)";
const USAGE_HEADER: &str = "usages of every symbol this diff adds or changes — textual (git grep -w, same language), so a comment or a same-named symbol counts. Start here for callers instead of grepping.";

#[derive(Debug, Serialize)]
struct Symbol {
    name: String,
    kind: String,
    file: String,
    line: Option<usize>,
    status: &'static str,
    ambiguous: bool,
    usage_count: usize,
    usages: Vec<Usage>,
    #[serde(skip)]
    language: usize,
}

#[derive(Clone, Debug, Serialize)]
struct Usage {
    file: String,
    line: usize,
    text: String,
}

struct Language {
    extensions: &'static [&'static str],
    search: Regex,
    anchored: Regex,
}

/// `crtool facts <rundir>`: rebuild the facts of a gathered run.
pub fn facts(run: &ReviewRun) -> Result<String> {
    crate::git::require_root(run)?;
    let mut manifest = read_manifest(run)?;
    build_facts(run, &mut manifest)
}

/// Write `facts/symbols.json` and the usage pages, record them and the
/// change's own documents in the manifest, and return the step's stdout line.
pub(crate) fn build_facts(run: &ReviewRun, manifest: &mut Manifest) -> Result<String> {
    let facts_dir = run.path("facts");
    fs::create_dir_all(&facts_dir)
        .map_err(|source| io_error("create directory", facts_dir, source))?;
    let skip = run.repo_prefix();
    let is_review_artifact = |path: &str| {
        skip.as_deref()
            .is_some_and(|prefix| path.starts_with(prefix))
    };

    // The change's own documents: what the same diff says about its intent. A
    // narrow review scope hides them from the patches; verifiers need them.
    let mut args: Vec<&str> = DIFF.to_vec();
    args.extend(["--name-only", "-z", &manifest.target]);
    let names = git::git(run, &args)?;
    let mut docs = Vec::new();
    for path in git::nul_records(&names)? {
        let lower = path.to_lowercase();
        if !DOC_EXTENSIONS
            .iter()
            .any(|extension| lower.ends_with(extension))
            || is_review_artifact(path)
        {
            continue;
        }
        if let Ok(metadata) = fs::metadata(run.workspace().join(path))
            && metadata.is_file()
        {
            docs.push(serde_json::json!({"path": path, "bytes": metadata.len()}));
        }
    }
    manifest.rest.insert(
        "change_docs".to_owned(),
        Value::Array(docs.iter().take(MAX_CHANGE_DOCS).cloned().collect()),
    );

    let languages = languages()?;
    let mut symbols: Vec<Symbol> = changed_symbols(run, &manifest.files, &languages)?
        .into_iter()
        .filter(|symbol| !(symbol.kind == "mod" && COMMON.contains(&symbol.name.as_str())))
        .collect();
    let usages = usages(run, &symbols, &languages, &is_review_artifact)?;
    for symbol in &mut symbols {
        let hits: Vec<Usage> = usages
            .get(&(symbol.language, symbol.name.clone()))
            .into_iter()
            .flatten()
            .filter(|usage| !(usage.file == symbol.file && Some(usage.line) == symbol.line))
            .cloned()
            .collect();
        symbol.ambiguous =
            COMMON.contains(&symbol.name.as_str()) || symbol.name.chars().count() < 4;
        symbol.usage_count = hits.len();
        symbol.usages = hits;
    }
    write_json(&run.path("facts/symbols.json"), &symbols)?;

    let mut cap = USAGE_CAPS[0];
    let mut body = Vec::new();
    for candidate in USAGE_CAPS {
        cap = candidate;
        body = render(&symbols, cap);
        if text_size(&body) <= USAGE_PAGES * crate::run::PAGE_BUDGET {
            break;
        }
    }
    let pages = write_pages(run, "facts/usages", USAGE_HEADER, &body)?;
    manifest.update_facts(|facts| {
        facts.insert("symbols".to_owned(), Value::from(symbols.len()));
        facts.insert(
            "usages_pages".to_owned(),
            Value::Array(pages.iter().cloned().map(Value::String).collect()),
        );
    });
    write_json(&run.path("manifest.json"), manifest)?;
    Ok(format!(
        "facts: {} changed symbols (usages capped at {cap} each), {} change docs -> {}\n",
        symbols.len(),
        docs.len(),
        pages.join(" ")
    ))
}

fn languages() -> Result<Vec<Language>> {
    LANGUAGES
        .iter()
        .map(|(extensions, pattern)| {
            Ok(Language {
                extensions,
                search: Regex::new(pattern)?,
                anchored: Regex::new(&format!("^(?:{pattern})"))?,
            })
        })
        .collect()
}

fn language_of(path: &str, languages: &[Language]) -> Option<usize> {
    languages.iter().position(|language| {
        language
            .extensions
            .iter()
            .any(|extension| path.ends_with(extension))
    })
}

/// Symbols the diff ADDS (a `+` definition line) or MODIFIES (a hunk's
/// enclosing definition), sorted by file, line and name.
fn changed_symbols(
    run: &ReviewRun,
    files: &[ManifestFile],
    languages: &[Language],
) -> Result<Vec<Symbol>> {
    let test_attribute = Regex::new(TEST_ATTRIBUTE)?;
    let hunk = Regex::new(HUNK)?;
    let mut found: HashMap<(String, String), Symbol> = HashMap::new();
    for file in files {
        let Some(language) = language_of(&file.path, languages) else {
            continue;
        };
        if file.binary {
            continue;
        }
        let rules = &languages[language];
        let path = run.path(&file.patch);
        let bytes = fs::read(&path).map_err(|source| io_error("read", path, source))?;
        let text = String::from_utf8_lossy(&bytes);
        let symbol = |name: &str, kind: &str, line: Option<usize>, status: &'static str| Symbol {
            name: name.to_owned(),
            kind: kind.to_owned(),
            file: file.path.clone(),
            line,
            status,
            ambiguous: false,
            usage_count: 0,
            usages: Vec::new(),
            language,
        };
        let mut new_line = 0usize;
        let mut previous_added = String::new();
        for line in text.lines() {
            if let Some(header) = hunk.captures(line) {
                new_line = header[1].parse::<usize>().unwrap_or(1).saturating_sub(1);
                if let Some(definition) = rules.search.captures(&header[2]) {
                    let key = (file.path.clone(), definition[2].to_owned());
                    if let Entry::Vacant(slot) = found.entry(key) {
                        slot.insert(symbol(&definition[2], &definition[1], None, "modified"));
                    }
                }
                previous_added.clear();
                continue;
            }
            if line.starts_with("+++")
                || line.starts_with("---")
                || (new_line == 0 && !line.starts_with('+') && !line.starts_with(' '))
                || line.starts_with('-')
            {
                continue;
            }
            new_line += 1;
            if let Some(added) = line.strip_prefix('+') {
                if let Some(definition) = rules.anchored.captures(added.trim_start())
                    && !test_attribute.is_match(&previous_added)
                {
                    let key = (file.path.clone(), definition[2].to_owned());
                    found.insert(
                        key,
                        symbol(&definition[2], &definition[1], Some(new_line), "added"),
                    );
                }
                if !added.trim().is_empty() {
                    previous_added = added.trim().to_owned();
                }
            } else {
                previous_added.clear();
            }
        }
    }
    let mut symbols: Vec<Symbol> = found.into_values().collect();
    symbols.sort_by(|a, b| {
        (&a.file, a.line.unwrap_or(0), &a.name).cmp(&(&b.file, b.line.unwrap_or(0), &b.name))
    });
    Ok(symbols)
}

/// Usages of every symbol name, per language: a few batched `git grep -w -F`
/// calls over the whole repository, hits assigned to names afterwards.
fn usages(
    run: &ReviewRun,
    symbols: &[Symbol],
    languages: &[Language],
    is_review_artifact: &dyn Fn(&str) -> bool,
) -> Result<HashMap<(usize, String), Vec<Usage>>> {
    let mut found: HashMap<(usize, String), Vec<Usage>> = HashMap::new();
    for (index, language) in languages.iter().enumerate() {
        let mut names: Vec<&str> = symbols
            .iter()
            .filter(|symbol| symbol.language == index)
            .map(|symbol| symbol.name.as_str())
            .collect();
        names.sort_unstable();
        names.dedup();
        for batch in batches(&names) {
            let mut args = vec![
                "-c".to_owned(),
                "grep.column=false".to_owned(),
                "grep".to_owned(),
                "-n".to_owned(),
                "-w".to_owned(),
                "-F".to_owned(),
                "-I".to_owned(),
                "-z".to_owned(),
                "--full-name".to_owned(),
                "--no-color".to_owned(),
            ];
            for name in batch {
                args.push("-e".to_owned());
                args.push((*name).to_owned());
            }
            args.push("--".to_owned());
            args.extend(
                language
                    .extensions
                    .iter()
                    .map(|extension| format!(":(top)*{extension}")),
            );
            let output = git::git_output(run, &args)?;
            // grep exits 1 for "no match"; anything else is a real failure.
            if !matches!(output.status.code(), Some(0 | 1)) {
                return Err(git::failure(&args, &output));
            }
            for (path, line, text) in grep_hits(&output.stdout) {
                if is_review_artifact(&path) {
                    continue;
                }
                // Match on the whole line, as `git grep -w` did; store it shortened.
                let shown: String = text.trim().chars().take(USAGE_TEXT_CHARS).collect();
                for name in batch.iter().filter(|name| contains_word(&text, name)) {
                    found
                        .entry((index, (*name).to_owned()))
                        .or_default()
                        .push(Usage {
                            file: path.clone(),
                            line,
                            text: shown.clone(),
                        });
                }
            }
        }
    }
    Ok(found)
}

fn batches<'a>(names: &'a [&'a str]) -> Vec<&'a [&'a str]> {
    let mut batches = Vec::new();
    let mut start = 0;
    let mut size = 0;
    for (index, name) in names.iter().enumerate() {
        if index > start && size + name.len() > GREP_BATCH_CHARS {
            batches.push(&names[start..index]);
            start = index;
            size = 0;
        }
        size += name.len() + 4;
    }
    if start < names.len() {
        batches.push(&names[start..]);
    }
    batches
}

/// `git grep -z -n` records: `path NUL line NUL text LF`.
fn grep_hits(bytes: &[u8]) -> Vec<(String, usize, String)> {
    let mut hits = Vec::new();
    for record in bytes.split(|byte| *byte == b'\n') {
        let mut fields = record.splitn(3, |byte| *byte == 0);
        let (Some(path), Some(number), Some(text)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let Some(line) = std::str::from_utf8(number)
            .ok()
            .and_then(|number| number.parse().ok())
        else {
            continue;
        };
        hits.push((
            String::from_utf8_lossy(path).into_owned(),
            line,
            String::from_utf8_lossy(text).into_owned(),
        ));
    }
    hits
}

/// Whether `text` contains `name` as git grep `-w` matches it: bounded by
/// non-word bytes, where a word byte is an ASCII letter, digit or `_`.
fn contains_word(text: &str, name: &str) -> bool {
    let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let bytes = text.as_bytes();
    text.match_indices(name).any(|(start, _)| {
        let end = start + name.len();
        (start == 0 || !is_word(bytes[start - 1])) && (end == bytes.len() || !is_word(bytes[end]))
    })
}

fn render(symbols: &[Symbol], cap: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut unused = Vec::new();
    let mut common = Vec::new();
    for symbol in symbols {
        let at = match symbol.line {
            Some(line) if line > 0 => format!("{}:{line}", symbol.file),
            _ => symbol.file.clone(),
        };
        if symbol.ambiguous {
            common.push(format!(
                "{} ({at}, {} textual hits)",
                symbol.name, symbol.usage_count
            ));
        } else if symbol.usages.is_empty() {
            unused.push(format!("{} ({}, {at})", symbol.name, symbol.kind));
        } else {
            let more = if symbol.usage_count > cap {
                format!(", first {cap} shown")
            } else {
                String::new()
            };
            out.push(format!(
                "## {}  ({}, {}, {at}) — {} usages{more}",
                symbol.name, symbol.kind, symbol.status, symbol.usage_count
            ));
            out.extend(
                symbol
                    .usages
                    .iter()
                    .take(cap)
                    .map(|usage| format!("{}:{}: {}", usage.file, usage.line, usage.text)),
            );
            out.push(String::new());
        }
    }
    if !unused.is_empty() {
        out.push(
            "## defined or changed by this diff, used NOWHERE else in code (tests have no callers; for anything else this is worth a look):"
                .to_owned(),
        );
        out.push(unused.join("; "));
        out.push(String::new());
    }
    if !common.is_empty() {
        out.push(
            "## names too common for a textual map to mean anything — search these yourself:"
                .to_owned(),
        );
        out.push(common.join("; "));
    }
    if out.is_empty() {
        out.push("(no symbols detected)".to_owned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_words_follow_git_grep_w() {
        assert!(contains_word("helper();", "helper"));
        assert!(contains_word("x = a.helper", "helper"));
        assert!(!contains_word("helpers()", "helper"));
        assert!(!contains_word("my_helper()", "helper"));
        assert!(contains_word("my_helper() helper", "helper"));
        assert!(contains_word("é helper", "helper"));
    }

    #[test]
    fn grep_records_split_on_nul_and_newline() {
        let hits = grep_hits(b"src/a.rs\x003\x00    helper(); // a:b\nsrc/b.rs\x0010\x00x\n");
        assert_eq!(
            hits,
            [
                ("src/a.rs".to_owned(), 3, "    helper(); // a:b".to_owned()),
                ("src/b.rs".to_owned(), 10, "x".to_owned()),
            ]
        );
    }

    #[test]
    fn batches_cover_every_name_once() {
        let names: Vec<String> = (0..3_000).map(|n| format!("symbol_{n}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let batches = batches(&names);
        assert!(batches.len() > 1);
        assert_eq!(
            batches.iter().map(|batch| batch.len()).sum::<usize>(),
            names.len()
        );
    }
}
