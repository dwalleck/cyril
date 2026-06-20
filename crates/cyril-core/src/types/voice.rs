//! Voice-input control-plane types (ROADMAP CN2 / V1a).
//!
//! These are the data types that cross the App ↔ voice-engine boundary. They
//! live in `cyril-core` (not the `cyril-voice` engine crate) so the App's voice
//! field and `select!` arm compile whether or not the `voice` feature is on —
//! only the engine (and its heavy audio/ML deps) is feature-gated. This mirrors
//! how `BridgeCommand`/`Notification` live here while the bridge engine lives in
//! `protocol/bridge.rs`.

/// What the voice subsystem is currently doing. Drives the meter/indicator in
/// the UI and the `/voice` toggle direction. `Idle` means "not capturing".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VoiceStatus {
    /// Not capturing. The default and resting state.
    #[default]
    Idle,
    /// Capturing audio from the microphone.
    Listening,
    /// Capture stopped; transcription in flight (e.g. the remote POST).
    Transcribing,
}

/// A command sent *to* the voice engine thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceCommand {
    /// Begin capturing audio.
    Start,
    /// Stop capturing and transcribe what was captured.
    Stop,
    /// Abandon the in-progress capture without transcribing.
    Cancel,
}

/// An event emitted *from* the voice engine thread.
///
/// The engine never fails at spawn time — any capture/transcription failure
/// arrives here as [`VoiceEvent::Error`] so the App can recover and surface it.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceEvent {
    /// Input level in `0.0..=1.0`, emitted periodically while listening so the
    /// UI can animate a meter.
    Level(f32),
    /// The engine's status changed.
    Status(VoiceStatus),
    /// A final transcript ready to insert into the input buffer.
    Transcript(String),
    /// A capture or transcription error, already human-readable.
    Error(String),
}

/// Errors the voice engine can produce. Kept minimal for V1a (the stub engine
/// cannot fail); real capture/transcription variants land with V1b.
#[derive(Debug, thiserror::Error)]
pub enum VoiceError {
    /// The command queue to the engine is full — the engine is alive but
    /// backpressured. Transient: the caller should retry, not treat it as fatal.
    #[error("voice command queue full")]
    Busy,
    /// A voice channel closed — the engine thread (or the App) is gone.
    #[error("voice channel closed")]
    ChannelClosed,
}
