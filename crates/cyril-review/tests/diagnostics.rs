use cyril_review::{
    Cancellation, DiagnosticsError, DiagnosticsOptions, DiagnosticsOutcome, DiagnosticsResult,
    ReviewClock, ReviewError, ReviewRun, diagnostics,
};
use serde_json::{Value, json};
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

type ArtifactSnapshot = Vec<(PathBuf, Vec<u8>)>;

const HEAD: &str = "abcdef0123456789abcdef0123456789abcdef0123";
const ELAPSED: Duration = Duration::from_millis(7_750);

struct FixedClock;

impl ReviewClock for FixedClock {
    fn gathered_at_utc(&self) -> cyril_review::Result<String> {
        Ok("2026-09-30T12:34:56+00:00".to_owned())
    }

    fn diagnostics_elapsed(&self, _started: Instant) -> Duration {
        ELAPSED
    }
}

struct Fixture {
    tree: tempfile::TempDir,
    run: ReviewRun,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let tree = tempfile::tempdir()?;
        let workspace = tree.path().join("workspace with spaces");
        let directory = tree.path().join("run");
        fs::create_dir_all(&workspace)?;
        fs::create_dir_all(directory.join("facts"))?;
        let run = ReviewRun::new(workspace, directory)?;
        let metadata = json!({"status": "pending", "records": ["café", "東京", "\u{20000}"]});
        let manifest = json!({
            "requested_target": "HEAD",
            "target": "HEAD",
            "scope": ["src"],
            "head": HEAD,
            "worktree_matches_diff_head": true,
            "gathered_at": "2026-09-30T12:34:56+00:00",
            "crtool_version": env!("CARGO_PKG_VERSION"),
            "total_files": 1,
            "total_patch_bytes": 0,
            "warnings": [],
            "files": [{
                "index": 1, "path": "src/lib.rs", "status": "M",
                "insertions": 1, "deletions": 0, "binary": false,
                "patch": "patches/001.patch", "patch_bytes": 0,
                "later_step": metadata
            }],
            "change_docs": [{"path": "NOTES.md", "bytes": 17}],
            "facts": {
                "symbols": 1, "usages_pages": ["facts/usages-1.txt"],
                "later_step": metadata
            },
            "later_step": metadata
        });
        fs::write(
            run.directory().join("manifest.json"),
            serde_json::to_vec(&manifest)?,
        )?;
        fs::write(
            run.directory().join("facts/symbols.json"),
            b"symbols sentinel",
        )?;
        fs::write(
            run.directory().join("facts/usages-1.txt"),
            b"usage sentinel",
        )?;
        Ok(Self { tree, run })
    }

    fn manifest(&self) -> Result<Value, Box<dyn Error>> {
        Ok(serde_json::from_slice(&fs::read(
            self.run.directory().join("manifest.json"),
        )?)?)
    }

    fn write_manifest(&self, manifest: &Value) -> Result<(), Box<dyn Error>> {
        fs::write(
            self.run.directory().join("manifest.json"),
            serde_json::to_vec(manifest)?,
        )?;
        Ok(())
    }

    fn script(
        &self,
        stdout: &[u8],
        stderr: &[u8],
        exit_code: i32,
        wait_for_release: bool,
    ) -> Result<String, Box<dyn Error>> {
        fs::write(self.tree.path().join("stdout.bin"), stdout)?;
        fs::write(self.tree.path().join("stderr.bin"), stderr)?;
        native_script(self.tree.path(), exit_code, wait_for_release)
    }

    fn report(&self) -> Result<String, Box<dyn Error>> {
        Ok(fs::read_to_string(
            self.run.directory().join("facts/diagnostics.txt"),
        )?)
    }

    fn assert_raw(&self, stdout: &[u8], stderr: &[u8]) -> Result<(), Box<dyn Error>> {
        let raw = fs::read(self.run.directory().join("facts/diagnostics-raw.txt"))?;
        assert!(
            raw.starts_with(stdout),
            "stdout bytes were changed or reordered"
        );
        assert!(
            raw.ends_with(stderr),
            "stderr bytes were changed or reordered"
        );
        assert!(raw.len() >= stdout.len() + stderr.len());
        let separator = &raw[stdout.len()..raw.len() - stderr.len()];
        assert!(
            matches!(separator, b"\n" | b"\r\n"),
            "unexpected separator: {separator:?}"
        );
        Ok(())
    }

    fn snapshot(&self) -> Result<ArtifactSnapshot, Box<dyn Error>> {
        let mut paths = vec!["manifest.json", "facts/symbols.json", "facts/usages-1.txt"];
        paths.extend(
            ["facts/diagnostics.txt", "facts/diagnostics-raw.txt"]
                .into_iter()
                .filter(|relative| self.run.directory().join(relative).exists()),
        );
        paths
            .into_iter()
            .map(|relative| {
                let path = self.run.directory().join(relative);
                let bytes = fs::read(&path)?;
                Ok((path, bytes))
            })
            .collect()
    }

    fn assert_no_reports(&self, snapshot: &[(PathBuf, Vec<u8>)]) -> Result<(), Box<dyn Error>> {
        for (path, bytes) in snapshot {
            assert_eq!(
                &fs::read(path)?,
                bytes,
                "prelaunch refusal replaced {path:?}"
            );
        }
        for relative in ["facts/diagnostics.txt", "facts/diagnostics-raw.txt"] {
            let path = self.run.directory().join(relative);
            if !snapshot.iter().any(|(artifact, _)| artifact == &path) {
                assert!(!path.exists(), "created {relative}");
            }
        }
        assert!(
            !self.tree.path().join("ready").exists(),
            "refused command launched"
        );
        Ok(())
    }
}

#[cfg(unix)]
fn shell_literal(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

#[cfg(unix)]
fn native_script(
    root: &Path,
    exit_code: i32,
    wait_for_release: bool,
) -> Result<String, Box<dyn Error>> {
    let script_path = root.join("fixture.sh");
    let wait = if wait_for_release {
        format!(
            "i=0\nwhile [ ! -e {} ] && [ \"$i\" -lt 200 ]; do\n  /bin/sleep 0.05\n  i=$((i + 1))\ndone\n",
            shell_literal(&root.join("release"))
        )
    } else {
        String::new()
    };
    fs::write(
        &script_path,
        format!(
            "#!/bin/sh\nset -eu\n/bin/cat {}\n/bin/cat {} >&2\n: > {}\n{wait}: > {}\nexit {exit_code}\n",
            shell_literal(&root.join("stdout.bin")),
            shell_literal(&root.join("stderr.bin")),
            shell_literal(&root.join("ready")),
            shell_literal(&root.join("done")),
        ),
    )?;
    Ok(format!("/bin/sh {}", shell_literal(&script_path)))
}

#[cfg(windows)]
fn powershell_literal(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

#[cfg(windows)]
fn native_script(
    root: &Path,
    exit_code: i32,
    wait_for_release: bool,
) -> Result<String, Box<dyn Error>> {
    let system_root = std::env::var_os("SystemRoot")
        .ok_or_else(|| io::Error::other("Windows fixture requires native SystemRoot"))?;
    let powershell =
        PathBuf::from(system_root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let script_path = root.join("fixture.ps1");
    let wait = if wait_for_release {
        format!(
            "$deadline = [DateTime]::UtcNow.AddSeconds(10)\nwhile (!(Test-Path -LiteralPath {}) -and [DateTime]::UtcNow -lt $deadline) {{ Start-Sleep -Milliseconds 50 }}\n",
            powershell_literal(&root.join("release"))
        )
    } else {
        String::new()
    };
    // Windows PowerShell 5.1 needs a BOM to read non-ASCII script paths losslessly.
    // Direct byte writes avoid PowerShell's text/output encoding pipeline.
    fs::write(
        &script_path,
        format!(
            "\u{feff}$ErrorActionPreference = 'Stop'\n$out = [Console]::OpenStandardOutput()\n$err = [Console]::OpenStandardError()\n$bytes = [IO.File]::ReadAllBytes({})\n$out.Write($bytes, 0, $bytes.Length)\n$out.Flush()\n$bytes = [IO.File]::ReadAllBytes({})\n$err.Write($bytes, 0, $bytes.Length)\n$err.Flush()\n[IO.File]::WriteAllText({}, 'ready')\n{wait}[IO.File]::WriteAllText({}, 'done')\nexit {exit_code}\n",
            powershell_literal(&root.join("stdout.bin")),
            powershell_literal(&root.join("stderr.bin")),
            powershell_literal(&root.join("ready")),
            powershell_literal(&root.join("done")),
        ),
    )?;
    Ok(format!(
        "\"{}\" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{}\"",
        powershell.display(),
        script_path.display()
    ))
}

fn run_diagnostics(
    fixture: &Fixture,
    command: &str,
    timeout: Duration,
    cancel: &Cancellation,
) -> cyril_review::Result<DiagnosticsResult> {
    diagnostics(
        &fixture.run,
        command,
        &DiagnosticsOptions { timeout },
        cancel,
        &FixedClock,
    )
}

fn assert_outcome_text(text: &str, outcome: DiagnosticsOutcome) -> Result<(), Box<dyn Error>> {
    let status = text.to_ascii_uppercase();
    match outcome {
        DiagnosticsOutcome::Clean => assert!(status.contains("CLEAN"), "{status}"),
        DiagnosticsOutcome::Failed { exit_code } => {
            assert!(status.contains("FAILED"), "{status}");
            let code = regex::Regex::new(&format!(r"\b{exit_code}\b"))?;
            assert!(code.is_match(&status), "missing actual exit code: {status}");
        }
        DiagnosticsOutcome::TimedOut => assert!(status.contains("TIMED OUT"), "{status}"),
        DiagnosticsOutcome::Cancelled => assert!(status.contains("CANCELLED"), "{status}"),
    }
    Ok(())
}

fn assert_status(manifest: &Value, outcome: DiagnosticsOutcome) -> Result<(), Box<dyn Error>> {
    let status = manifest["facts"]["diagnostics_status"]
        .as_str()
        .ok_or_else(|| io::Error::other("diagnostics status must be a string"))?;
    assert_outcome_text(status, outcome)?;
    assert_eq!(manifest["facts"]["diagnostics"], "facts/diagnostics.txt");
    Ok(())
}

#[test]
fn clean_and_nonzero_preserve_lossless_streams_context_and_metadata() -> Result<(), Box<dyn Error>>
{
    let seconds = regex::Regex::new(r"\b8(?:\.0+)?\s*s(?:ec(?:ond)?s?)?\b")?;
    let stdout = b"src\\lib.rs:4: stdout evidence\r\nraw invalid: \xff\0stdout end";
    let stderr = b"src/lib.rs:9: stderr evidence\nraw invalid: \xfe\0stderr end";
    for (exit_code, outcome) in [
        (0, DiagnosticsOutcome::Clean),
        (23, DiagnosticsOutcome::Failed { exit_code: 23 }),
    ] {
        let fixture = Fixture::new()?;
        let before = fixture.manifest()?;
        let command = fixture.script(stdout, stderr, exit_code, false)?;
        let result = run_diagnostics(
            &fixture,
            &command,
            Duration::from_secs(20),
            &Cancellation::default(),
        )?;
        assert!(result.started());
        assert_eq!(result.outcome(), outcome);
        fixture.assert_raw(stdout, stderr)?;
        let report = fixture.report()?;
        assert_outcome_text(&report, outcome)?;
        assert!(report.contains("src\\lib.rs:4: stdout evidence"));
        assert!(report.contains("src/lib.rs:9: stderr evidence"));
        assert!(
            report.contains('\u{fffd}'),
            "invalid UTF-8 must render with replacement"
        );
        assert!(
            report.contains(&command),
            "configured command context is missing"
        );
        assert!(
            report.contains(&HEAD[..12]),
            "gathered HEAD context is missing"
        );
        assert!(
            seconds.is_match(&report),
            "7.75 elapsed seconds must round to 8: {report}"
        );
        let operation_output = std::str::from_utf8(result.output().stdout())?;
        assert_outcome_text(operation_output, outcome)?;
        assert!(
            seconds.is_match(operation_output),
            "elapsed sample lost from operation output"
        );
        let mut after = fixture.manifest()?;
        assert_status(&after, outcome)?;
        let facts = after["facts"].as_object_mut().expect("facts is an object");
        drop(facts.remove("diagnostics"));
        drop(facts.remove("diagnostics_status"));
        assert_eq!(
            after, before,
            "diagnostics replaced unrelated gathered/facts metadata"
        );
        assert_eq!(
            fs::read(fixture.run.directory().join("facts/symbols.json"))?,
            b"symbols sentinel"
        );
        assert_eq!(
            fs::read(fixture.run.directory().join("facts/usages-1.txt"))?,
            b"usage sentinel"
        );
    }
    Ok(())
}

#[test]
fn empty_streams_are_clean_evidence_not_a_missing_report() -> Result<(), Box<dyn Error>> {
    for facts_metadata in [None, Some(Value::Null), Some(json!({}))] {
        let fixture = Fixture::new()?;
        let mut manifest = fixture.manifest()?;
        if let Some(value) = facts_metadata {
            manifest["facts"] = value;
        } else {
            drop(manifest.as_object_mut().expect("object").remove("facts"));
        }
        fixture.write_manifest(&manifest)?;
        let command = fixture.script(b"", b"", 0, false)?;
        let result = run_diagnostics(
            &fixture,
            &command,
            Duration::from_secs(20),
            &Cancellation::default(),
        )?;
        assert_eq!(result.outcome(), DiagnosticsOutcome::Clean);
        assert!(result.started());
        fixture.assert_raw(b"", b"")?;
        let rewritten = fixture.manifest()?;
        assert_status(&rewritten, DiagnosticsOutcome::Clean)?;
        assert_eq!(rewritten["later_step"], manifest["later_step"]);
        assert_eq!(rewritten["files"], manifest["files"]);
        assert_eq!(
            fs::read(fixture.run.directory().join("facts/symbols.json"))?,
            b"symbols sentinel"
        );
        assert_eq!(
            fs::read(fixture.run.directory().join("facts/usages-1.txt"))?,
            b"usage sentinel"
        );
        assert!(fixture.report()?.contains(&HEAD[..12]));
    }
    Ok(())
}

#[test]
fn changed_path_matches_keep_first_200_and_tail_keeps_last_15_nonempty_lines()
-> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let matches: Vec<String> = (1..=201)
        .map(|index| format!("src\\lib.rs:42: match-{index:04}"))
        .collect();
    let tails: Vec<String> = (1..=16)
        .map(|index| format!("tail-only-{index:04}"))
        .collect();
    let mut stdout = matches.join("\r\n");
    stdout.push_str("\n\n  \n");
    // Independently chosen line separators check meaningful nonempty lines,
    // not only Rust's LF/CRLF lines() convention or native wrapper spelling.
    let separators = [
        "\r", "\n", "\r\n", "\u{85}", "\u{2028}", "\u{2029}", "\x0b", "\x0c",
    ];
    for (index, line) in tails.iter().enumerate() {
        stdout.push_str(line);
        stdout.push_str(separators[index % separators.len()]);
        stdout.push_str(" \n");
    }
    let command = fixture.script(stdout.as_bytes(), b"", 0, false)?;
    let result = run_diagnostics(
        &fixture,
        &command,
        Duration::from_secs(20),
        &Cancellation::default(),
    )?;
    assert_eq!(result.outcome(), DiagnosticsOutcome::Clean);
    fixture.assert_raw(stdout.as_bytes(), b"")?;
    let report = fixture.report()?;
    let rendered: Vec<&str> = report
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .collect();
    for line in &matches[..200] {
        assert!(
            rendered.contains(&line.as_str()),
            "missing changed-path match: {line}"
        );
    }
    assert!(
        !report.contains(&matches[200]),
        "201st match exceeded the report cap"
    );
    assert!(
        !report.contains(&tails[0]),
        "16th-from-last line exceeded the tail cap"
    );
    for line in &tails[1..] {
        assert!(
            rendered.contains(&line.as_str()),
            "missing nonempty tail line: {line}"
        );
    }
    let count = regex::Regex::new(r"\b201\b")?;
    assert!(
        count.is_match(&report),
        "total matches must count the omitted match, not just retained lines"
    );
    Ok(())
}

#[test]
fn invalid_stamps_manifest_and_facts_refuse_before_launch_or_reports() -> Result<(), Box<dyn Error>>
{
    for case in 0..8 {
        let fixture = Fixture::new()?;
        let command = fixture.script(b"should never launch", b"", 0, false)?;
        let mut manifest = fixture.manifest()?;
        match case {
            0 => {
                drop(
                    manifest
                        .as_object_mut()
                        .expect("object")
                        .remove("crtool_version"),
                );
            }
            1 => manifest["crtool_version"] = json!(""),
            2 => manifest["crtool_version"] = json!("stale-version"),
            3 => manifest["crtool_version"] = json!(17),
            4 => manifest["files"][0]["path"] = Value::Null,
            5 => manifest["facts"] = json!(["not an object"]),
            6 => {
                drop(manifest.as_object_mut().expect("object").remove("head"));
            }
            7 => manifest["facts"] = json!(17),
            _ => unreachable!("bounded case matrix"),
        }
        fixture.write_manifest(&manifest)?;
        let snapshot = fixture.snapshot()?;
        let error = run_diagnostics(
            &fixture,
            &command,
            Duration::from_secs(20),
            &Cancellation::default(),
        )
        .expect_err("invalid gathered input must refuse");
        match case {
            0 | 1 => assert!(matches!(error, ReviewError::MissingStamp)),
            2 | 3 => assert!(matches!(error, ReviewError::StampMismatch { .. })),
            4..=7 => assert!(matches!(error, ReviewError::InvalidManifest { .. })),
            _ => unreachable!("bounded case matrix"),
        }
        fixture.assert_no_reports(&snapshot)?;
    }
    Ok(())
}

#[test]
fn cannot_start_and_empty_command_are_errors_without_reports() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    #[cfg(unix)]
    let absent = shell_literal(&fixture.tree.path().join("absent-executable"));
    #[cfg(windows)]
    let absent = format!(
        "\"{}\"",
        fixture.tree.path().join("absent-executable.exe").display()
    );
    let cases = [
        ("", true),
        ("   \t", true),
        ("   \t\n", cfg!(not(windows))),
        (absent.as_str(), false),
    ];
    for (command, expected_invalid_command) in cases {
        let snapshot = fixture.snapshot()?;
        let error = run_diagnostics(
            &fixture,
            command,
            Duration::from_secs(20),
            &Cancellation::default(),
        )
        .expect_err("invalid or missing executable must be an error");
        if expected_invalid_command {
            assert!(
                matches!(
                    &error,
                    ReviewError::Diagnostics(DiagnosticsError::InvalidCommand { .. })
                ),
                "command {command:?} expected InvalidCommand, got {error:?}"
            );
        } else {
            assert!(
                matches!(
                    &error,
                    ReviewError::Diagnostics(DiagnosticsError::CannotStart { .. })
                ),
                "command {command:?} expected CannotStart, got {error:?}"
            );
        }
        fixture.assert_no_reports(&snapshot)?;
    }
    Ok(())
}

#[test]
fn missing_and_corrupt_manifest_refuse_without_diagnostic_artifacts() -> Result<(), Box<dyn Error>>
{
    for case in 0..3 {
        let fixture = Fixture::new()?;
        let command = fixture.script(b"must not launch", b"", 0, false)?;
        let path = fixture.run.directory().join("manifest.json");
        let mut snapshot = fixture.snapshot()?;
        if case == 0 {
            fs::remove_file(&path)?;
            snapshot.retain(|(artifact, _)| artifact != &path);
        } else {
            if case == 1 {
                fs::write(&path, b"{ invalid JSON")?;
            } else {
                let mut manifest = fixture.manifest()?;
                manifest["later_step"]["unknown"] = json!("invalid-utf8-marker");
                let mut bytes = serde_json::to_vec(&manifest)?;
                let offset = bytes
                    .windows(b"invalid-utf8-marker".len())
                    .position(|window| window == b"invalid-utf8-marker")
                    .expect("unknown string is encoded literally");
                bytes[offset] = 0xff;
                fs::write(&path, bytes)?;
                fs::write(
                    fixture.run.directory().join("facts/diagnostics.txt"),
                    b"prior diagnostics report, not child output",
                )?;
                fs::write(
                    fixture.run.directory().join("facts/diagnostics-raw.txt"),
                    b"prior raw diagnostics, not child output",
                )?;
            }
            snapshot = fixture.snapshot()?;
        }
        let error = run_diagnostics(
            &fixture,
            &command,
            Duration::from_secs(20),
            &Cancellation::default(),
        )
        .expect_err("missing or corrupt manifest must refuse");
        if case == 0 {
            assert!(
                matches!(error, ReviewError::Io { source, .. } if source.kind() == io::ErrorKind::NotFound)
            );
            assert!(!path.exists(), "missing manifest was synthesized");
        } else {
            assert!(matches!(error, ReviewError::Json { .. }));
        }
        fixture.assert_no_reports(&snapshot)?;
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn malformed_posix_quotes_refuse_before_launch_or_reports() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let valid_command = fixture.script(b"should never launch", b"", 0, false)?;
    for malformed in [
        format!("{valid_command} 'unterminated"),
        format!("{valid_command} dangling\\"),
    ] {
        let snapshot = fixture.snapshot()?;
        let error = run_diagnostics(
            &fixture,
            &malformed,
            Duration::from_secs(20),
            &Cancellation::default(),
        )
        .expect_err("malformed shlex input must refuse");
        assert!(matches!(
            error,
            ReviewError::Diagnostics(DiagnosticsError::InvalidCommand { .. })
        ));
        fixture.assert_no_reports(&snapshot)?;
    }
    Ok(())
}

#[test]
fn precancelled_signal_never_launches_and_returns_no_evidence() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let command = fixture.script(b"must not launch", b"", 0, false)?;
    let snapshot = fixture.snapshot()?;
    let cancellation = Cancellation::default();
    let caller_signal = cancellation.clone();
    assert!(!caller_signal.is_cancelled());
    caller_signal.cancel();
    assert!(cancellation.is_cancelled());
    let result = run_diagnostics(&fixture, &command, Duration::from_secs(20), &cancellation)?;
    assert_eq!(result.outcome(), DiagnosticsOutcome::Cancelled);
    assert!(!result.started());
    fixture.assert_no_reports(&snapshot)?;
    Ok(())
}

fn wait_for_file(path: &Path, timeout: Duration) -> io::Result<()> {
    let deadline = Instant::now() + timeout;
    while !path.try_exists()? {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("fixture did not acknowledge {path:?}"),
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

// Release is scoped to this fixture, never a process-name kill. On assertion
// unwind or a lifecycle mutation, a surviving shell cooperates within 50 ms;
// its own ten-second watchdog also bounds a lost release or crashed test runner.
struct CooperativeRelease {
    root: PathBuf,
}

impl CooperativeRelease {
    fn release(&self) -> io::Result<()> {
        fs::write(self.root.join("release"), b"release")
    }
}

impl Drop for CooperativeRelease {
    fn drop(&mut self) {
        if let Err(error) = self.release() {
            eprintln!("owned diagnostics fixture release failed: {error}");
        }
        // A killed child cannot acknowledge completion. The bounded wait is
        // cleanup only, not an assertion or proof of process reaping.
        let deadline = Instant::now() + Duration::from_millis(250);
        while !self.root.join("done").exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
}

#[test]
fn timeout_preserves_ready_child_partial_output() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let stdout = b"src/lib.rs:17: partial stdout\n";
    let stderr = b"partial stderr\xff";
    let command = fixture.script(stdout, stderr, 0, true)?;
    let _release = CooperativeRelease {
        root: fixture.tree.path().to_path_buf(),
    };
    let result = run_diagnostics(
        &fixture,
        &command,
        Duration::from_secs(3),
        &Cancellation::default(),
    )?;
    assert!(
        fixture.tree.path().join("ready").exists(),
        "timeout fixture never became live"
    );
    assert_eq!(result.outcome(), DiagnosticsOutcome::TimedOut);
    assert!(result.started());
    fixture.assert_raw(stdout, stderr)?;
    assert_status(&fixture.manifest()?, DiagnosticsOutcome::TimedOut)?;
    let report = fixture.report()?;
    assert_outcome_text(&report, DiagnosticsOutcome::TimedOut)?;
    assert!(report.contains("src/lib.rs:17: partial stdout"));
    Ok(())
}

#[test]
fn live_cancellation_after_ready_retains_partial_evidence() -> Result<(), Box<dyn Error>> {
    let fixture = Fixture::new()?;
    let stdout = b"src\\lib.rs:27: cancellation stdout\n";
    let stderr = b"cancellation stderr\xfe";
    let command = fixture.script(stdout, stderr, 0, true)?;
    let cancellation = Cancellation::default();
    thread::scope(|scope| -> Result<(), Box<dyn Error>> {
        // Declare inside the scope so unwind releases before scope joins.
        let release = CooperativeRelease {
            root: fixture.tree.path().to_path_buf(),
        };
        let worker = scope
            .spawn(|| run_diagnostics(&fixture, &command, Duration::from_secs(20), &cancellation));
        wait_for_file(&fixture.tree.path().join("ready"), Duration::from_secs(5))?;
        cancellation.cancel();
        let result = worker
            .join()
            .map_err(|_| io::Error::other("diagnostics worker panicked"))??;
        release.release()?;
        assert_eq!(result.outcome(), DiagnosticsOutcome::Cancelled);
        assert!(result.started());
        fixture.assert_raw(stdout, stderr)?;
        assert_status(&fixture.manifest()?, DiagnosticsOutcome::Cancelled)?;
        let report = fixture.report()?;
        assert_outcome_text(&report, DiagnosticsOutcome::Cancelled)?;
        assert!(report.contains("src\\lib.rs:27: cancellation stdout"));
        Ok(())
    })
}
