//! Standalone empirical instrument. Not production feature code.
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let python = std::env::args()
        .nth(1)
        .ok_or("python executable required")?;
    let mut argv_probe = Command::new(&python);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        argv_probe.raw_arg(
            r#"-c "import sys;print(repr(sys.argv[1:]))" "two words" "quote\"inside" back\slash"#,
        );
    }
    #[cfg(not(windows))]
    argv_probe.args([
        "-c",
        "import sys;print(repr(sys.argv[1:]))",
        "two words",
        "quote\"inside",
        "back\\slash",
    ]);
    let output = argv_probe.output()?;
    if !output.status.success() {
        return Err(format!("argv child failed: {:?}", output).into());
    }
    let expected = if cfg!(windows) {
        b"['two words', 'quote\"inside', 'back\\\\slash']\r\n".as_slice()
    } else {
        b"['two words', 'quote\"inside', 'back\\\\slash']\n".as_slice()
    };
    if output.stdout != expected || !output.stderr.is_empty() {
        return Err(format!("argv mismatch: {:?}, expected {:?}", output, expected).into());
    }
    let bytes = Command::new(&python)
        .args([
            "-c",
            "import os;os.write(1,b'x'*131072+b'\\xff');os.write(2,b'y'*131072+b'\\xfe')",
        ])
        .output()?;
    if !bytes.status.success()
        || bytes.stdout.len() != 131_073
        || bytes.stderr.len() != 131_073
        || !bytes.stdout[..131_072].iter().all(|b| *b == b'x')
        || !bytes.stderr[..131_072].iter().all(|b| *b == b'y')
        || bytes.stdout[131_072] != 0xff
        || bytes.stderr[131_072] != 0xfe
    {
        return Err("std dual-pipe/raw-byte mismatch".into());
    }
    println!("PASS std argv: spaces, literal quote, backslash; no shell");
    println!("PASS std capture: 131073 stdout bytes + 131073 stderr bytes, invalid UTF-8 intact");
    Ok(())
}
