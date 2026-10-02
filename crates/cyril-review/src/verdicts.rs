//! `ballots` gives unstable verdicts two more votes; `collate` tallies every
//! vote into the kept and refuted findings.

use crate::merge::{also_count, id, write_queue};
use crate::record::{
    MAX_PER_ANGLE, Record, blank, load_candidates, location, one_line, text, to_line, unreadable,
};
use crate::run::{ReviewRun, read_json, read_manifest, write_json, write_pages};
use crate::{Result, io_error};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const VERDICTS: [&str; 3] = ["CONFIRMED", "PLAUSIBLE", "REFUTED"];
const UNVERIFIED: &str = "UNVERIFIED";
const BALLOT_TAGS: [(&str, &str); 2] = [("r1", "v2"), ("r2", "v3")];

/// `crtool ballots <rundir>`: a single vote proved unstable exactly where it
/// matters (identical runs flipped REFUTED <-> CONFIRMED on rule
/// interpretation), so every first-round REFUTED and every conventions claim
/// gets two more sessions that cannot see the first.
pub fn ballots(run: &ReviewRun) -> Result<String> {
    read_manifest(run)?;
    let (_, candidates, _) = all_candidates(run, None)?;
    let mut chosen = Vec::new();
    for candidate in &candidates {
        let candidate_id = id(candidate);
        let first = verdict_of(&load_verdict(run, &candidate_id)?);
        let mut why = Vec::new();
        if first == "REFUTED" {
            why.push("first vote REFUTED");
        }
        if is_conventions(candidate) {
            why.push("conventions claim");
        }
        if why.is_empty() {
            continue;
        }
        chosen.push(json!({"id": candidate_id, "first_vote": first, "reason": why.join(" + ")}));
        for (_, tag) in BALLOT_TAGS {
            let mut ballot = candidate.clone();
            ballot.insert("id".to_owned(), json!(format!("{candidate_id}.{tag}")));
            ballot.insert("ballot_of".to_owned(), json!(candidate_id));
            write_json(
                &run.path(&format!("deduped/{candidate_id}.{tag}.json")),
                &ballot,
            )?;
        }
    }
    for (queue, tag) in BALLOT_TAGS {
        let ids: Vec<String> = chosen
            .iter()
            .map(|ballot| format!("{}.{tag}", ballot["id"].as_str().unwrap_or_default()))
            .collect();
        write_queue(run, &format!("queue-{queue}"), queue, &ids)?;
    }
    let reason_count = |word: &str| {
        chosen
            .iter()
            .filter(|ballot| {
                ballot["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.contains(word))
            })
            .count()
    };
    let (refuted, conventions) = (reason_count("REFUTED"), reason_count("conventions"));
    let count = chosen.len();
    write_json(&run.path("ballots.json"), &json!({"balloted": chosen}))?;
    Ok(format!(
        "ballots: {count} of {} candidates get two more votes ({refuted} refuted, {conventions} conventions) -> queues/queue-r1.json, queue-r2.json\n",
        candidates.len()
    ))
}

/// `crtool collate <rundir>`
pub fn collate(run: &ReviewRun) -> Result<String> {
    read_manifest(run)?;
    let mut sweep_warnings = Vec::new();
    let (index, candidates, sweep_count) = all_candidates(run, Some(&mut sweep_warnings))?;
    let mut warnings: Vec<Value> = index
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    warnings.extend(sweep_warnings.into_iter().map(Value::String));
    let ballots_path = run.path("ballots.json");
    let balloted: HashSet<String> = if ballots_path.exists() {
        let ballots: Value = read_json(&ballots_path)?;
        ballots["balloted"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|ballot| text(&ballot["id"]))
            .collect()
    } else {
        HashSet::new()
    };

    let (mut kept, mut refuted, mut overturned) = (Vec::new(), Vec::new(), Vec::new());
    for candidate in &candidates {
        let candidate_id = id(candidate);
        let is_balloted = balloted.contains(&candidate_id);
        let mut ballots = vec![load_verdict(run, &candidate_id)?];
        if is_balloted {
            for (_, tag) in BALLOT_TAGS {
                ballots.push(load_verdict(run, &format!("{candidate_id}.{tag}"))?);
            }
        }
        let votes: Vec<String> = ballots.iter().map(verdict_of).collect();
        let fin = if is_balloted {
            tally(&votes)
        } else {
            votes[0].clone()
        };
        let shown = ballots
            .iter()
            .find(|ballot| verdict_of(ballot) == fin)
            .unwrap_or(&ballots[0]);
        let mut record = candidate.clone();
        record.insert("verdict".to_owned(), json!(fin));
        let verification: Map<String, Value> = [
            "evidence",
            "reasoning",
            "would_confirm",
            "corrected_line",
            "raw_verdict",
        ]
        .into_iter()
        .filter_map(|key| match shown.get(key) {
            None | Some(Value::Null) => None,
            Some(Value::String(text)) if text.is_empty() => None,
            Some(value) => Some((key.to_owned(), value.clone())),
        })
        .collect();
        record.insert("verification".to_owned(), Value::Object(verification));
        // An accepted limitation is still a finding: any vote's by_design note
        // rides along instead of the finding being refuted away.
        let note = ballots
            .iter()
            .find_map(|ballot| ballot.get("by_design").filter(|note| !blank(Some(note))));
        if let Some(note) = note
            && fin != "REFUTED"
        {
            record.insert("by_design".to_owned(), json!(text(note)));
        }
        if is_balloted {
            record.insert("votes".to_owned(), json!(votes));
            if fin != votes[0] {
                overturned.push(format!(
                    "{candidate_id}: {} -> {fin} ({})",
                    votes[0],
                    votes.join(", ")
                ));
            }
        }
        if fin == "REFUTED" {
            refuted.push(record)
        } else {
            kept.push(record)
        }
    }

    let order = ["CONFIRMED", "PLAUSIBLE", "REFUTED", UNVERIFIED];
    let mut counts = vec![0usize; order.len()];
    for record in kept.iter().chain(&refuted) {
        if let Some(slot) = order.iter().position(|name| record["verdict"] == *name) {
            counts[slot] += 1;
        }
    }
    let mut per_angle: Vec<(String, usize)> = Vec::new();
    for candidate in &candidates {
        let angle = candidate.get("angle").map_or_else(|| "?".to_owned(), text);
        match per_angle.iter_mut().find(|(name, _)| *name == angle) {
            Some((_, count)) => *count += 1,
            None => per_angle.push((angle, 1)),
        }
    }
    let unverified: Vec<String> = kept
        .iter()
        .filter(|record| record["verdict"] == UNVERIFIED)
        .map(id)
        .collect();
    let lines: Vec<String> = kept
        .iter()
        .map(|record| {
            let votes = record
                .get("votes")
                .and_then(Value::as_array)
                .filter(|votes| !votes.is_empty())
                .map(|votes| {
                    let initials: Vec<String> = votes
                        .iter()
                        .map(|vote| text(vote).chars().take(1).collect())
                        .collect();
                    format!(" (votes {})", initials.join("/"))
                })
                .unwrap_or_default();
            let by_design = if record
                .get("by_design")
                .is_some_and(|note| !blank(Some(note)))
            {
                " [by design]"
            } else {
                ""
            };
            format!(
                "{} | {}{votes}{by_design} | x{} | {} | {} | {} || fails: {}",
                id(record),
                text(&record["verdict"]),
                1 + also_count(record),
                one_line(record.get("category"), 14),
                location(record),
                one_line(record.get("summary"), 300),
                one_line(record.get("failure_scenario"), 160)
            )
        })
        .collect();
    let summary_counts: Vec<String> = order
        .iter()
        .zip(&counts)
        .filter(|(name, _)| **name != "REFUTED")
        .map(|(name, count)| format!("{name}={count}"))
        .collect();
    let (kept_count, refuted_count) = (kept.len(), refuted.len());
    write_json(
        &run.path("verified.json"),
        &json!({
            "kept": kept,
            "refuted": refuted,
            "stats": {
                "angles_reported": index.get("angles_reported"),
                "angles_missing": index.get("angles_missing"),
                "raw_candidates": index.get("raw_count"),
                "after_dedup": index.get("deduped_count"),
                "sweep_candidates": sweep_count,
                "verdicts": order.iter().zip(&counts).map(|(name, count)| ((*name).to_owned(), json!(count))).collect::<Map<_, _>>(),
                "balloted": balloted.len(),
                "overturned_by_vote": overturned,
                "candidates_per_angle_after_dedup": per_angle.into_iter().map(|(angle, count)| (angle, json!(count))).collect::<Map<_, _>>(),
                "unverified_ids": unverified,
            },
            "warnings": warnings,
        }),
    )?;
    let pages = write_pages(
        run,
        "verified-digest",
        "id | verdict | angles | category | file:line | summary || fails: excerpt   (full: deduped/<id>.json or candidates/sweep.json, verdicts/<id>.json)",
        &lines,
    )?;
    let mut stdout = format!(
        "digest pages (read these, not verified.json): {}\n",
        pages.join(" ")
    );
    if !overturned.is_empty() {
        stdout += &format!("votes changed the outcome: {}\n", overturned.join("; "));
    }
    stdout += &format!(
        "collated: kept {kept_count} ({}), refuted {refuted_count}\n",
        summary_counts.join(", ")
    );
    Ok(stdout)
}

/// The deduplicated candidates plus the gap sweep's: (index, candidates,
/// sweep count). A sweep problem is noted in `warnings` when one is given.
fn all_candidates(
    run: &ReviewRun,
    warnings: Option<&mut Vec<String>>,
) -> Result<(Record, Vec<Record>, usize)> {
    let index: Record = read_json(&run.path("deduped/index.json"))?;
    let mut candidates: Vec<Record> = index
        .get("candidates")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_object().cloned())
                .collect()
        })
        .unwrap_or_default();
    let sweep_path = run.path("candidates/sweep.json");
    let mut sweep_count = 0;
    if sweep_path.exists() {
        let (items, problem) = load_candidates(&sweep_path);
        if let (Some(problem), Some(warnings)) = (problem, warnings) {
            warnings.push(format!("sweep.json: {problem}"));
        }
        for (number, item) in items.into_iter().take(MAX_PER_ANGLE).enumerate() {
            let line = to_line(item.get("line"));
            let mut record = item;
            record
                .entry("id")
                .or_insert_with(|| json!(format!("S{:02}", number + 1)));
            record.insert("angle".to_owned(), json!("sweep"));
            record.insert("line".to_owned(), line);
            candidates.push(record);
            sweep_count += 1;
        }
    } else if let Some(warnings) = warnings {
        warnings.push("candidates/sweep.json missing: the gap sweep did not report".to_owned());
    }
    Ok((index, candidates, sweep_count))
}

fn is_conventions(candidate: &Record) -> bool {
    let category = candidate
        .get("category")
        .map_or_else(String::new, text)
        .to_lowercase();
    category.contains("convention")
        || candidate.get("angle").and_then(Value::as_str) == Some("conventions")
}

fn verdict_of(verdict: &Record) -> String {
    verdict.get("verdict").map(text).unwrap_or_default()
}

/// A verdict file, wherever under a `verdicts` directory a loop wrote it,
/// normalized: an unknown verdict becomes UNVERIFIED with the raw value kept.
fn load_verdict(run: &ReviewRun, candidate_id: &str) -> Result<Record> {
    let name = format!("{candidate_id}.json");
    let mut path = run.path(&format!("verdicts/{name}"));
    if !path.exists()
        && let Some(found) = find_verdict(run.dir(), run.dir(), &name, false)?
    {
        path = found;
    }
    let unverified = |reasoning: String| {
        let mut verdict = Record::new();
        verdict.insert("verdict".to_owned(), json!(UNVERIFIED));
        verdict.insert("reasoning".to_owned(), json!(reasoning));
        verdict
    };
    if !path.exists() {
        return Ok(unverified("no verdict file was written".to_owned()));
    }
    let mut verdict = match read_json::<Value>(&path) {
        Ok(Value::Object(verdict)) => verdict,
        Ok(_) => return Ok(unverified("verdict file is not a JSON object".to_owned())),
        Err(error) => {
            return Ok(unverified(format!(
                "verdict file unreadable: {}",
                unreadable(&error)
            )));
        }
    };
    let raw = verdict.get("verdict").cloned().unwrap_or(Value::Null);
    let normalized = match &raw {
        Value::String(verdict) => verdict.trim().to_uppercase(),
        _ => String::new(),
    };
    if VERDICTS.contains(&normalized.as_str()) {
        verdict.insert("verdict".to_owned(), json!(normalized));
    } else {
        verdict.insert("raw_verdict".to_owned(), raw);
        verdict.insert("verdict".to_owned(), json!(UNVERIFIED));
    }
    Ok(verdict)
}

/// Depth-first, in name order, like `os.walk`: the first `name` inside a
/// directory whose path below the run has a `verdicts` component.
fn find_verdict(
    root: &Path,
    directory: &Path,
    name: &str,
    under_verdicts: bool,
) -> Result<Option<PathBuf>> {
    let candidate = directory.join(name);
    if under_verdicts && candidate.is_file() {
        return Ok(Some(candidate));
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if directory == root => return Err(io_error("list", directory, error)),
        Err(_) => return Ok(None),
    };
    let mut subdirectories: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    subdirectories.sort();
    for subdirectory in subdirectories {
        let verdicts = under_verdicts
            || subdirectory
                .file_name()
                .is_some_and(|name| name == "verdicts");
        if let Some(found) = find_verdict(root, &subdirectory, name, verdicts)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// Final verdict from 1-3 votes. REFUTED needs two; so does CONFIRMED; a
/// split is PLAUSIBLE.
fn tally(votes: &[String]) -> String {
    let valid: Vec<&str> = votes
        .iter()
        .map(String::as_str)
        .filter(|vote| VERDICTS.contains(vote))
        .collect();
    match valid.as_slice() {
        [] => UNVERIFIED.to_owned(),
        ["REFUTED"] => "PLAUSIBLE".to_owned(),
        [only] => (*only).to_owned(),
        _ => ["REFUTED", "CONFIRMED"]
            .into_iter()
            .find(|verdict| valid.iter().filter(|vote| *vote == verdict).count() >= 2)
            .unwrap_or("PLAUSIBLE")
            .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn votes(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn refuted_and_confirmed_each_need_two_votes() {
        assert_eq!(tally(&votes(&[])), "UNVERIFIED");
        assert_eq!(tally(&votes(&["UNVERIFIED", "UNVERIFIED"])), "UNVERIFIED");
        assert_eq!(tally(&votes(&["REFUTED"])), "PLAUSIBLE");
        assert_eq!(tally(&votes(&["CONFIRMED"])), "CONFIRMED");
        assert_eq!(
            tally(&votes(&["REFUTED", "REFUTED", "CONFIRMED"])),
            "REFUTED"
        );
        assert_eq!(
            tally(&votes(&["REFUTED", "CONFIRMED", "CONFIRMED"])),
            "CONFIRMED"
        );
        assert_eq!(
            tally(&votes(&["REFUTED", "CONFIRMED", "PLAUSIBLE"])),
            "PLAUSIBLE"
        );
        assert_eq!(
            tally(&votes(&["REFUTED", "UNVERIFIED", "CONFIRMED"])),
            "PLAUSIBLE"
        );
    }
}
