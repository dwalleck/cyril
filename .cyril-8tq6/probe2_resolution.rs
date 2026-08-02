//! PROBE 2 — cyril-8tq6 falsifiable-design cheapest falsifiers. NOT a permanent test.
//! Run via .cyril-8tq6/run-probe2.sh (same copy/run/delete mechanism as probe 1).
//!
//! F-C6: distro-resolution order prototype (env override → cwd-derived → None)
//!       over all 7 input shapes.
//! F-C2': the to-Windows conformance table under the `\\wsl$` COMPAT emission
//!        prefix (design's recommended default), not just `\\wsl.localhost`.
//! F-C3': foreign-distro reverse inputs expressed as PASSTHROUGH (the impl
//!        semantics; probe 1 expressed the same fact as `None`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

const PREFIXES: [&str; 2] = [r"\\wsl.localhost\", r"\\wsl$\"];

/// Proposed resolution: non-empty env wins; else a cwd under a WSL UNC prefix
/// donates its distro segment; else None. Empty env is treated as unset.
fn proto_resolve_distro(env: Option<&str>, cwd: Option<&str>) -> Option<String> {
    if let Some(e) = env
        && !e.is_empty()
    {
        return Some(e.to_string());
    }
    let cwd = cwd?;
    let rest = PREFIXES.iter().find_map(|p| cwd.strip_prefix(p))?;
    let rest = rest.replace('\\', "/");
    let seg = rest.split('/').next().unwrap_or("");
    if seg.is_empty() {
        None
    } else {
        Some(seg.to_string())
    }
}

fn proto_wsl_to_win_compat(path: &str, distro: &str) -> String {
    if path.starts_with('/') && !path.starts_with("/mnt/") {
        format!(r"\\wsl$\{distro}{}", path.replace('/', "\\"))
    } else {
        path.to_string()
    }
}

/// Reverse with passthrough semantics: a non-matching distro segment (or a
/// non-WSL UNC) returns the input unchanged rather than None.
fn proto_win_to_wsl_passthru(path: &str, distro: &str) -> String {
    let Some(rest) = PREFIXES.iter().find_map(|p| path.strip_prefix(p)) else {
        return path.to_string();
    };
    let norm = rest.replace('\\', "/");
    let (seg, tail) = match norm.find('/') {
        Some(i) => (&norm[..i], &norm[i..]),
        None => (norm.as_str(), ""),
    };
    if seg != distro {
        return path.to_string();
    }
    if tail.is_empty() {
        "/".to_string()
    } else {
        tail.to_string()
    }
}

#[test]
fn probe2() {
    // ── F-C6: resolution-order table (7 shapes) ─────────────────────────────
    let cases: &[(Option<&str>, Option<&str>, Option<&str>, &str)] = &[
        (Some("Ubuntu"), None, Some("Ubuntu"), "env only"),
        (
            None,
            Some(r"\\wsl$\Debian\home\u"),
            Some("Debian"),
            "cwd only (compat prefix)",
        ),
        (
            None,
            Some(r"\\wsl.localhost\Debian\home\u"),
            Some("Debian"),
            "cwd only (canonical)",
        ),
        (
            Some("Ubuntu"),
            Some(r"\\wsl$\Debian\home\u"),
            Some("Ubuntu"),
            "both -> env wins",
        ),
        (None, None, None, "neither"),
        (
            Some(""),
            Some(r"C:\Users\u"),
            None,
            "empty env treated unset; drive cwd",
        ),
        (
            None,
            Some(r"\\wsl$\Ubuntu"),
            Some("Ubuntu"),
            "UNC root cwd, no tail",
        ),
    ];
    let mut fail = 0;
    for (env, cwd, want, label) in cases {
        let got = proto_resolve_distro(*env, *cwd);
        let ok = got.as_deref() == *want;
        println!(
            "{} resolve [{label}] env={env:?} cwd={cwd:?} -> {got:?}",
            if ok { "PASS" } else { "FAIL" }
        );
        if !ok {
            fail += 1;
        }
    }

    // ── F-C2': to-Windows conformance under \\wsl$ emission ────────────────
    let to_win: &[(&str, &str)] = &[
        ("/", r"\\wsl$\Ubuntu\"),
        ("/root", r"\\wsl$\Ubuntu\root"),
        ("/proc/stat", r"\\wsl$\Ubuntu\proc\stat"),
        ("/proc/1/", r"\\wsl$\Ubuntu\proc\1\"),
        (
            "/home/dwalleck/.claude/tmp/kas-5-fsterm-cpyeva4m/summary.txt",
            r"\\wsl$\Ubuntu\home\dwalleck\.claude\tmp\kas-5-fsterm-cpyeva4m\summary.txt",
        ),
    ];
    for (input, want) in to_win {
        let got = proto_wsl_to_win_compat(input, "Ubuntu");
        let ok = got == *want;
        println!(
            "{} to_win_compat {input} -> {got}",
            if ok { "PASS" } else { "FAIL" }
        );
        if !ok {
            fail += 1;
        }
    }

    // ── F-C3': foreign/malformed reverse inputs pass through unchanged ─────
    let passthru: &[&str] = &[
        r"\\wsl.localhost\Ubuntu-other\foo",
        r"\\wsl$\UbuntuX\foo",
        r"\\wsl$\",
        r"\\server\share\file.txt",
        r"C:\Users\u",
    ];
    for input in passthru {
        let got = proto_win_to_wsl_passthru(input, "Ubuntu");
        let ok = got == *input;
        println!(
            "{} passthru {input} -> {got}",
            if ok { "PASS" } else { "FAIL" }
        );
        if !ok {
            fail += 1;
        }
    }
    // and the matching-distro forms still translate under passthrough semantics
    assert_eq!(
        proto_win_to_wsl_passthru(r"\\wsl$\Ubuntu\home\u", "Ubuntu"),
        "/home/u"
    );
    assert_eq!(proto_win_to_wsl_passthru(r"\\wsl$\Ubuntu", "Ubuntu"), "/");

    println!("probe2: {fail} FAIL");
    assert_eq!(fail, 0);
}
