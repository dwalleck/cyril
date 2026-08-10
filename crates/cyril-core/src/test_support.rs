//! Shared test scaffolding for cyril-core's test modules and — behind the
//! `test-support` feature — the test suites of downstream crates.
//!
//! Nothing here is production code: the module compiles only under
//! `cfg(test)` or when a dependent crate enables the `test-support` feature —
//! intended from `[dev-dependencies]` only; a `[dependencies]` entry could
//! technically enable it too (cargo features are additive), so treat any
//! non-dev enablement as a review error.

use std::io;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

static TRACING_CAPTURE_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Serializes tests that install a tracing capture subscriber. Belt and
/// braces: `tracing::subscriber::with_default` is thread-scoped (the only
/// installer these suites use), so interleaving requires a future test to
/// install globally — this lock keeps that mistake from corrupting captures.
/// Recovers from poisoning — a panicking test must not cascade.
pub fn tracing_capture_lock() -> MutexGuard<'static, ()> {
    match TRACING_CAPTURE_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Unwraps `result`, panicking with `context` and the debug-formatted error
/// on failure — so fixture setup fails loudly with a pointer to what broke.
pub fn must_succeed<T, E: std::fmt::Debug>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{context}: {error:?}"),
    }
}

/// A cloneable [`tracing_subscriber::fmt::MakeWriter`] that appends every
/// write to a shared in-memory buffer, so tests can assert on log output.
#[derive(Clone, Default)]
pub struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl CaptureWriter {
    /// Returns a copy of everything captured so far, recovering the buffer
    /// even if a writer panicked while holding the lock.
    pub fn captured(&self) -> Vec<u8> {
        match self.0.lock() {
            Ok(captured) => captured.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl io::Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut captured = self
            .0
            .lock()
            .map_err(|error| io::Error::other(error.to_string()))?;
        captured.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}
