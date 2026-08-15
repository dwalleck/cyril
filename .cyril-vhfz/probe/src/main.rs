use std::{env, error::Error, fs, path::Path};

use cyril_core::{
    test_support::kas_capture_to_routed,
    types::{Notification, WorkflowNodeStatus, WorkflowRunStatus},
    workflow::WorkflowTracker,
};

fn raw_capture(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut raw = String::new();
    for line in fs::read_to_string(path)?.lines() {
        let frame: serde_json::Value = serde_json::from_str(line)?;
        let wire = frame.get("parsed").unwrap_or(&frame);
        raw.push_str(&serde_json::to_string(wire)?);
        raw.push('\n');
    }
    Ok(raw)
}

fn main() -> Result<(), Box<dyn Error>> {
    let paths = env::args_os().skip(1).collect::<Vec<_>>();
    if paths.is_empty() {
        return Err("usage: cyril-vhfz-probe CAPTURE...".into());
    }
    for path in paths {
        let path = Path::new(&path).canonicalize()?;
        let label = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("capture");
        let mut tracker = WorkflowTracker::new();
        for (_, notification) in kas_capture_to_routed(&raw_capture(&path)?) {
            let Notification::Workflow(event) = notification else {
                continue;
            };
            let method = event.method_name();
            let workflow_id = event.workflow_id().clone();
            tracker.apply_event(*event)?;
            if !matches!(
                method,
                "node_paused" | "paused" | "steps_queued" | "run_complete"
            ) {
                continue;
            }
            let run = tracker
                .get(&workflow_id)
                .ok_or("workflow event lost its run")?;
            let paused_nodes = run
                .nodes()
                .filter(|(_, node)| node.status() == Some(WorkflowNodeStatus::Paused))
                .count();
            println!(
                "{label}\t{method}\trun={:?}\tpaused_nodes={paused_nodes}\trun_reason={:?}",
                run.status(),
                run.run_pause_reason()
            );
            if method == "run_complete" && run.status() == Some(WorkflowRunStatus::Paused) {
                break;
            }
        }
    }
    Ok(())
}
