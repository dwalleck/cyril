use super::*;

mod commands;
mod routing;
mod saturation;

fn command_name(command: &BridgeCommand) -> &'static str {
    match command {
        BridgeCommand::SendPrompt { .. } => "SendPrompt",
        BridgeCommand::NewSession { .. } => "NewSession",
        BridgeCommand::LoadSession { .. } => "LoadSession",
        BridgeCommand::CancelRequest => "CancelRequest",
        BridgeCommand::SetMode { .. } => "SetMode",
        BridgeCommand::SetModel { .. } => "SetModel",
        BridgeCommand::ExtMethod { .. } => "ExtMethod",
        BridgeCommand::ListSettings => "ListSettings",
        BridgeCommand::QueryUsageAccount => "QueryUsageAccount",
        BridgeCommand::QueryCommandOptions { .. } => "QueryCommandOptions",
        BridgeCommand::ExecuteCommand { .. } => "ExecuteCommand",
        BridgeCommand::SpawnSession { .. } => "SpawnSession",
        BridgeCommand::TerminateSession { .. } => "TerminateSession",
        BridgeCommand::SendMessage { .. } => "SendMessage",
        BridgeCommand::SteerSession { .. } => "SteerSession",
        BridgeCommand::ClearSteering { .. } => "ClearSteering",
        BridgeCommand::ListKasHooks { .. } => "ListKasHooks",
        BridgeCommand::Workflow { .. } => "Workflow",
        BridgeCommand::SetKasHookEnabled { .. } => "SetKasHookEnabled",
        BridgeCommand::Shutdown => "Shutdown",
    }
}

async fn next_notification(
    cell: &str,
    rx: &mut mpsc::Receiver<RoutedNotification>,
) -> Notification {
    recv_notif(rx, 5)
        .await
        .unwrap_or_else(|| panic!("C5 {cell}: expected one typed notification"))
}

async fn assert_notification_quiet(cell: &str, rx: &mut mpsc::Receiver<RoutedNotification>) {
    assert!(
        tokio::time::timeout(Duration::from_millis(25), rx.recv())
            .await
            .is_err(),
        "C5 {cell}: command promised no immediate notification"
    );
}

fn assert_bridge_error(cell: &str, notification: &Notification, operation: &str) {
    assert!(
        matches!(notification, Notification::BridgeError { operation: actual, .. } if actual == operation),
        "C5 {cell}: expected BridgeError operation {operation:?}, got {notification:?}"
    );
}
