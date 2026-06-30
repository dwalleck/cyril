// KAS-2b prove-it-prototype PROBE (artifact — was run as a throwaway #[ignore]
// test appended to crates/cyril-core/src/protocol/convert/mod.rs, then removed).
//
// Purpose: exercise cyril's ACTUAL deserialization path on the live-captured
// 2.10.0 context_usage frame — serde -> acp::SessionNotification -> navigate
// siu.meta -> "kiro" -> "breakdown" -> per-bucket {tokens, percent, items?}.
// This is the exact mechanism the KAS-2b converter will use; the probe uses NO
// not-yet-written KAS-2b abstraction.
//
// Run (with the block pasted back into convert/mod.rs):
//   cargo test -p cyril-core probe_kas2b -- --nocapture --ignored
//
// Oracle (independent): .cyril-5et2/oracle.sh (jq navigation of the same bytes).
// Probe and oracle agreed on all 6 values (usagePercentage + 5 buckets incl. the
// items-absent-vs-empty split). See .cyril-5et2/design.md.

#[cfg(test)]
mod probe_kas2b_tmp {
    #![allow(clippy::unwrap_used)]
    use std::path::Path;

    use super::*;

    #[test]
    #[ignore]
    fn probe_extract_context_usage_breakdown() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.cyril-5et2/context_usage_raw.json");
        let raw = std::fs::read_to_string(&path).unwrap();
        // cyril's path: deserialize the wire frame into the acp schema type.
        let sn: acp::SessionNotification = serde_json::from_str(&raw).unwrap();
        let acp::SessionUpdate::SessionInfoUpdate(siu) = &sn.update else {
            panic!("not a session_info_update");
        };
        // Mirror the converter's navigation: siu.meta -> "kiro" -> "breakdown".
        let kiro = siu.meta.as_ref().unwrap().get("kiro").unwrap();
        let usage = kiro
            .get("usagePercentage")
            .and_then(|v| v.as_f64())
            .unwrap();
        let bd = kiro.get("breakdown").unwrap();
        eprintln!("PROBE usagePercentage: {usage}");
        eprintln!("bucket            tokens   percent  items");
        for b in [
            "contextFiles",
            "tools",
            "yourPrompts",
            "kiroResponses",
            "sessionFiles",
        ] {
            let cat = bd.get(b).unwrap();
            let tokens = cat.get("tokens").and_then(|v| v.as_i64()).unwrap();
            let percent = cat.get("percent").and_then(|v| v.as_f64()).unwrap();
            let items = match cat.get("items") {
                Some(v) => format!("items[{}]", v.as_array().map(|a| a.len()).unwrap()),
                None => "items-ABSENT".to_string(),
            };
            eprintln!("PROBE {b:16}  {tokens:<7} {percent:<7} {items}");
        }
    }
}
