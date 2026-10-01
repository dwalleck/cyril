use crate::git;
use crate::run::{
    Manifest, ManifestFile, ReviewRun, facts_dir, facts_metadata, manifest_path, read_binary,
    read_manifest, write_json, write_text,
};
use crate::{Result, StepOutput, regex_error};
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashSet, btree_map::Entry};
use std::fs;

const PAGE_BUDGET: usize = 20_000;
const DOCUMENT_EXTENSIONS: [&str; 4] = [".md", ".rst", ".adoc", ".txt"];
const COMMON: [&str; 25] = [
    "new", "default", "fmt", "from", "into", "main", "run", "get", "set", "render", "update",
    "init", "len", "is_empty", "clone", "drop", "build", "name", "id", "parse", "apply", "handle",
    "tests", "test", "value",
];

#[derive(Debug, Serialize)]
struct Usage {
    file: String,
    line: usize,
    text: String,
}

#[derive(Debug, Serialize)]
struct Symbol {
    name: String,
    kind: String,
    file: String,
    line: Option<usize>,
    status: String,
    ambiguous: bool,
    usage_count: usize,
    usages: Vec<Usage>,
}

#[derive(Debug)]
struct SymbolSeed<'a> {
    name: String,
    kind: String,
    file: &'a ManifestFile,
    line: Option<usize>,
    status: String,
    declaration: Option<ModifiedDeclaration>,
}

#[derive(Debug)]
struct ModifiedDeclaration {
    header: String,
    new_before: usize,
    removed_before: Option<usize>,
    added_lines: HashSet<usize>,
}

struct RemovedDeclaration<'a> {
    old_line: u32,
    new_before: usize,
    text: &'a [u8],
}

pub fn facts(run: &ReviewRun) -> Result<StepOutput> {
    let mut manifest = read_manifest(run)?;
    let line = build_facts(run, &mut manifest)?;
    Ok(StepOutput::from_text(line))
}

pub(crate) fn build_facts(run: &ReviewRun, manifest: &mut Manifest) -> Result<String> {
    git::admit_target(&manifest.target)?;
    facts_metadata(run, manifest)?;
    let documents = changed_documents(run, &manifest.target)?;
    let mut seeds = changed_symbols(run, &manifest.files)?;
    seeds.retain(|seed| !(seed.kind == "mod" && is_common(&seed.name)));

    let mut symbols = Vec::with_capacity(seeds.len());
    for seed in seeds {
        let usages = usages(run, &seed)?;
        let ambiguous = is_common(&seed.name) || seed.name.chars().count() < 4;
        let usage_count = usages.len();
        symbols.push(Symbol {
            name: seed.name,
            kind: seed.kind,
            file: seed.file.path.clone(),
            line: seed.line,
            status: seed.status,
            ambiguous,
            usage_count,
            usages,
        });
    }

    let symbols_path = facts_dir(run).join("symbols.json");
    write_json(&symbols_path, &symbols)?;

    let mut chosen_cap = 25;
    let mut body = Vec::new();
    for cap in [25, 12, 6, 3] {
        body = render(&symbols, cap);
        chosen_cap = cap;
        if body_characters(&body) <= 4 * PAGE_BUDGET || cap == 3 {
            break;
        }
    }
    let pages = write_pages(run, &body)?;

    let document_count = documents.len();
    let mut output = format!(
        "facts: {} changed symbols (usages capped at {chosen_cap} each), {document_count} change docs -> ",
        symbols.len(),
    );
    for (index, page) in pages.iter().enumerate() {
        if index > 0 {
            output.push(' ');
        }
        output.push_str(page);
    }
    output.push('\n');

    manifest.metadata.insert(
        "change_docs".to_owned(),
        Value::Array(documents.into_iter().take(40).map(Value::Object).collect()),
    );
    let facts_map = facts_metadata(run, manifest)?;
    facts_map.insert("symbols".to_owned(), Value::from(symbols.len() as u64));
    facts_map.insert(
        "usages_pages".to_owned(),
        Value::Array(pages.into_iter().map(Value::String).collect()),
    );
    write_json(&manifest_path(run), manifest)?;
    Ok(output)
}

struct DefinitionPattern {
    regex: &'static str,
    usage_globs: &'static [&'static str],
}

fn definition_pattern(path: &str) -> Option<DefinitionPattern> {
    let (regex, usage_globs): (&'static str, &'static [&'static str]) = if path.ends_with(".rs") {
        (
            r#"(?:pub(?:\([^)]*\))?\s+)?(?:(?:async|const|unsafe|default)\s+)*(?:extern\s+"[^"]*"\s+)?(fn|struct|enum|trait|type|const|static|mod|union)\s+([A-Za-z_][A-Za-z0-9_]*)"#,
            &["*.rs"],
        )
    } else if path.ends_with(".py") {
        (r#"(?:async\s+)?(def|class)\s+([A-Za-z_]\w*)"#, &["*.py"])
    } else if path.ends_with(".js")
        || path.ends_with(".jsx")
        || path.ends_with(".ts")
        || path.ends_with(".tsx")
        || path.ends_with(".mjs")
    {
        (
            r#"(?:export\s+)?(?:default\s+)?(?:async\s+)?(function|class|interface|type|enum|const)\s+([A-Za-z_$][\w$]*)"#,
            &["*.js", "*.jsx", "*.ts", "*.tsx", "*.mjs"],
        )
    } else if path.ends_with(".go") {
        (
            r#"(func|type)\s+(?:\([^)]*\)\s*)?([A-Za-z_]\w*)"#,
            &["*.go"],
        )
    } else {
        return None;
    };
    Some(DefinitionPattern { regex, usage_globs })
}

fn changed_symbols<'a>(run: &ReviewRun, files: &'a [ManifestFile]) -> Result<Vec<SymbolSeed<'a>>> {
    let test_attr =
        Regex::new(r"#\[(?:[a-z_:]+::)?test\b|@pytest|\bit\(|\btest\(").map_err(regex_error)?;
    let mut found = BTreeMap::<(&'a [u8], String), SymbolSeed<'a>>::new();
    let mut definitions = BTreeMap::new();
    for file in files {
        if file.binary {
            continue;
        }
        let Some(pattern) = definition_pattern(&file.path) else {
            continue;
        };
        let definition = match definitions.entry(pattern.regex) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Regex::new(pattern.regex).map_err(regex_error)?),
        };
        let patch_path = run.directory().join(&file.patch);
        let bytes = read_binary(&patch_path)?;
        let diff = git::parse_patch(&bytes, &file.patch)?;
        let mut added_lines = BTreeMap::<String, HashSet<usize>>::new();
        for delta_index in 0..diff.deltas().len() {
            let patch = git::patch(&diff, delta_index)?;
            let mut removed = BTreeMap::<(String, String), Vec<RemovedDeclaration<'_>>>::new();
            for hunk_index in 0..patch.num_hunks() {
                let (hunk, line_count) = patch
                    .hunk(hunk_index)
                    .map_err(|error| git::operation("read stored patch hunk", error))?;
                let old_before = hunk
                    .old_start()
                    .saturating_sub(u32::from(hunk.old_lines() > 0));
                let mut new_before = hunk
                    .new_start()
                    .saturating_sub(u32::from(hunk.new_lines() > 0))
                    as usize;
                let header = String::from_utf8_lossy(hunk.header());
                let context = header.split_once(" @@").map_or("", |(_, context)| {
                    context
                        .strip_prefix(' ')
                        .unwrap_or(context)
                        .trim_end_matches('\n')
                });
                if let Some(captures) = definition.captures(context) {
                    let (_, [kind, name]) = captures.extract();
                    if let Entry::Vacant(entry) = found.entry((file.path_bytes(), name.to_owned()))
                    {
                        let removed_before = removed
                            .get(&(kind.to_owned(), name.to_owned()))
                            .and_then(|lines| {
                                lines.iter().rev().find(|line| {
                                    line.old_line <= old_before
                                        && line.text.starts_with(context.as_bytes())
                                })
                            })
                            .map(|line| line.new_before);
                        entry.insert(SymbolSeed {
                            name: name.to_owned(),
                            kind: kind.to_owned(),
                            file,
                            line: None,
                            status: "modified".to_owned(),
                            declaration: Some(ModifiedDeclaration {
                                header: context.to_owned(),
                                new_before,
                                removed_before,
                                added_lines: HashSet::new(),
                            }),
                        });
                    }
                }
                let mut previous_added: &[u8] = &[];
                for line_index in 0..line_count {
                    let line = patch
                        .line_in_hunk(hunk_index, line_index)
                        .map_err(|error| git::operation("read stored patch line", error))?;
                    let content = String::from_utf8_lossy(line.content());
                    if matches!(line.origin(), '-' | '+')
                        && let Some(captures) = definition.captures(content.trim_start())
                        && captures.get(0).is_some_and(|matched| matched.start() == 0)
                    {
                        let (_, [kind, name]) = captures.extract();
                        if line.origin() == '-' {
                            let old_line = line.old_lineno().ok_or_else(|| {
                                git::operation("read removed declaration", "missing old coordinate")
                            })?;
                            removed
                                .entry((kind.to_owned(), name.to_owned()))
                                .or_default()
                                .push(RemovedDeclaration {
                                    old_line,
                                    new_before,
                                    text: line.content(),
                                });
                        } else {
                            let number = line.new_lineno().ok_or_else(|| {
                                git::operation("read added declaration", "missing new coordinate")
                            })? as usize;
                            added_lines
                                .entry(name.to_owned())
                                .or_default()
                                .insert(number);
                            if !test_attr.is_match(&String::from_utf8_lossy(previous_added)) {
                                found.insert(
                                    (file.path_bytes(), name.to_owned()),
                                    SymbolSeed {
                                        name: name.to_owned(),
                                        kind: kind.to_owned(),
                                        file,
                                        line: Some(number),
                                        status: "added".to_owned(),
                                        declaration: None,
                                    },
                                );
                            }
                        }
                    }
                    if let Some(number) = line.new_lineno() {
                        new_before = number as usize;
                    }
                    if line.origin() == '+' && !content.trim().is_empty() {
                        previous_added = line.content();
                    } else if line.origin() == ' ' {
                        previous_added = &[];
                    }
                }
            }
        }
        for (name, lines) in added_lines {
            if let Some(seed) = found.get_mut(&(file.path_bytes(), name))
                && let Some(declaration) = &mut seed.declaration
            {
                declaration.added_lines = lines;
            }
        }
    }
    let mut symbols = found.into_values().collect::<Vec<_>>();
    symbols.sort_by(|left, right| {
        (
            left.file.path.as_str(),
            left.line.unwrap_or(0),
            left.name.as_str(),
            left.file.index,
        )
            .cmp(&(
                right.file.path.as_str(),
                right.line.unwrap_or(0),
                right.name.as_str(),
                right.file.index,
            ))
    });
    Ok(symbols)
}

fn usages(run: &ReviewRun, symbol: &SymbolSeed<'_>) -> Result<Vec<Usage>> {
    let Some(pattern) = definition_pattern(&symbol.file.path) else {
        return Ok(Vec::new());
    };
    let mut args = vec![
        "grep",
        "--no-color",
        "--no-column",
        "--full-name",
        "-z",
        "-n",
        "-w",
        "-F",
        "-I",
        "-e",
        &symbol.name,
        "--",
    ];
    args.extend_from_slice(pattern.usage_globs);
    let output = git::grep(run, &args)?;
    let own_line = if let Some(declaration) = &symbol.declaration {
        let definition = Regex::new(pattern.regex).map_err(regex_error)?;
        resolve_declaration_line(symbol, declaration, &definition, &output)
    } else {
        symbol.line
    };
    let mut hits = Vec::new();
    for (path, line, text) in grep_hits(&output) {
        if path.starts_with(b".code-review/")
            || (path == symbol.file.path_bytes() && own_line == Some(line))
        {
            continue;
        }
        hits.push(Usage {
            file: String::from_utf8_lossy(path).into_owned(),
            line,
            text: String::from_utf8_lossy(text)
                .trim()
                .chars()
                .take(140)
                .collect(),
        });
    }
    Ok(hits)
}

fn resolve_declaration_line(
    symbol: &SymbolSeed<'_>,
    declaration: &ModifiedDeclaration,
    definition: &Regex,
    output: &[u8],
) -> Option<usize> {
    let candidate = grep_hits(output)
        .filter(|(path, line, text)| {
            *path == symbol.file.path_bytes()
                && *line <= declaration.new_before
                && !declaration.added_lines.contains(line)
                && text.starts_with(declaration.header.as_bytes())
                && definition
                    .find(String::from_utf8_lossy(text).trim_start())
                    .is_some_and(|matched| matched.start() == 0)
        })
        .map(|(_, line, _)| line)
        .max()?;
    declaration
        .removed_before
        .is_none_or(|barrier| candidate > barrier)
        .then_some(candidate)
}

fn grep_hits(mut bytes: &[u8]) -> impl Iterator<Item = (&[u8], usize, &[u8])> {
    std::iter::from_fn(move || {
        loop {
            let path_end = bytes.iter().position(|byte| *byte == 0)?;
            let path = &bytes[..path_end];
            bytes = &bytes[path_end + 1..];
            let number_end = bytes.iter().position(|byte| *byte == 0)?;
            let number = &bytes[..number_end];
            bytes = &bytes[number_end + 1..];
            let text_end = bytes
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap_or(bytes.len());
            let text = &bytes[..text_end];
            bytes = &bytes[(text_end + 1).min(bytes.len())..];
            if let Ok(number) = std::str::from_utf8(number)
                && let Ok(line) = number.parse()
            {
                return Some((path, line, text));
            }
        }
    })
}

fn changed_documents(run: &ReviewRun, target: &str) -> Result<Vec<Map<String, Value>>> {
    let repo = git::repository(run)?;
    let root = repo
        .workdir()
        .ok_or_else(|| git::operation("locate worktree root", "repository has no worktree"))?;
    let diff = git::target_diff(&repo, target)?;
    let mut docs = Vec::new();
    let mut seen = HashSet::new();
    for (index, delta) in diff.deltas().enumerate() {
        let path = git::delta_path(&delta)?;
        if path.starts_with(b".code-review/")
            || !DOCUMENT_EXTENSIONS.iter().any(|extension| {
                path.len() >= extension.len()
                    && path[path.len() - extension.len()..]
                        .eq_ignore_ascii_case(extension.as_bytes())
            })
        {
            continue;
        }
        let patch = git::patch(&diff, index)?;
        let delta = patch.delta();
        if (!delta.new_file().id().is_zero()
            && delta.old_file().id() == delta.new_file().id()
            && delta.old_file().mode() == delta.new_file().mode())
            || !seen.insert(path)
        {
            continue;
        }
        let path_arg = git::raw_path_arg(path)?;
        let source = root.join(path_arg);
        if !source.is_file() {
            continue;
        }
        let bytes = fs::metadata(&source)
            .map_err(|error| crate::io_error("read document metadata", source.as_path(), error))?
            .len();
        let mut document = Map::new();
        document.insert(
            "path".to_owned(),
            Value::String(String::from_utf8_lossy(path).into_owned()),
        );
        document.insert("bytes".to_owned(), Value::from(bytes));
        docs.push(document);
    }
    Ok(docs)
}

fn render(symbols: &[Symbol], cap: usize) -> Vec<String> {
    let mut output = Vec::new();
    let mut unused = Vec::new();
    let mut common = Vec::new();
    for symbol in symbols {
        let location = match symbol.line {
            Some(line) => format!("{}:{line}", symbol.file),
            None => symbol.file.clone(),
        };
        if symbol.ambiguous {
            common.push(format!(
                "{} ({location}, {} textual hits)",
                symbol.name, symbol.usage_count
            ));
        } else if symbol.usages.is_empty() {
            unused.push(format!("{} ({}, {location})", symbol.name, symbol.kind));
        } else {
            let more = if symbol.usage_count > cap {
                format!(", first {cap} shown")
            } else {
                String::new()
            };
            output.push(format!(
                "## {}  ({}, {}, {location}) — {} usages{more}",
                symbol.name, symbol.kind, symbol.status, symbol.usage_count
            ));
            output.extend(
                symbol
                    .usages
                    .iter()
                    .take(cap)
                    .map(|usage| format!("{}:{}: {}", usage.file, usage.line, usage.text)),
            );
            output.push(String::new());
        }
    }
    if !unused.is_empty() {
        output.push(
            "## defined or changed by this diff, used NOWHERE else in code (tests have no callers; for anything else this is worth a look):"
                .to_owned(),
        );
        output.push(unused.join("; "));
        output.push(String::new());
    }
    if !common.is_empty() {
        output.push(
            "## names too common for a textual map to mean anything — search these yourself:"
                .to_owned(),
        );
        output.push(common.join("; "));
    }
    if output.is_empty() {
        output.push("(no symbols detected)".to_owned());
    }
    output
}

fn body_characters(lines: &[String]) -> usize {
    lines.iter().map(|line| line.chars().count() + 1).sum()
}

fn write_pages(run: &ReviewRun, lines: &[String]) -> Result<Vec<String>> {
    let directory = facts_dir(run);
    fs::create_dir_all(&directory)
        .map_err(|source| crate::io_error("create facts directory", directory.as_path(), source))?;
    for entry in fs::read_dir(&directory)
        .map_err(|source| crate::io_error("read facts directory", directory.as_path(), source))?
    {
        let entry = entry.map_err(|source| {
            crate::io_error("read facts directory entry", directory.as_path(), source)
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(number) = name
            .strip_prefix("usages-")
            .and_then(|rest| rest.strip_suffix(".txt"))
            && !number.is_empty()
            && number.chars().all(|character| character.is_ascii_digit())
        {
            fs::remove_file(entry.path()).map_err(|source| {
                crate::io_error("remove stale facts page", entry.path(), source)
            })?;
        }
    }

    let mut pages = Vec::new();
    let mut start = 0;
    let mut size = 0usize;
    for (index, line) in lines.iter().enumerate() {
        let line_size = line.chars().count() + 1;
        if index > start && size + line_size > PAGE_BUDGET {
            pages.push(&lines[start..index]);
            start = index;
            size = 0;
        }
        size += line_size;
    }
    pages.push(&lines[start..]);

    let header = "usages of every symbol this diff adds or changes — textual (git grep -w, same language), so a comment or a same-named symbol counts. Start here for callers instead of grepping.";
    let total = pages.len();
    let mut paths = Vec::with_capacity(total);
    for (index, page) in pages.into_iter().enumerate() {
        let relative = format!("facts/usages-{}.txt", index + 1);
        let mut text = format!("# {header} -- page {} of {total}\n", index + 1);
        text.reserve(page.iter().map(|line| line.len() + 1).sum());
        for (line_index, line) in page.iter().enumerate() {
            if line_index > 0 {
                text.push('\n');
            }
            text.push_str(line);
        }
        text.push('\n');
        write_text(&run.directory().join(&relative), &text)?;
        paths.push(relative);
    }
    Ok(paths)
}

fn is_common(name: &str) -> bool {
    COMMON.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::{body_characters, write_pages};
    use crate::ReviewRun;

    #[test]
    fn pages_pack_unicode_scalars_greedily_without_losing_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let tree = tempfile::tempdir()?;
        let run = ReviewRun::new(tree.path(), tree.path().join("run"))?;
        let lines = ["λ".repeat(9_999), "x".repeat(9_999), "after".to_owned()];
        assert_eq!(body_characters(&lines), 20_006);

        let pages = write_pages(&run, &lines)?;
        assert_eq!(pages, ["facts/usages-1.txt", "facts/usages-2.txt"]);
        let first = std::fs::read_to_string(run.directory().join(&pages[0]))?;
        let second = std::fs::read_to_string(run.directory().join(&pages[1]))?;
        let first_lines = first.trim_end().lines().collect::<Vec<_>>();
        assert!(first_lines.ends_with(&[lines[0].as_str(), lines[1].as_str()]));
        assert_eq!(second.trim_end().lines().last(), Some("after"));
        Ok(())
    }
}
