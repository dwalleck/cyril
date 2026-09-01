use super::*;

trait ExpectContract<T> {
    fn expect_contract(self, message: &str) -> T;
}

impl<T> ExpectContract<T> for Option<T> {
    fn expect_contract(self, message: &str) -> T {
        match self {
            Some(value) => value,
            None => panic!("{message}"),
        }
    }
}

impl<T, E: std::fmt::Debug> ExpectContract<T> for Result<T, E> {
    fn expect_contract(self, message: &str) -> T {
        match self {
            Ok(value) => value,
            Err(error) => panic!("{message}: {error:?}"),
        }
    }
}

trait ExpectErrContract<E> {
    fn expect_err_contract(self, message: &str) -> E;
}

impl<T: std::fmt::Debug, E> ExpectErrContract<E> for Result<T, E> {
    fn expect_err_contract(self, message: &str) -> E {
        match self {
            Ok(value) => panic!("{message}: got {value:?}"),
            Err(error) => error,
        }
    }
}

// C5 freezes the default-engine dispatch surface; `kas` builds route several
// of these commands (hooks, workflow) through different `run_loop` arms with
// different outcomes, so the module only exists in default builds. The
// default-features CI lane is what keeps it running.
#[cfg(not(feature = "kas"))]
mod commands;
mod death;
mod fingerprint_stops;
mod lifecycle;
mod routing;
mod saturation;
mod stall;

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
