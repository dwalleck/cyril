use std::env;
use std::error::Error;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use cyril_core::protocol::bridge::{SpawnConfig, spawn_bridge};
use cyril_core::types::{
    AgentCommand, AgentEngine, BridgeCommand, KasSpawn, Notification, PermissionResponse,
};
use serde_json::{Value, json};

// Exact authoritative Matt Pocock installation pinned by trailblazer-mayb.
const DEFAULT_WAYFINDER_SKILL: &str = "/home/dwalleck/.omp/plugins/cache/marketplaces/mattpocock/skills/engineering/wayfinder/SKILL.md";
const DEFAULT_GRILLING_SKILL: &str = "/home/dwalleck/.omp/plugins/cache/marketplaces/mattpocock/skills/productivity/grilling/SKILL.md";
const DEFAULT_DOMAIN_MODELING_SKILL: &str = "/home/dwalleck/.omp/plugins/cache/marketplaces/mattpocock/skills/engineering/domain-modeling/SKILL.md";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // bh7g instrumentation: cyril-core's mediator/conversion debug traces to
    // stderr with microsecond stamps, controlled by RUST_LOG.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();
    let mut args = env::args().skip(1);
    let fixture_arg = args.next().ok_or("usage: probe <fixture-directory>")?;
    if args.next().is_some() {
        return Err(
            "the current probe uses cyril-core's pinned KAS free path; no agent argv is accepted"
                .into(),
        );
    }
    let fixture = PathBuf::from(fixture_arg).canonicalize()?;

    let prompt = build_initial_prompt()?;
    let bridge = spawn_bridge(
        AgentCommand::new("unused-for-kas-free-path"),
        SpawnConfig {
            engine: AgentEngine::Kas,
            kas_spawn: KasSpawn::Free,
            ..SpawnConfig::default()
        },
        fixture.clone(),
    )?;
    let (sender, mut notifications, mut permissions) = bridge.split();

    // bh7g: persistent stdin reader. Lines flow through a channel so the
    // driver can inject "CANCEL" mid-turn (routed to CancelRequest by the
    // select arm below); during an answer prompt, lines are the answer.
    let (line_tx, mut line_rx) = tokio::sync::mpsc::channel::<String>(8);
    std::thread::spawn(move || {
        use std::io::BufRead as _;
        for line in io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if line_tx.blocking_send(line).is_err() {
                break;
            }
        }
    });

    println!("PROTOTYPE — throwaway authoritative Wayfinder transport probe");
    println!("FIXTURE {}", fixture.display());
    sender
        .send(BridgeCommand::NewSession {
            cwd: fixture.clone(),
        })
        .await?;

    let mut session_id = None;
    let mut visible_agent_text = String::new();

    loop {
        tokio::select! {
            line = line_rx.recv() => {
                let Some(line) = line else {
                    return Err("stdin closed while idle".into());
                };
                if line.trim() == "CANCEL" {
                    println!("\nCANCEL_SENT");
                    sender.send(BridgeCommand::CancelRequest).await?;
                } else {
                    println!("\nUNEXPECTED_STDIN_LINE {line}");
                }
            }
            routed = notifications.recv() => {
                let Some(routed) = routed else {
                    return Err("ACP notification channel closed before map creation".into());
                };
                match routed.notification {
                    Notification::SessionCreated { session_id: created, .. } => {
                        println!("{}", json!({
                            "event": "session_created",
                            "session_id": created.as_str(),
                        }));
                        session_id = Some(created.clone());
                        sender.send(BridgeCommand::SendPrompt {
                            session_id: created,
                            content_blocks: vec![prompt.clone()],
                        }).await?;
                    }
                    Notification::AgentMessage(message) => {
                        print!("{}", message.text);
                        io::stdout().flush()?;
                        visible_agent_text.push_str(&message.text);
                    }
                    Notification::AgentThought(_) => {
                        // Deliberately excluded from the visible transcript.
                    }
                    Notification::ToolCallStarted(tool) | Notification::ToolCallUpdated(tool) => {
                        println!("\n{}", json!({
                            "event": "tool",
                            "id": tool.id().as_str(),
                            "title": tool.title(),
                            "kind": format!("{:?}", tool.kind()),
                            "status": format!("{:?}", tool.status()),
                            "raw_input": tool.raw_input(),
                            "raw_output": tool.raw_output(),
                        }));
                    }
                    Notification::BridgeError { operation, message } => {
                        return Err(format!("bridge operation {operation} failed: {message}").into());
                    }
                    Notification::BridgeDisconnected { reason } => {
                        return Err(format!("ACP bridge disconnected: {reason}").into());
                    }
                    Notification::TurnCompleted { stop_reason } => {
                        println!("\n{}", json!({
                            "event": "turn_completed",
                            "stop_reason": format!("{stop_reason:?}"),
                        }));

                        if let Some(map) = read_persisted_map(&fixture)? {
                            println!("PERSISTED_WAYFINDER_MAP {}", serde_json::to_string_pretty(&map)?);
                            sender.send(BridgeCommand::Shutdown).await?;
                            return Ok(());
                        }

                        let active_session = session_id.clone().ok_or("turn completed before session creation")?;
                        let form = structured_form(&visible_agent_text);
                        println!("STRUCTURED_CHOICE_FORM {}", serde_json::to_string_pretty(&form)?);
                        let answer = read_human_answer("Answer this round; preserve Q numbers: ", &mut line_rx).await?;
                        visible_agent_text.clear();
                        sender.send(BridgeCommand::SendPrompt {
                            session_id: active_session,
                            content_blocks: vec![answer],
                        }).await?;
                    }
                    other => {
                        println!("\n{}", json!({
                            "event": "notification",
                            "kind": format!("{other:?}"),
                        }));
                    }
                }
            }
            request = permissions.recv() => {
                let Some(request) = request else {
                    return Err("ACP permission channel closed before map creation".into());
                };
                let payload = json!({
                    "event": "permission",
                    "session_id": request.session_id.as_str(),
                    "message": request.message,
                    "tool": {
                        "id": request.tool_call.id().as_str(),
                        "title": request.tool_call.title(),
                        "kind": format!("{:?}", request.tool_call.kind()),
                        "raw_input": request.tool_call.raw_input(),
                    },
                    "options": request.options.iter().map(|option| json!({
                        "id": option.id.as_str(),
                        "label": option.label,
                        "kind": format!("{:?}", option.kind),
                        "destructive": option.is_destructive,
                    })).collect::<Vec<_>>(),
                });
                println!("\nPERMISSION_REQUEST {}", serde_json::to_string_pretty(&payload)?);

                if is_fixture_rivets_operation(request.tool_call.raw_input(), &fixture) {
                    let option = request.options.iter()
                        .find(|option| matches!(option.kind, cyril_core::types::PermissionOptionKind::AllowOnce))
                        .ok_or("fixture Rivets permission offered no allow-once option")?;
                    println!("AUTO_APPROVED_FIXTURE_OPERATION {}", option.id.as_str());
                    request.responder.send(PermissionResponse::Selected {
                        option_id: option.id.clone(),
                        trust_option: None,
                    }).map_err(|_| "agent dropped fixture permission responder")?;
                } else if is_human_choice(
                    &request.message,
                    request.tool_call.title(),
                    request.tool_call.raw_input(),
                    &request.options,
                ) {
                    let choices: Vec<_> = request.options.iter()
                        .filter(|option| !matches!(option.kind,
                            cyril_core::types::PermissionOptionKind::RejectOnce |
                            cyril_core::types::PermissionOptionKind::RejectAlways))
                        .collect();
                    if choices.is_empty() {
                        return Err("human choice request exposed no selectable answer options".into());
                    }
                    let form = json!({
                        "control": "ordered_choice",
                        "prompt": request.message,
                        "choices": choices.iter().enumerate().map(|(index, option)| json!({
                            "ordinal": index + 1,
                            "id": option.id.as_str(),
                            "label": option.label,
                        })).collect::<Vec<_>>(),
                    });
                    println!("STRUCTURED_PERMISSION_CHOICE {}", serde_json::to_string_pretty(&form)?);
                    let selected = read_human_answer("Choose an ordinal: ", &mut line_rx).await?;
                    let index: usize = selected.trim().parse()?;
                    let option = choices.get(index.checked_sub(1).ok_or("choice ordinals start at 1")?)
                        .ok_or("choice ordinal is outside the offered range")?;
                    request.responder.send(PermissionResponse::Selected {
                        option_id: option.id.clone(),
                        trust_option: None,
                    }).map_err(|_| "agent dropped human-choice responder")?;
                } else {
                    // bh7g harness: the fixture is disposable and artificial
                    // aborts cost wedge reproductions — approve-and-log
                    // instead of blocking. (fs_write of spec docs mid-charting
                    // is a normal model behavior variant.)
                    match request.options.iter().find(|option| {
                        matches!(option.kind, cyril_core::types::PermissionOptionKind::AllowOnce)
                    }) {
                        Some(option) => {
                            println!("AUTO_APPROVED_FALLBACK {}", option.id.as_str());
                            request.responder.send(PermissionResponse::Selected {
                                option_id: option.id.clone(),
                                trust_option: None,
                            }).map_err(|_| "agent dropped fallback permission responder")?;
                        }
                        None => {
                            println!("UNEXPECTED_PERMISSION_BLOCKED (no allow-once option)");
                            request.responder.send(PermissionResponse::Cancel)
                                .map_err(|_| "agent dropped unexpected permission responder")?;
                        }
                    }
                }
            }
        }
    }
}

fn build_initial_prompt() -> Result<String, Box<dyn Error>> {
    let wayfinder = read_skill("WAYFINDER_SKILL_PATH", DEFAULT_WAYFINDER_SKILL)?;
    let grilling = read_skill("GRILLING_SKILL_PATH", DEFAULT_GRILLING_SKILL)?;
    let domain_modeling = read_skill("DOMAIN_MODELING_SKILL_PATH", DEFAULT_DOMAIN_MODELING_SKILL)?;
    let request = env::var("TRAILBLAZER_CHART_REQUEST").unwrap_or_else(|_| {
        "Chart a Wayfinder map toward a decision-complete implementation specification for a small terminal habit tracker that stores data locally and is intended for one user.".into()
    });

    Ok(format!(
        r#"The host explicitly supplies the following authoritative instructions. Treat them as active instructions for this session. Do not discover or load any other copy of these skills.

<authoritative-skill name="wayfinder">
{wayfinder}
</authoritative-skill>

<authoritative-skill name="grilling">
{grilling}
</authoritative-skill>

<authoritative-skill name="domain-modeling">
{domain_modeling}
</authoritative-skill>

<tracker-contract>
This disposable workspace uses Rivets. Use only the installed `rivets` CLI from the workspace root; never edit `.rivets/issues.jsonl` directly.
- Create the map with `rivets create --title "..." --kind epic --priority 2 --description "..." --label wayfinder:map --json`.
- Create each child as kind `task`, priority 2, description `## Question\\n\\n...`, and exactly one of: `wayfinder:research`, `wayfinder:grilling`, `wayfinder:prototype`, `wayfinder:task`.
- After every issue exists, add its parent edge with `rivets dep add <child-id> <map-id> -t parent-child -y`.
- Add blockers with `rivets dep add <dependent-id> <blocker-id> -t blocks -y`; wire multiple blockers sequentially.
- Read with `rivets show <id> --json`, `rivets ready -n 100 --json`, and filtered `rivets list --json`.
- Claiming means assigning before work; resolution means an immutable note, close, then a one-line Decisions-so-far pointer.
Charting creates tracker issues; it does not create a standalone Markdown map.
</tracker-contract>

<user-request>
{request}
</user-request>

Begin real Wayfinder Charting now. Ask the human every decision required by the authoritative instructions, wait for answers between rounds, and create the persisted Rivets map and child tickets only after the human decisions required for Charting are settled."#
    ))
}

fn read_skill(env_name: &str, default_path: &str) -> Result<String, Box<dyn Error>> {
    let path = PathBuf::from(env::var(env_name).unwrap_or_else(|_| default_path.into()));
    let canonical = path.canonicalize()?;
    Ok(std::fs::read_to_string(canonical)?)
}

fn structured_form(message: &str) -> Value {
    let mut questions = Vec::new();
    let mut current = String::new();
    for line in message.lines() {
        if line.trim_start().starts_with('❓') && !current.trim().is_empty() {
            questions.push(current.trim().to_owned());
            current.clear();
        }
        if line.trim_start().starts_with('❓') || !current.is_empty() {
            current.push_str(line);
            current.push('\n');
        }
    }
    if !current.trim().is_empty() {
        questions.push(current.trim().to_owned());
    }
    if questions.is_empty() {
        questions.push(message.trim().to_owned());
    }

    json!({
        "control": "ordered_question_round",
        "questions": questions.into_iter().enumerate().map(|(index, markdown)| json!({
            "ordinal": index + 1,
            "markdown": markdown,
            "answer_control": "custom_text",
        })).collect::<Vec<_>>(),
    })
}

fn is_fixture_rivets_operation(raw_input: Option<&Value>, fixture: &Path) -> bool {
    let Some(raw_input) = raw_input else {
        return false;
    };
    let mut commands = Vec::new();
    collect_command_fields(raw_input, &mut commands);
    let fixture_prefix = format!("cd {} && ", fixture.display());
    !commands.is_empty()
        && commands.iter().all(|command| {
            let trimmed = command.trim();
            let body = trimmed.strip_prefix(&fixture_prefix).unwrap_or(trimmed);
            let executable = body.split_whitespace().next().unwrap_or_default();
            let is_rivets = executable == "rivets" || executable.ends_with("/rivets");
            // bh7g harness: the fixture is disposable and the goal is wedge
            // hunting, so quoted multi-line `rivets create` bodies (newlines,
            // \`, $) must pass. Keep only the path-escape guard.
            is_rivets && !body.contains("../")
        })
}

fn collect_command_fields<'a>(value: &'a Value, commands: &mut Vec<&'a str>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if matches!(key.as_str(), "command" | "cmd") {
                    if let Some(command) = child.as_str() {
                        commands.push(command);
                    }
                } else {
                    collect_command_fields(child, commands);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_command_fields(item, commands);
            }
        }
        _ => {}
    }
}

fn is_human_choice(
    message: &str,
    title: &str,
    raw_input: Option<&Value>,
    options: &[cyril_core::types::PermissionOption],
) -> bool {
    let uniform_option_kind =
        options.len() >= 2 && options.windows(2).all(|pair| pair[0].kind == pair[1].kind);
    let mut haystack = format!("{message} {title}").to_lowercase();
    if let Some(input) = raw_input {
        haystack.push(' ');
        haystack.push_str(&input.to_string().to_lowercase());
    }
    uniform_option_kind
        || [
            "ask_user",
            "present_question",
            "human choice",
            "choose an option",
        ]
        .iter()
        .any(|needle| haystack.contains(needle))
}

async fn read_human_answer(
    prompt: &'static str,
    lines: &mut tokio::sync::mpsc::Receiver<String>,
) -> Result<String, Box<dyn Error>> {
    print!("{prompt}");
    io::stdout().flush()?;
    let answer = lines.recv().await.ok_or("stdin closed during answer")?;
    if answer.trim().is_empty() {
        return Err("human answer must not be empty".into());
    }
    Ok(answer)
}

fn read_persisted_map(fixture: &Path) -> Result<Option<Value>, Box<dyn Error>> {
    let maps = run_rivets_json(
        fixture,
        &[
            "list",
            "--json",
            "--label",
            "wayfinder:map",
            "--kind",
            "epic",
            "--status",
            "open",
            "-n",
            "2",
        ],
    )?;
    let maps = maps
        .as_array()
        .ok_or("rivets map discovery did not return an array")?;
    if maps.is_empty() {
        return Ok(None);
    }
    if maps.len() != 1 {
        return Err("fixture contains more than one open wayfinder:map epic".into());
    }
    let map_id = maps[0]
        .get("id")
        .and_then(Value::as_str)
        .ok_or("map discovery result has no id")?;
    let shown = run_rivets_json(fixture, &["show", map_id, "--json"])?;
    let map = shown
        .as_array()
        .and_then(|items| items.first())
        .ok_or("rivets show returned no map")?;
    let child_ids: Vec<&str> = map
        .get("dependents")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|edge| edge.get("dep_type").and_then(Value::as_str) == Some("parent-child"))
        .filter_map(|edge| edge.get("depends_on_id").and_then(Value::as_str))
        .collect();
    let children = if child_ids.is_empty() {
        Value::Array(Vec::new())
    } else {
        let mut args = vec!["show"];
        args.extend(child_ids);
        args.push("--json");
        run_rivets_json(fixture, &args)?
    };
    let ready_bound = children
        .as_array()
        .map_or(2, |items| items.len() + 2)
        .to_string();
    let ready = run_rivets_json(fixture, &["ready", "--json", "-n", &ready_bound])?;
    let blocked = run_rivets_json(fixture, &["blocked", "--json"])?;
    Ok(Some(
        json!({ "map": map, "children": children, "ready": ready, "blocked": blocked }),
    ))
}

fn run_rivets_json(fixture: &Path, args: &[&str]) -> Result<Value, Box<dyn Error>> {
    let output = Command::new("rivets")
        .args(args)
        .current_dir(fixture)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "rivets {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
