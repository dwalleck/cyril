use cyril_ui::theme::{ThemeId, resolve_ansi16};
use std::{hint::black_box, process::ExitCode, time::Instant};

const BATCHES: usize = 20;
const RESOLUTIONS_PER_BATCH: usize = 5_000;
const LIMIT_NS: u128 = 100_000;

fn main() -> ExitCode {
    for _ in 0..1_000 {
        black_box(resolve_ansi16(ThemeId::CyrilDark));
    }

    let mut samples = Vec::with_capacity(BATCHES);
    for _ in 0..BATCHES {
        let started = Instant::now();
        for _ in 0..RESOLUTIONS_PER_BATCH {
            black_box(resolve_ansi16(ThemeId::CyrilDark));
        }
        samples.push(started.elapsed().as_nanos() / RESOLUTIONS_PER_BATCH as u128);
    }
    samples.sort_unstable();
    let median_ns = samples[BATCHES / 2];
    println!(
        "budget resolutions={} batches={BATCHES} median_ns={median_ns} limit_ns={LIMIT_NS}",
        BATCHES * RESOLUTIONS_PER_BATCH
    );
    if median_ns <= LIMIT_NS {
        ExitCode::SUCCESS
    } else {
        eprintln!("budget exceeded: median_ns={median_ns} limit_ns={LIMIT_NS}");
        ExitCode::FAILURE
    }
}
