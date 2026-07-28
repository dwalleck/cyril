#!/usr/bin/env python3
"""cyril-nd4h cheapest falsifier (claim C5): is field removal parse-compatible?

The whole "remove the two dead fields" half of the design rests on one
assumption: an existing user config.toml that still names them keeps loading.

The trap this is built to catch: `load_from_path` swallows a parse error and
returns Self::default() with only a warn!. So "it returned a Config" proves
NOTHING -- a rejecting deserializer and an accepting one both yield a Config.
The test therefore sets a KNOWN field to a NON-default value alongside the
unknown keys:

    max_messages = 999  +  unknown keys
      -> 999  means: parsed, unknown fields ignored  (C5 holds)
      -> 500  means: errored, silently fell back to defaults (C5 FALSE)

Falsified if the loaded max_messages is not 999.
"""
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TEST = ROOT / "crates/cyril-core/tests/nd4h_falsifier_c5.rs"

SRC = """\
use cyril_core::types::config::Config;
use std::io::Write;

#[test]
fn legacy_config_with_removed_fields_still_parses() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    // A user's config from before the removal: known field at a NON-default
    // value, plus the fields this ticket deletes, plus a never-known key.
    let mut f = std::fs::File::create(&path).unwrap();
    write!(
        f,
        r#"
[ui]
max_messages = 999
highlight_cache_size = 40
stream_buffer_timeout_ms = 300
totally_unknown_key = "xyz"
"#
    )
    .unwrap();
    drop(f);

    let cfg = Config::load_from_path(&path);
    println!("NDC5 max_messages={} (999=parsed/ignored, 500=fell back)",
             cfg.ui.max_messages);
    assert_eq!(
        cfg.ui.max_messages, 999,
        "C5 FALSIFIED: unknown/removed keys caused a parse error and a silent \
         fallback to defaults -- removal would NOT be compat-safe"
    );
}
"""


def main():
    TEST.write_text(SRC)
    try:
        p = subprocess.run(
            ["cargo", "test", "-p", "cyril-core", "--test",
             "nd4h_falsifier_c5", "--", "--nocapture"],
            cwd=ROOT, capture_output=True, text=True,
        )
        for l in p.stdout.splitlines():
            if "NDC5" in l or "test result" in l or "FALSIFIED" in l:
                print(l)
        print("EXIT", p.returncode)
        if p.returncode != 0:
            print(p.stdout[-1500:], p.stderr[-1500:])
        return p.returncode
    finally:
        TEST.unlink(missing_ok=True)
        print("removed temp test")


if __name__ == "__main__":
    sys.exit(main())
