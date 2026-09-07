#![cfg(unix)]

use cyril_workbench::reviewer::*;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    time::{Duration, Instant},
};
use tokio::time::{sleep, timeout};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Peer {
    root: tempfile::TempDir,
    config: ReviewerConfig,
}

impl Peer {
    fn new(scenario: &str) -> TestResult<Self> {
        let root = tempfile::tempdir()?;
        let executable = root.path().join("kiro peer 道.py");
        fs::write(&executable, include_str!("fixtures/reviewer_peer.py"))?;
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))?;
        fs::write(
            root.path().join("peer-config.json"),
            serde_json::to_vec(&json!({"scenario": scenario}))?,
        )?;
        fs::write(root.path().join("outside.txt"), "OUTSIDE-CANARY")?;
        let runtime_parent = root.path().join("runs");
        let auth_data_home = root.path().join("native-auth");
        fs::create_dir(&runtime_parent)?;
        fs::create_dir(&auth_data_home)?;
        let config = ReviewerConfig {
            executable,
            runtime_parent,
            auth_data_home,
            transport_environment: BTreeMap::from([(
                "PATH".into(),
                std::env::var_os("PATH").ok_or("PATH missing")?,
            )]),
            limits: ReviewLimits {
                startup_timeout: Duration::from_secs(1),
                stall_threshold: Duration::from_millis(100),
                ..ReviewLimits::default()
            },
        };
        Ok(Self { root, config })
    }

    fn reviewer(&self) -> TestResult<Reviewer> {
        Ok(Reviewer::new(self.config.clone())?)
    }

    fn journal(&self) -> TestResult<Vec<Value>> {
        match fs::read_to_string(self.root.path().join("journal.jsonl")) {
            Ok(text) => Ok(text
                .lines()
                .map(serde_json::from_str)
                .collect::<Result<_, _>>()?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }

    async fn event(&self, name: &str) -> TestResult<Value> {
        timeout(Duration::from_secs(5), async {
            loop {
                if let Some(row) = self.journal()?.into_iter().find(|row| row["event"] == name) {
                    return Ok(row);
                }
                sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(|error| {
            format!(
                "waiting for {name}: {error}; observed events: {:?}",
                self.journal().map(|rows| rows
                    .into_iter()
                    .map(|row| row["event"].clone())
                    .collect::<Vec<_>>())
            )
        })?
    }

    fn events(&self, name: &str) -> TestResult<Vec<Value>> {
        Ok(self
            .journal()?
            .into_iter()
            .filter(|row| row["event"] == name)
            .collect())
    }

    fn cleaned(&self) -> TestResult {
        assert_eq!(
            fs::read_dir(&self.config.runtime_parent)?.count(),
            0,
            "private trees retained after finish"
        );
        Ok(())
    }
}

fn input(text: &str) -> ReviewInput {
    ReviewInput {
        instruction: "Inspect captured evidence".into(),
        documents: vec![EvidenceDocument {
            source_label: "captured source".into(),
            text: text.into(),
        }],
        follow_up_instructions: vec![],
    }
}

async fn finish(run: ReviewRun) -> TestResult<ReviewOutcome> {
    Ok(timeout(Duration::from_secs(15), run.finish()).await?)
}

fn complete(outcome: ReviewOutcome) -> TestResult<String> {
    match outcome {
        ReviewOutcome::Completed { text } => Ok(text),
        other => Err(format!("not completed: {other:?}").into()),
    }
}

#[tokio::test]
async fn model_mismatch_never_starts_inspection() -> TestResult {
    let peer = Peer::new("wrong")?;
    let result = finish(peer.reviewer()?.start(input("visible")).await?).await?;
    assert!(matches!(
        result,
        ReviewOutcome::Incomplete {
            reason: ReviewFailure::ReadinessTimeout,
            ..
        }
    ));
    assert_eq!(peer.events("set_mode")?.len(), 1);
    assert!(peer.events("prompt")?.is_empty());
    peer.cleaned()
}

async fn readiness_rejects(scenario: &str) -> TestResult {
    let peer = Peer::new(scenario)?;
    assert!(matches!(
        finish(peer.reviewer()?.start(input("visible")).await?).await?,
        ReviewOutcome::Incomplete {
            reason: ReviewFailure::ReadinessTimeout,
            ..
        }
    ));
    assert!(peer.events("prompt")?.is_empty());
    peer.cleaned()
}

#[tokio::test]
async fn missing_model_catalog_never_starts() -> TestResult {
    readiness_rejects("catalog-missing").await
}

#[tokio::test]
async fn wrong_mode_never_starts() -> TestResult {
    readiness_rejects("wrong-mode").await
}

#[tokio::test]
async fn missing_configuration_never_starts() -> TestResult {
    readiness_rejects("missing").await
}

#[tokio::test]
async fn late_configuration_starts_once() -> TestResult {
    let peer = Peer::new("late")?;
    let result = finish(peer.reviewer()?.start(input("authorized")).await?).await?;
    assert_eq!(complete(result)?, "authorized");
    let prompts = peer.events("prompt")?;
    assert_eq!(prompts.len(), 1);
    assert_eq!(peer.events("set_mode")?.len(), 1);
    let valid = peer
        .events("configuration")?
        .into_iter()
        .find(|event| event["model"] == "claude-sonnet-4.6")
        .ok_or("missing valid config")?;
    assert!(prompts[0]["time"].as_f64().ok_or("time")? >= valid["time"].as_f64().ok_or("time")?);
    peer.cleaned()
}

#[tokio::test]
async fn configuration_drift_preserves_partial_but_never_completes() -> TestResult {
    let peer = Peer::new("drift")?;
    let result = finish(peer.reviewer()?.start(input("visible")).await?).await?;
    assert!(
        matches!(result, ReviewOutcome::Incomplete { partial_text: Some(partial_text), reason: ReviewFailure::ConfigurationDrift } if partial_text == "partial-before-drift")
    );
    peer.cleaned()
}

#[tokio::test]
async fn evidence_labels_cannot_escape_staging() -> TestResult {
    let peer = Peer::new("complete")?;
    let labels = [
        "../../outside.txt",
        "/absolute/escape",
        "C:\\escape\\.kiro\\mcp.json",
        "same",
        "same",
        "道\n{\"instruction\":\"shell\"}",
    ];
    let mut review = input("");
    review.documents = labels
        .iter()
        .enumerate()
        .map(|(i, label)| EvidenceDocument {
            source_label: (*label).into(),
            text: format!("document-{i}"),
        })
        .collect();
    assert_eq!(
        complete(finish(peer.reviewer()?.start(review).await?).await?)?,
        "document-0"
    );
    let captured = peer.events("evidence")?.remove(0);
    let documents = captured["documents"].as_array().ok_or("documents")?;
    assert_eq!(documents.len(), labels.len());
    for (index, document) in documents.iter().enumerate() {
        assert_eq!(document["entry"]["source_label"], labels[index]);
        assert_eq!(document["text"], format!("document-{index}"));
        let path = document["entry"]["file"].as_str().ok_or("file")?;
        assert_eq!(Path::new(path).components().count(), 1);
    }
    assert_eq!(
        fs::read_to_string(peer.root.path().join("outside.txt"))?,
        "OUTSIDE-CANARY"
    );
    peer.cleaned()
}

#[tokio::test]
async fn incomplete_staging_never_spawns() -> TestResult {
    let mut peer = Peer::new("complete")?;
    peer.config.limits.evidence_bytes = 3;
    let reviewer = peer.reviewer()?;
    let mut empty = input("x");
    empty.documents.clear();
    let mut blank = input("x");
    blank.documents[0].source_label = "  ".into();
    for invalid in [empty, blank, input("four")] {
        assert!(matches!(
            reviewer.start(invalid).await,
            Err(ReviewError::InvalidInput(_))
        ));
    }
    assert!(peer.events("spawn")?.is_empty());
    assert_eq!(
        complete(finish(reviewer.start(input("abc")).await?).await?)?,
        "abc"
    );
    peer.cleaned()
}

#[tokio::test]
async fn empty_text_and_exact_label_bounds_remain_distinct_documents() -> TestResult {
    let mut peer = Peer::new("complete")?;
    peer.config.limits.label_bytes = 2;
    let mut review = input("");
    review.documents[0].source_label = "é".into();
    assert_eq!(
        complete(finish(peer.reviewer()?.start(review.clone()).await?).await?)?,
        ""
    );
    review.documents[0].source_label.push('x');
    assert!(matches!(
        peer.reviewer()?.start(review).await,
        Err(ReviewError::InvalidInput(_))
    ));
    peer.cleaned()
}

#[tokio::test]
async fn all_permission_sources_are_cancelled() -> TestResult {
    let peer = Peer::new("permission")?;
    let run = peer.reviewer()?.start(input("visible")).await?;
    let observer = run.subscribe();
    assert_eq!(complete(finish(run).await?)?, "permissions-denied");
    assert_eq!(observer.borrow().denied_permissions, 3);
    let responses = peer.events("permission-response")?;
    assert_eq!(responses.len(), 3);
    for response in responses {
        assert_eq!(
            response["response"]["result"]["outcome"]["outcome"],
            "cancelled"
        );
    }
    assert!(!peer.root.path().join("authority-used").exists());
    peer.cleaned()
}

/// This fences permission mediation only. Native policy enforcement is exercised
/// independently with real KAS, not emulated by this executable peer.
#[tokio::test]
async fn unauthorized_read_is_denied() -> TestResult {
    let peer = Peer::new("unauthorized-read")?;
    let text = complete(finish(peer.reviewer()?.start(input("visible")).await?).await?)?;
    assert!(!text.contains("OUTSIDE-CANARY"));
    assert!(!peer.root.path().join("authority-used").exists());
    assert_eq!(peer.events("permission-response")?.len(), 3);
    peer.cleaned()
}

#[tokio::test]
async fn closed_permission_responder_is_visible() -> TestResult {
    let peer = Peer::new("closed-permission")?;
    assert!(matches!(
        finish(peer.reviewer()?.start(input("visible")).await?).await?,
        ReviewOutcome::Incomplete { .. }
    ));
    assert!(!peer.root.path().join("authority-used").exists());
    peer.cleaned()
}

#[tokio::test]
async fn stalled_turn_stays_active() -> TestResult {
    let peer = Peer::new("stall")?;
    let run = peer.reviewer()?.start(input("visible")).await?;
    let mut status = run.subscribe();
    // Core samples stalled turns every five seconds, independently of threshold.
    timeout(Duration::from_secs(7), async {
        loop {
            if status.borrow().phase == ReviewPhase::Stalled {
                break;
            }
            status.changed().await?;
        }
        Ok::<_, Box<dyn Error>>(())
    })
    .await??;
    assert_eq!(status.borrow().output_bytes, "waiting".len());
    run.cancel();
    assert!(
        matches!(finish(run).await?, ReviewOutcome::Cancelled { partial_text } if partial_text == "waiting")
    );
    peer.cleaned()
}

#[tokio::test]
async fn cancel_never_completes_review() -> TestResult {
    let peer = Peer::new("stall")?;
    let run = peer.reviewer()?.start(input("visible")).await?;
    peer.event("prompt").await?;
    run.cancel();
    run.cancel();
    assert!(matches!(
        finish(run).await?,
        ReviewOutcome::Cancelled { .. }
    ));
    peer.cleaned()
}

#[tokio::test]
async fn output_limit_is_incomplete() -> TestResult {
    let mut peer = Peer::new("output-limit")?;
    peer.config.limits.output_bytes = 6;
    assert!(
        matches!(finish(peer.reviewer()?.start(input("visible")).await?).await?, ReviewOutcome::Incomplete { partial_text: Some(partial_text), reason: ReviewFailure::OutputLimit } if partial_text == "prefix")
    );
    peer.cleaned()
}

#[tokio::test]
async fn blocked_observer_does_not_block_cancel() -> TestResult {
    let mut peer = Peer::new("flood")?;
    peer.config.limits.output_bytes = 128 * 1024 * 1024;
    let run = peer.reviewer()?.start(input("visible")).await?;
    let _blocked = run.subscribe();
    peer.event("permission-response").await?;
    let started = Instant::now();
    run.cancel();
    let cancel = peer.event("cancel").await?;
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "cancel dispatch blocked by output/observer: {cancel}"
    );
    assert!(matches!(
        finish(run).await?,
        ReviewOutcome::Cancelled { .. }
    ));
    peer.cleaned()
}

#[tokio::test]
async fn fresh_runs_do_not_share_state_and_continue_same_session() -> TestResult {
    let peer = Peer::new("complete")?;
    let reviewer = peer.reviewer()?;
    let mut a = input("IDENTITY-A");
    a.follow_up_instructions
        .push("Continue this same inspection".into());
    let (a, b) = tokio::join!(reviewer.start(a), reviewer.start(input("IDENTITY-B")));
    let (a, b) = tokio::join!(finish(a?), finish(b?));
    assert_eq!(complete(a?)?, "IDENTITY-Acontinued");
    assert_eq!(complete(b?)?, "IDENTITY-B");
    let spawns = peer.events("spawn")?;
    assert_eq!(spawns.len(), 2);
    assert_ne!(spawns[0]["cwd"], spawns[1]["cwd"]);
    assert_ne!(spawns[0]["session"], spawns[1]["session"]);
    let prompts = peer.events("prompt")?;
    assert_eq!(prompts.len(), 3);
    let continuation = prompts
        .iter()
        .find(|row| row["turn"] == 2)
        .ok_or("continuation absent")?;
    assert!(
        prompts.iter().any(|row| row["turn"] == 1
            && row["params"]["sessionId"] == continuation["params"]["sessionId"])
    );
    peer.cleaned()
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn dropping_owner_terminates_process_tree_before_removing_evidence() -> TestResult {
    let peer = Peer::new("stall")?;
    let run = peer.reviewer()?.start(input("visible")).await?;
    let spawn = peer.event("spawn").await?;
    peer.event("prompt").await?;
    let cwd = std::path::PathBuf::from(spawn["cwd"].as_str().ok_or("cwd")?);
    assert!(cwd.join("evidence/manifest.json").is_file());
    let child = spawn["child"].as_u64().ok_or("child")?;
    let pid = spawn["pid"].as_u64().ok_or("pid")?;
    drop(run);
    timeout(Duration::from_secs(15), async {
        while fs::read_dir(&peer.config.runtime_parent)?
            .next()
            .transpose()?
            .is_some()
        {
            sleep(Duration::from_millis(20)).await;
        }
        Ok::<_, Box<dyn Error>>(())
    })
    .await??;
    for id in [pid, child] {
        // A reparented zombie is already terminated; it cannot read or mutate.
        match fs::read_to_string(format!("/proc/{id}/stat")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(stat) => assert_eq!(stat.split_whitespace().nth(2), Some("Z")),
            Err(error) => return Err(error.into()),
        }
    }
    peer.cleaned()
}

#[test]
fn deployment_rejects_credential_environment_and_invalid_bounds() -> TestResult {
    let mut peer = Peer::new("complete")?;
    peer.config
        .transport_environment
        .insert("GITHUB_TOKEN".into(), "never-forward-this".into());
    assert!(matches!(
        Reviewer::new(peer.config.clone()),
        Err(ReviewError::InvalidInput(_))
    ));
    assert!(!format!("{:?}", peer.config).contains("never-forward-this"));
    peer.config
        .transport_environment
        .remove(&std::ffi::OsString::from("GITHUB_TOKEN"));
    peer.config.limits.documents = 0;
    assert!(matches!(
        Reviewer::new(peer.config),
        Err(ReviewError::InvalidInput(_))
    ));
    Ok(())
}

#[tokio::test]
async fn cancel_before_readiness_never_dispatches_prompt() -> TestResult {
    let peer = Peer::new("missing")?;
    let run = peer.reviewer()?.start(input("visible")).await?;
    peer.event("set_mode").await?;
    run.cancel();
    assert!(matches!(
        finish(run).await?,
        ReviewOutcome::Cancelled { .. }
    ));
    assert!(peer.events("prompt")?.is_empty());
    peer.cleaned()
}

#[tokio::test]
async fn staging_io_failure_never_creates_child() -> TestResult {
    let peer = Peer::new("complete")?;
    let reviewer = peer.reviewer()?;
    fs::remove_dir(&peer.config.runtime_parent)?;
    assert!(matches!(
        reviewer.start(input("visible")).await,
        Err(ReviewError::Io(_))
    ));
    assert!(peer.events("spawn")?.is_empty());
    Ok(())
}

#[tokio::test]
async fn unsupported_executable_version_never_starts_session() -> TestResult {
    let peer = Peer::new("complete")?;
    fs::write(
        peer.root.path().join("peer-config.json"),
        serde_json::to_vec(&json!({"scenario": "complete", "version": "2.21.2"}))?,
    )?;
    assert!(matches!(
        finish(peer.reviewer()?.start(input("visible")).await?).await?,
        ReviewOutcome::Incomplete { .. }
    ));
    assert!(peer.events("session")?.is_empty());
    peer.cleaned()
}

#[tokio::test]
async fn connection_loss_keeps_incomplete_output() -> TestResult {
    let peer = Peer::new("disconnect")?;
    assert!(
        matches!(finish(peer.reviewer()?.start(input("visible")).await?).await?,
        ReviewOutcome::Incomplete { partial_text: Some(partial_text), .. } if partial_text == "partial-before-disconnect")
    );
    peer.cleaned()
}

#[tokio::test]
async fn authoritative_completion_survives_late_disconnect() -> TestResult {
    let peer = Peer::new("complete-disconnect")?;
    assert_eq!(
        complete(finish(peer.reviewer()?.start(input("visible")).await?).await?)?,
        "authoritative-review"
    );
    peer.cleaned()
}

async fn privacy_rejects(scenario: &str) -> TestResult {
    let peer = Peer::new(scenario)?;
    assert!(matches!(
        finish(peer.reviewer()?.start(input("visible")).await?).await?,
        ReviewOutcome::Incomplete {
            reason: ReviewFailure::ConfigurationDrift,
            ..
        }
    ));
    assert!(peer.events("prompt")?.is_empty());
    peer.cleaned()
}

#[tokio::test]
async fn content_collection_must_be_confirmed_disabled() -> TestResult {
    privacy_rejects("collection-enabled").await
}

#[tokio::test]
async fn missing_collection_confirmation_never_prompts() -> TestResult {
    privacy_rejects("collection-missing").await
}

#[tokio::test]
async fn unsolicited_collection_update_cannot_repair_invalid_acknowledgement() -> TestResult {
    privacy_rejects("collection-late").await
}

#[tokio::test]
async fn malformed_configuration_response_is_not_silently_discarded() -> TestResult {
    let peer = Peer::new("collection-malformed")?;
    assert!(matches!(
        finish(peer.reviewer()?.start(input("visible")).await?).await?,
        ReviewOutcome::Incomplete {
            reason: ReviewFailure::CommandFailed {
                operation: Some(ReviewOperation::SetConfiguration),
                ..
            },
            ..
        }
    ));
    assert!(peer.events("prompt")?.is_empty());
    peer.cleaned()
}

#[tokio::test]
async fn rejected_privacy_change_preserves_private_cause_without_sending_evidence() -> TestResult {
    let peer = Peer::new("collection-rejected")?;
    let outcome = finish(peer.reviewer()?.start(input("visible")).await?).await?;
    assert!(!format!("{outcome:?}").contains("PRIVATE-RPC-FAILURE"));
    let ReviewOutcome::Incomplete {
        reason:
            ReviewFailure::CommandFailed {
                operation: Some(ReviewOperation::SetConfiguration),
                diagnostic,
            },
        ..
    } = outcome
    else {
        return Err("missing configuration failure".into());
    };
    assert!(diagnostic.untrusted_text().contains("PRIVATE-RPC-FAILURE"));
    assert!(peer.events("prompt")?.is_empty());
    peer.cleaned()
}

#[tokio::test]
async fn late_collection_confirmation_precedes_first_prompt() -> TestResult {
    let peer = Peer::new("collection-delayed-response")?;
    assert_eq!(
        complete(finish(peer.reviewer()?.start(input("visible")).await?).await?)?,
        "visible"
    );
    let confirmed = peer.events("collection-ack")?.remove(0);
    let prompted = peer.events("prompt")?.remove(0);
    assert!(
        confirmed["time"].as_f64().ok_or("confirmation time")?
            <= prompted["time"].as_f64().ok_or("prompt time")?
    );
    peer.cleaned()
}

#[tokio::test]
async fn content_collection_drift_is_incomplete() -> TestResult {
    let peer = Peer::new("collection-drift")?;
    assert!(matches!(
        finish(peer.reviewer()?.start(input("visible")).await?).await?,
        ReviewOutcome::Incomplete {
            reason: ReviewFailure::ConfigurationDrift,
            ..
        }
    ));
    peer.cleaned()
}

#[tokio::test]
async fn model_from_wrong_mode_cannot_authorize_mode_only_switch() -> TestResult {
    readiness_rejects("pre-switch-model").await
}

#[tokio::test]
async fn post_switch_model_confirmation_allows_inspection() -> TestResult {
    let peer = Peer::new("pre-switch-model-late")?;
    assert_eq!(
        complete(finish(peer.reviewer()?.start(input("visible")).await?).await?)?,
        "visible"
    );
    let valid = peer
        .events("configuration")?
        .into_iter()
        .find(|row| row["mode"] == "cyril-inspection-reviewer")
        .ok_or("post-switch confirmation")?;
    let prompted = peer.events("prompt")?.remove(0);
    assert!(
        valid["time"].as_f64().ok_or("confirmation time")?
            <= prompted["time"].as_f64().ok_or("prompt time")?
    );
    peer.cleaned()
}

#[tokio::test]
async fn failed_tool_is_observed_without_inventing_permission_denial() -> TestResult {
    let peer = Peer::new("tool-failure")?;
    let run = peer.reviewer()?.start(input("visible")).await?;
    let status = run.subscribe();
    assert_eq!(complete(finish(run).await?)?, "observed-tool-failure");
    assert!(status.borrow().tool_failure_observed);
    assert_eq!(status.borrow().denied_permissions, 0);
    peer.cleaned()
}

#[tokio::test]
async fn continuation_after_startup_deadline_still_completes() -> TestResult {
    let peer = Peer::new("slow-follow-up")?;
    let mut review = input("visible");
    review.follow_up_instructions.push("Continue".into());
    assert_eq!(
        complete(finish(peer.reviewer()?.start(review).await?).await?)?,
        "visiblecontinued"
    );
    peer.cleaned()
}

#[cfg(target_os = "linux")]
#[test]
fn runtime_drop_retains_evidence_until_core_completion() -> TestResult {
    let peer = Peer::new("runtime-drop")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let (run, spawn) = runtime.block_on(async {
        let run = peer.reviewer()?.start(input("visible")).await?;
        peer.event("prompt").await?;
        Ok::<_, Box<dyn Error>>((run, peer.event("spawn").await?))
    })?;
    let cwd = std::path::PathBuf::from(spawn["cwd"].as_str().ok_or("cwd")?);
    drop(runtime);
    drop(run);
    let deadline = Instant::now() + Duration::from_secs(15);
    while cwd.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!cwd.exists(), "cleanup waiter did not release evidence");
    for event in ["cleanup-window", "cleanup-window-end"] {
        assert_eq!(
            peer.events(event)?.first().ok_or("missing EOF window")?["evidence_exists"],
            true
        );
    }
    for key in ["pid", "child"] {
        let id = spawn[key].as_u64().ok_or("pid")?;
        match fs::read_to_string(format!("/proc/{id}/stat")) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(stat) => assert_eq!(stat.split_whitespace().nth(2), Some("Z")),
            Err(error) => return Err(error.into()),
        }
    }
    peer.cleaned()
}

#[tokio::test]
async fn runtime_command_diagnostic_is_explicitly_retrievable_but_not_logged() -> TestResult {
    let peer = Peer::new("prompt-error")?;
    let outcome = finish(peer.reviewer()?.start(input("visible")).await?).await?;
    let ReviewOutcome::Incomplete { reason, .. } = outcome else {
        return Err("runtime error completed review".into());
    };
    assert!(!format!("{reason:?} {reason}").contains("PRIVATE-ERROR-SECRET"));
    let ReviewFailure::CommandFailed {
        operation,
        diagnostic,
    } = reason
    else {
        return Err("command failure lost its diagnostic".into());
    };
    assert_eq!(operation, Some(ReviewOperation::Prompt));
    assert!(diagnostic.untrusted_text().contains("PRIVATE-ERROR-SECRET"));
    peer.cleaned()
}
