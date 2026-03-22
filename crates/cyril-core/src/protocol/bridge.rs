use tokio::sync::mpsc;

use crate::types::event::{BridgeCommand, Notification, PermissionRequest};

/// Channel capacities
const COMMAND_CAPACITY: usize = 32;
const NOTIFICATION_CAPACITY: usize = 256;
const PERMISSION_CAPACITY: usize = 16;

/// Handle held by the App (Send side) to communicate with the ACP bridge.
pub struct BridgeHandle {
    command_tx: mpsc::Sender<BridgeCommand>,
    pub(crate) notification_rx: mpsc::Receiver<Notification>,
    pub(crate) permission_rx: mpsc::Receiver<PermissionRequest>,
}

impl BridgeHandle {
    /// Receive the next notification. Returns None if bridge is dead.
    pub async fn recv_notification(&mut self) -> Option<Notification> {
        self.notification_rx.recv().await
    }

    /// Receive the next permission request. Returns None if bridge is dead.
    pub async fn recv_permission(&mut self) -> Option<PermissionRequest> {
        self.permission_rx.recv().await
    }

    /// Get a cloneable sender for sending commands to the bridge.
    pub fn sender(&self) -> BridgeSender {
        BridgeSender {
            command_tx: self.command_tx.clone(),
        }
    }
}

/// Cloneable handle for sending commands to the bridge.
/// Can be passed to spawned tasks.
#[derive(Clone)]
pub struct BridgeSender {
    command_tx: mpsc::Sender<BridgeCommand>,
}

impl BridgeSender {
    /// Send a command to the ACP bridge. Returns Err if bridge is dead.
    pub async fn send(&self, cmd: BridgeCommand) -> crate::Result<()> {
        self.command_tx
            .send(cmd)
            .await
            .map_err(|_| crate::Error::from_kind(crate::ErrorKind::BridgeClosed))
    }
}

/// The bridge side of the channels (held by the bridge thread).
pub(crate) struct BridgeChannels {
    pub command_rx: mpsc::Receiver<BridgeCommand>,
    pub notification_tx: mpsc::Sender<Notification>,
    pub permission_tx: mpsc::Sender<PermissionRequest>,
}

/// Create a matched pair of BridgeHandle + BridgeChannels.
pub(crate) fn create_channel_pair() -> (BridgeHandle, BridgeChannels) {
    let (command_tx, command_rx) = mpsc::channel(COMMAND_CAPACITY);
    let (notification_tx, notification_rx) = mpsc::channel(NOTIFICATION_CAPACITY);
    let (permission_tx, permission_rx) = mpsc::channel(PERMISSION_CAPACITY);

    let handle = BridgeHandle {
        command_tx,
        notification_rx,
        permission_rx,
    };

    let channels = BridgeChannels {
        command_rx,
        notification_tx,
        permission_tx,
    };

    (handle, channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn send_on_closed_channel_returns_error() {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(1);
        let (_, notif_rx) = tokio::sync::mpsc::channel(1);
        let (_, perm_rx) = tokio::sync::mpsc::channel(1);

        let bridge_handle = BridgeHandle {
            command_tx: cmd_tx,
            notification_rx: notif_rx,
            permission_rx: perm_rx,
        };

        let sender = bridge_handle.sender();
        // Drop the receiver for commands (simulating bridge death)
        drop(cmd_rx);
        drop(bridge_handle);

        let result = sender.send(BridgeCommand::Shutdown).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn recv_notification_returns_none_when_sender_dropped() {
        let (mut handle, bridge_side) = create_channel_pair();
        drop(bridge_side.notification_tx);
        let result = handle.recv_notification().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn recv_permission_returns_none_when_sender_dropped() {
        let (mut handle, bridge_side) = create_channel_pair();
        drop(bridge_side.permission_tx);
        let result = handle.recv_permission().await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn notification_roundtrip() -> anyhow::Result<()> {
        let (mut handle, bridge_side) = create_channel_pair();
        let notification = Notification::TurnCompleted;
        bridge_side.notification_tx.send(notification).await?;
        let received = handle.recv_notification().await;
        assert!(matches!(received, Some(Notification::TurnCompleted)));
        Ok(())
    }

    #[tokio::test]
    async fn command_roundtrip() -> anyhow::Result<()> {
        let (handle, mut bridge_side) = create_channel_pair();
        let sender = handle.sender();
        sender.send(BridgeCommand::Shutdown).await?;
        let received = bridge_side.command_rx.recv().await;
        assert!(matches!(received, Some(BridgeCommand::Shutdown)));
        Ok(())
    }

    #[tokio::test]
    async fn sender_is_cloneable() -> anyhow::Result<()> {
        let (handle, mut bridge_side) = create_channel_pair();
        let sender1 = handle.sender();
        let sender2 = sender1.clone();
        sender1.send(BridgeCommand::CancelRequest).await?;
        sender2.send(BridgeCommand::Shutdown).await?;
        let r1 = bridge_side.command_rx.recv().await;
        let r2 = bridge_side.command_rx.recv().await;
        assert!(matches!(r1, Some(BridgeCommand::CancelRequest)));
        assert!(matches!(r2, Some(BridgeCommand::Shutdown)));
        Ok(())
    }
}
