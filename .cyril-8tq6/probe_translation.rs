//! PROBE — cyril-8tq6 prove-it-prototype. NOT a permanent test.
//!
//! Run via `.cyril-8tq6/run-probe.sh`, which copies this file to
//! `crates/cyril-core/tests/probe_cyril_8tq6.rs`, runs it with `--nocapture`,
//! captures output to `.cyril-8tq6/probe-output.txt`, and deletes the copy.
//! The committed copy under `.cyril-8tq6/` is the audit trail.
//!
//! Part A: what cyril's REAL `wsl_to_win` does to every absolute path in the
//!         real KAS 2.10.0 host-callback capture (production-shape data).
//! Part B: a ~30-line PROTOTYPE of the proposed rule, checked item-by-item
//!         against Microsoft's own wslpath conformance tests
//!         (microsoft/WSL test/linux/unit_tests/wslpath.c — the oracle).
//! Part C: round-trip capture paths through the prototype.
//! Part D: the REAL `translate_paths_in_json` on a real capture envelope —
//!         proving the JSON layer also leaves /home paths untouched.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use cyril_core::platform::path::{Direction, translate_paths_in_json, wsl_to_win};
use serde_json::Value;

// ── Part B prototype (proposed rule — lives ONLY in this probe) ──────────────

const CANONICAL_PREFIX: &str = r"\\wsl.localhost\";
const COMPAT_PREFIX: &str = r"\\wsl$\";

/// Proposed: `/mnt/<drive>` keeps the existing drive translation; any other
/// `/`-rooted path becomes `\\wsl.localhost\<distro>\<path with backslashes>`.
fn proto_wsl_to_win(path: &str, distro: &str) -> String {
    let drive = wsl_to_win(path);
    let drive = drive.to_string_lossy();
    if drive != path {
        return drive.into_owned(); // existing /mnt/<letter> handling won
    }
    if path.starts_with('/') {
        format!("{CANONICAL_PREFIX}{distro}{}", path.replace('/', "\\"))
    } else {
        path.to_string()
    }
}

/// Proposed reverse: accept BOTH `\\wsl.localhost\` and `\\wsl$\`, both slash
/// kinds in the tail, require an EXACT distro segment match (MS treats
/// `<distro>-other` / `<distro>X` as errors), map to the POSIX path.
fn proto_win_to_wsl(path: &str, distro: &str) -> Option<String> {
    let rest = path
        .strip_prefix(CANONICAL_PREFIX)
        .or_else(|| path.strip_prefix(COMPAT_PREFIX))?;
    let rest = rest.replace('\\', "/");
    let (seg, tail) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest.as_str(), ""),
    };
    if seg != distro {
        return None;
    }
    Some(if tail.is_empty() {
        "/".to_string()
    } else {
        tail.to_string()
    })
}

fn collect_abs_strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) if s.starts_with('/') => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|v| collect_abs_strings(v, out)),
        Value::Object(m) => m.values().for_each(|v| collect_abs_strings(v, out)),
        _ => {}
    }
}

#[test]
fn probe() {
    // ── Part A ───────────────────────────────────────────────────────────────
    let capture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../.cyril-7bdu/host_callbacks_2.10.0.json"
    );
    let capture: Value =
        serde_json::from_str(&std::fs::read_to_string(capture_path).unwrap()).unwrap();
    let mut abs = Vec::new();
    collect_abs_strings(&capture, &mut abs);
    let mut passthru = 0;
    let mut translated = 0;
    println!("── Part A: REAL wsl_to_win over capture absolute strings ──");
    for p in &abs {
        let out = wsl_to_win(p);
        let out = out.to_string_lossy();
        if out == *p {
            passthru += 1;
            println!("PASSTHRU   {p}");
        } else {
            translated += 1;
            println!("TRANSLATED {p} -> {out}");
        }
    }
    // control: a /mnt drive path DOES translate today
    let control = wsl_to_win("/mnt/c/Users/foo");
    println!("CONTROL    /mnt/c/Users/foo -> {}", control.display());
    println!(
        "A: {} abs strings, {passthru} PASSTHRU, {translated} translated",
        abs.len()
    );
    assert_eq!(control.to_string_lossy(), r"C:\Users\foo");

    // ── Part B: MS wslpath conformance (oracle: microsoft/WSL wslpath.c) ─────
    println!("── Part B: prototype vs Microsoft wslpath conformance ──");
    let d = "Ubuntu";
    let to_win: &[(&str, &str)] = &[
        ("/", r"\\wsl.localhost\Ubuntu\"),
        ("/root", r"\\wsl.localhost\Ubuntu\root"),
        ("/proc/stat", r"\\wsl.localhost\Ubuntu\proc\stat"),
        ("/proc/1/", r"\\wsl.localhost\Ubuntu\proc\1\"),
        // drive mounts keep drive translation (WslPathTestDrvFs*)
        ("/mnt/c/Users", r"C:\Users"),
    ];
    let from_win: &[(&str, Option<&str>)] = &[
        (r"\\wsl.localhost\Ubuntu", Some("/")),
        (r"\\wsl.localhost\Ubuntu\", Some("/")),
        (r"\\wsl.localhost\Ubuntu\root", Some("/root")),
        (r"\\wsl.localhost\Ubuntu\proc\stat", Some("/proc/stat")),
        (r"\\wsl.localhost\Ubuntu/proc/stat", Some("/proc/stat")),
        (r"\\wsl$\Ubuntu\proc\stat", Some("/proc/stat")),
        (r"\\wsl$\Ubuntu", Some("/")),
        // exact-segment guard: prefix-colliding distro names are errors
        (r"\\wsl.localhost\Ubuntu-other\foo", None),
        (r"\\wsl.localhost\UbuntuX\foo", None),
        (r"\\wsl$\Ubuntu-other\foo", None),
        (r"\\wsl$\UbuntuX\foo", None),
    ];
    let mut pass = 0;
    let mut fail = 0;
    for (input, want) in to_win {
        let got = proto_wsl_to_win(input, d);
        let ok = got == *want;
        println!(
            "{} to_win   {input:24} -> {got}",
            if ok { "PASS" } else { "FAIL" }
        );
        if ok { pass += 1 } else { fail += 1 }
    }
    for (input, want) in from_win {
        let got = proto_win_to_wsl(input, d);
        let ok = got.as_deref() == *want;
        println!(
            "{} from_win {input:40} -> {got:?}",
            if ok { "PASS" } else { "FAIL" }
        );
        if ok { pass += 1 } else { fail += 1 }
    }
    println!(
        "B: {pass} PASS, {fail} FAIL of {}",
        to_win.len() + from_win.len()
    );
    assert_eq!(fail, 0, "prototype disagrees with MS wslpath conformance");

    // ── Part C: round-trip every capture path through the prototype ──────────
    let mut rt_ok = 0;
    for p in &abs {
        let unc = proto_wsl_to_win(p, d);
        if unc.starts_with(CANONICAL_PREFIX) {
            let back = proto_win_to_wsl(&unc, d).unwrap();
            assert_eq!(&back, p, "round-trip failed for {p}");
            rt_ok += 1;
        }
    }
    println!("C: {rt_ok}/{} capture paths round-trip via UNC", abs.len());

    // ── Part D: REAL translate_paths_in_json leaves /home untouched ──────────
    let mut envelope = serde_json::json!({
        "method": "fs/write_text_file",
        "params": {
            "sessionId": "sess_x",
            "path": "/home/dwalleck/.claude/tmp/kas-5-fsterm-cpyeva4m/summary.txt",
            "content": "magic=4242\n"
        }
    });
    let before = envelope.clone();
    translate_paths_in_json(&mut envelope, Direction::WslToWin);
    let untouched = envelope == before;
    println!("D: JSON envelope untouched by WslToWin translation = {untouched}");
    assert!(
        untouched,
        "expected /home path to survive untranslated (the bug)"
    );
}
