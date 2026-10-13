//! `finalize` ranks the verified findings into `findings.json`, `report.md`
//! and the commenter's briefs; `comments` turns the commenter's files into
//! postable Conventional Comments (conventionalcomments.org).

use crate::record::{Record, blank, id, object, one_line, quoted, required_records, text, to_line};
use crate::run::{ReviewRun, read_json, read_manifest, write, write_json, write_pages};
use crate::{Result, Verdict, io_error};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;

/// Findings reported in full; the rest are recorded below the cap.
const MAX_FINDINGS: usize = 15;
const LABELS: [&str; 12] = [
    "praise",
    "nitpick",
    "suggestion",
    "issue",
    "todo",
    "question",
    "thought",
    "chore",
    "note",
    "typo",
    "polish",
    "quibble",
];
/// Non-blocking by nature, per the Conventional Comments spec.
const NEVER_BLOCKING: [&str; 4] = ["nitpick", "thought", "note", "praise"];

/// One reported finding, as `findings.json` holds it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Finding {
    pub id: String,
    pub file: Option<String>,
    pub line: Option<i64>,
    pub summary: Option<String>,
    pub failure_scenario: Option<String>,
    pub verdict: Verdict,
    pub angles: Vec<String>,
    /// The rendered review comment, once `comments` has run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Set instead of `comment` when this is the same defect as a better-ranked finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duplicate_of: Option<String>,
}

/// Why `findings.json` could not be read: never reported as "0 findings".
#[derive(Debug, thiserror::Error)]
pub enum FindingsError {
    #[error("{} is missing: the review produced no findings file", .path.display())]
    Missing { path: std::path::PathBuf },
    #[error("{} is corrupt: {source}", .path.display())]
    Corrupt {
        path: std::path::PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("cannot read {}: {source}", .path.display())]
    Unreadable {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Read a finished run's `findings.json`.
pub fn read_findings(run: &ReviewRun) -> std::result::Result<Vec<Finding>, FindingsError> {
    let path = run.path("findings.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Err(FindingsError::Missing { path });
        }
        Err(source) => return Err(FindingsError::Unreadable { path, source }),
    };
    serde_json::from_slice(&bytes).map_err(|source| FindingsError::Corrupt { path, source })
}

/// `crtool finalize <rundir>`
pub fn finalize(run: &ReviewRun) -> Result<String> {
    let manifest = read_manifest(run)?;
    let verified_path = run.path("verified.json");
    let verified: Record = read_json(&verified_path)?;
    let kept = required_records(&verified, "kept", &verified_path)?;
    let refuted = required_records(&verified, "refuted", &verified_path)?;
    let mut warnings: Vec<String> = strings(verified.get("warnings"));

    let Ranking { order, notes } = ranking(run, &mut warnings);
    let mut ranked: Vec<&Record> = Vec::new();
    let mut seen = HashSet::new();
    for wanted in &order {
        match kept.iter().find(|record| id(record) == *wanted) {
            Some(record) => {
                if seen.insert(wanted.clone()) {
                    ranked.push(record);
                }
            }
            None => warnings.push(format!(
                "ranking names unknown or refuted id {}; ignored",
                quoted(wanted)
            )),
        }
    }
    // Recall mode: an unranked finding is appended, never dropped.
    let unranked: Vec<&Record> = kept
        .iter()
        .filter(|record| !seen.contains(&id(record)))
        .collect();
    if !unranked.is_empty() {
        let ids: Vec<String> = unranked.iter().map(|record| id(record)).collect();
        warnings.push(format!(
            "not ranked, appended in discovery order: {}",
            ids.join(", ")
        ));
        ranked.extend(unranked);
    }
    let top = &ranked[..ranked.len().min(MAX_FINDINGS)];
    let below_cap = ranked.len() - top.len();
    let findings: Vec<Finding> = top.iter().map(|record| finding(record)).collect();
    write_json(&run.path("findings.json"), &findings)?;

    let stats = object(verified.get("stats"));
    let list = |key: &str| strings(stats.get(key));
    let number = |key: &str| stats.get(key).map(text).unwrap_or_else(|| "?".to_owned());
    let missing = list("angles_missing");
    let verdicts = object(stats.get("verdicts"));
    let balloted = stats.get("balloted").and_then(Value::as_u64).unwrap_or(0);
    let mut md = vec![
        format!("# Code review — `{}`", manifest.target),
        String::new(),
        format!(
            "- **Head:** `{}`  **Scope:** `{}`  **Files:** {}  **Patch bytes:** {}",
            manifest.head.chars().take(12).collect::<String>(),
            manifest.scope.join(" "),
            manifest.total_files,
            manifest.total_patch_bytes
        ),
        format!(
            "- **Pipeline:** {} finder angles reported{} → {} candidates → {} after dedup + {} from the gap sweep",
            list("angles_reported").len(),
            if missing.is_empty() { String::new() } else { format!(" (**missing: {}**)", missing.join(", ")) },
            number("raw_candidates"),
            number("after_dedup"),
            number("sweep_candidates")
        ),
        format!(
            "- **Verdicts:** {}{}",
            Verdict::REPORT_ORDER
                .iter()
                .map(|name| format!("{name} {}", verdicts.get(name.as_str()).map(text).unwrap_or_else(|| "0".to_owned())))
                .collect::<Vec<_>>()
                .join(", "),
            if balloted > 0 {
                format!(
                    " — {balloted} candidates took three votes, {} changed outcome",
                    list("overturned_by_vote").len()
                )
            } else {
                String::new()
            }
        ),
        "- `CONFIRMED` = trigger and wrong outcome named, line quoted. `PLAUSIBLE` = mechanism real, trigger uncertain. `UNVERIFIED` = no verdict was produced; kept because this is a recall-mode review.".to_owned(),
        String::new(),
        "## Summary".to_owned(),
        String::new(),
        "| # | Id | Location | Verdict | Issue |".to_owned(),
        "|---|---|---|---|---|".to_owned(),
    ];
    for (number, record) in (1..).zip(&ranked) {
        md.push(format!(
            "| {number} | {} | {} | {} | {} |",
            id(record),
            location_md(record),
            field(record, "verdict"),
            cell(record.get("summary"))
        ));
        if number == MAX_FINDINGS && below_cap > 0 {
            md.push(format!(
                "| | | | | *— reporting cap ({MAX_FINDINGS}); items below are recorded in rank order —* |"
            ));
        }
    }
    md.extend(["".to_owned(), "## Findings".to_owned(), String::new()]);
    for (number, record) in (1..).zip(&ranked) {
        let verification = verification(record);
        let failure = record
            .get("failure_scenario")
            .filter(|value| !blank(Some(value)));
        md.push(format!("### {number}. {}", cell(record.get("summary"))));
        md.push(String::new());
        md.push(format!(
            "- **Where:** {}  **Verdict:** {}  **Id:** {}  **Angles:** {}",
            location_md(record),
            field(record, "verdict"),
            id(record),
            angles(record).join(", ")
        ));
        md.push(format!(
            "- **Failure scenario:** {}",
            failure.map_or_else(|| "—".to_owned(), text)
        ));
        let votes = strings(record.get("votes"));
        if !votes.is_empty() {
            md.push(format!(
                "- **Votes:** {} → {}",
                votes.join(", "),
                field(record, "verdict")
            ));
        }
        for (value, label) in [
            (record.get("by_design"), "Accepted by design"),
            (record.get("evidence"), "Finder evidence"),
            (verification.get("evidence"), "Verifier evidence"),
            (verification.get("reasoning"), "Verifier reasoning"),
            (verification.get("would_confirm"), "Would confirm"),
            (notes.get(&id(record)), "Ranking note"),
        ] {
            if let Some(value) = value.filter(|value| !blank(Some(value))) {
                md.push(format!("- **{label}:** {}", text(value)));
            }
        }
        md.push(String::new());
    }
    md.extend([
        "## Refuted during verification".to_owned(),
        String::new(),
        "Listed so they are not re-derived later.".to_owned(),
        String::new(),
    ]);
    if refuted.is_empty() {
        md.push("None.".to_owned());
    }
    for record in &refuted {
        let verification = verification(record);
        let votes = strings(record.get("votes"));
        let votes = if votes.is_empty() {
            String::new()
        } else {
            format!(" *(votes: {})*", votes.join(", "))
        };
        let why = [verification.get("reasoning"), verification.get("evidence")]
            .into_iter()
            .flatten()
            .find(|value| !blank(Some(value)))
            .map_or_else(|| "no reasoning recorded".to_owned(), text);
        md.push(format!(
            "- **{}** {} — {}{votes}  \n  *Refuted:* {why}",
            id(record),
            location_md(record),
            cell(record.get("summary"))
        ));
    }
    if !warnings.is_empty() {
        md.extend(["".to_owned(), "## Run warnings".to_owned(), String::new()]);
        md.extend(warnings.iter().map(|warning| format!("- {warning}")));
    }
    write(
        &run.path("report.md"),
        format!("{}\n", md.join("\n")).as_bytes(),
    )?;

    // Everything the commenter needs, in one place, so it never has to find a
    // verdict file or page through verified.json.
    let mut brief = Vec::new();
    for (rank, record) in (1..).zip(top) {
        let verification = verification(record);
        let angles = angles(record);
        let votes = strings(record.get("votes"));
        brief.push(format!(
            "===== {}  (rank {rank})  {}",
            id(record),
            brief_location(record)
        ));
        brief.push(format!(
            "verdict: {}{}  | category: {}  | raised by {} angle(s): {}",
            field(record, "verdict"),
            if votes.is_empty() {
                String::new()
            } else {
                format!("  votes: {}", votes.join(", "))
            },
            record.get("category").map_or_else(|| "?".to_owned(), text),
            angles.len(),
            angles.join(", ")
        ));
        brief.push(format!("summary: {}", one_line(record.get("summary"), 600)));
        brief.push(format!(
            "failure scenario: {}",
            one_line(record.get("failure_scenario"), 900)
        ));
        for (value, label, limit) in [
            (record.get("evidence"), "finder evidence", 500),
            (verification.get("evidence"), "verifier evidence", 900),
            (verification.get("reasoning"), "verifier reasoning", 900),
            (verification.get("would_confirm"), "would confirm", 900),
            (
                record.get("by_design"),
                "ACCEPTED BY DESIGN (the author documented this)",
                500,
            ),
        ] {
            if !blank(value) {
                brief.push(format!("{label}: {}", one_line(value, limit)));
            }
        }
        brief.push(String::new());
    }
    if brief.is_empty() {
        brief.push("(no findings)".to_owned());
    }
    let pages = write_pages(
        run,
        "comments/brief",
        "one block per reported finding, in rank order",
        &brief,
    )?;
    Ok(format!(
        "comment briefs (one block per reported finding): {}\nfinalized: {} findings reported, {below_cap} below the cap, {} refuted -> findings.json, report.md\n",
        pages.join(" "),
        top.len(),
        refuted.len()
    ))
}

/// `crtool comments <rundir> [--no-trailer]`
pub fn comments(run: &ReviewRun, trailer: bool) -> Result<String> {
    read_manifest(run)?;
    let mut findings: Vec<Finding> = read_json(&run.path("findings.json"))?;
    let verified_path = run.path("verified.json");
    let verified: Record = read_json(&verified_path)?;
    let kept = required_records(&verified, "kept", &verified_path)?;
    let reported: HashSet<String> = findings.iter().map(|finding| finding.id.clone()).collect();
    let (mut out, mut warnings) = (Vec::new(), Vec::new());
    let (mut model, mut template, mut duplicates, mut blocking) = (0, 0, 0, 0);
    let mut labels: Vec<(String, usize)> = Vec::new();
    let mut commented: HashSet<String> = HashSet::new();
    for finding in &mut findings {
        let found = kept.iter().find(|record| id(record) == finding.id);
        if found.is_none() {
            warnings.push(format!(
                "{}: no record in verified.json; its votes and by-design note are unknown, so it never blocks",
                finding.id
            ));
        }
        let record = found.cloned().unwrap_or_else(Record::new);
        let mut written = comment_file(run, &finding.id, &mut warnings);
        let mut entry = json!({"id": finding.id, "file": finding.file, "line": finding.line, "verdict": finding.verdict});
        if let Some(duplicate_of) = written
            .as_ref()
            .and_then(|written| written.get("duplicate_of"))
            .filter(|value| !blank(Some(value)))
        {
            let target = text(duplicate_of);
            // Only a better-ranked finding with its own comment: a mutual or
            // chained duplicate would leave the defect with no comment at all.
            if commented.contains(&target) {
                entry["duplicate_of"] = json!(target);
                entry["source"] = json!("model");
                out.push(entry);
                duplicates += 1;
                finding.comment = None;
                finding.duplicate_of = Some(target);
                continue;
            }
            let why = if reported.contains(&target) {
                "is not a better-ranked finding with its own comment"
            } else {
                "is not a reported finding"
            };
            warnings.push(format!(
                "{}: duplicate_of names {}, which {why}; template used",
                finding.id,
                quoted(&target)
            ));
            written = None;
        }
        let resolution = resolve_comment(finding, found, written);
        warnings.extend(resolution.warnings);
        let comment = resolution.comment;
        let votes: Vec<Verdict> = record
            .get("votes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|vote| Verdict::from_agent(Some(vote)))
            .collect();
        let body = render_comment(&comment, finding, &votes, trailer);
        let NormalizedComment {
            label,
            decorations,
            subject,
            discussion,
            source,
        } = comment;
        if has_decoration(&decorations, "blocking") {
            blocking += 1;
        }
        match labels.iter_mut().find(|(name, _)| *name == label) {
            Some((_, count)) => *count += 1,
            None => labels.push((label.clone(), 1)),
        }
        if source == CommentSource::Model {
            model += 1
        } else {
            template += 1
        }
        entry["label"] = json!(label);
        entry["decorations"] = json!(decorations);
        entry["subject"] = json!(subject);
        entry["discussion"] = json!(discussion);
        entry["body"] = json!(body);
        entry["source"] = json!(source);
        out.push(entry);
        commented.insert(finding.id.clone());
        finding.duplicate_of = None;
        finding.comment = Some(body);
    }
    write_json(&run.path("comments.json"), &out)?;
    write_json(&run.path("findings.json"), &findings)?;

    let mut md = vec![
        "# Review comments".to_owned(),
        String::new(),
        "Conventional Comments format (conventionalcomments.org). Each block is postable as-is at the location above it."
            .to_owned(),
        String::new(),
    ];
    for (number, entry) in (1..).zip(&out) {
        let file = entry["file"].as_str().unwrap_or("?");
        let at = match entry["line"].as_i64() {
            Some(line) if line != 0 => format!("`{file}:{line}`"),
            _ => format!("`{file}`"),
        };
        md.push(format!("## {number}. {at} — {}", text(&entry["id"])));
        md.push(String::new());
        match entry.get("duplicate_of") {
            Some(target) => md.push(format!(
                "Same defect as {}; no separate comment.",
                text(target)
            )),
            None => md.push(text(&entry["body"])),
        }
        md.push(String::new());
    }
    let rendered = format!("{}\n", md.join("\n"));
    write(&run.path("comments.md"), rendered.as_bytes())?;
    let report_path = run.path("report.md");
    if report_path.exists() {
        let report = fs::read_to_string(&report_path)
            .map_err(|source| io_error("read", &report_path, source))?;
        let report = report
            .split("\n# Review comments\n")
            .next()
            .unwrap_or_default()
            .trim_end_matches('\n');
        write(&report_path, format!("{report}\n\n{rendered}").as_bytes())?;
    }
    let labels: Vec<String> = labels
        .iter()
        .map(|(label, count)| format!("'{label}': {count}"))
        .collect();
    let mut stdout = format!(
        "comments: {} findings -> {model} model-written, {template} template, {duplicates} marked duplicate | labels {{{}}} | blocking: {blocking}",
        out.len(),
        labels.join(", ")
    );
    if !warnings.is_empty() {
        stdout += &format!(" | WARNINGS: {}", warnings.join("; "));
    }
    stdout += " -> comments.json, comments.md\n";
    Ok(stdout)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum CommentSource {
    Model,
    Template,
}

struct NormalizedComment {
    label: String,
    decorations: Vec<String>,
    subject: String,
    discussion: String,
    source: CommentSource,
}

struct CommentResolution {
    comment: NormalizedComment,
    warnings: Vec<String>,
}

/// Interpret one model comment and enforce blocking policy without file I/O.
fn resolve_comment(
    finding: &Finding,
    verification: Option<&Record>,
    written: Option<Record>,
) -> CommentResolution {
    let mut warnings = Vec::new();
    let empty = Record::new();
    let record = verification.unwrap_or(&empty);
    // Unknown verification never authorizes a blocking comment.
    let by_design = verification.is_none_or(|record| !blank(record.get("by_design")));
    let parsed = written.and_then(|written| {
        let label = str_field(&written, "label").trim().to_lowercase();
        let label = label.trim_end_matches(':').to_owned();
        let subject = str_field(&written, "subject")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if LABELS.contains(&label.as_str()) && !subject.is_empty() {
            Some((label, subject, written))
        } else {
            warnings.push(format!(
                "{}: label {} or subject invalid; template used",
                finding.id,
                written
                    .get("label")
                    .map_or_else(|| "null".to_owned(), |label| quoted(&text(label)))
            ));
            None
        }
    });
    let source = if parsed.is_some() {
        CommentSource::Model
    } else {
        CommentSource::Template
    };
    let (label, subject, written) =
        parsed.unwrap_or_else(|| template_comment(finding, record, by_design));
    let mut decorations: Vec<String> = Vec::new();
    for decoration in written
        .get("decorations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let decoration = text(decoration).trim().to_lowercase();
        let decoration = decoration.trim_matches(|c| c == '(' || c == ')').to_owned();
        if decoration.is_empty() || decorations.contains(&decoration) {
            continue;
        }
        let word = decoration.starts_with(|c: char| c.is_ascii_lowercase())
            && decoration
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if word {
            decorations.push(decoration);
        } else {
            warnings.push(format!(
                "{}: decoration {} dropped (not a lowercase word)",
                finding.id,
                quoted(&decoration)
            ));
        }
    }
    let confirmed = finding.verdict == Verdict::Confirmed;
    // This pipeline does not block a merge on a claim it could not confirm
    // or that the author documented, nor on a label the spec calls non-blocking.
    if has_decoration(&decorations, "blocking")
        && (NEVER_BLOCKING.contains(&label.as_str()) || !confirmed || by_design)
    {
        for decoration in &mut decorations {
            if decoration == "blocking" {
                *decoration = "non-blocking".to_owned();
            }
        }
        warnings.push(format!(
            "{}: `blocking` downgraded (label {label}, verdict {}{})",
            finding.id,
            finding.verdict,
            if by_design { ", by design" } else { "" }
        ));
    }
    if has_decoration(&decorations, "blocking") {
        decorations.retain(|decoration| decoration != "non-blocking");
    }
    if (!confirmed || by_design) && !has_decoration(&decorations, "non-blocking") {
        // Say so explicitly: a bare label reads as "must fix".
        decorations.push("non-blocking".to_owned());
    }
    let discussion = written
        .get("discussion")
        .filter(|value| !blank(Some(value)))
        .map(text)
        .unwrap_or_default()
        .trim()
        .to_owned();
    CommentResolution {
        comment: NormalizedComment {
            label,
            decorations,
            subject,
            discussion,
            source,
        },
        warnings,
    }
}

fn render_comment(
    comment: &NormalizedComment,
    finding: &Finding,
    votes: &[Verdict],
    trailer: bool,
) -> String {
    let NormalizedComment {
        label,
        decorations,
        subject,
        discussion,
        ..
    } = comment;
    let head = if decorations.is_empty() {
        format!("**{label}:** {subject}")
    } else {
        format!("**{label} ({}):** {subject}", decorations.join(","))
    };
    let mut footer = format!(
        "_Automated review · {}",
        finding.verdict.as_str().to_lowercase()
    );
    if !votes.is_empty() {
        footer += &format!(
            " by vote ({})",
            votes
                .iter()
                .map(|vote| vote.as_str())
                .collect::<Vec<_>>()
                .join(", ")
                .to_lowercase()
        );
    }
    if finding.angles.len() > 1 {
        footer += &format!(
            " · raised independently by {} review angles",
            finding.angles.len()
        );
    }
    footer += &format!(" · {}_", finding.id);
    [
        Some(head),
        (!discussion.is_empty()).then(|| discussion.clone()),
        trailer.then_some(footer),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n\n")
}

fn has_decoration(decorations: &[String], wanted: &str) -> bool {
    decorations.iter().any(|decoration| decoration == wanted)
}

/// A string field of an agent's record; anything else reads as empty.
fn str_field<'a>(record: &'a Record, key: &str) -> &'a str {
    record.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// The commenter's file for one finding, or `None` (with a warning when it
/// exists but cannot be used).
fn comment_file(run: &ReviewRun, finding_id: &str, warnings: &mut Vec<String>) -> Option<Record> {
    let path = run.path(&format!("comments/{finding_id}.json"));
    if !path.exists() {
        return None;
    }
    let problem = match read_json::<Value>(&path) {
        Ok(Value::Object(written)) => return Some(written),
        Ok(_) => "not a JSON object".to_owned(),
        Err(error) => crate::record::unreadable(&error),
    };
    warnings.push(format!(
        "{finding_id}: comment file unreadable ({problem}); template used"
    ));
    None
}

/// The fallback when no usable model-written comment exists: mechanical, but never absent.
fn template_comment(
    finding: &Finding,
    record: &Record,
    by_design: bool,
) -> (String, String, Record) {
    let category = record
        .get("category")
        .map(text)
        .unwrap_or_default()
        .to_lowercase();
    let (label, decorations): (&str, &[&str]) = if by_design {
        ("thought", &["non-blocking"])
    } else if finding.verdict != Verdict::Confirmed {
        ("question", &["non-blocking"])
    } else if category == "cleanup" || category == "altitude" {
        ("suggestion", &["non-blocking"])
    } else {
        ("issue", &[])
    };
    let mut parts = vec![
        finding
            .failure_scenario
            .clone()
            .unwrap_or_default()
            .trim()
            .to_owned(),
    ];
    if by_design && let Some(note) = record.get("by_design") {
        parts.push(format!("This looks deliberate: {}", text(note)));
    }
    let discussion = parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    let subject = one_line(finding.summary.clone().map(Value::String).as_ref(), 240);
    let written = json!({"label": label, "decorations": decorations, "subject": subject, "discussion": discussion});
    (
        label.to_owned(),
        subject,
        written.as_object().cloned().unwrap_or_default(),
    )
}

#[derive(Default)]
struct Ranking {
    order: Vec<String>,
    notes: Record,
}

fn decode_ranking(value: Value) -> Result<Ranking, &'static str> {
    let Value::Object(mut fields) = value else {
        return Err("not a JSON object");
    };
    let order = match fields.remove("order") {
        None => Vec::new(),
        Some(Value::Array(items)) => items.iter().map(text).collect(),
        Some(_) => return Err("order is not an array"),
    };
    let notes = match fields.remove("notes") {
        None => Record::new(),
        Some(Value::Object(notes)) => notes,
        Some(_) => return Err("notes is not an object"),
    };
    Ok(Ranking { order, notes })
}

fn ranking(run: &ReviewRun, warnings: &mut Vec<String>) -> Ranking {
    let path = run.path("ranking.json");
    if !path.exists() {
        warnings.push("ranking.json missing; findings are in discovery order".to_owned());
        return Ranking::default();
    }
    let result = read_json::<Value>(&path)
        .map_err(|error| crate::record::unreadable(&error))
        .and_then(|value| decode_ranking(value).map_err(str::to_owned));
    match result {
        Ok(ranking) => ranking,
        Err(problem) => {
            warnings.push(format!(
                "ranking.json unreadable ({problem}); findings are in discovery order"
            ));
            Ranking::default()
        }
    }
}

fn finding(record: &Record) -> Finding {
    let string = |key: &str| record.get(key).filter(|value| !value.is_null()).map(text);
    Finding {
        id: id(record),
        file: string("file"),
        line: best_line(record),
        summary: string("summary"),
        failure_scenario: string("failure_scenario"),
        verdict: Verdict::from_agent(record.get("verdict")),
        angles: angles(record),
        comment: None,
        duplicate_of: None,
    }
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().map(text).collect())
        .unwrap_or_default()
}

fn field(record: &Record, key: &str) -> String {
    record.get(key).map(text).unwrap_or_default()
}

fn verification(record: &Record) -> Record {
    object(record.get("verification"))
}

/// The finding's own angle followed by every angle that raised it too.
fn angles(record: &Record) -> Vec<String> {
    let also = record
        .get("also_flagged_by")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    std::iter::once(field(record, "angle"))
        .chain(also.map(|flagged| flagged.get("angle").map(text).unwrap_or_default()))
        .collect()
}

/// A verifier's corrected line wins over the finder's.
fn best_line(record: &Record) -> Option<i64> {
    let corrected = verification(record)
        .get("corrected_line")
        .map(|line| to_line(Some(line)));
    corrected
        .and_then(|line| line.as_i64())
        .filter(|line| *line != 0)
        .or_else(|| record.get("line").and_then(Value::as_i64))
}

fn location_md(record: &Record) -> String {
    let file = record
        .get("file")
        .filter(|file| !file.is_null())
        .map_or_else(|| "?".to_owned(), text);
    match best_line(record) {
        Some(line) if line != 0 => format!("`{file}:{line}`"),
        _ => format!("`{file}`"),
    }
}

/// `file:line` for a brief header: an absent file or line prints `?`.
fn brief_location(record: &Record) -> String {
    let file = record
        .get("file")
        .filter(|file| !file.is_null())
        .map_or_else(|| "?".to_owned(), text);
    let line = best_line(record).map_or_else(|| "?".to_owned(), |line| line.to_string());
    format!("{file}:{line}")
}

/// A Markdown table cell: pipes escaped, newlines flattened.
fn cell(value: Option<&Value>) -> String {
    match value {
        Some(value) if !blank(Some(value)) => text(value)
            .replace('|', "\\|")
            .replace('\n', " ")
            .trim()
            .to_owned(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(value: Value) -> Record {
        value.as_object().cloned().unwrap_or_default()
    }

    fn finding_json(id: &str, verdict: &str) -> Value {
        json!({"id": id, "file": "a.rs", "line": 1, "summary": format!("defect {id}"),
               "failure_scenario": "f", "verdict": verdict, "angles": ["a"]})
    }

    #[test]
    fn a_corrupt_verified_json_is_an_error_not_zero_findings() -> crate::Result<()> {
        let (_tree, run) = crate::run::stamped_run()?;
        write_json(
            &run.path("verified.json"),
            &json!({"refuted": [], "stats": {}}),
        )?;
        assert!(matches!(
            finalize(&run),
            Err(crate::ReviewError::CorruptRunFile { .. })
        ));
        Ok(())
    }

    #[test]
    fn duplicates_must_name_a_better_ranked_finding_with_its_own_comment()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_tree, run) = crate::run::stamped_run()?;
        let kept =
            json!([{"id": "C01", "verdict": "CONFIRMED"}, {"id": "C02", "verdict": "CONFIRMED"}]);
        write_json(
            &run.path("verified.json"),
            &json!({"kept": kept, "refuted": []}),
        )?;
        write_json(
            &run.path("findings.json"),
            &json!([
                finding_json("C01", "CONFIRMED"),
                finding_json("C02", "CONFIRMED")
            ]),
        )?;
        // A mutual pair: C01 may not point down the ranking; C02 may point up.
        write_json(
            &run.path("comments/C01.json"),
            &json!({"duplicate_of": "C02"}),
        )?;
        write_json(
            &run.path("comments/C02.json"),
            &json!({"duplicate_of": "C01"}),
        )?;
        let stdout = comments(&run, true)?;
        assert!(stdout.contains(
            "C01: duplicate_of names 'C02', which is not a better-ranked finding with its own comment; template used"
        ));
        let findings = crate::read_findings(&run)?;
        assert!(findings[0].comment.is_some() && findings[0].duplicate_of.is_none());
        assert_eq!(findings[1].duplicate_of.as_deref(), Some("C01"));
        Ok(())
    }

    #[test]
    fn ranking_decoder_distinguishes_absence_from_malformed_fields() {
        let missing = decode_ranking(json!({}));
        assert!(missing.is_ok_and(|ranking| ranking.order.is_empty() && ranking.notes.is_empty()));
        let valid = decode_ranking(json!({"order": ["C02", "C01"], "notes": {"C02": "important"}}));
        assert!(valid.is_ok_and(
            |ranking| ranking.order == ["C02", "C01"] && ranking.notes["C02"] == "important"
        ));
        for value in [
            json!(null),
            json!([]),
            json!({"order": null}),
            json!({"order": 42}),
            json!({"notes": null}),
            json!({"notes": 42}),
        ] {
            assert!(decode_ranking(value).is_err());
        }
    }

    #[test]
    fn malformed_ranking_warns_and_restores_discovery_order() -> crate::Result<()> {
        let (_tree, run) = crate::run::stamped_run()?;
        write_json(
            &run.path("verified.json"),
            &json!({"kept": [
            {"id": "C01", "verdict": "CONFIRMED"},
            {"id": "C02", "verdict": "PLAUSIBLE"}
        ], "refuted": [], "stats": {}}),
        )?;
        write_json(
            &run.path("ranking.json"),
            &json!({"order": ["C02", "C01"], "notes": 42}),
        )?;
        finalize(&run)?;
        let report = fs::read_to_string(run.path("report.md"))
            .map_err(|error| io_error("read", run.path("report.md"), error))?;
        assert!(report.contains("ranking.json unreadable"), "{report}");
        let findings: Vec<Finding> = read_json(&run.path("findings.json"))?;
        assert_eq!(findings[0].id, "C01");
        Ok(())
    }

    #[test]
    fn comment_policy_normalizes_labels_and_enforces_blocking() -> Result<(), serde_json::Error> {
        for (verdict, label, by_design, known, blocking) in [
            ("CONFIRMED", "issue", false, true, true),
            ("PLAUSIBLE", "issue", false, true, false),
            ("UNVERIFIED", "issue", false, true, false),
            ("CONFIRMED", "nitpick", false, true, false),
            ("CONFIRMED", "issue", true, true, false),
            ("CONFIRMED", "issue", false, false, false),
        ] {
            let finding: Finding = serde_json::from_value(finding_json("C01", verdict))?;
            let verification = if by_design {
                record(json!({"by_design": "accepted"}))
            } else {
                Record::new()
            };
            let written = record(
                json!({"label": format!(" {}: ", label.to_uppercase()), "subject": " a   subject ", "decorations": ["(BLOCKING)", "blocking", "bad word"]}),
            );
            let resolved = resolve_comment(&finding, known.then_some(&verification), Some(written));
            assert_eq!(resolved.comment.source, CommentSource::Model);
            assert_eq!(resolved.comment.subject, "a subject");
            assert_eq!(
                resolved.comment.decorations,
                if blocking {
                    vec!["blocking"]
                } else {
                    vec!["non-blocking"]
                }
            );
            assert!(
                resolved
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("bad word"))
            );
        }
        Ok(())
    }

    #[test]
    fn unusable_comments_fall_back_and_render_without_a_trailer() -> Result<(), serde_json::Error> {
        let finding: Finding = serde_json::from_value(finding_json("C01", "CONFIRMED"))?;
        let verification = Record::new();
        for written in [
            None,
            Some(record(json!({"label": "invalid", "subject": "s"}))),
            Some(record(json!({"label": "issue", "subject": "   "}))),
        ] {
            let invalid = written.is_some();
            let resolved = resolve_comment(&finding, Some(&verification), written);
            assert_eq!(resolved.comment.source, CommentSource::Template);
            assert_eq!(!resolved.warnings.is_empty(), invalid);
            let body = render_comment(&resolved.comment, &finding, &[], false);
            assert!(body.starts_with("**issue:**"));
            assert!(!body.contains("Automated review"));
            assert!(
                render_comment(&resolved.comment, &finding, &[], true).contains("Automated review")
            );
        }
        let written = record(
            json!({"label": "issue", "subject": "s", "decorations": ["blocking", "non-blocking"]}),
        );
        let resolved = resolve_comment(&finding, Some(&verification), Some(written));
        assert_eq!(resolved.comment.decorations, ["blocking"]);
        Ok(())
    }

    #[test]
    fn a_finding_without_a_verified_record_never_blocks() -> crate::Result<()> {
        let (_tree, run) = crate::run::stamped_run()?;
        write_json(
            &run.path("verified.json"),
            &json!({"kept": [], "refuted": []}),
        )?;
        write_json(
            &run.path("findings.json"),
            &json!([finding_json("C07", "CONFIRMED")]),
        )?;
        write_json(
            &run.path("comments/C07.json"),
            &json!({"label": "issue", "decorations": ["blocking"], "subject": "s", "discussion": "d"}),
        )?;
        let stdout = comments(&run, true)?;
        assert!(
            stdout.contains("C07: no record in verified.json"),
            "{stdout}"
        );
        assert!(stdout.contains("blocking: 0"), "{stdout}");
        Ok(())
    }

    #[test]
    fn an_absent_file_or_line_prints_a_question_mark() {
        let missing_file = record(json!({"file": null, "line": 4}));
        assert_eq!(location_md(&missing_file), "`?:4`");
        assert_eq!(brief_location(&missing_file), "?:4");
        let missing_line = record(json!({"file": "src/a.rs", "line": null}));
        assert_eq!(location_md(&missing_line), "`src/a.rs`");
        assert_eq!(brief_location(&missing_line), "src/a.rs:?");
        let corrected =
            record(json!({"file": "a.rs", "line": 3, "verification": {"corrected_line": 9}}));
        assert_eq!(brief_location(&corrected), "a.rs:9");
    }
}
