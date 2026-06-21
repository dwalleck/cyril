//! Voice-engine channel plumbing (ROADMAP CN2 / V1a).
//!
//! The lightweight half of the voice subsystem: the typed channels between the
//! App and the voice engine thread. The engine itself (audio capture + STT,
//! with its heavy deps) lives in the separate `cyril-voice` crate behind the
//! `voice` cargo feature; it builds its thread around [`VoiceChannels`] and
//! hands the App a [`VoiceHandle`]. Keeping the handle here (always compiled)
//! lets the App hold an `Option<VoiceHandle>` and poll it in `select!` without
//! any `#[cfg]` at the call sites — same split as the bridge.

use tokio::sync::mpsc;

pub use crate::types::voice::{VoiceCommand, VoiceError, VoiceEvent, VoiceStatus};

/// Bound on queued commands to the engine. Commands are rare (one per
/// `/voice` toggle), so a small buffer is plenty.
const COMMAND_CAPACITY: usize = 8;

/// Bound on queued events from the engine. Level ticks are frequent while
/// listening; a modest buffer absorbs bursts without unbounded growth.
const EVENT_CAPACITY: usize = 64;

/// The App-side handle: send [`VoiceCommand`]s in, receive [`VoiceEvent`]s out.
pub struct VoiceHandle {
    command_tx: mpsc::Sender<VoiceCommand>,
    event_rx: mpsc::Receiver<VoiceEvent>,
}

impl VoiceHandle {
    /// Send a command to the engine without blocking. Returns
    /// [`VoiceError::ChannelClosed`] if the engine thread is gone, or
    /// [`VoiceError::Busy`] if its queue is momentarily full (alive but
    /// backpressured) — distinguishing the two lets the caller report and
    /// recover accurately instead of declaring a live engine dead.
    pub fn try_send_command(&self, cmd: VoiceCommand) -> Result<(), VoiceError> {
        use tokio::sync::mpsc::error::TrySendError;
        self.command_tx.try_send(cmd).map_err(|e| match e {
            TrySendError::Full(_) => VoiceError::Busy,
            TrySendError::Closed(_) => VoiceError::ChannelClosed,
        })
    }

    /// Await the next event from the engine. `None` means the engine thread has
    /// exited and the channel is closed.
    pub async fn recv_event(&mut self) -> Option<VoiceEvent> {
        self.event_rx.recv().await
    }
}

/// The engine-side channels (held by the voice thread).
pub struct VoiceChannels {
    /// Commands arriving from the App.
    pub command_rx: mpsc::Receiver<VoiceCommand>,
    /// Events to push back to the App.
    pub event_tx: mpsc::Sender<VoiceEvent>,
}

/// Create a connected [`VoiceHandle`] / [`VoiceChannels`] pair. The engine crate
/// calls this, spawns its thread around the channels, and returns the handle.
pub fn create_voice_channels() -> (VoiceHandle, VoiceChannels) {
    let (command_tx, command_rx) = mpsc::channel(COMMAND_CAPACITY);
    let (event_tx, event_rx) = mpsc::channel(EVENT_CAPACITY);
    let handle = VoiceHandle {
        command_tx,
        event_rx,
    };
    let channels = VoiceChannels {
        command_rx,
        event_tx,
    };
    (handle, channels)
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests legitimately panic on failure; .expect() messages double as assertion context"
)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn command_and_event_round_trip() {
        let (handle, mut channels) = create_voice_channels();

        handle
            .try_send_command(VoiceCommand::Start)
            .expect("send should succeed while engine side is alive");
        assert_eq!(channels.command_rx.recv().await, Some(VoiceCommand::Start));

        channels
            .event_tx
            .send(VoiceEvent::Status(VoiceStatus::Listening))
            .await
            .expect("send should succeed while handle is alive");
        // recv_event needs &mut; rebind the handle.
        let mut handle = handle;
        assert_eq!(
            handle.recv_event().await,
            Some(VoiceEvent::Status(VoiceStatus::Listening))
        );
    }

    #[tokio::test]
    async fn try_send_after_engine_drops_reports_closed() {
        let (handle, channels) = create_voice_channels();
        drop(channels);
        assert!(matches!(
            handle.try_send_command(VoiceCommand::Stop),
            Err(VoiceError::ChannelClosed)
        ));
    }

    #[tokio::test]
    async fn try_send_when_queue_full_reports_busy_not_closed() {
        // Keep the engine side alive but never drain it, then overflow the
        // command queue: a backpressured-but-live engine must report Busy, not
        // ChannelClosed, so the caller retries instead of declaring it dead.
        let (handle, _channels) = create_voice_channels();
        for _ in 0..COMMAND_CAPACITY {
            handle
                .try_send_command(VoiceCommand::Start)
                .expect("sends within capacity succeed");
        }
        assert!(matches!(
            handle.try_send_command(VoiceCommand::Start),
            Err(VoiceError::Busy)
        ));
    }
}
