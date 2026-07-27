#!/usr/bin/env python3
"""cyril-nd4h probe 2: what capacity does the highlight cache ACTUALLY hold?

Slice-1 proved `highlight_cache_size` has no consumer. This slice measures the
gap: the documented default is 20, the production literal is 256. Rather than
trust either number, RUN the real `HashCache` and count retained entries.

Mechanism: behavioral (execute the real type, measure the high-water mark).
The oracle for this slice is textual (the source literal + AGENTS.md), so the
two remain independent.

Writes a temp integration test, runs it, then removes it -- the repo is left
byte-identical (verified by `git status`).
"""
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TEST = ROOT / "crates/cyril-ui/tests/nd4h_probe.rs"

SRC = """\
use cyril_ui::cache::HashCache;

#[test]
fn nd4h_effective_capacity() {
    // 20 = documented default (UiConfig::default), 256 = production literal
    // (highlight.rs:22, widgets/markdown.rs:20).
    for cap in [20usize, 256usize] {
        let mut peak = 0usize;
        let mut c: HashCache<u32> = HashCache::new(cap);
        for i in 0..1000u64 {
            c.insert(i, i as u32);
            let live = (0..=i).filter(|k| c.get(*k).is_some()).count();
            if live > peak {
                peak = live;
            }
        }
        let retained = (0..1000u64).filter(|k| c.get(*k).is_some()).count();
        println!("NDPROBE cap={cap} inserted=1000 peak_held={peak} final={retained}");
    }
}
"""


def main():
    TEST.write_text(SRC)
    try:
        p = subprocess.run(
            ["cargo", "test", "-p", "cyril-ui", "--test", "nd4h_probe",
             "--", "--nocapture"],
            cwd=ROOT, capture_output=True, text=True,
        )
        lines = [l for l in p.stdout.splitlines() if "NDPROBE" in l]
        print("\n".join(lines) if lines else p.stdout[-2000:] + p.stderr[-2000:])
        return 0 if lines else 1
    finally:
        TEST.unlink(missing_ok=True)
        print("removed temp test")


if __name__ == "__main__":
    sys.exit(main())
