use super::*;

// C5 freezes the default-engine dispatch surface; `kas` builds route several
// of these commands (hooks, workflow) through different `run_loop` arms with
// different outcomes, so the module only exists in default builds. The
// default-features CI lane is what keeps it running.
#[cfg(not(feature = "kas"))]
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
