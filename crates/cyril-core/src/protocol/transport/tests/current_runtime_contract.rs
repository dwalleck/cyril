use std::path::Path;
use std::time::Duration;

use tokio::io::AsyncReadExt;

use super::*;

#[cfg(unix)]
#[tokio::test]
async fn agent_process_preserves_wire_bytes_before_sdk2_runtime() {
    const SEGMENTS: &[&str] = &[
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n",
        " { \"jsonrpc\" : \"2.0\", \"method\" : \"session/update\", \"params\" : { \"future\" : true } } \n",
        "{malformed-frame\n",
    ];
    let root = tempfile::tempdir().expect("fixture tempdir");
    let mut args = vec![
        "-c".to_string(),
        "printf '%s' \"$@\"".to_string(),
        "wire-fixture".to_string(),
    ];
    args.extend(SEGMENTS.iter().map(|segment| (*segment).to_owned()));
    let mut process = AgentProcess::spawn(&AgentCommand::new("sh").with_args(args), root.path())
        .await
        .expect("fixture spawn");
    let mut stdout = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(5),
        process.stdout.read_to_end(&mut stdout),
    )
    .await
    .expect("stdout read timeout")
    .expect("stdout read");
    assert_eq!(stdout, SEGMENTS.join("").as_bytes());
}

#[cfg(unix)]
#[tokio::test]
async fn agent_process_uses_requested_working_directory_and_arguments() {
    let root = tempfile::tempdir().expect("fixture tempdir");
    let cwd = root.path().join("cwd with spaces – 道");
    std::fs::create_dir(&cwd).expect("create cwd");
    let command = AgentCommand::new("sh").with_args(vec![
        "-c".to_string(),
        "printf '%s\\n%s\\n' \"$PWD\" \"$1\"".to_string(),
        "argv-fixture".to_string(),
        "argument with spaces – α".to_string(),
    ]);
    let mut process = AgentProcess::spawn(&command, Path::new(&cwd))
        .await
        .expect("fixture spawn");
    let mut stdout = Vec::new();
    process
        .stdout
        .read_to_end(&mut stdout)
        .await
        .expect("stdout read");
    let expected_cwd = cwd.canonicalize().expect("canonical cwd");
    let expected = format!("{}\nargument with spaces – α\n", expected_cwd.display());
    assert_eq!(String::from_utf8(stdout).expect("utf8 output"), expected);
    process._child.wait().await.expect("child wait");
}
