//! `_meta.kiro.workflowPlan` of a KAS `session/request_permission`
//! (cyril-8wh0): the plan KAS 0.66.43 asks the user to approve before a
//! workflow's steps run.
//!
//! The wire shape is the bundle's plan builder (`P0o`), its node projection
//! (`M0o` = `nxe` + `Ydu` + `wHr` details) and the recipe schema (`Xk`). Every
//! field it emits is modelled in [`crate::types::WorkflowPlan`]. A field this
//! module does not know is reported once per request at debug level and
//! ignored. A field of the wrong shape, a missing required field or an
//! unknown enum value makes the whole plan malformed: it is dropped with a
//! warning, and the approval still shows the request's Text rendering of the
//! plan.

use serde_json::{Map, Value};

use crate::types::{
    WorkflowCascadeSource, WorkflowCompletionSignal, WorkflowFileCheck, WorkflowId,
    WorkflowJoinPolicy, WorkflowNodeId, WorkflowPlan, WorkflowPlanInclude, WorkflowPlanKind,
    WorkflowPlanNode, WorkflowPlanNodeKind, WorkflowPlanOmission, WorkflowPlanStep,
    WorkflowPlanTask, WorkflowRepeatExhaustion, WorkflowRepeatStop, WorkflowStopCondition,
};

/// Parse a `workflowPlan` value. `None` (after a warning) when it is
/// malformed; unknown fields are logged at debug and ignored.
pub(crate) fn parse(value: &Value) -> Option<WorkflowPlan> {
    let mut unknown = Vec::new();
    match plan(value, &mut unknown) {
        Ok(plan) => {
            if !unknown.is_empty() {
                tracing::debug!(
                    workflow_id = %plan.workflow_id,
                    fields = ?unknown,
                    "workflowPlan carries fields cyril does not model; ignored"
                );
            }
            Some(plan)
        }
        Err(error) => {
            tracing::warn!(
                %error,
                "malformed _meta.kiro.workflowPlan; the approval shows the request's text only"
            );
            None
        }
    }
}

/// Why a `workflowPlan` was rejected: the JSON path and the problem there.
#[derive(Debug, thiserror::Error)]
#[error("{path}: {problem}")]
pub(crate) struct PlanError {
    path: String,
    problem: String,
}

fn plan(value: &Value, unknown: &mut Vec<String>) -> Result<WorkflowPlan, PlanError> {
    let mut fields = Fields::new(value, "workflowPlan".to_owned())?;
    let workflow_id = WorkflowId::try_from(fields.string("workflowId")?)
        .map_err(|error| fields.error("workflowId", error.to_string()))?;
    let proposed_by = fields
        .opt_string("nodeId")?
        .map(|id| node_id(id, &fields, "nodeId"))
        .transpose()?;
    let kind = fields.enumerated::<WorkflowPlanKind>("kind")?;
    let workflow_ref = fields.string("workflowRef")?;
    let kept_step_ids = fields
        .array("keptStepIds")?
        .iter()
        .enumerate()
        .map(|(index, id)| match id {
            Value::String(id) => node_id(id.clone(), &fields, "keptStepIds"),
            other => Err(fields.error(
                &format!("keptStepIds[{index}]"),
                format!("expected a string, got {}", shape(other)),
            )),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let steps = children(&mut fields, "steps", unknown)?;
    let omitted = fields.opt_enumerated::<WorkflowPlanOmission>("omitted")?;
    fields.finish(unknown);
    Ok(WorkflowPlan {
        workflow_id,
        proposed_by,
        kind,
        workflow_ref,
        kept_step_ids,
        steps,
        omitted,
    })
}

fn children(
    fields: &mut Fields<'_>,
    key: &'static str,
    unknown: &mut Vec<String>,
) -> Result<Vec<WorkflowPlanNode>, PlanError> {
    let path = format!("{}.{key}", fields.path);
    fields
        .array(key)?
        .iter()
        .enumerate()
        .map(|(index, child)| node(child, format!("{path}[{index}]"), unknown))
        .collect()
}

fn node(
    value: &Value,
    path: String,
    unknown: &mut Vec<String>,
) -> Result<WorkflowPlanNode, PlanError> {
    let mut fields = Fields::new(value, path)?;
    let node_id = node_id(fields.string("nodeId")?, &fields, "nodeId")?;
    let kind = match fields.string("type")?.as_str() {
        "step" => WorkflowPlanNodeKind::Step(step(&mut fields, unknown)?),
        "sequence" => WorkflowPlanNodeKind::Sequence {
            steps: children(&mut fields, "steps", unknown)?,
            included_from: fields
                .opt_object("includedFrom")?
                .map(|object| include(object, &fields, unknown))
                .transpose()?,
        },
        "repeat" => {
            let steps = children(&mut fields, "steps", unknown)?;
            let max_iterations = fields.u32("maxIterations")?;
            let on_max_iterations =
                fields.enumerated::<WorkflowRepeatExhaustion>("onMaxIterations")?;
            let condition = fields
                .opt_object("stopCondition")?
                .map(|object| {
                    stop_condition(object, format!("{}.stopCondition", fields.path), unknown)
                })
                .transpose()?;
            let stop = match (condition, fields.opt_string("stopWhen")?) {
                (Some(condition), None) => Some(WorkflowRepeatStop::Condition(condition)),
                (None, Some(rule)) => Some(WorkflowRepeatStop::When(rule)),
                (None, None) => None,
                (Some(_), Some(_)) => {
                    return Err(fields.error(
                        "stopWhen",
                        "a repeat allows at most one of stopCondition and stopWhen".to_owned(),
                    ));
                }
            };
            WorkflowPlanNodeKind::Repeat {
                steps,
                max_iterations,
                on_max_iterations,
                stop,
            }
        }
        "parallel" => WorkflowPlanNodeKind::Parallel {
            branches: children(&mut fields, "branches", unknown)?,
            join_policy: fields.opt_enumerated::<WorkflowJoinPolicy>("joinPolicy")?,
        },
        "watch" => WorkflowPlanNodeKind::Watch {
            handler: fields.string("agentName")?,
            idle_timeout_sec: fields.opt_f64("idleTimeoutSec")?,
            config: fields.opt_object("config")?.cloned(),
        },
        other => {
            return Err(fields.error("type", format!("unknown node type `{other}`")));
        }
    };
    fields.finish(unknown);
    Ok(WorkflowPlanNode { node_id, kind })
}

fn step(fields: &mut Fields<'_>, unknown: &mut Vec<String>) -> Result<WorkflowPlanStep, PlanError> {
    let agent_name = fields.string("agentName")?;
    let model_id = fields.opt_string("modelId")?;
    let model_source = fields.opt_enumerated::<WorkflowCascadeSource>("modelSource")?;
    let effort_level = fields.opt_string("effortLevel")?;
    let effort_source = fields.opt_enumerated::<WorkflowCascadeSource>("effortSource")?;
    let task = match fields.opt_object("task")? {
        Some(object) => {
            let mut task = Fields::from_object(object, format!("{}.task", fields.path));
            let parsed = WorkflowPlanTask {
                text: task.string("text")?,
                truncated: task.bool("truncated")?,
            };
            task.finish(unknown);
            Some(parsed)
        }
        None => None,
    };
    let artifacts = match fields.opt_object("artifacts")? {
        Some(object) => object
            .iter()
            .map(|(name, path)| match path {
                Value::String(path) => Ok((name.clone(), path.clone())),
                other => Err(fields.error(
                    &format!("artifacts.{name}"),
                    format!("expected a string, got {}", shape(other)),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?,
        None => Vec::new(),
    };
    let completion = fields
        .opt_object("completion")?
        .map(|object| stop_condition(object, format!("{}.completion", fields.path), unknown))
        .transpose()?;
    Ok(WorkflowPlanStep {
        agent_name,
        model_id,
        model_source,
        effort_level,
        effort_source,
        task,
        artifacts,
        completion,
    })
}

fn stop_condition(
    object: &Map<String, Value>,
    path: String,
    unknown: &mut Vec<String>,
) -> Result<WorkflowStopCondition, PlanError> {
    let mut fields = Fields::from_object(object, path);
    let contains_text = fields.opt_string("containsText")?;
    let file_check = match fields.opt_object("fileCheck")? {
        Some(object) => {
            let mut check = Fields::from_object(object, format!("{}.fileCheck", fields.path));
            let path = check.string("path")?;
            let json_path = check.string("jsonPath")?;
            let value = check
                .get("value")
                .cloned()
                .ok_or_else(|| check.error("value", "missing".to_owned()))?;
            check.finish(unknown);
            Some(WorkflowFileCheck {
                path,
                json_path,
                value,
            })
        }
        None => None,
    };
    let completion_signal =
        fields.opt_enumerated::<WorkflowCompletionSignal>("completionSignal")?;
    fields.finish(unknown);
    Ok(WorkflowStopCondition {
        contains_text,
        file_check,
        completion_signal,
    })
}

fn include(
    object: &Map<String, Value>,
    parent: &Fields<'_>,
    unknown: &mut Vec<String>,
) -> Result<WorkflowPlanInclude, PlanError> {
    let mut fields = Fields::from_object(object, format!("{}.includedFrom", parent.path));
    let include = WorkflowPlanInclude {
        name: fields.string("name")?,
        path: fields.string("path")?,
    };
    fields.finish(unknown);
    Ok(include)
}

fn node_id(id: String, fields: &Fields<'_>, key: &str) -> Result<WorkflowNodeId, PlanError> {
    WorkflowNodeId::try_from(id).map_err(|error| fields.error(key, error.to_string()))
}

/// One-word JSON shape tag for error messages (never the value itself).
fn shape(value: &Value) -> &'static str {
    match value {
        Value::Object(_) => "an object",
        Value::Array(_) => "an array",
        Value::String(_) => "a string",
        Value::Number(_) => "a number",
        Value::Bool(_) => "a bool",
        Value::Null => "null",
    }
}

/// The fields of one wire object, recording which keys were read so the
/// rest can be reported as unknown.
struct Fields<'a> {
    object: &'a Map<String, Value>,
    path: String,
    read: Vec<&'static str>,
}

impl<'a> Fields<'a> {
    fn new(value: &'a Value, path: String) -> Result<Self, PlanError> {
        match value {
            Value::Object(object) => Ok(Self::from_object(object, path)),
            other => Err(PlanError {
                problem: format!("expected an object, got {}", shape(other)),
                path,
            }),
        }
    }

    fn from_object(object: &'a Map<String, Value>, path: String) -> Self {
        Self {
            object,
            path,
            read: Vec::new(),
        }
    }

    fn error(&self, key: &str, problem: String) -> PlanError {
        PlanError {
            path: format!("{}.{key}", self.path),
            problem,
        }
    }

    fn get(&mut self, key: &'static str) -> Option<&'a Value> {
        self.read.push(key);
        self.object.get(key)
    }

    fn mismatch(&self, key: &str, expected: &str, got: &Value) -> PlanError {
        self.error(key, format!("expected {expected}, got {}", shape(got)))
    }

    fn opt_string(&mut self, key: &'static str) -> Result<Option<String>, PlanError> {
        match self.get(key) {
            None => Ok(None),
            Some(Value::String(text)) => Ok(Some(text.clone())),
            Some(other) => Err(self.mismatch(key, "a string", other)),
        }
    }

    fn string(&mut self, key: &'static str) -> Result<String, PlanError> {
        self.opt_string(key)?
            .ok_or_else(|| self.error(key, "missing".to_owned()))
    }

    fn bool(&mut self, key: &'static str) -> Result<bool, PlanError> {
        match self.get(key) {
            Some(Value::Bool(flag)) => Ok(*flag),
            Some(other) => Err(self.mismatch(key, "a bool", other)),
            None => Err(self.error(key, "missing".to_owned())),
        }
    }

    fn u32(&mut self, key: &'static str) -> Result<u32, PlanError> {
        match self.get(key) {
            Some(Value::Number(number)) => number
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| self.error(key, format!("expected a u32, got {number}"))),
            Some(other) => Err(self.mismatch(key, "a number", other)),
            None => Err(self.error(key, "missing".to_owned())),
        }
    }

    fn opt_f64(&mut self, key: &'static str) -> Result<Option<f64>, PlanError> {
        match self.get(key) {
            None => Ok(None),
            Some(Value::Number(number)) => number
                .as_f64()
                .map(Some)
                .ok_or_else(|| self.error(key, format!("expected a finite number, got {number}"))),
            Some(other) => Err(self.mismatch(key, "a number", other)),
        }
    }

    fn array(&mut self, key: &'static str) -> Result<&'a Vec<Value>, PlanError> {
        match self.get(key) {
            Some(Value::Array(items)) => Ok(items),
            Some(other) => Err(self.mismatch(key, "an array", other)),
            None => Err(self.error(key, "missing".to_owned())),
        }
    }

    fn opt_object(
        &mut self,
        key: &'static str,
    ) -> Result<Option<&'a Map<String, Value>>, PlanError> {
        match self.get(key) {
            None => Ok(None),
            Some(Value::Object(object)) => Ok(Some(object)),
            Some(other) => Err(self.mismatch(key, "an object", other)),
        }
    }

    fn opt_enumerated<T>(&mut self, key: &'static str) -> Result<Option<T>, PlanError>
    where
        T: for<'s> TryFrom<&'s str, Error = crate::types::WorkflowEnumParseError>,
    {
        self.opt_string(key)?
            .map(|text| {
                T::try_from(text.as_str()).map_err(|error| self.error(key, error.to_string()))
            })
            .transpose()
    }

    fn enumerated<T>(&mut self, key: &'static str) -> Result<T, PlanError>
    where
        T: for<'s> TryFrom<&'s str, Error = crate::types::WorkflowEnumParseError>,
    {
        self.opt_enumerated(key)?
            .ok_or_else(|| self.error(key, "missing".to_owned()))
    }

    /// Report every key of this object that was never read.
    fn finish(self, unknown: &mut Vec<String>) {
        for key in self.object.keys() {
            if !self.read.contains(&key.as_str()) {
                unknown.push(format!("{}.{key}", self.path));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use agent_client_protocol::schema::v1 as acp;
    use serde_json::json;

    use super::*;

    /// The cyril-8wh0 fixture, carved from the 2.32.0 audit's live KAS
    /// captures (provenance in its `_provenance` entry).
    fn fixture(name: &str) -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/kas_permission/workflow-plan-2.32.0.json");
        let raw = std::fs::read_to_string(&path).expect("read fixture");
        let fixture: Value = serde_json::from_str(&raw).expect("fixture is JSON");
        fixture[name].clone()
    }

    fn request(name: &str) -> acp::RequestPermissionRequest {
        serde_json::from_value(fixture(name)).expect("request parses")
    }

    fn id(text: &str) -> WorkflowNodeId {
        WorkflowNodeId::try_from(text.to_owned()).expect("node id")
    }

    fn workflow(text: &str) -> WorkflowId {
        WorkflowId::try_from(text.to_owned()).expect("workflow id")
    }

    fn agent_step(node: &str, agent: &str) -> WorkflowPlanNode {
        WorkflowPlanNode {
            node_id: id(node),
            kind: WorkflowPlanNodeKind::Step(WorkflowPlanStep {
                agent_name: agent.to_owned(),
                model_id: None,
                model_source: None,
                effort_level: None,
                effort_source: None,
                task: None,
                artifacts: Vec::new(),
                completion: None,
            }),
        }
    }

    /// The live `update_workflow` approval of a prompt-created run (kind
    /// `launch`) converts into the typed plan, field for field.
    #[test]
    fn live_launch_plan_converts() {
        let plan = crate::protocol::convert::kas::permission_workflow_plan(&request("launch_live"))
            .expect("the live request carries a plan");
        assert_eq!(
            plan,
            WorkflowPlan {
                workflow_id: workflow("wf_6b22f0c99f407f0e"),
                proposed_by: Some(id("create-workflow")),
                kind: WorkflowPlanKind::Launch,
                workflow_ref: "generated://gen_09d8668976ecd154".to_owned(),
                kept_step_ids: vec![id("create-workflow")],
                steps: vec![WorkflowPlanNode {
                    node_id: id("write-hello"),
                    kind: WorkflowPlanNodeKind::Step(WorkflowPlanStep {
                        agent_name: "wf-coder".to_owned(),
                        model_id: None,
                        model_source: None,
                        effort_level: Some("low".to_owned()),
                        effort_source: Some(WorkflowCascadeSource::Step),
                        task: Some(WorkflowPlanTask {
                            text: "Create a file named hello.txt at the absolute path <LIVE_TMP>/ws-wfplan-nnzrhi04/hello.txt con".to_owned(),
                            truncated: true,
                        }),
                        artifacts: Vec::new(),
                        completion: None,
                    }),
                }],
                omitted: None,
            }
        );
    }

    /// The live client-launched creator's plan (kind `replace`).
    #[test]
    fn live_replace_plan_converts() {
        let plan =
            crate::protocol::convert::kas::permission_workflow_plan(&request("replace_live"))
                .expect("the live request carries a plan");
        assert_eq!(plan.kind, WorkflowPlanKind::Replace);
        assert_eq!(plan.proposed_by, Some(id("run")));
        assert_eq!(plan.kept_step_ids, vec![id("run")]);
        assert_eq!(plan.steps.len(), 1);
    }

    /// A request without `_meta.kiro.workflowPlan` (any ordinary approval)
    /// carries no plan.
    #[test]
    fn request_without_a_plan_has_none() {
        let mut args = fixture("launch_live");
        let kiro = args["_meta"]["kiro"]
            .as_object_mut()
            .expect("fixture has _meta.kiro");
        kiro.remove("workflowPlan");
        let args: acp::RequestPermissionRequest =
            serde_json::from_value(args).expect("request parses");
        assert_eq!(
            crate::protocol::convert::kas::permission_workflow_plan(&args),
            None
        );
    }

    /// Every node type and optional field the 0.66.43 bundle can emit is
    /// modelled: the synthetic full-shape plan converts with nothing left
    /// unknown.
    #[test]
    fn every_bundle_field_is_modelled() {
        let mut unknown = Vec::new();
        let plan = plan(&fixture("full_shape_synthetic"), &mut unknown).expect("plan converts");
        assert_eq!(unknown, Vec::<String>::new(), "unmodelled fields");
        let review = WorkflowPlanNode {
            node_id: id("review"),
            kind: WorkflowPlanNodeKind::Step(WorkflowPlanStep {
                effort_level: Some("low".to_owned()),
                effort_source: Some(WorkflowCascadeSource::Step),
                ..match agent_step("review", "wf-reviewer").kind {
                    WorkflowPlanNodeKind::Step(step) => step,
                    _ => unreachable!("agent_step builds a step"),
                }
            }),
        };
        assert_eq!(
            plan,
            WorkflowPlan {
                workflow_id: workflow("wf_synthetic"),
                proposed_by: Some(id("edit-workflow-r1")),
                kind: WorkflowPlanKind::Edit,
                workflow_ref: "generated://gen_synthetic".to_owned(),
                kept_step_ids: vec![id("plan"), id("edit-workflow-r1")],
                steps: vec![
                    WorkflowPlanNode {
                        node_id: id("build"),
                        kind: WorkflowPlanNodeKind::Step(WorkflowPlanStep {
                            agent_name: "wf-coder".to_owned(),
                            model_id: Some("claude-sonnet-4.5".to_owned()),
                            model_source: Some(WorkflowCascadeSource::Workflow),
                            effort_level: Some("high".to_owned()),
                            effort_source: Some(WorkflowCascadeSource::Parent),
                            task: Some(WorkflowPlanTask {
                                text: "Implement the parser".to_owned(),
                                truncated: false,
                            }),
                            artifacts: vec![("report".to_owned(), "out/report.md".to_owned())],
                            completion: Some(WorkflowStopCondition {
                                contains_text: None,
                                file_check: None,
                                completion_signal: Some(WorkflowCompletionSignal::Success),
                            }),
                        }),
                    },
                    WorkflowPlanNode {
                        node_id: id("review-loop"),
                        kind: WorkflowPlanNodeKind::Repeat {
                            steps: vec![review],
                            max_iterations: 3,
                            on_max_iterations: WorkflowRepeatExhaustion::Pause,
                            stop: Some(WorkflowRepeatStop::Condition(WorkflowStopCondition {
                                contains_text: Some("LGTM".to_owned()),
                                file_check: Some(WorkflowFileCheck {
                                    path: "out/status.json".to_owned(),
                                    json_path: "$.ok".to_owned(),
                                    value: json!(true),
                                }),
                                completion_signal: None,
                            })),
                        },
                    },
                    WorkflowPlanNode {
                        node_id: id("fanout"),
                        kind: WorkflowPlanNodeKind::Parallel {
                            join_policy: Some(WorkflowJoinPolicy::AllSettled),
                            branches: vec![
                                WorkflowPlanNode {
                                    node_id: id("docs"),
                                    kind: WorkflowPlanNodeKind::Sequence {
                                        steps: vec![agent_step("write-docs", "wf-writer")],
                                        included_from: Some(WorkflowPlanInclude {
                                            name: "docs-recipe".to_owned(),
                                            path: ".kiro/workflows/docs.workflow.json".to_owned(),
                                        }),
                                    },
                                },
                                WorkflowPlanNode {
                                    node_id: id("watch-ci"),
                                    kind: WorkflowPlanNodeKind::Watch {
                                        handler: "wf-ci-watcher".to_owned(),
                                        idle_timeout_sec: Some(90.0),
                                        config: json!({"branch": "main"}).as_object().cloned(),
                                    },
                                },
                            ],
                        },
                    },
                    WorkflowPlanNode {
                        node_id: id("polish"),
                        kind: WorkflowPlanNodeKind::Repeat {
                            steps: vec![agent_step("tidy", "wf-coder")],
                            max_iterations: 1,
                            on_max_iterations: WorkflowRepeatExhaustion::Continue,
                            stop: Some(WorkflowRepeatStop::When("the code is tidy".to_owned())),
                        },
                    },
                ],
                omitted: None,
            }
        );
    }

    /// `omitted: "steps"` — KAS's last resort for an oversized plan — keeps
    /// the header and an empty step list.
    #[test]
    fn omitted_steps_plan_converts() {
        let plan = parse(&json!({
            "workflowId": "wf_1",
            "kind": "replace",
            "workflowRef": "generated://g",
            "keptStepIds": [],
            "steps": [],
            "omitted": "steps"
        }))
        .expect("plan converts");
        assert_eq!(plan.omitted, Some(WorkflowPlanOmission::Steps));
        assert_eq!(plan.proposed_by, None);
        assert!(plan.steps.is_empty());
    }

    /// A field cyril does not model is ignored — the plan still converts —
    /// and named once at debug level, with its path.
    #[test]
    fn unknown_fields_are_ignored_with_a_debug_log() {
        let mut value = fixture("full_shape_synthetic");
        value["planRevision"] = json!(2);
        value["steps"][0]["capturePinned"] = json!(true);
        let (_guard, capture, dispatch) = crate::test_support::capture_json_subscriber();
        let plan = tracing::dispatcher::with_default(&dispatch, || parse(&value));
        assert!(plan.is_some(), "an unknown field must not drop the plan");
        let events = capture.captured();
        assert_eq!(events.len(), 1, "one debug event: {events:?}");
        assert_eq!(events[0]["level"], "DEBUG", "{events:?}");
        let fields = events[0]["fields"]["fields"].to_string();
        for path in [
            "workflowPlan.planRevision",
            "workflowPlan.steps[0].capturePinned",
        ] {
            assert!(fields.contains(path), "{path} not named: {fields}");
        }
    }

    /// A malformed plan is dropped with one warning naming where it broke;
    /// the approval then shows the request's Text rendering only.
    #[rstest::rstest]
    #[case::missing_workflow_id(
        json!({"kind": "launch", "workflowRef": "g", "keptStepIds": [], "steps": []}),
        "workflowPlan.workflowId: missing"
    )]
    #[case::unknown_kind(
        json!({"workflowId": "wf", "kind": "rewrite", "workflowRef": "g", "keptStepIds": [], "steps": []}),
        "workflowPlan.kind: unknown workflow plan kind value `rewrite`"
    )]
    #[case::step_without_agent(
        json!({"workflowId": "wf", "kind": "launch", "workflowRef": "g", "keptStepIds": [],
               "steps": [{"nodeId": "a", "type": "step"}]}),
        "workflowPlan.steps[0].agentName: missing"
    )]
    #[case::unknown_node_type(
        json!({"workflowId": "wf", "kind": "launch", "workflowRef": "g", "keptStepIds": [],
               "steps": [{"nodeId": "a", "type": "loop"}]}),
        "workflowPlan.steps[0].type: unknown node type `loop`"
    )]
    #[case::non_string_kept_id(
        json!({"workflowId": "wf", "kind": "launch", "workflowRef": "g", "keptStepIds": [7], "steps": []}),
        "workflowPlan.keptStepIds[0]: expected a string, got a number"
    )]
    #[case::both_repeat_stops(
        json!({"workflowId": "wf", "kind": "launch", "workflowRef": "g", "keptStepIds": [],
               "steps": [{"nodeId": "r", "type": "repeat", "steps": [], "maxIterations": 2,
                          "onMaxIterations": "abort", "stopWhen": "done",
                          "stopCondition": {"containsText": "x"}}]}),
        "workflowPlan.steps[0].stopWhen: a repeat allows at most one of stopCondition and stopWhen"
    )]
    fn malformed_plans_are_dropped_with_a_warning(#[case] value: Value, #[case] message: &str) {
        let mut unknown = Vec::new();
        let error = plan(&value, &mut unknown).expect_err("malformed plan");
        assert_eq!(error.to_string(), message);

        let (_guard, capture, dispatch) = crate::test_support::capture_json_subscriber();
        let parsed = tracing::dispatcher::with_default(&dispatch, || parse(&value));
        assert_eq!(parsed, None);
        let events = capture.captured();
        assert_eq!(events.len(), 1, "one warning: {events:?}");
        assert_eq!(events[0]["level"], "WARN", "{events:?}");
    }
}
