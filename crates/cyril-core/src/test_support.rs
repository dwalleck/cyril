//! Shared test scaffolding for cyril-core's test modules and — behind the
//! `test-support` feature — the test suites of downstream crates.
//!
//! Nothing here is production code: the module compiles only under
//! `cfg(test)` or when a dependent crate enables the `test-support` feature —
//! intended from `[dev-dependencies]` only; a `[dependencies]` entry could
//! technically enable it too (cargo features are additive), so treat any
//! non-dev enablement as a review error.

use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
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

/// Creates a structural event capture subscriber while holding the shared
/// capture lock. `register_callsite` returns `always`, avoiding tracing's
/// process-global interest cache when parallel tests use different dispatches.
pub fn capture_json_subscriber() -> (MutexGuard<'static, ()>, JsonEventCapture, tracing::Dispatch) {
    let guard = tracing_capture_lock();
    let capture = JsonEventCapture::default();
    let subscriber = JsonEventSubscriber {
        capture: capture.clone(),
        next_span_id: AtomicU64::new(1),
    };
    (guard, capture, tracing::Dispatch::new(subscriber))
}

#[derive(Clone, Default)]
pub struct JsonEventCapture(Arc<Mutex<Vec<serde_json::Value>>>);

impl JsonEventCapture {
    pub fn captured(&self) -> Vec<serde_json::Value> {
        match self.0.lock() {
            Ok(captured) => captured.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

struct JsonEventSubscriber {
    capture: JsonEventCapture,
    next_span_id: AtomicU64,
}

impl tracing::Subscriber for JsonEventSubscriber {
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }

    fn register_callsite(
        &self,
        _metadata: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::always()
    }

    fn max_level_hint(&self) -> Option<tracing::metadata::LevelFilter> {
        Some(tracing::metadata::LevelFilter::TRACE)
    }

    fn new_span(&self, _attributes: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(self.next_span_id.fetch_add(1, Ordering::Relaxed))
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut visitor = JsonEventVisitor::default();
        event.record(&mut visitor);
        let captured = serde_json::json!({
            "level": event.metadata().level().as_str(),
            "fields": visitor.fields,
        });
        match self.capture.0.lock() {
            Ok(mut events) => events.push(captured),
            Err(poisoned) => poisoned.into_inner().push(captured),
        }
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}

#[derive(Default)]
struct JsonEventVisitor {
    fields: serde_json::Map<String, serde_json::Value>,
}

impl tracing::field::Visit for JsonEventVisitor {
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        let value = serde_json::Number::from_f64(value)
            .map_or(serde_json::Value::Null, serde_json::Value::Number);
        self.fields.insert(field.name().to_owned(), value);
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.fields
            .insert(field.name().to_owned(), serde_json::Value::from(value));
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.fields
            .insert(field.name().to_owned(), serde_json::Value::from(value));
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.fields
            .insert(field.name().to_owned(), serde_json::Value::from(value));
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), serde_json::Value::from(value));
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        self.record_str(field, &value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.fields.insert(
            field.name().to_owned(),
            serde_json::Value::from(format!("{value:?}")),
        );
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

/// Replays a raw KAS JSONL capture through the SAME conversion path the live
/// bridge uses (cyril-jxfu C6): `session/update` params deserialize at the
/// acp layer and convert via `KasEngine::convert_session_update`; extension
/// notifications convert via `KasEngine::convert_ext_notification` after the
/// acp layer's leading-underscore strip. Returns the forwarded notifications
/// as `(scope, notification)` pairs — `Some(sid)` for the session/update
/// envelope scope, `None` for global extension frames — mirroring the
/// `RoutedNotification` split in `client.rs`. Unconvertible frames drop
/// exactly as production drops them (`Ok(None)` / warned `Err`); client
/// requests and responses in the capture are skipped.
#[cfg(feature = "kas")]
pub fn kas_capture_to_routed(
    capture: &str,
) -> Vec<(Option<crate::types::SessionId>, crate::types::Notification)> {
    use agent_client_protocol as acp;

    use crate::protocol::engine::{Engine, KasEngine};

    let engine = KasEngine::default();
    let mut forwarded = Vec::new();
    for line in capture.lines().filter(|line| !line.is_empty()) {
        let frame: serde_json::Value =
            must_succeed(serde_json::from_str(line), "capture line is valid JSON");
        if frame.get("id").is_some() {
            // A request (agent→client, e.g. _kiro/auth/getAccessToken) or a
            // response to one of cyril's own — either way not a notification.
            continue;
        }
        let Some(method) = frame.get("method").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if method == "session/update" {
            let args: acp::SessionNotification = must_succeed(
                serde_json::from_value(frame["params"].clone()),
                "session/update params deserialize at the acp layer",
            );
            let session_id = crate::types::SessionId::new(args.session_id.to_string());
            if let Some(notification) = engine.convert_session_update(&args) {
                forwarded.push((Some(session_id), notification));
            }
            continue;
        }
        let Some(normalized) = method.strip_prefix('_') else {
            continue; // not an extension notification
        };
        match engine.convert_ext_notification(normalized, &frame["params"]) {
            Ok(Some(notification)) => forwarded.push((None, notification)),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, method, "malformed extension notification in capture");
            }
        }
    }
    forwarded
}
