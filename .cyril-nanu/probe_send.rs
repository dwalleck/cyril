//! P3/P4 — can a snapshot cross a thread boundary and ride the App's existing
//! async-result channel?
//!
//! The design computes `UsageSnapshot` off the event loop and delivers it back
//! through the same `mpsc` + `tokio::select!` pattern the App already uses for
//! `UsageEnrichmentResult` (`app.rs:76`, `:748`). Two things must hold:
//!
//!   P3 — `UsageSnapshot` is `Send + 'static`, so it can be produced on a
//!        blocking task and moved to the loop.
//!   P4 — the existing carried type is `Send + 'static` too, i.e. that channel
//!        already carries owned values across a thread boundary and is a
//!        reusable pattern rather than a same-thread convenience.
//!
//! Mechanism: the compiler. These are trait bounds checked at compile time;
//! the file either builds or it does not, and a failure names the offending
//! field.
//!
//! Run (from the worktree root):
//!   cp .cyril-nanu/probe_send.rs crates/cyril-core/tests/probe_nanu_send.rs
//!   cargo test -p cyril-core --test probe_nanu_send
//!   rm crates/cyril-core/tests/probe_nanu_send.rs

use cyril_core::types::UsageSnapshot;
use cyril_core::usage::UsageEnrichmentResult;

fn assert_send_static<T: Send + 'static>() {}

#[test]
fn snapshot_and_the_existing_channel_payload_cross_threads() {
    // P3
    assert_send_static::<UsageSnapshot>();
    // P4 — the pattern already carries an owned value across a thread boundary.
    assert_send_static::<UsageEnrichmentResult>();

    // Positive control: asserting a bound proves nothing unless the same
    // assertion can be shown to reject something. `Rc` is the canonical
    // non-Send type; that this line is commented out and the ones above are
    // not is the whole result.
    //   assert_send_static::<std::rc::Rc<()>>();   // must NOT compile

    // Drive the bound at runtime too, so the test is not vacuously green:
    // actually move a snapshot into a spawned thread and back.
    let snapshot = UsageSnapshot::default();
    let handle = std::thread::spawn(move || snapshot);
    let returned = handle.join().expect("probe thread joins");
    assert_eq!(returned.overview.requests, 0, "moved snapshot survives the hop");
}
