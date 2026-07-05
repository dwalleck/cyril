//! Probe for cyril-0v42: what do (a) the CURRENT production write path,
//! (b) the naive tempfile-persist mechanism, and (c) the proposed fixed
//! sequence ACTUALLY do to mode / inode / symlinks / cross-device targets?
//! Reports via Rust std::fs; the independent oracle (../oracle.sh)
//! re-measures every claim with coreutils + strace + python.

use std::io::Write as _;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

fn stat_line(tag: &str, p: &Path) {
    match std::fs::symlink_metadata(p) {
        Ok(m) => {
            let kind = if m.file_type().is_symlink() {
                "symlink"
            } else {
                "regular"
            };
            println!(
                "{tag} kind={kind} mode={:o} ino={} size={}",
                m.permissions().mode() & 0o7777,
                m.ino(),
                m.len()
            );
        }
        Err(e) => println!("{tag} ABSENT ({e})"),
    }
}

/// Linux-only, unsafe-free umask read (workspace forbids unsafe; libc::umask is unsafe).
fn umask_from_proc() -> u32 {
    let s = std::fs::read_to_string("/proc/self/status").expect("proc status");
    let line = s
        .lines()
        .find(|l| l.starts_with("Umask:"))
        .expect("Umask line");
    u32::from_str_radix(line.split_whitespace().nth(1).expect("value"), 8).expect("octal")
}

/// Proposed-fix shape (issue notes): canonicalize (falling back to parent for a
/// fresh file), temp in the SAME dir, write, fsync, chmod existing-or-umask
/// mode, atomic rename over the canonical target.
fn fixed_atomic(target: &Path, content: &[u8]) -> std::io::Result<()> {
    let canonical = match std::fs::canonicalize(target) {
        Ok(p) => p,
        Err(_) => {
            let parent = target.parent().expect("has parent");
            std::fs::create_dir_all(parent)?;
            std::fs::canonicalize(parent)?.join(target.file_name().expect("has file name"))
        }
    };
    let mode = match std::fs::metadata(&canonical) {
        Ok(m) => m.permissions().mode() & 0o7777,
        Err(_) => 0o666 & !umask_from_proc(),
    };
    let dir = canonical.parent().expect("canonical parent");
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(content)?;
    tmp.as_file().sync_all()?;
    tmp.as_file()
        .set_permissions(std::fs::Permissions::from_mode(mode))?;
    tmp.persist(&canonical)?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (cmd, target) = (args[1].as_str(), PathBuf::from(&args[2]));
    stat_line("before:", &target);
    match cmd {
        // host_io.rs:47-54 verbatim: mkdir -p parent, then tokio::fs::write.
        "current-write" => {
            if let Some(parent) = target.parent() {
                tokio::fs::create_dir_all(parent).await.expect("mkdir");
            }
            tokio::fs::write(&target, b"NEW-CONTENT-FROM-PROBE\n")
                .await
                .expect("write");
        }
        // Naive tempfile shape with NO mode/symlink handling.
        "naive-atomic" => {
            let dir = target.parent().expect("parent");
            let mut tmp = tempfile::NamedTempFile::new_in(dir).expect("tempfile");
            tmp.write_all(b"NEW-CONTENT-FROM-PROBE\n")
                .expect("write_all");
            tmp.as_file().sync_all().expect("fsync");
            tmp.persist(&target).expect("persist");
        }
        "fixed-atomic" => {
            fixed_atomic(&target, b"NEW-CONTENT-FROM-PROBE\n").expect("fixed_atomic");
        }
        // Temp on /tmp (tmpfs), target on another fs: is persist really EXDEV?
        "exdev" => {
            let mut tmp = tempfile::NamedTempFile::new().expect("tempfile in /tmp");
            tmp.write_all(b"x").expect("write_all");
            match tmp.persist(&target) {
                Ok(_) => println!("exdev: persist UNEXPECTEDLY SUCCEEDED"),
                Err(e) => println!(
                    "exdev: persist failed raw_os_error={:?} ({})",
                    e.error.raw_os_error(),
                    e.error
                ),
            }
        }
        // Kill-window probes: big content so oracle.sh can SIGKILL mid-write.
        // mb=0 doubles as the empty-content check.
        "kill-write" => {
            let mb: usize = args[3].parse().expect("mb");
            tokio::fs::write(&target, vec![b'N'; mb << 20])
                .await
                .expect("write");
        }
        "kill-atomic" => {
            let mb: usize = args[3].parse().expect("mb");
            fixed_atomic(&target, &vec![b'N'; mb << 20]).expect("fixed_atomic");
        }
        // What does canonicalize say about a missing file vs a dangling link?
        "canon" => println!("canon: {:?}", std::fs::canonicalize(&target)),
        other => panic!("unknown probe {other}"),
    }
    stat_line("after: ", &target);
}
