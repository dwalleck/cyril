use std::error::Error;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;

fn wait_for(path: &Path) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        if Instant::now() >= deadline {
            return Err(format!("fixture handshake timed out: {}", path.display()).into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if let Some(mode) = args.get(1) {
        let root = Path::new(args.get(2).ok_or("missing fixture root")?);
        if mode == "holder" {
            print!("B");
            std::io::stdout().flush()?;
            eprint!("C");
            std::io::stderr().flush()?;
            fs::write(root.join("ready"), b"ready")?;
            wait_for(&root.join("release"))?;
            // These handles remain writable after the parent unlinks their names.
            print!("late");
            std::io::stdout().flush()?;
            fs::write(root.join("done"), b"done")?;
            return Ok(());
        }
        if mode == "child" {
            print!("A");
            std::io::stdout().flush()?;
            let _holder = Command::new(std::env::current_exe()?)
                .arg("holder")
                .arg(root)
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?;
            wait_for(&root.join("ready"))?;
            std::process::exit(7);
        }
        return Err("unknown fixture mode".into());
    }
    let root = std::env::temp_dir().join(format!("cyril-spool-proof-{}", std::process::id()));
    fs::create_dir(&root)?;
    let out = root.join("stdout");
    let err = root.join("stderr");
    let status = Command::new(std::env::current_exe()?)
        .arg("child")
        .arg(&root)
        .stdin(Stdio::null())
        .stdout(File::create(&out)?)
        .stderr(File::create(&err)?)
        .status()?;
    assert_eq!(status.code(), Some(7), "direct child exit");
    assert!(!root.join("release").exists(), "holder still owns handles");
    let stdout_file = File::open(&out)?;
    let snapshot_len = stdout_file.metadata()?.len();
    let mut snapshot = Vec::new();
    stdout_file.take(snapshot_len).read_to_end(&mut snapshot)?;
    assert_eq!(snapshot, b"AB", "exact bounded stdout snapshot");
    assert_eq!(fs::read(&err)?, b"C", "independent stderr capture");
    fs::remove_file(&out)?;
    fs::remove_file(&err)?;
    fs::write(root.join("release"), b"release")?;
    wait_for(&root.join("done"))?;
    assert_eq!(
        snapshot, b"AB",
        "late writes cannot mutate retained evidence"
    );
    fs::remove_dir_all(&root)?;
    println!("PASS F0: direct child reaped; snapshot AB/C; inherited handles unlinked while live; holder writes after unlink and exits cooperatively");
    Ok(())
}
