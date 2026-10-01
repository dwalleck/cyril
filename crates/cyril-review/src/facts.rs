use crate::git;
use crate::run::{
    Manifest, ManifestFile, ReviewRun, facts_dir, facts_metadata, manifest_path, read_manifest,
    read_text, write_json, write_text,
};
use crate::{Result, StepOutput, regex_error};
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, btree_map::Entry};
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
}

pub fn facts(run: &ReviewRun) -> Result<StepOutput> {
    let mut manifest = read_manifest(run)?;
    let line = build_facts(run, &mut manifest)?;
    Ok(StepOutput::from_text(line))
}

pub(crate) fn build_facts(run: &ReviewRun, manifest: &mut Manifest) -> Result<String> {
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
    let hunk_regex =
        Regex::new(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@ ?(.*)").map_err(regex_error)?;
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
        let patch = read_text(&patch_path)?;
        let mut new_line = 0usize;
        let mut previous_added = String::new();
        for line in patch.lines() {
            if let Some(captures) = hunk_regex.captures(line) {
                let Some(start) = captures.get(1) else {
                    continue;
                };
                let Ok(start) = start.as_str().parse::<usize>() else {
                    continue;
                };
                new_line = start.saturating_sub(1);
                if let Some(context) = captures.get(2)
                    && let Some(definition_match) = definition.captures(context.as_str())
                {
                    let Some(name) = definition_match.get(2) else {
                        continue;
                    };
                    let Some(kind) = definition_match.get(1) else {
                        continue;
                    };
                    let key = (file.path_bytes(), name.as_str().to_owned());
                    found.entry(key).or_insert_with(|| SymbolSeed {
                        name: name.as_str().to_owned(),
                        kind: kind.as_str().to_owned(),
                        file,
                        line: None,
                        status: "modified".to_owned(),
                    });
                }
                previous_added.clear();
                continue;
            }
            if line.starts_with("+++")
                || line.starts_with("---")
                || (new_line == 0 && !line.starts_with('+') && !line.starts_with(' '))
            {
                continue;
            }
            if line.starts_with('-') {
                continue;
            }
            new_line += 1;
            if let Some(added) = line.strip_prefix('+') {
                let candidate = added.trim_start();
                if let Some(definition_match) = definition.captures(candidate)
                    && definition_match
                        .get(0)
                        .is_some_and(|matched| matched.start() == 0)
                    && !test_attr.is_match(&previous_added)
                {
                    let Some(name) = definition_match.get(2) else {
                        continue;
                    };
                    let Some(kind) = definition_match.get(1) else {
                        continue;
                    };
                    let key = (file.path_bytes(), name.as_str().to_owned());
                    found.insert(
                        key,
                        SymbolSeed {
                            name: name.as_str().to_owned(),
                            kind: kind.as_str().to_owned(),
                            file,
                            line: Some(new_line),
                            status: "added".to_owned(),
                        },
                    );
                }
                let trimmed = added.trim();
                if !trimmed.is_empty() {
                    previous_added = trimmed.to_owned();
                }
            } else {
                previous_added.clear();
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
        "grep".to_owned(),
        "-n".to_owned(),
        "-z".to_owned(),
        "-w".to_owned(),
        "-F".to_owned(),
        "-I".to_owned(),
        "--".to_owned(),
        symbol.name.clone(),
        "--".to_owned(),
    ];
    args.extend(pattern.usage_globs.iter().map(|glob| (*glob).to_owned()));
    let output = git::grep(run, &args)?;
    let mut hits = Vec::new();
    let mut cursor = 0usize;
    while cursor < output.len() {
        let Some(path_offset) = output[cursor..].iter().position(|byte| *byte == 0) else {
            break;
        };
        let path_end = cursor + path_offset;
        let path = &output[cursor..path_end];
        cursor = path_end + 1;

        let Some(line_offset) = output[cursor..].iter().position(|byte| *byte == 0) else {
            break;
        };
        let line_end = cursor + line_offset;
        let number = &output[cursor..line_end];
        cursor = line_end + 1;

        let text_end = cursor
            + output[cursor..]
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap_or(output.len() - cursor);
        let text = &output[cursor..text_end];
        cursor = if text_end < output.len() {
            text_end + 1
        } else {
            text_end
        };

        let Ok(number) = std::str::from_utf8(number) else {
            continue;
        };
        let Ok(line) = number.parse::<usize>() else {
            continue;
        };
        if path.starts_with(b".code-review/")
            || (path == symbol.file.path_bytes()
                && symbol
                    .line
                    .is_some_and(|definition_line| definition_line == line))
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

fn changed_documents(run: &ReviewRun, target: &str) -> Result<Vec<Map<String, Value>>> {
    let args = vec![
        "diff".to_owned(),
        "--no-renames".to_owned(),
        "--name-only".to_owned(),
        "-z".to_owned(),
        target.to_owned(),
    ];
    let names = git::bytes(run, &args)?;
    let mut docs = Vec::new();
    for path in names.split(|byte| *byte == 0) {
        if path.is_empty() {
            continue;
        }
        if path.starts_with(b".code-review/")
            || !DOCUMENT_EXTENSIONS.iter().any(|extension| {
                path.len() >= extension.len()
                    && path[path.len() - extension.len()..]
                        .eq_ignore_ascii_case(extension.as_bytes())
            })
        {
            continue;
        }
        let path_arg = git::raw_path_arg(path)?;
        let source = run.workspace().join(path_arg);
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
