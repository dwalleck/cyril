// Temporary Empirical instrumentation, not a production tracing API.
use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const INSTRUMENTATION_EDIT_BASE: &str = "4ef93cad80a47a5fb49a08794e80de57b13054cb";
const PROBE_ID: &str = "cyril-u62f-phase-v1";
const RECORD_CAP: usize = 256;
const DETAIL_CAP: usize = 2_048;

struct Probe {
    enabled: bool,
    started: Instant,
    records: AtomicUsize,
    sink: Option<Mutex<File>>,
}

fn probe() -> &'static Probe {
    static PROBE: LazyLock<Probe> = LazyLock::new(|| {
        let enabled =
            std::env::var_os("CYRIL_MEMORY_PHASE_PROBE").is_some_and(|value| value == "1");
        let sink = if enabled {
            std::env::var_os("CYRIL_MEMORY_PHASE_DIRECTORY").and_then(|directory| {
                let stamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
                    Ok(duration) => duration.as_nanos().to_string(),
                    Err(_) => "before-unix-epoch".to_owned(),
                };
                let path = PathBuf::from(directory).join(format!(
                    "phase-{}-{}-{stamp}.log",
                    std::process::id(),
                    env!("CARGO_PKG_NAME"),
                ));
                match OpenOptions::new().create_new(true).write(true).open(path) {
                    Ok(file) => Some(Mutex::new(file)),
                    Err(error) => {
                        eprintln!(
                            "CYRIL_U62F_PHASE_FILE_ERROR pid={} error={error:?}",
                            std::process::id()
                        );
                        None
                    }
                }
            })
        } else {
            None
        };
        Probe {
            enabled,
            started: Instant::now(),
            records: AtomicUsize::new(0),
            sink,
        }
    });
    &PROBE
}

pub fn enabled() -> bool {
    probe().enabled
}

pub fn event(event: &str, detail: impl Debug) {
    let probe = probe();
    if !probe.enabled {
        return;
    }
    let sequence = probe.records.fetch_add(1, Ordering::Relaxed);
    if sequence >= RECORD_CAP {
        return;
    }
    let (event, mut detail) = if sequence == RECORD_CAP - 1 {
        (
            "probe_record_cap_hit",
            "record cap reached; later phases missing; mechanism unattributed".to_owned(),
        )
    } else {
        (event, format!("{detail:?}"))
    };
    if detail.len() > DETAIL_CAP {
        let boundary = detail.floor_char_boundary(DETAIL_CAP);
        detail.truncate(boundary);
        detail.push_str(" [truncated]");
    }
    let utc_us = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => Some(duration.as_micros()),
        Err(_) => None,
    };
    let line = format!(
        "CYRIL_U62F_PHASE pid={} component={} seq={sequence} utc_us={utc_us:?} elapsed_us={} event={event:?} detail={detail:?}",
        std::process::id(),
        env!("CARGO_PKG_NAME"),
        probe.started.elapsed().as_micros(),
    );
    if let Some(sink) = &probe.sink {
        let mut file = match sink.lock() {
            Ok(file) => file,
            Err(poisoned) => {
                eprintln!("CYRIL_U62F_PHASE_FILE_ERROR poisoned diagnostic sink");
                poisoned.into_inner()
            }
        };
        if let Err(error) = writeln!(file, "{line}") {
            eprintln!(
                "CYRIL_U62F_PHASE_FILE_ERROR pid={} error={error:?}",
                std::process::id()
            );
        }
    }
    eprintln!("{line}");
}

pub fn identity(role: &str) {
    if !enabled() {
        return;
    }
    let exported_baseline = std::env::var("CYRIL_MEMORY_PROBE_SOURCE")
        .ok()
        .filter(|value| value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()));
    event(
        "identity",
        (
            role,
            INSTRUMENTATION_EDIT_BASE,
            exported_baseline,
            PROBE_ID,
            std::env::current_exe(),
        ),
    );
}

pub struct Phase {
    name: &'static str,
    started: Option<Instant>,
}

impl Phase {
    pub fn new(name: &'static str) -> Self {
        let started = if enabled() {
            event("phase_begin", name);
            Some(Instant::now())
        } else {
            None
        };
        Self { name, started }
    }

    pub fn finish(mut self, outcome: impl Debug) {
        if let Some(started) = self.started.take() {
            event(
                "phase_end",
                (self.name, started.elapsed().as_micros(), outcome),
            );
        }
    }

    pub fn result<T, E: Debug>(self, result: Result<T, E>) -> Result<T, E> {
        match &result {
            Ok(_) => self.finish("ok"),
            Err(error) => self.finish(error),
        }
        result
    }
}

impl Drop for Phase {
    fn drop(&mut self) {
        if let Some(started) = self.started.take() {
            event(
                "phase_interrupted",
                (
                    self.name,
                    started.elapsed().as_micros(),
                    std::thread::panicking(),
                ),
            );
        }
    }
}
