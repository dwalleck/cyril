use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use agent_client_protocol as acp;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::capabilities;
use crate::capabilities::terminal::{TerminalId, TerminalManager};
use crate::event::{AppEvent, KiroExtCommand};
use crate::hooks::{HookContext, HookRegistry, HookResult, HookTarget, HookTiming};
use crate::path;

/// Construct an ACP internal error with a message.
fn internal_err(msg: impl Into<String>) -> acp::Error {
    acp::Error::new(-32603, msg)
}

/// Payload for `kiro.dev/commands/available` ext_notification.
/// We try multiple shapes since the format isn't documented.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum KiroCommandsPayload {
    /// `{ "commands": [...] }`
    Wrapped { commands: Vec<KiroExtCommand> },
    /// `{ "availableCommands": [...] }` (ACP-style)
    AcpStyle {
        #[serde(rename = "availableCommands")]
        commands: Vec<KiroExtCommand>,
    },
    /// Top-level array `[...]`
    Bare(Vec<KiroExtCommand>),
}

impl KiroCommandsPayload {
    fn commands(self) -> Vec<KiroExtCommand> {
        match self {
            Self::Wrapped { commands } => commands,
            Self::AcpStyle { commands } => commands,
            Self::Bare(commands) => commands,
        }
    }
}

/// The central ACP Client implementation.
/// Handles all agent callbacks: fs, terminal, permissions, notifications.
///
/// Uses `Rc<RefCell<_>>` for interior mutability since everything is `!Send`
/// (required by `#[async_trait(?Send)]` on the ACP `Client` trait).
pub struct KiroClient {
    event_tx: mpsc::UnboundedSender<AppEvent>,
    terminal_manager: RefCell<TerminalManager>,
    hooks: RefCell<HookRegistry>,
}

impl KiroClient {
    pub fn new(
        event_tx: mpsc::UnboundedSender<AppEvent>,
        hooks: HookRegistry,
    ) -> Rc<Self> {
        Rc::new(Self {
            event_tx,
            terminal_manager: RefCell::new(TerminalManager::new()),
            hooks: RefCell::new(hooks),
        })
    }

    /// Send an event to the TUI, logging if the receiver has been dropped.
    fn emit(&self, event: AppEvent) {
        if self.event_tx.send(event).is_err() {
            tracing::error!("Event channel closed — TUI receiver is gone, events are being dropped");
        }
    }
}

#[async_trait(?Send)]
impl acp::Client for KiroClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        let (tx, rx) = oneshot::channel();
        self.emit(AppEvent::PermissionRequest {
            request: args,
            responder: tx,
        });

        rx.await.map_err(|_| internal_err("Permission request channel closed"))
    }

    async fn session_notification(
        &self,
        args: acp::SessionNotification,
    ) -> acp::Result<()> {
        match args.update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                self.emit(AppEvent::AgentMessage {
                    session_id: args.session_id,
                    chunk,
                });
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                self.emit(AppEvent::AgentThought {
                    session_id: args.session_id,
                    chunk,
                });
            }
            acp::SessionUpdate::ToolCall(tool_call) => {
                self.emit(AppEvent::ToolCallStarted {
                    session_id: args.session_id,
                    tool_call,
                });
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                self.emit(AppEvent::ToolCallUpdated {
                    session_id: args.session_id,
                    update,
                });
            }
            acp::SessionUpdate::Plan(plan) => {
                self.emit(AppEvent::PlanUpdated {
                    session_id: args.session_id,
                    plan,
                });
            }
            acp::SessionUpdate::AvailableCommandsUpdate(commands) => {
                self.emit(AppEvent::CommandsUpdated {
                    session_id: args.session_id,
                    commands,
                });
            }
            acp::SessionUpdate::CurrentModeUpdate(mode) => {
                self.emit(AppEvent::ModeChanged {
                    session_id: args.session_id,
                    mode,
                });
            }
            acp::SessionUpdate::ConfigOptionUpdate(update) => {
                self.emit(AppEvent::ConfigOptionsUpdated {
                    session_id: args.session_id,
                    config_options: update.config_options,
                });
            }
            _ => {
                tracing::debug!("Unhandled session notification variant");
            }
        }
        Ok(())
    }

    async fn ext_notification(&self, args: acp::ExtNotification) -> acp::Result<()> {
        tracing::info!("Received ext_notification: method={}", args.method);
        tracing::info!("ext_notification params: {}", args.params);

        if args.method.as_ref() == "kiro.dev/commands/available" {
            match serde_json::from_str::<KiroCommandsPayload>(args.params.get()) {
                Ok(payload) => {
                    let commands = payload.commands();
                    tracing::info!(
                        "Parsed {} Kiro commands from ext_notification",
                        commands.len()
                    );
                    self.emit(AppEvent::KiroCommandsAvailable { commands });
                }
                Err(e) => {
                    tracing::warn!("Failed to parse kiro.dev/commands/available: {e}");
                }
            }
        } else if args.method.as_ref() == "kiro.dev/metadata" {
            // Log the full raw payload so we can discover all available fields
            tracing::info!("kiro.dev/metadata raw: {}", args.params.get());

            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct MetadataPayload {
                session_id: String,
                #[serde(default)]
                context_usage_percentage: f64,
            }
            match serde_json::from_str::<MetadataPayload>(args.params.get()) {
                Ok(payload) => {
                    self.emit(AppEvent::KiroMetadata {
                        session_id: payload.session_id,
                        context_usage_pct: payload.context_usage_percentage,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to parse kiro.dev/metadata: {e}");
                }
            }
        }

        Ok(())
    }

    async fn read_text_file(
        &self,
        args: acp::ReadTextFileRequest,
    ) -> acp::Result<acp::ReadTextFileResponse> {
        let native_path = path::to_native(&args.path);
        tracing::info!("fs.readTextFile: {} -> {}", args.path.display(), native_path.display());

        let hook_ctx = HookContext {
            target: HookTarget::FsRead,
            timing: HookTiming::Before,
            path: Some(native_path.clone()),
            content: None,
            command: None,
        };
        if let HookResult::Blocked { reason } = self.hooks.borrow().run_before(&hook_ctx).await {
            return Err(internal_err(reason));
        }

        let content = capabilities::fs::read_text_file(&native_path)
            .await
            .map_err(|e| internal_err(e.to_string()))?;

        Ok(acp::ReadTextFileResponse::new(content))
    }

    async fn write_text_file(
        &self,
        args: acp::WriteTextFileRequest,
    ) -> acp::Result<acp::WriteTextFileResponse> {
        let native_path = path::to_native(&args.path);
        tracing::info!("fs.writeTextFile: {} -> {}", args.path.display(), native_path.display());

        let mut content = args.content.clone();

        let hook_ctx = HookContext {
            target: HookTarget::FsWrite,
            timing: HookTiming::Before,
            path: Some(native_path.clone()),
            content: Some(content.clone()),
            command: None,
        };
        match self.hooks.borrow().run_before(&hook_ctx).await {
            HookResult::Blocked { reason } => return Err(internal_err(reason)),
            HookResult::ModifiedArgs { content: Some(c), .. } => content = c,
            _ => {}
        }

        capabilities::fs::write_text_file(&native_path, &content)
            .await
            .map_err(|e| internal_err(e.to_string()))?;

        // Run after hooks
        let after_ctx = HookContext {
            target: HookTarget::FsWrite,
            timing: HookTiming::After,
            path: Some(native_path),
            content: Some(content),
            command: None,
        };
        let after_results = self.hooks.borrow().run_after(&after_ctx).await;
        for result in after_results {
            if let HookResult::FeedbackPrompt { text } = result {
                self.emit(AppEvent::HookFeedback { text });
            }
        }

        Ok(acp::WriteTextFileResponse::new())
    }

    async fn create_terminal(
        &self,
        args: acp::CreateTerminalRequest,
    ) -> acp::Result<acp::CreateTerminalResponse> {
        let command = args.command.clone();
        tracing::info!("terminal.create: {command}");

        let hook_ctx = HookContext {
            target: HookTarget::Terminal,
            timing: HookTiming::Before,
            path: None,
            content: None,
            command: Some(command.clone()),
        };
        if let HookResult::Blocked { reason } = self.hooks.borrow().run_before(&hook_ctx).await {
            return Err(internal_err(reason));
        }

        let id = self
            .terminal_manager
            .borrow_mut()
            .create_terminal(&command)
            .map_err(|e| internal_err(e.to_string()))?;

        Ok(acp::CreateTerminalResponse::new(id.to_string()))
    }

    async fn terminal_output(
        &self,
        args: acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        let id = TerminalId::from_str(&args.terminal_id.to_string())
            .map_err(|e| internal_err(format!("Invalid terminal ID: {e}")))?;

        let output = self
            .terminal_manager
            .borrow_mut()
            .get_output(&id)
            .map_err(|e| internal_err(e.to_string()))?;

        Ok(acp::TerminalOutputResponse::new(output, false))
    }

    async fn wait_for_terminal_exit(
        &self,
        args: acp::WaitForTerminalExitRequest,
    ) -> acp::Result<acp::WaitForTerminalExitResponse> {
        let id = TerminalId::from_str(&args.terminal_id.to_string())
            .map_err(|e| internal_err(format!("Invalid terminal ID: {e}")))?;

        let exit_code = self
            .terminal_manager
            .borrow_mut()
            .wait_for_exit(&id)
            .await
            .map_err(|e| internal_err(e.to_string()))?;

        let exit_status = acp::TerminalExitStatus::new()
            .exit_code(exit_code.max(0) as u32);

        Ok(acp::WaitForTerminalExitResponse::new(exit_status))
    }

    async fn release_terminal(
        &self,
        args: acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        let id = TerminalId::from_str(&args.terminal_id.to_string())
            .map_err(|e| internal_err(format!("Invalid terminal ID: {e}")))?;

        self.terminal_manager
            .borrow_mut()
            .release(&id)
            .map_err(|e| internal_err(e.to_string()))?;

        Ok(acp::ReleaseTerminalResponse::new())
    }

    async fn kill_terminal_command(
        &self,
        args: acp::KillTerminalCommandRequest,
    ) -> acp::Result<acp::KillTerminalCommandResponse> {
        let id = TerminalId::from_str(&args.terminal_id.to_string())
            .map_err(|e| internal_err(format!("Invalid terminal ID: {e}")))?;

        self.terminal_manager
            .borrow_mut()
            .kill(&id)
            .await
            .map_err(|e| internal_err(e.to_string()))?;

        Ok(acp::KillTerminalCommandResponse::new())
    }
}
