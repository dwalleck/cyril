use std::error::Error;
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use std::time::Duration;

use constant_time_eq::constant_time_eq;
use process_wrap::tokio::{CommandWrap, KillOnDrop};
use serde::Deserialize;
use serde_json::json;

const MAX_FRAME_LENGTH: usize = 1024 * 1024;
const CREDENTIAL: [u8; 32] = [0x5a; 32];

#[derive(Deserialize)]
struct Request {
    auth: Option<Vec<u8>>,
    version: u32,
    operation: String,
}

fn evaluate_frame(frame: &[u8]) -> &'static str {
    let Some(header) = frame.get(..4) else {
        return "malformed_frame";
    };
    let announced = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
    if announced > MAX_FRAME_LENGTH {
        return "frame_too_large";
    }
    let Some(payload) = frame.get(4..) else {
        return "malformed_frame";
    };
    if payload.len() != announced {
        return "malformed_frame";
    }
    let Ok(request) = serde_json::from_slice::<Request>(payload) else {
        return "malformed_frame";
    };
    let authenticated = request.auth.as_deref().is_some_and(|provided| {
        provided.len() == CREDENTIAL.len() && constant_time_eq(provided, &CREDENTIAL)
    });
    if !authenticated {
        return "unauthorized";
    }
    if request.version != 1 {
        return "unsupported_version";
    }
    match request.operation.as_str() {
        "health" | "shutdown" => "ok",
        _ => "unknown_operation",
    }
}

fn frame(value: serde_json::Value) -> Result<Vec<u8>, serde_json::Error> {
    let payload = serde_json::to_vec(&value)?;
    let mut framed = Vec::with_capacity(payload.len() + 4);
    framed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    framed.extend_from_slice(&payload);
    Ok(framed)
}

fn framing_matrix() -> Result<serde_json::Value, Box<dyn Error>> {
    let missing = frame(json!({"operation": "health", "version": 1}))?;
    let invalid =
        frame(json!({"auth": vec![0x11; 32], "operation": "health", "version": 1}))?;
    let valid = frame(json!({"auth": CREDENTIAL, "operation": "health", "version": 1}))?;
    let unknown = frame(json!({"auth": CREDENTIAL, "operation": "future", "version": 1}))?;
    let unsupported =
        frame(json!({"auth": CREDENTIAL, "operation": "health", "version": 2}))?;
    let malformed = [0_u8, 0, 0, 2, b'{'];
    let oversized = ((MAX_FRAME_LENGTH + 1) as u32).to_be_bytes();
    Ok(json!({
        "invalid_auth": evaluate_frame(&invalid),
        "malformed": evaluate_frame(&malformed),
        "missing_auth": evaluate_frame(&missing),
        "oversized": evaluate_frame(&oversized),
        "unknown_operation": evaluate_frame(&unknown),
        "unsupported_version": evaluate_frame(&unsupported),
        "valid_health": evaluate_frame(&valid),
    }))
}

#[cfg(unix)]
fn run_tree_child(marker: &Path) -> Result<(), Box<dyn Error>> {
    let mut grandchild = std::process::Command::new("sleep").arg("30").spawn()?;
    std::fs::write(marker, grandchild.id().to_string())?;
    let _status = grandchild.wait()?;
    Ok(())
}

#[cfg(windows)]
fn run_tree_child(marker: &Path) -> Result<(), Box<dyn Error>> {
    let mut grandchild = std::process::Command::new("cmd")
        .args(["/C", "ping -n 31 127.0.0.1 >NUL"])
        .spawn()?;
    std::fs::write(marker, grandchild.id().to_string())?;
    let _status = grandchild.wait()?;
    Ok(())
}

async fn wait_for_file(path: &Path) -> Result<(), Box<dyn Error>> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if path.exists() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(format!("timed out waiting for {}", path.display()).into());
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(unix)]
async fn process_tree_probe(marker: &Path) -> Result<serde_json::Value, Box<dyn Error>> {
    use process_wrap::tokio::ProcessGroup;

    let executable = std::env::current_exe()?;
    let mut command = CommandWrap::with_new(executable, |inner| {
        inner.arg("--tree-child").arg(marker);
    });
    command.wrap(ProcessGroup::leader()).wrap(KillOnDrop);
    let mut child = command.spawn()?;
    wait_for_file(marker).await?;
    let grandchild_pid = std::fs::read_to_string(marker)?;
    let kill = Box::into_pin(child.kill());
    tokio::time::timeout(Duration::from_secs(2), kill).await??;

    let process_path = PathBuf::from(format!("/proc/{}", grandchild_pid.trim()));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while process_path.exists() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Ok(json!({
        "grandchild_reaped": !process_path.exists(),
        "kill_completed_within_two_seconds": true,
        "mechanism": "process_group",
    }))
}

#[cfg(windows)]
async fn process_tree_probe(marker: &Path) -> Result<serde_json::Value, Box<dyn Error>> {
    use process_wrap::tokio::JobObject;

    let executable = std::env::current_exe()?;
    let mut command = CommandWrap::with_new(executable, |inner| {
        inner.arg("--tree-child").arg(marker);
    });
    command.wrap(JobObject).wrap(KillOnDrop);
    let mut child = command.spawn()?;
    wait_for_file(marker).await?;
    let kill = Box::into_pin(child.kill());
    tokio::time::timeout(Duration::from_secs(2), kill).await??;
    Ok(json!({
        "kill_completed_within_two_seconds": true,
        "mechanism": "job_object",
    }))
}

#[cfg(windows)]
fn create_user_only_pipe_probe() -> Result<(), Box<dyn Error>> {
    use interprocess::os::windows::named_pipe::{pipe_mode, PipeListenerOptions};
    use interprocess::os::windows::security_descriptor::SecurityDescriptor;
    use widestring::U16CString;
    use win_security_identifier::{GetCurrentSid, SecurityIdentifier};

    let current_user = SecurityIdentifier::get_current_user_sid()?;
    let serialized = U16CString::from_str(format!("D:P(A;;GA;;;{current_user})"))?;
    let descriptor = SecurityDescriptor::deserialize(serialized.as_ucstr())?;
    let _listener = PipeListenerOptions::new()
        .path(Path::new(r"\\.\pipe\cyril-j7um-probe"))
        .accept_remote(false)
        .security_descriptor(Some(descriptor))
        .create_tokio_duplex::<pipe_mode::Bytes>()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os();
    let _program = args.next();
    if args.next().as_deref() == Some(std::ffi::OsStr::new("--tree-child")) {
        let marker = args.next().ok_or("tree child marker missing")?;
        return run_tree_child(Path::new(&marker));
    }

    #[cfg(windows)]
    create_user_only_pipe_probe()?;

    let root = std::env::temp_dir().join(format!("cyril-j7um-platform-{}", std::process::id()));
    std::fs::create_dir_all(&root)?;
    let marker = root.join("grandchild.pid");
    let result = json!({
        "framing": framing_matrix()?,
        "process_tree": process_tree_probe(&marker).await?,
        "windows_pipe_api": "safe SecurityDescriptor + local-only Tokio listener compiled",
    });
    println!("{}", serde_json::to_string_pretty(&result)?);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
