use std::cell::RefCell;
use std::path::Path;
use std::pin::Pin;
use std::process::ExitStatus;
use std::rc::Rc;
use std::task::Poll;
use std::time::Duration;

use agent_client_protocol as acp;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, ReadBuf};

use super::*;

/// Design C2's private test adapter — no production seam: records exactly the
/// bytes `poll_read` hands onward, so the oracle observes the byte stream the
/// wrapped ACP connection actually ingests, at ingress granularity.
struct RecordingReader<R> {
    inner: R,
    recorded: Rc<RefCell<Vec<u8>>>,
}

impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for RecordingReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let this = &mut *self;
        match Pin::new(&mut this.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                this.recorded
                    .borrow_mut()
                    .extend_from_slice(&buf.filled()[before..]);
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

/// C2 freezes the reader's ingress bytes, not client behavior — every frame
/// still flows through the REAL `ClientSideConnection` parse loop, this
/// client just discards what survives parsing.
struct DiscardingClient;

#[async_trait::async_trait(?Send)]
impl acp::Client for DiscardingClient {
    async fn request_permission(
        &self,
        _args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        Err(acp::Error::new(
            -32603,
            "C2 byte oracle has no permission broker",
        ))
    }

    async fn session_notification(&self, _args: acp::SessionNotification) -> acp::Result<()> {
        Ok(())
    }
}

/// The fixture and oracle intentionally carry separate copies of the wire
/// segments. The child is the source of observed bytes; the expected bytes are
/// assembled independently so changing the fixture cannot change the oracle.
#[cfg(unix)]
const FIXTURE_SEGMENTS: &[&str] = &[
    "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n",
    "[{\"jsonrpc\":\"2.0\",\"id\":42,\"result\":{\"ok\":true}},{\"jsonrpc\":\"2.0\",\"method\":\"session/update\",\"params\":{\"sessionId\":\"s1\",\"update\":{\"sessionUpdate\":\"future_variant\",\"payload\":{\"ok\":true}}}},{\"jsonrpc\":\"2.0\",\"id\":\"batch-id\",\"error\":{\"code\":-32000,\"message\":\"bad\"}},{\"jsonrpc\":\"2.0\",\"id\":\"batch-id\",\"error\":{\"code\":-32000,\"message\":\"bad\"}}]\n",
    "{malformed-frame\n",
    "{\"jsonrpc\":\"2.0\",\"method\":\"future/method\",\"params\":{\"unknown\":true}}\n",
    " { \"jsonrpc\" : \"2.0\", \"method\" : \"_kiro.dev/metadata\", \"params\" : { \"extreme\" : 1e400, \"label\" : \"probe\" } } \n",
];

#[cfg(unix)]
const EXPECTED_SEGMENTS: &[&[u8]] = &[
    br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
"#,
    br#"[{"jsonrpc":"2.0","id":42,"result":{"ok":true}},{"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"s1","update":{"sessionUpdate":"future_variant","payload":{"ok":true}}}},{"jsonrpc":"2.0","id":"batch-id","error":{"code":-32000,"message":"bad"}},{"jsonrpc":"2.0","id":"batch-id","error":{"code":-32000,"message":"bad"}}]
"#,
    br#"{malformed-frame
"#,
    br#"{"jsonrpc":"2.0","method":"future/method","params":{"unknown":true}}
"#,
    br#" { "jsonrpc" : "2.0", "method" : "_kiro.dev/metadata", "params" : { "extreme" : 1e400, "label" : "probe" } } 
"#,
];

fn expected_preparse_bytes() -> Vec<u8> {
    EXPECTED_SEGMENTS
        .iter()
        .flat_map(|segment| segment.iter().copied())
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_exact_c2_bytes(actual: &[u8], expected: &[u8]) {
    let first_difference = (0..actual.len().max(expected.len()))
        .find(|index| actual.get(*index) != expected.get(*index));
    if let Some(index) = first_difference {
        panic!(
            "C2 exact preparse bytes differ at byte {index}: expected {:?}, actual {:?}",
            expected.get(index),
            actual.get(index)
        );
    }

    let expected_digest = sha256_hex(expected);
    let actual_digest = sha256_hex(actual);
    assert_eq!(
        actual_digest, expected_digest,
        "C2 exact preparse byte digest differs"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn c2_exact_preparse_bytes() {
    let fixture_dir = tempfile::tempdir().expect("C2 fixture tempdir");
    let mut args = vec![
        "-c".to_string(),
        // Emit the segments, close stdout (a clean EOF for the ACP reader),
        // then park consuming stdin: the connection's replies to the fixture's
        // request/malformed frames must never hit EPIPE and tear the io task
        // down while it still owns unread bytes.
        "printf '%s' \"$@\"; exec >&-; cat >/dev/null".to_string(),
        "c2-raw-segments".to_string(),
    ];
    args.extend(
        FIXTURE_SEGMENTS
            .iter()
            .map(|segment| (*segment).to_string()),
    );

    let command = AgentCommand::new("sh").with_args(args);
    let process = AgentProcess::spawn(&command, fixture_dir.path())
        .await
        .expect("C2 fixture spawn");

    // The recording sits BEFORE parsing: no serde round-trip may normalize
    // whitespace, malformed evidence, IDs, or the 1e400 source lexeme before
    // the contract comparison — but the bytes must still be pulled by the
    // current ACP reader, so a regression in the real read path cannot pass.
    let recorded = Rc::new(RefCell::new(Vec::new()));
    let reader = RecordingReader {
        inner: process.stdout,
        recorded: Rc::clone(&recorded),
    };
    let stdin = process.stdin;

    tokio::task::LocalSet::new()
        .run_until(async move {
            use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};
            // Mirror run_bridge's construction exactly: same connection type,
            // same compat adapters, same spawner shape — the recorded bytes
            // are what the CURRENT ACP parse loop ingests.
            let (_conn, io_task) = acp::ClientSideConnection::new(
                DiscardingClient,
                stdin.compat_write(),
                reader.compat(),
                |fut| {
                    tokio::task::spawn_local(fut);
                },
            );
            let _ = tokio::time::timeout(Duration::from_secs(5), io_task).await;
        })
        .await;

    let expected = expected_preparse_bytes();
    assert_exact_c2_bytes(&recorded.borrow(), &expected);
}

async fn run_c13_case(name: &str, command: AgentCommand, cwd: &Path) -> (Vec<u8>, ExitStatus) {
    let mut process = AgentProcess::spawn(&command, cwd)
        .await
        .unwrap_or_else(|error| panic!("C13 {name}: fixture spawn failed: {error}"));

    let mut stdout = Vec::new();
    tokio::time::timeout(
        Duration::from_secs(5),
        process.stdout.read_to_end(&mut stdout),
    )
    .await
    .unwrap_or_else(|_| panic!("C13 {name}: stdout read timed out"))
    .unwrap_or_else(|error| panic!("C13 {name}: stdout read failed: {error}"));

    let status = tokio::time::timeout(Duration::from_secs(5), process._child.wait())
        .await
        .unwrap_or_else(|_| panic!("C13 {name}: child wait timed out"))
        .unwrap_or_else(|error| panic!("C13 {name}: child wait failed: {error}"));

    (stdout, status)
}

async fn assert_c13_case(
    name: &str,
    command: AgentCommand,
    cwd: &Path,
    expected_stdout: &[u8],
    expected_code: i32,
) {
    let (stdout, status) = run_c13_case(name, command, cwd).await;
    assert_eq!(stdout, expected_stdout, "C13 {name}: stdout bytes differ");
    assert_eq!(
        status.code(),
        Some(expected_code),
        "C13 {name}: exit status differs"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn c13_process_contract_matrix() {
    const ARG_ONE: &str = "argument with spaces – α";
    const ARG_TWO: &str = "second/argument 東京";

    let root = tempfile::tempdir().expect("C13 fixture tempdir");
    let cwd = root.path().join("cwd with spaces – 道");
    std::fs::create_dir(&cwd).expect("C13 create Unicode-space cwd");
    let expected_cwd = cwd.canonicalize().expect("C13 canonicalize cwd");
    let expected_cwd = format!("{}\n", expected_cwd.to_string_lossy()).into_bytes();

    let argv_command = AgentCommand::new("sh").with_args(vec![
        "-c".to_string(),
        "printf '%s\\n%s\\n' \"$1\" \"$2\"".to_string(),
        "c13-argv".to_string(),
        ARG_ONE.to_string(),
        ARG_TWO.to_string(),
    ]);
    let expected_argv = format!("{ARG_ONE}\n{ARG_TWO}\n").into_bytes();
    assert_c13_case("argv_unicode_spaces", argv_command, &cwd, &expected_argv, 0).await;

    // AgentProcess::spawn has NO environment plumbing — the frozen contract
    // is pure inheritance of cyril's own environment, probed with a variable
    // guaranteed present (PATH: `sh` was found through it) and one guaranteed
    // absent. A spawn that gains env_clear() or injection turns a probe red.
    let parent_path = std::env::var("PATH").expect("C13 test process has PATH");
    let env_command = AgentCommand::new("sh").with_args(vec![
        "-c".to_string(),
        "printf '%s\\n%s\\n' \"$PATH\" \"${C13_CONTRACT_ABSENT-absent}\"".to_string(),
    ]);
    let expected_env = format!("{parent_path}\nabsent\n").into_bytes();
    assert_c13_case("env_inherited", env_command, &cwd, &expected_env, 0).await;

    let cwd_command =
        AgentCommand::new("sh").with_args(vec!["-c".to_string(), "pwd -P".to_string()]);
    assert_c13_case("cwd_unicode_spaces", cwd_command, &cwd, &expected_cwd, 0).await;

    let clean_command = AgentCommand::new("sh")
        .with_args(vec!["-c".to_string(), "printf '%s\\n' clean".to_string()]);
    assert_c13_case("clean_exit", clean_command, &cwd, b"clean\n", 0).await;

    let nonzero_command = AgentCommand::new("sh").with_args(vec![
        "-c".to_string(),
        "printf '%s\\n' nonzero; exit 23".to_string(),
    ]);
    assert_c13_case("nonzero_exit", nonzero_command, &cwd, b"nonzero\n", 23).await;

    let eof_command = AgentCommand::new("sh").with_args(vec!["-c".to_string(), ":".to_string()]);
    assert_c13_case("eof", eof_command, &cwd, &[], 0).await;
}
