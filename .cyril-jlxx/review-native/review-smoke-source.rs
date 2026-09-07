//! Throwaway acceptance driver. Remove after native evidence is captured.
use cyril_workbench::reviewer::{
    EvidenceDocument, ReviewInput, ReviewLimits, ReviewOutcome, Reviewer, ReviewerConfig,
};
use std::{collections::BTreeMap, error::Error, path::PathBuf};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cyril_workbench=debug".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let mut args = std::env::args_os().skip(1);
    let executable = PathBuf::from(args.next().ok_or("ABS_EXEC required")?);
    let runtime_parent = PathBuf::from(args.next().ok_or("RUNTIME_PARENT required")?);
    let auth_data_home = PathBuf::from(args.next().ok_or("AUTH_DATA required")?);
    let prompt_path = PathBuf::from(args.next().ok_or("PROMPT_FILE required")?);
    let evidence_path = PathBuf::from(args.next().ok_or("EVIDENCE_FILE required")?);
    let follow_up_instructions = args
        .map(|path| std::fs::read_to_string(PathBuf::from(path)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut transport_environment = BTreeMap::new();
    for name in [
        "PATH",
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "https_proxy",
        "http_proxy",
        "all_proxy",
        "no_proxy",
        "SSL_CERT_FILE",
        "SSL_CERT_DIR",
        "NODE_EXTRA_CA_CERTS",
        "SystemRoot",
        "WINDIR",
        "PATHEXT",
    ] {
        if let Some(value) = std::env::var_os(name) {
            transport_environment.insert(name.into(), value);
        }
    }
    let reviewer = Reviewer::new(ReviewerConfig {
        executable,
        runtime_parent,
        auth_data_home,
        transport_environment,
        limits: ReviewLimits::default(),
    })?;
    let run = reviewer
        .start(ReviewInput {
            instruction: std::fs::read_to_string(prompt_path)?,
            documents: vec![EvidenceDocument {
                source_label: evidence_path.to_string_lossy().into_owned(),
                text: std::fs::read_to_string(&evidence_path)?,
            }],
            follow_up_instructions,
        })
        .await?;
    let mut status = run.subscribe();
    let projection = tokio::spawn(async move {
        loop {
            println!("STATUS {:?}", *status.borrow_and_update());
            if status.changed().await.is_err() {
                break;
            }
        }
    });
    let outcome = run.finish().await;
    projection.await?;
    match outcome {
        ReviewOutcome::Completed { text } => {
            println!("COMPLETED_AFTER_TEARDOWN\n{text}");
            Ok(())
        }
        ReviewOutcome::Cancelled { partial_text } => {
            println!("CANCELLED_AFTER_TEARDOWN\n{partial_text}");
            Err("review cancelled".into())
        }
        ReviewOutcome::Incomplete {
            partial_text,
            reason,
        } => {
            println!("INCOMPLETE_AFTER_TEARDOWN {reason:?}");
            match partial_text {
                Some(text) => println!("{text}"),
                None => println!("OUTPUT_UNAVAILABLE"),
            }
            Err(reason.into())
        }
    }
}
