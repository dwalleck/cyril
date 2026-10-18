//! A workflow plan KAS asks the user to approve before any of its steps run
//! (cyril-8wh0).
//!
//! KAS 0.66.43 (kiro-cli 2.32.0) raises the plan as an ordinary
//! `session/request_permission` for `update_workflow`, carrying it as
//! `_meta.kiro.workflowPlan` beside a Text rendering of the same plan. The
//! wire shape is the bundle's `P0o`/`M0o`/`Ydu`/`nxe`/`wHr`; the conversion
//! lives in `protocol/convert/kas/workflow_plan.rs`. Every field of that
//! shape is modelled here.

use std::fmt;

use super::{
    WorkflowCompletionSignal, WorkflowEnumParseError, WorkflowId, WorkflowNodeId,
    WorkflowRepeatExhaustion,
};

workflow_enum! {
    /// Why KAS raised the plan (`I0o`): the first plan of a run created from
    /// a prompt, a plain-language edit of a running workflow, or a
    /// replacement of a run's remaining steps.
    pub enum WorkflowPlanKind as "workflow plan kind" {
        Launch => "launch",
        Edit => "edit",
        Replace => "replace",
    }
}

workflow_enum! {
    /// What KAS left out to keep the plan within its 32 000-character
    /// budget: each step's details (task, writes, completion, config), or
    /// the steps altogether.
    pub enum WorkflowPlanOmission as "workflow plan omission" {
        Details => "details",
        Steps => "steps",
    }
}

workflow_enum! {
    /// Where a step's model or effort level comes from: the step itself,
    /// the workflow's default, or the parent session.
    pub enum WorkflowCascadeSource as "workflow cascade source" {
        Step => "step",
        Workflow => "workflow",
        Parent => "parent",
    }
}

workflow_enum! {
    /// How a parallel node joins its branches.
    pub enum WorkflowJoinPolicy as "workflow join policy" {
        All => "all",
        AllSettled => "allSettled",
        Any => "any",
    }
}

/// A workflow plan awaiting the user's approval.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowPlan {
    /// The run the plan changes.
    pub workflow_id: WorkflowId,
    /// The step that proposed the plan (the creator step); the new steps
    /// run after it. Absent when the parent session proposed it.
    pub proposed_by: Option<WorkflowNodeId>,
    pub kind: WorkflowPlanKind,
    /// The saved definition behind the plan (`generated://…`).
    pub workflow_ref: String,
    /// Top-level steps the run keeps; the plan's steps replace the rest.
    pub kept_step_ids: Vec<WorkflowNodeId>,
    /// The steps the plan adds, as a tree. Empty when
    /// [`WorkflowPlanOmission::Steps`] applies.
    pub steps: Vec<WorkflowPlanNode>,
    pub omitted: Option<WorkflowPlanOmission>,
}

/// One node of a plan's step tree.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowPlanNode {
    pub node_id: WorkflowNodeId,
    pub kind: WorkflowPlanNodeKind,
}

/// What a plan node is, with the fields its type carries.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowPlanNodeKind {
    Step(WorkflowPlanStep),
    Sequence {
        steps: Vec<WorkflowPlanNode>,
        /// The recipe this sequence was included from.
        included_from: Option<WorkflowPlanInclude>,
    },
    Repeat {
        steps: Vec<WorkflowPlanNode>,
        max_iterations: u32,
        on_max_iterations: WorkflowRepeatExhaustion,
        /// A repeat carries at most one of a stop condition and a
        /// plain-language stop rule.
        stop: Option<WorkflowRepeatStop>,
    },
    Parallel {
        branches: Vec<WorkflowPlanNode>,
        join_policy: Option<WorkflowJoinPolicy>,
    },
    Watch {
        /// The agent that handles the watch.
        handler: String,
        idle_timeout_sec: Option<f64>,
        /// The watch's configuration, kept as sent (KAS types it as a free
        /// record). Present only when the plan carries details.
        config: Option<serde_json::Map<String, serde_json::Value>>,
    },
}

/// When a repeat stops before its iteration cap.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkflowRepeatStop {
    Condition(WorkflowStopCondition),
    /// A plain-language rule (`stopWhen`).
    When(String),
}

/// One agent step of a plan.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowPlanStep {
    pub agent_name: String,
    /// The resolved model; absent means the agent's default ("auto").
    pub model_id: Option<String>,
    pub model_source: Option<WorkflowCascadeSource>,
    /// The resolved effort level; absent means the default ("auto").
    pub effort_level: Option<String>,
    pub effort_source: Option<WorkflowCascadeSource>,
    /// The first line of the step's prompt. Absent when details are
    /// omitted.
    pub task: Option<WorkflowPlanTask>,
    /// Files the step writes, as `(name, path)` pairs in the order the JSON
    /// map yields them: wire order when serde_json's `preserve_order` is on
    /// (another dependency enables it in workspace builds), else by name.
    /// Empty when the step writes none or details are omitted.
    pub artifacts: Vec<(String, String)>,
    /// What the step waits for before it completes.
    pub completion: Option<WorkflowStopCondition>,
}

/// The first non-blank line of a step's prompt, at most 160 characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowPlanTask {
    pub text: String,
    /// The prompt was cut, or has more lines.
    pub truncated: bool,
}

/// A stop or completion condition: any of its parts satisfies it.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowStopCondition {
    pub contains_text: Option<String>,
    pub file_check: Option<WorkflowFileCheck>,
    pub completion_signal: Option<WorkflowCompletionSignal>,
}

/// A JSON value in a file that satisfies a condition.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowFileCheck {
    pub path: String,
    pub json_path: String,
    /// The value compared against, kept as sent (KAS types it `unknown`).
    pub value: serde_json::Value,
}

/// The recipe a sequence node was included from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowPlanInclude {
    pub name: String,
    pub path: String,
}
