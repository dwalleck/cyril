use super::*;

// Every C7 test drives the real in-process memory runtime, which binds a
// unix-domain socket — unix-only, like app.rs's memory tests.
#[cfg(unix)]
mod memory;
mod ordering;
mod shutdown;
