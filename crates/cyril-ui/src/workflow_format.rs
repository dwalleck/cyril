//! Text rendering for `/workflow` command outcomes (cyril-0qe6).
//!
//! One pure function per concern; `UiState` turns each
//! `Notification::WorkflowCommand` into exactly one system message built
//! here. Run *progress* rendering (panel, drill-in) is deliberately not
//! this module's job — that is cyril-zd8u; these are one-shot command
//! results.

use cyril_core::types::{
    WorkflowCommandOutcome, WorkflowFetchVerb, WorkflowNodeSnapshot, WorkflowRunStatus,
    WorkflowRunSummary, WorkflowSnapshot,
};

/// Renders one command outcome as the body of a system message.
#[must_use]
pub fn format_workflow_outcome(outcome: &WorkflowCommandOutcome) -> String {
    match outcome {
        WorkflowCommandOutcome::Recipes { recipes, skipped } => format_recipes(recipes, *skipped),
        WorkflowCommandOutcome::Runs { runs, skipped } => format_runs(runs, *skipped),
        WorkflowCommandOutcome::Fetched { verb, snapshot } => format_fetched(*verb, snapshot),
        WorkflowCommandOutcome::Launched { workflow_id, name } => format!(
            "Launched {name} — run {workflow_id}. Lifecycle events stream as it \
             executes; /workflow status shows what is known."
        ),
        WorkflowCommandOutcome::Cancelled {
            workflow_id,
            previous_status,
        } => format!(
            "Cancelled {workflow_id} (was {}).",
            status_or_dash(*previous_status)
        ),
        WorkflowCommandOutcome::Resumed {
            workflow_id,
            status,
        } => format!(
            "Resumed {workflow_id} (status {}).",
            status_or_dash(*status)
        ),
        WorkflowCommandOutcome::Failed {
            operation,
            code,
            details,
        } => match code {
            Some(code) => format!("/{operation} failed ({code}): {details}"),
            None => format!("/{operation} failed: {details}"),
        },
    }
}

fn status_or_dash(status: Option<WorkflowRunStatus>) -> String {
    status.map_or_else(|| "—".to_owned(), |status| status.to_string())
}

fn skipped_note(skipped: usize) -> String {
    if skipped == 0 {
        String::new()
    } else {
        format!(
            "\n(+{skipped} unreadable entr{} — see cyril.log)",
            plural_y(skipped)
        )
    }
}

fn plural_y(n: usize) -> &'static str {
    if n == 1 { "y" } else { "ies" }
}

fn format_recipes(recipes: &[cyril_core::types::WorkflowRecipe], skipped: usize) -> String {
    if recipes.is_empty() && skipped == 0 {
        return "No workflow recipes available.".to_owned();
    }
    let mut text = format!("Workflow recipes ({}):", recipes.len());
    for recipe in recipes {
        text.push_str(&format!(
            "\n  {} — {}",
            recipe.name,
            recipe.description.as_deref().unwrap_or("—")
        ));
        // Workspace recipes show where they live; `bundled://` provenance is
        // implied by the name and would only repeat it.
        if let Some(source) = recipe
            .source
            .as_deref()
            .filter(|source| !source.starts_with("bundled://"))
        {
            text.push_str(&format!("\n      from {source}"));
        }
    }
    text.push_str(&skipped_note(skipped));
    text.push_str("\n(/workflow run <name> launches one)");
    text
}

fn format_runs(runs: &[WorkflowRunSummary], skipped: usize) -> String {
    if runs.is_empty() && skipped == 0 {
        return "No workflow runs in this workspace.".to_owned();
    }
    let mut text = format!("Workflow runs ({}):", runs.len());
    for run in runs {
        text.push_str(&format!(
            "\n  {}  {}  {}  (started {}, ended {})",
            run.workflow_id,
            run.status,
            run.name,
            run.started_at.as_deref().unwrap_or("—"),
            run.ended_at.as_deref().unwrap_or("—"),
        ));
    }
    text.push_str(&skipped_note(skipped));
    text
}

fn format_fetched(verb: WorkflowFetchVerb, snapshot: &WorkflowSnapshot) -> String {
    let mut text = match verb {
        WorkflowFetchVerb::Attach => format!(
            "Attached to {} — {} ({}).",
            snapshot.workflow_id(),
            snapshot.workflow_name(),
            snapshot.status()
        ),
        WorkflowFetchVerb::Status => format!(
            "Run {} — {} ({}).",
            snapshot.workflow_id(),
            snapshot.workflow_name(),
            snapshot.status()
        ),
    };
    for child in snapshot.root().children() {
        push_node_lines(child, 1, &mut text);
    }
    if matches!(verb, WorkflowFetchVerb::Attach) {
        text.push_str("\n(/workflow status shows this run from now on)");
    }
    text
}

fn push_node_lines(node: &WorkflowNodeSnapshot, depth: usize, text: &mut String) {
    let indent = "  ".repeat(depth);
    text.push_str(&format!(
        "\n{indent}• {} {}",
        node.descriptor().node_id(),
        node.status()
    ));
    // The failure reason is the actionable part (a stale-token step failure
    // names the token, a validation failure names the field) — verbatim.
    if let Some(reason) = node.failure_reason() {
        text.push_str(&format!(" — {reason}"));
    }
    for child in node.children() {
        push_node_lines(child, depth + 1, text);
    }
}

#[cfg(test)]
mod tests {
    use cyril_core::types::{
        WorkflowId, WorkflowNodeDescriptor, WorkflowNodeId, WorkflowNodeStatus, WorkflowRecipe,
        WorkflowSnapshotData, WorkflowSnapshotMetadata,
    };

    use super::*;

    /// The live listRecipes reply (7 bundled recipes), extracted here with
    /// plain serde — deliberately NOT cyril-core's parser, so this test's
    /// input is an independent reading of the same capture bytes.
    const RECIPES_FIXTURE: &str =
        include_str!("../../cyril-core/tests/fixtures/kas/workflow/recipes-reply-2.16.2.json");
    const DISK_RECIPES_FIXTURE: &str = include_str!(
        "../../cyril-core/tests/fixtures/kas/workflow/recipes-reply-diskrecipe-2.16.2.json"
    );

    fn recipes_from(raw: &str) -> Vec<WorkflowRecipe> {
        let value: serde_json::Value = match serde_json::from_str(raw) {
            Ok(value) => value,
            Err(error) => panic!("fixture parses: {error}"),
        };
        let Some(entries) = value["recipes"].as_array() else {
            panic!("fixture has recipes");
        };
        entries
            .iter()
            .map(|entry| WorkflowRecipe {
                name: entry["name"].as_str().unwrap_or_default().to_owned(),
                description: entry["description"].as_str().map(str::to_owned),
                source: entry["source"].as_str().map(str::to_owned),
            })
            .collect()
    }

    fn wid(raw: &str) -> WorkflowId {
        match WorkflowId::try_from(raw.to_owned()) {
            Ok(id) => id,
            Err(error) => panic!("fixture id: {error}"),
        }
    }

    fn node_id(raw: &str) -> WorkflowNodeId {
        match WorkflowNodeId::try_from(raw.to_owned()) {
            Ok(id) => id,
            Err(error) => panic!("fixture node id: {error}"),
        }
    }

    #[test]
    fn recipes_render_all_seven_bundled_names() {
        let recipes = recipes_from(RECIPES_FIXTURE);
        let text = format_workflow_outcome(&WorkflowCommandOutcome::Recipes {
            recipes,
            skipped: 0,
        });
        for name in [
            "autoresearch",
            "feature-pipeline",
            "goal",
            "investigate",
            "publish-pr",
            "ralph",
            "semantic-review-multi-model",
        ] {
            assert!(text.contains(name), "recipe {name} missing from:\n{text}");
        }
        assert!(text.contains("Workflow recipes (7):"));
    }

    #[test]
    fn workspace_recipe_shows_its_path_bundled_do_not_repeat_theirs() {
        let recipes = recipes_from(DISK_RECIPES_FIXTURE);
        let text = format_workflow_outcome(&WorkflowCommandOutcome::Recipes {
            recipes,
            skipped: 0,
        });
        assert!(
            text.contains("from /"),
            "the workspace recipe's absolute path must render:\n{text}"
        );
        assert!(
            !text.contains("from bundled://"),
            "bundled provenance is implied, not repeated:\n{text}"
        );
    }

    #[test]
    fn skipped_entries_are_visible_not_absorbed() {
        let text = format_workflow_outcome(&WorkflowCommandOutcome::Recipes {
            recipes: Vec::new(),
            skipped: 2,
        });
        assert!(text.contains("+2 unreadable entries"), "{text}");
    }

    #[test]
    fn runs_render_dashes_for_absent_timestamps_not_sentinels() {
        let runs = vec![WorkflowRunSummary {
            workflow_id: wid("wf_1"),
            name: "cyril-cancel-probe".to_owned(),
            status: WorkflowRunStatus::Aborted,
            created_at: Some("2026-08-13T05:31:40.721Z".to_owned()),
            updated_at: Some("2026-08-13T05:31:40.721Z".to_owned()),
            started_at: None,
            ended_at: None,
            parent_session_id: None,
        }];
        let text = format_workflow_outcome(&WorkflowCommandOutcome::Runs { runs, skipped: 0 });
        assert!(
            text.contains("(started —, ended —)"),
            "absent timestamps render as dashes, never fabricated values:\n{text}"
        );
        assert!(text.contains("aborted"));
    }

    #[test]
    fn empty_runs_say_so() {
        let text = format_workflow_outcome(&WorkflowCommandOutcome::Runs {
            runs: Vec::new(),
            skipped: 0,
        });
        assert_eq!(text, "No workflow runs in this workspace.");
    }

    #[test]
    fn fetched_renders_nodes_and_failure_reasons_verbatim() {
        let failed_step = WorkflowNodeSnapshot::new(
            WorkflowNodeDescriptor::step(node_id("only"), "wf-coder".to_owned(), None, None),
            WorkflowNodeStatus::Failed,
            Vec::new(),
        )
        .with_failure_reason(
            "Authentication token is invalid: Host refresh callback returned token \
             already inside 180000ms refresh buffer"
                .to_owned(),
        );
        let snapshot = WorkflowSnapshot::new(
            wid("wf_f"),
            "cyril-reattach-r1".to_owned(),
            WorkflowRunStatus::Failed,
            WorkflowSnapshotData::new(
                serde_json::json!({}),
                serde_json::json!({}),
                serde_json::json!({}),
            ),
            WorkflowNodeSnapshot::new(
                WorkflowNodeDescriptor::sequence(node_id("wf_f"), Vec::new()),
                WorkflowNodeStatus::Failed,
                vec![failed_step],
            ),
            WorkflowSnapshotMetadata::new("2026-08-11T05:11:07.773Z".to_owned(), 0),
        );
        let text = format_workflow_outcome(&WorkflowCommandOutcome::Fetched {
            verb: WorkflowFetchVerb::Status,
            snapshot: Box::new(snapshot),
        });
        assert!(text.contains("Run wf_f — cyril-reattach-r1 (failed)."));
        assert!(text.contains("• only failed — Authentication token is invalid"));
    }

    #[test]
    fn failed_outcome_carries_the_details_verbatim() {
        let refusal = "Workflow 'wf_c598b543bbc7cb2b' appears to be running in another \
                       process (owner pid 1920787, liveness verdict: live); refusing to \
                       load it here. Retry after that process releases it or its run \
                       goes stale.";
        let text = format_workflow_outcome(&WorkflowCommandOutcome::Failed {
            operation: "workflow resume".to_owned(),
            code: Some(-32603),
            details: refusal.to_owned(),
        });
        assert!(
            text.contains("running in another process (owner pid 1920787"),
            "the ownership refusal must surface verbatim:\n{text}"
        );
        assert!(text.starts_with("/workflow resume failed (-32603):"));
    }

    #[test]
    fn every_outcome_variant_renders_nonempty() {
        let outcomes = [
            WorkflowCommandOutcome::Launched {
                workflow_id: wid("wf_l"),
                name: "ralph".to_owned(),
            },
            WorkflowCommandOutcome::Cancelled {
                workflow_id: wid("wf_c"),
                previous_status: Some(WorkflowRunStatus::Running),
            },
            WorkflowCommandOutcome::Resumed {
                workflow_id: wid("wf_r"),
                status: None,
            },
        ];
        for outcome in &outcomes {
            assert!(!format_workflow_outcome(outcome).is_empty());
        }
    }
}
