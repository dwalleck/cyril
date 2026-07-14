use std::path::Path;

const MODULES: [(&str, &str); 5] = [
    ("chat", "src/widgets/chat.rs"),
    ("markdown", "src/widgets/markdown.rs"),
    ("input", "src/widgets/input.rs"),
    ("suggestions", "src/widgets/suggestions.rs"),
    ("highlight", "src/highlight.rs"),
];

#[derive(Debug, PartialEq, Eq)]
enum FindingKind {
    PaletteAccess,
    HardCodedColor,
}

#[derive(Debug, PartialEq, Eq)]
struct Finding {
    line: usize,
    token: String,
    kind: FindingKind,
}

fn function_body_range(source: &str, name: &str) -> Option<std::ops::Range<usize>> {
    let signature = format!("fn {name}(");
    let function_start = source.find(&signature)?;
    let body_start = function_start + source[function_start..].find('{')?;
    let mut depth = 0usize;
    for (offset, byte) in source.as_bytes()[body_start..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(function_start..body_start + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_signed_conversion(offset: usize, ranges: &[std::ops::Range<usize>]) -> bool {
    ranges.iter().any(|range| range.contains(&offset))
}

fn is_reset_comparison(line: &str, column: usize, token: &str) -> bool {
    if token != "Color::Reset" {
        return false;
    }
    let before = line[..column].trim_end();
    let after = line[column + token.len()..].trim_start();
    before.ends_with("==")
        || before.ends_with("!=")
        || after.starts_with("==")
        || after.starts_with("!=")
}

fn audit_source(source: &str) -> Vec<Finding> {
    let signed_ranges = [
        function_body_range(source, "tint_with_diff_color"),
        function_body_range(source, "color_to_rgb"),
        function_body_range(source, "syntect_to_ratatui"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let mut findings = Vec::new();
    let mut source_offset = 0usize;

    for (line_index, line) in source.lines().enumerate() {
        if line.contains("crate::palette") {
            findings.push(Finding {
                line: line_index + 1,
                token: "crate::palette".into(),
                kind: FindingKind::PaletteAccess,
            });
        } else {
            for _ in line.match_indices("palette::") {
                findings.push(Finding {
                    line: line_index + 1,
                    token: "palette::".into(),
                    kind: FindingKind::PaletteAccess,
                });
            }
        }

        for (column, _) in line.match_indices("Color::") {
            let suffix = &line[column + "Color::".len()..];
            let variant_len = suffix
                .bytes()
                .take_while(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                .count();
            let token = &line[column..column + "Color::".len() + variant_len];
            let absolute_offset = source_offset + column;
            if !is_signed_conversion(absolute_offset, &signed_ranges)
                && !is_reset_comparison(line, column, token)
            {
                findings.push(Finding {
                    line: line_index + 1,
                    token: token.into(),
                    kind: FindingKind::HardCodedColor,
                });
            }
        }

        source_offset += line.len() + 1;
    }
    findings
}

fn production_source(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    // Normalize CRLF (cyril-xi4a): `audit_source` advances its running offset
    // by `line.len() + 1` per line, but `signed_ranges` come from true byte
    // offsets — on a CRLF checkout (Windows runners; no .gitattributes) every
    // token drifts one byte per preceding line and deep tokens fall out of
    // their signed exemption. Auditing an LF-normalized copy keeps offsets
    // and ranges in the same coordinate space on every platform.
    let source = source.replace("\r\n", "\n");
    source
        .split_once("#[cfg(test)]")
        .map_or(source.as_str(), |(production, _)| production)
        .to_string()
}

fn assert_module_clean(label: &str, relative: &str) {
    let source = production_source(relative);
    let findings = audit_source(&source);
    assert!(findings.is_empty(), "{label}: {findings:#?}");
}

#[test]
fn rejects_palette_import() {
    let findings = audit_source("use crate::palette;\nfn render() {}\n");
    assert_eq!(
        findings,
        vec![Finding {
            line: 1,
            token: "crate::palette".into(),
            kind: FindingKind::PaletteAccess,
        }]
    );
}

#[test]
fn rejects_hard_coded_display_color() {
    let findings = audit_source("fn render() { Style::default().fg(Color::White); }\n");
    assert_eq!(
        findings,
        vec![Finding {
            line: 1,
            token: "Color::White".into(),
            kind: FindingKind::HardCodedColor,
        }]
    );
}

#[test]
fn accepts_signed_syntect_rgb_conversion() {
    let source = "fn syntect_to_ratatui(style: SynStyle) -> Style {\n    Style::default().fg(Color::Rgb(style.r, style.g, style.b))\n}\n";
    assert!(audit_source(source).is_empty());
}

#[test]
fn accepts_signed_diff_rgb_conversion() {
    let source = "fn tint_with_diff_color(fg: Color, diff: Color) -> Color {\n    Color::Rgb(fg.r + diff.r, fg.g + diff.g, fg.b + diff.b)\n}\n";
    assert!(audit_source(source).is_empty());
}

// cyril-xi4a: the offset arithmetic (`line.len() + 1`) and the signed ranges
// (true byte offsets) must share a coordinate space. On raw CRLF input they
// don't — every token drifts one byte per preceding line, and tokens deep in
// a file escape their signed exemption (main's Windows CI red, 2026-07-12).
// `production_source` normalizes to LF; this pins the failure shape so the
// normalization can't be dropped: the same source must stay clean when
// CRLF-encoded AND still flag a genuine violation outside the signed fn.
#[test]
fn crlf_source_keeps_signed_exemption_aligned() {
    // Padding lines push the signed fn deep enough that one-byte-per-line
    // drift exceeds the token's depth into the fn body (~76 bytes) and expels
    // it from the exemption range — highlight.rs hit this at ~line 197.
    let mut source = "// pad\n".repeat(128);
    source.push_str(
        "fn syntect_to_ratatui(style: SynStyle) -> Style {\n    Style::default().fg(Color::Rgb(style.r, style.g, style.b))\n}\n",
    );
    let crlf = source.replace('\n', "\r\n");
    let normalized = crlf.replace("\r\n", "\n");
    assert!(
        audit_source(&normalized).is_empty(),
        "signed conversion must stay exempt after LF normalization"
    );
    // Un-normalized CRLF is exactly the drift bug — keep the failure shape
    // documented: the exemption misses and the token is (wrongly) flagged.
    assert!(
        !audit_source(&crlf).is_empty(),
        "raw CRLF drifts offsets; if this starts passing, audit_source went \
         byte-offset-consistent and the normalization in production_source \
         can be retired"
    );
    // A genuine violation outside the signed fn is still caught post-normalization.
    let mut bad = normalized;
    bad.push_str("fn render() { Style::default().fg(Color::White); }\n");
    assert_eq!(audit_source(&bad).len(), 1, "real violations still flagged");
}

#[test]
fn one_mebibyte_audit_finds_violation() {
    let mut source = " ".repeat(1024 * 1024);
    source.push_str("\nfn render() { Style::default().fg(Color::White); }\n");
    let findings = audit_source(&source);
    assert_eq!(findings.len(), 1);
}

#[test]
fn production_chat_is_clean() {
    assert_module_clean(MODULES[0].0, MODULES[0].1);
}

#[test]
fn production_markdown_is_clean() {
    assert_module_clean(MODULES[1].0, MODULES[1].1);
}

#[test]
fn production_input_is_clean() {
    assert_module_clean(MODULES[2].0, MODULES[2].1);
}

#[test]
fn production_suggestions_is_clean() {
    assert_module_clean(MODULES[3].0, MODULES[3].1);
}

#[test]
fn production_highlight_is_clean() {
    assert_module_clean(MODULES[4].0, MODULES[4].1);
}
