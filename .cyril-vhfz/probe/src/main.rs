use std::{env, error::Error, fs, path::Path};

use cyril_core::{
    test_support::kas_capture_to_routed,
    types::{Notification, WorkflowNodeStatus::Paused},
    workflow::WorkflowTracker,
};
const CHECKPOINTS: [&str; 4] = ["node_paused", "paused", "steps_queued", "run_complete"];

fn main() -> Result<(), Box<dyn Error>> {
    let paths = env::args_os().skip(1).collect::<Vec<_>>();
    if paths.is_empty() {
        return Err("usage: cyril-vhfz-probe CAPTURE...".into());
    }
    for path in paths {
        let path = Path::new(&path).canonicalize()?;
        let label = path.file_name().ok_or("no file name")?.to_string_lossy();
        let mut tracker = WorkflowTracker::new();
        for (_, notification) in kas_capture_to_routed(&fs::read_to_string(&path)?) {
            let Notification::Workflow(event) = notification else {
                continue;
            };
            let method = event.method_name();
            let workflow_id = event.workflow_id().clone();
            tracker.apply_event(*event)?;
            if !CHECKPOINTS.contains(&method) {
                continue;
            }
            let run = tracker.get(&workflow_id).ok_or("missing run")?;
            let paused = run
                .nodes()
                .filter(|(_, node)| node.status() == Some(Paused))
                .collect::<Vec<_>>();
            let mut reasons = paused
                .iter()
                .filter_map(|(path, node)| Some(format!("{path}={}", node.node_pause_reason()?)))
                .collect::<Vec<_>>();
            reasons.sort_unstable();
            println!(
                "{label}\t{method}\trun={:?}\tpaused_nodes={}\tnode_reasons={reasons:?}\trun_reason={:?}",
                run.status(), paused.len(), run.run_pause_reason()
            );
            if method == "run_complete" {
                break;
            }
        }
    }
    Ok(())
}
