#!/usr/bin/env python3
"""cyril-nd4h probe 2: what capacity do the live caches ACTUALLY hold?

Slice 1 proved `highlight_cache_size` had no consumer. This measures the gap it
left behind: the removed option documented 20, while the live caches construct
at whatever `HashCache::new(N)` says.

The capacity is DERIVED from the production statics, not hardcoded. An earlier
version hardcoded 20 and 256, which meant changing both production sites to 20
would have left the probe's output identical -- it characterised `HashCache`
rather than measuring cyril's caches, while the audit claimed the latter.

Mechanism: behavioral (execute the real type at the real capacity, measure the
high-water mark). The oracle for this slice is textual (the source literal),
so the two stay independent.

Writes a temp integration test, runs it, then removes it. The repo is left
byte-identical (verify with `git status`).

Exit status is 0 only when cargo succeeded AND both expected measurements are
present AND each peak equals its requested capacity.
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TEST = ROOT / "crates/cyril-ui/tests/nd4h_probe.rs"
CACHE_SITES = [
    "crates/cyril-ui/src/highlight.rs",
    "crates/cyril-ui/src/widgets/markdown.rs",
]
DOCUMENTED_DEFAULT = 20  # what the removed `highlight_cache_size` advertised


def live_capacity():
    """The capacity the production caches are actually constructed with."""
    found = {}
    for rel in CACHE_SITES:
        src = (ROOT / rel).read_text()
        caps = {
            int(m)
            for m in re.findall(r"LazyLock::new\(\|\| Mutex::new\(HashCache::new\((\d+)\)\)\)", src)
        }
        if not caps:
            raise SystemExit(f"no production HashCache static found in {rel}")
        if len(caps) > 1:
            raise SystemExit(f"{rel} has differing cache capacities {caps}; probe assumes one")
        found[rel] = caps.pop()
    if len(set(found.values())) != 1:
        raise SystemExit(f"production caches disagree on capacity: {found}")
    return next(iter(found.values())), found


SRC = """\
use cyril_ui::cache::HashCache;

#[test]
fn nd4h_effective_capacity() {
    for cap in [%d_usize, %d_usize] {
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
    live, per_site = live_capacity()
    print(f"live cache capacity (derived from source): {live}")
    for rel, cap in per_site.items():
        print(f"    {rel} -> HashCache::new({cap})")
    print(f"documented default of the removed option: {DOCUMENTED_DEFAULT}\n")

    TEST.write_text(SRC % (DOCUMENTED_DEFAULT, live))
    try:
        p = subprocess.run(
            ["cargo", "test", "-p", "cyril-ui", "--test", "nd4h_probe",
             "--", "--nocapture"],
            cwd=ROOT, capture_output=True, text=True,
        )
    finally:
        TEST.unlink(missing_ok=True)

    if p.returncode != 0:
        print("FAIL: cargo test exited nonzero")
        print(p.stdout[-1500:], p.stderr[-1500:])
        return 1

    measured = {
        int(m.group(1)): int(m.group(2))
        for m in re.finditer(r"NDPROBE cap=(\d+) inserted=1000 peak_held=(\d+)", p.stdout)
    }
    for line in p.stdout.splitlines():
        if "NDPROBE" in line:
            print(line)

    expected = {DOCUMENTED_DEFAULT, live}
    if set(measured) != expected:
        print(f"\nFAIL: expected measurements for {sorted(expected)}, got {sorted(measured)}")
        return 1
    for cap, peak in measured.items():
        if peak != cap:
            print(f"\nFAIL: cap={cap} held {peak} at peak, expected {cap}")
            return 1

    ratio = live / DOCUMENTED_DEFAULT
    print(f"\nPASS: live caches hold {live}; the removed option documented "
          f"{DOCUMENTED_DEFAULT} ({ratio:.1f}x understated)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
