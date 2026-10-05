//! `merge` gathers every finder's candidates; `shard` applies the clerk's
//! duplicate decisions, assigns final ids and splits them into verifier queues.

use crate::record::{
    MAX_PER_ANGLE, Record, also_count, blank, field_or_empty, id, load_candidates, location,
    one_line, quoted, required_records, text, to_line, unreadable,
};
use crate::run::{ReviewRun, read_json, read_manifest, write_json, write_pages, write_queue};
use crate::{Result, ReviewError, io_error};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::fs;

/// `crtool merge <rundir> --expect a,b,...`
pub fn merge(run: &ReviewRun, expect: &str) -> Result<String> {
    read_manifest(run)?;
    let expected: Vec<&str> = expect
        .split(',')
        .filter(|angle| !angle.is_empty())
        .collect();
    let (mut reported, mut missing, mut problems, mut out) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for angle in &expected {
        let path = run.path(&format!("candidates/{angle}.json"));
        if !path.exists() {
            missing.push(*angle);
            continue;
        }
        let (mut items, problem) = load_candidates(&path);
        if let Some(problem) = &problem {
            problems.push(json!({"angle": angle, "file": format!("candidates/{angle}.json"), "problem": problem}));
            if items.is_empty() {
                missing.push(*angle);
                continue;
            }
        }
        reported.push(*angle);
        if items.len() > MAX_PER_ANGLE {
            problems.push(json!({
                "angle": angle,
                "problem": format!("{} candidates; kept first {MAX_PER_ANGLE}", items.len()),
            }));
            items.truncate(MAX_PER_ANGLE);
        }
        for (number, candidate) in items.into_iter().enumerate() {
            let line = to_line(candidate.get("line"));
            let absent: Vec<&str> = ["file", "summary", "failure_scenario"]
                .into_iter()
                .filter(|key| candidate.get(*key).is_none_or(|value| blank(Some(value))))
                .collect();
            let mut record = candidate;
            record.insert("pid".to_owned(), json!(format!("{angle}-{}", number + 1)));
            record.insert("angle".to_owned(), json!(angle));
            record.insert("line".to_owned(), line);
            if !absent.is_empty() {
                record.insert("incomplete".to_owned(), json!(absent));
            }
            out.push(record);
        }
    }
    write_json(
        &run.path("candidates/all.json"),
        &json!({
            "angles_expected": expected,
            "angles_reported": reported,
            "angles_missing": missing,
            "problems": problems,
            "raw_count": out.len(),
            "candidates": out,
        }),
    )?;
    for record in &out {
        write_json(
            &run.path(&format!("candidates/raw/{}.json", pid(record))),
            record,
        )?;
    }

    // Sorted by location so probable duplicates sit on adjacent lines.
    let mut ordered: Vec<&Record> = out.iter().collect();
    ordered.sort_by_key(|record| sort_key(record));
    // A shared location is a mechanical fact, so state it: a clerk once left
    // four candidates at the identical file:line unmerged.
    let mut at: HashMap<(String, Option<i64>), usize> = HashMap::new();
    for record in &ordered {
        *at.entry(place(record)).or_default() += 1;
    }
    let mut lines = Vec::new();
    let mut flagged = HashSet::new();
    for record in ordered {
        let key = place(record);
        let count = at.get(&key).copied().unwrap_or(0);
        if count > 1 && flagged.insert(key) {
            lines.push(format!(
                ">>> the next {count} candidates cite the SAME location, {}: if they describe one defect they are ONE group (keep them apart only if the reasons genuinely differ)",
                location(record)
            ));
        }
        lines.push(format!(
            "{} | {} | {} | {} || fails: {}",
            pid(record),
            location(record),
            one_line(record.get("category"), 14),
            one_line(record.get("summary"), 300),
            one_line(record.get("failure_scenario"), 160)
        ));
    }
    let pages = write_pages(
        run,
        "candidates/digest",
        "pid | file:line | category | summary || fails: failure-scenario excerpt   (full record: candidates/raw/<pid>.json)",
        &lines,
    )?;
    let mut stdout = format!(
        "digest pages (read these, not all.json): {}\n",
        pages.join(" ")
    );
    stdout += &format!(
        "merged {} candidates from {}/{} angles",
        out.len(),
        reported.len(),
        expected.len()
    );
    if !missing.is_empty() {
        stdout += &format!(" | MISSING: {}", missing.join(","));
    }
    if !problems.is_empty() {
        stdout += &format!(" | {} problem(s), see candidates/all.json", problems.len());
    }
    stdout.push('\n');
    Ok(stdout)
}

/// `crtool shard <rundir> [--shards N]`
pub fn shard(run: &ReviewRun, shards: usize) -> Result<String> {
    read_manifest(run)?;
    if shards == 0 {
        return Err(ReviewError::InvalidArgument {
            message: "--shards must be at least 1",
        });
    }
    let merged_path = run.path("candidates/all.json");
    let merged: Record = read_json(&merged_path)?;
    let mut candidates = required_records(&merged, "candidates", &merged_path)?;
    let order: HashMap<String, usize> = candidates
        .iter()
        .enumerate()
        .map(|(index, record)| (pid(record), index))
        .collect();
    let mut warnings = Vec::new();

    let decisions_path = run.path("deduped/decisions.json");
    let decisions = if decisions_path.exists() {
        match read_json::<Value>(&decisions_path) {
            Ok(Value::Object(decisions)) => decisions,
            Ok(_) => {
                warnings.push(
                    "decisions.json unreadable (top level is not an object); no dedup applied"
                        .to_owned(),
                );
                Record::new()
            }
            Err(error) => {
                warnings.push(format!(
                    "decisions.json unreadable ({}); no dedup applied",
                    unreadable(&error)
                ));
                Record::new()
            }
        }
    } else {
        warnings.push("decisions.json missing; no dedup applied".to_owned());
        Record::new()
    };

    // Groups ({"pids": [...], "keep"?, "reason"}) and the older pairs
    // ({"drop", "keep", "reason"}) both reduce to "these pids are one defect".
    let entries: Vec<(Vec<Value>, &Record)> = objects(decisions.get("groups"))
        .map(|group| {
            (
                group
                    .get("pids")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
                group,
            )
        })
        .chain(objects(decisions.get("duplicates")).map(|pair| {
            let named = |key| pair.get(key).cloned().unwrap_or(Value::Null);
            (vec![named("drop"), named("keep")], pair)
        }))
        .collect();
    let mut parent: Vec<usize> = (0..candidates.len()).collect();
    let mut preferred = HashSet::new();
    let mut reason_of: HashMap<usize, Value> = HashMap::new();
    for (raw, entry) in entries {
        let known: Vec<usize> = raw
            .iter()
            .filter_map(|value| value.as_str().and_then(|pid| order.get(pid)).copied())
            .collect();
        if known.len() != raw.len() {
            let unknown: Vec<Value> = raw
                .iter()
                .filter(|value| value.as_str().is_none_or(|pid| !order.contains_key(pid)))
                .cloned()
                .collect();
            warnings.push(format!(
                "decision names unknown pid(s) {}; ignored those",
                pid_list(&unknown)
            ));
        }
        if known.iter().collect::<HashSet<_>>().len() < 2 {
            continue;
        }
        for other in &known[1..] {
            let (root, first) = (find(&mut parent, *other), find(&mut parent, known[0]));
            parent[root] = first;
        }
        if let Some(keep) = entry
            .get("keep")
            .and_then(Value::as_str)
            .and_then(|pid| order.get(pid))
            && known.contains(keep)
        {
            preferred.insert(*keep);
        }
        for index in &known {
            reason_of
                .entry(*index)
                .or_insert_with(|| field_or_empty(entry, "reason"));
        }
    }

    let mut groups: Vec<(usize, Vec<usize>)> = Vec::new();
    for index in 0..candidates.len() {
        let root = find(&mut parent, index);
        match groups
            .iter_mut()
            .find(|(group_root, _)| *group_root == root)
        {
            Some((_, members)) => members.push(index),
            None => groups.push((root, vec![index])),
        }
    }
    let detail = |record: &Record| {
        ["failure_scenario", "evidence"]
            .iter()
            .map(|key| match record.get(*key) {
                Some(value) if !blank(Some(value)) => text(value).chars().count(),
                _ => 0,
            })
            .sum::<usize>()
    };
    let mut dropped = Vec::new();
    let mut dropped_indexes = HashSet::new();
    for (_, members) in groups.iter().filter(|(_, members)| members.len() > 1) {
        // Members are already in merge order.
        let keeper = members
            .iter()
            .copied()
            .find(|index| preferred.contains(index))
            .or_else(|| {
                members.iter().copied().max_by(|a, b| {
                    detail(&candidates[*a])
                        .cmp(&detail(&candidates[*b]))
                        .then(b.cmp(a))
                })
            })
            .unwrap_or(members[0]);
        for index in members.iter().copied().filter(|index| *index != keeper) {
            let flagged = json!({
                "pid": pid(&candidates[index]),
                "angle": candidates[index].get("angle").cloned().unwrap_or(Value::Null),
                "summary": field_or_empty(&candidates[index], "summary"),
            });
            let keeper_pid = pid(&candidates[keeper]);
            let also = candidates[keeper]
                .entry("also_flagged_by")
                .or_insert_with(|| json!([]));
            if !also.is_array() {
                // A finder wrote its own value here; the duplicates must not vanish.
                warnings.push(format!(
                    "{keeper_pid}: also_flagged_by was not a list; replaced"
                ));
                *also = json!([]);
            }
            if let Value::Array(also) = also {
                also.push(flagged);
            }
            dropped.push(json!({
                "pid": pid(&candidates[index]),
                "kept_as": pid(&candidates[keeper]),
                "reason": reason_of.get(&index).cloned().unwrap_or_else(|| json!("")),
            }));
            dropped_indexes.insert(index);
        }
    }

    let mut survivors = Vec::new();
    for (index, mut record) in candidates.drain(..).enumerate() {
        if dropped_indexes.contains(&index) {
            continue;
        }
        let id = format!("C{:02}", survivors.len() + 1);
        record.insert("id".to_owned(), json!(id));
        write_json(&run.path(&format!("deduped/{id}.json")), &record)?;
        survivors.push(record);
    }

    let mut queues: Vec<Vec<String>> = vec![Vec::new(); shards];
    for (number, record) in survivors.iter().enumerate() {
        queues[number % shards].push(id(record));
    }
    // `verdict_dir` is absolute and one per queue: a relative one was once
    // re-rooted under the queue's folder, and a shared one let a verifier
    // read another loop's file as its own.
    for (number, pending) in queues.iter().enumerate() {
        let name = format!("q{}", number + 1);
        write_queue(run, &format!("queue-{}", number + 1), &name, pending)?;
    }
    let sweep = run.path("verdicts/sweep");
    fs::create_dir_all(&sweep).map_err(|source| io_error("create directory", sweep, source))?;

    let lines: Vec<String> = survivors
        .iter()
        .map(|record| {
            format!(
                "{} | {} | x{} | {}",
                id(record),
                location(record),
                1 + also_count(record),
                one_line(record.get("summary"), 300)
            )
        })
        .collect();
    let survivor_count = survivors.len();
    write_json(
        &run.path("deduped/index.json"),
        &json!({
            "angles_reported": merged.get("angles_reported"),
            "angles_missing": merged.get("angles_missing"),
            "raw_count": merged.get("raw_count"),
            "deduped_count": survivor_count,
            "dropped": dropped,
            "warnings": warnings,
            "candidates": survivors,
        }),
    )?;
    let pages = write_pages(
        run,
        "deduped/digest",
        "id | file:line | angles | summary   (full record: deduped/<id>.json)",
        &lines,
    )?;
    let sizes: Vec<String> = queues.iter().map(|queue| queue.len().to_string()).collect();
    let mut stdout = format!(
        "digest pages (the already-raised list): {}\n",
        pages.join(" ")
    );
    stdout += &format!(
        "sharded {survivor_count} candidates ({} duplicates dropped) into {shards} queues: [{}]",
        dropped.len(),
        sizes.join(", ")
    );
    if !warnings.is_empty() {
        stdout += &format!(" | WARNINGS: {}", warnings.join("; "));
    }
    stdout.push('\n');
    Ok(stdout)
}

fn objects(value: Option<&Value>) -> impl Iterator<Item = &Record> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
}

/// The unknown pids as `crtool.py` lists them (`['ghost-9', 'x-2']`): the
/// clerk reads this warning, and a mistyped pid is an ordinary input.
fn pid_list(pids: &[Value]) -> String {
    let items: Vec<String> = pids
        .iter()
        .map(|pid| match pid {
            Value::String(pid) => quoted(pid),
            other => other.to_string(),
        })
        .collect();
    format!("[{}]", items.join(", "))
}

fn find(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

fn pid(record: &Record) -> String {
    record.get("pid").map(text).unwrap_or_default()
}

/// The file as the digest prints it: an absent or null file is `?`.
fn file_key(record: &Record) -> String {
    match record.get("file") {
        None | Some(Value::Null) => "?".to_owned(),
        Some(file) => text(file),
    }
}

/// Digest order: file, line (absent as 0), pid.
fn sort_key(record: &Record) -> (String, i64, String) {
    let file = file_key(record);
    let line = record.get("line").and_then(Value::as_i64).unwrap_or(0);
    (file, line, pid(record))
}

/// What counts as "the same location" in the digest.
fn place(record: &Record) -> (String, Option<i64>) {
    let file = file_key(record);
    (file, record.get("line").and_then(Value::as_i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::stamped_run;

    fn record(value: Value) -> Record {
        value.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn an_absent_or_null_file_sorts_and_groups_as_printed() {
        assert_eq!(file_key(&record(json!({}))), "?");
        assert_eq!(file_key(&record(json!({"file": null}))), "?");
        assert_eq!(
            place(&record(json!({"line": 3}))),
            place(&record(json!({"file": null, "line": 3})))
        );
    }

    #[test]
    fn a_corrupt_all_json_is_an_error_not_zero_candidates() -> Result<()> {
        let (_tree, run) = stamped_run()?;
        crate::run::write_json(&run.path("candidates/all.json"), &json!({"raw_count": 2}))?;
        assert!(matches!(
            shard(&run, 3),
            Err(ReviewError::CorruptRunFile { .. })
        ));
        Ok(())
    }

    #[test]
    fn a_keeper_with_a_non_list_also_flagged_by_keeps_its_duplicates() -> Result<()> {
        let (_tree, run) = stamped_run()?;
        let candidate = |pid: &str, also: Option<&str>| {
            let mut value = json!({"pid": pid, "angle": "a", "file": "x.rs", "line": 1, "failure_scenario": "f"});
            if let Some(also) = also {
                value["also_flagged_by"] = json!(also);
                value["failure_scenario"] = json!("the longest failure scenario wins");
            }
            value
        };
        crate::run::write_json(
            &run.path("candidates/all.json"),
            &json!({"angles_reported": ["a"], "angles_missing": [], "raw_count": 2,
                    "candidates": [candidate("a-1", Some("conventions")), candidate("a-2", None)]}),
        )?;
        crate::run::write_json(
            &run.path("deduped/decisions.json"),
            &json!({"groups": [{"pids": ["a-1", "a-2"]}]}),
        )?;
        let stdout = shard(&run, 1)?;
        assert!(
            stdout.contains("a-1: also_flagged_by was not a list; replaced"),
            "{stdout}"
        );
        let kept: Record = crate::run::read_json(&run.path("deduped/C01.json"))?;
        assert_eq!(
            kept["also_flagged_by"],
            json!([{"pid": "a-2", "angle": "a", "summary": ""}])
        );
        Ok(())
    }
}
