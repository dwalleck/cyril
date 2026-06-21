//! Voice input engine for cyril (ROADMAP CN2).
//!
//! Mirrors the bridge: a dedicated OS thread running a `current_thread` tokio
//! runtime, talking to the App over the typed channels defined in
//! `cyril_core::voice`. The App holds the returned [`VoiceHandle`] and polls it
//! as a `select!` arm; this crate (and its future heavy audio/ML deps) is pulled
//! in only under the `voice` cargo feature.
//!
//! **V1a is a stub engine** — no microphone, no network. On `Start` it reports
//! `Listening` and emits an oscillating level so the meter animates; on `Stop`
//! it returns a fixed placeholder transcript. This proves the full wiring
//! end-to-end (`/voice` → engine → `select!` → `UiState::insert_text`) before
//! real capture (V1b) or a local model (V2) land. The engine never fails at
//! spawn — any future capture/transcription error arrives as `VoiceEvent::Error`.

use std::time::Duration;

use cyril_core::types::{VoiceCommand, VoiceEvent, VoiceStatus};
use cyril_core::voice::{VoiceChannels, VoiceHandle, create_voice_channels};
use tokio::sync::mpsc::error::TrySendError;

/// How often the stub emits a level sample while listening.
const LEVEL_INTERVAL: Duration = Duration::from_millis(120);

/// Radians advanced per level sample — drives the synthetic meter oscillation.
const LEVEL_STEP: f32 = 0.35;

/// Placeholder transcript the stub "produces" on stop. Clearly labelled so it
/// is never mistaken for a real transcription.
const STUB_TRANSCRIPT: &str = "[voice stub] the quick brown fox jumps over the lazy dog";

/// Spawn the voice engine on a dedicated thread and return the App-side handle.
///
/// Infallible by contract: if the thread cannot be spawned we log and return a
/// handle whose channel simply never produces events (voice is inert, but the
/// rest of the app is unaffected). This matches the bridge's fail-stop ethos —
/// degraded, observable, never a panic.
pub fn spawn_voice() -> VoiceHandle {
    let (handle, channels) = create_voice_channels();
    match std::thread::Builder::new()
        .name("cyril-voice".to_string())
        .spawn(move || run_engine(channels))
    {
        Ok(_join) => tracing::debug!("voice: stub engine thread spawned"),
        Err(e) => {
            tracing::error!(error = %e, "voice: failed to spawn engine thread; voice inert")
        }
    }
    handle
}

/// Thread entry point: stand up a `current_thread` runtime and drive the loop.
fn run_engine(channels: VoiceChannels) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::error!(error = %e, "voice: failed to build engine runtime; voice inert");
            return;
        }
    };
    rt.block_on(engine_loop(channels));
}

/// The stub engine state machine. Returns when either channel closes (the App
/// is gone), so the thread exits cleanly.
async fn engine_loop(channels: VoiceChannels) {
    let VoiceChannels {
        mut command_rx,
        event_tx,
    } = channels;

    let mut recording = false;
    let mut tick = tokio::time::interval(LEVEL_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut phase: f32 = 0.0;

    loop {
        tokio::select! {
            cmd = command_rx.recv() => {
                let Some(cmd) = cmd else {
                    tracing::debug!("voice: command channel closed; engine exiting");
                    return;
                };
                let events = match cmd {
                    VoiceCommand::Start => {
                        recording = true;
                        phase = 0.0;
                        vec![VoiceEvent::Status(VoiceStatus::Listening)]
                    }
                    VoiceCommand::Stop if recording => {
                        recording = false;
                        // Real engines do capture→POST here; the stub returns a
                        // fixed transcript so the insert path is exercised.
                        vec![
                            VoiceEvent::Status(VoiceStatus::Transcribing),
                            VoiceEvent::Transcript(STUB_TRANSCRIPT.to_string()),
                            VoiceEvent::Status(VoiceStatus::Idle),
                        ]
                    }
                    // Stop while not recording is a no-op.
                    VoiceCommand::Stop => Vec::new(),
                    VoiceCommand::Cancel => {
                        recording = false;
                        vec![VoiceEvent::Status(VoiceStatus::Idle)]
                    }
                };
                for ev in events {
                    if event_tx.send(ev).await.is_err() {
                        tracing::debug!("voice: event channel closed; engine exiting");
                        return;
                    }
                }
            }
            _ = tick.tick(), if recording => {
                phase += LEVEL_STEP;
                // Map sin(-1..1) into 0..1 for a believable bouncing meter.
                let level = (phase.sin() * 0.5 + 0.5).clamp(0.0, 1.0);
                // Level samples are a lossy meter: drop them under backpressure
                // rather than blocking on `send().await`. Blocking here would
                // stall the whole loop, so a Stop/Cancel could not be processed
                // until the App drained the event queue — capture would hang.
                match event_tx.try_send(VoiceEvent::Level(level)) {
                    Ok(()) | Err(TrySendError::Full(_)) => {}
                    Err(TrySendError::Closed(_)) => {
                        tracing::debug!("voice: event channel closed; engine exiting");
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests legitimately panic on failure; .expect() messages double as assertion context"
)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn start_emits_listening_then_levels() {
        let mut handle = spawn_voice();
        handle
            .try_send_command(VoiceCommand::Start)
            .expect("engine accepts start");

        assert_eq!(
            handle.recv_event().await,
            Some(VoiceEvent::Status(VoiceStatus::Listening)),
        );
        match handle.recv_event().await {
            Some(VoiceEvent::Level(l)) => assert!((0.0..=1.0).contains(&l)),
            other => panic!("expected a level sample, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn stop_emits_transcript_and_returns_to_idle() {
        let mut handle = spawn_voice();
        handle
            .try_send_command(VoiceCommand::Start)
            .expect("engine accepts start");
        handle
            .try_send_command(VoiceCommand::Stop)
            .expect("engine accepts stop");

        // Level ticks may interleave before Stop is processed; drain until the
        // transcript arrives (bounded so a regression fails instead of hanging).
        let mut transcript = None;
        for _ in 0..64 {
            match handle.recv_event().await {
                Some(VoiceEvent::Transcript(t)) => {
                    transcript = Some(t);
                    break;
                }
                Some(_) => continue,
                None => break,
            }
        }
        assert_eq!(transcript.as_deref(), Some(STUB_TRANSCRIPT));
        assert_eq!(
            handle.recv_event().await,
            Some(VoiceEvent::Status(VoiceStatus::Idle)),
        );
    }

    #[tokio::test]
    async fn cancel_returns_to_idle_without_transcript() {
        let mut handle = spawn_voice();
        handle
            .try_send_command(VoiceCommand::Start)
            .expect("engine accepts start");
        handle
            .try_send_command(VoiceCommand::Cancel)
            .expect("engine accepts cancel");

        let mut saw_idle = false;
        for _ in 0..64 {
            match handle.recv_event().await {
                Some(VoiceEvent::Transcript(_)) => panic!("cancel must not transcribe"),
                Some(VoiceEvent::Status(VoiceStatus::Idle)) => {
                    saw_idle = true;
                    break;
                }
                Some(_) => continue,
                None => break,
            }
        }
        assert!(saw_idle, "cancel should return the engine to idle");
    }
}
