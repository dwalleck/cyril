use std::cell::RefCell;
use std::rc::Rc;

use agent_client_protocol as acp;
use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::capabilities;
use crate::capabilities::terminal::TerminalManager;
use crate::event::AppEvent;
use crate::hooks::{HookContext, HookRegistry, HookResult, HookTarget, HookTiming};
use crate::path;

/// Construct an ACP internal error with a message.
fn internal_err(msg: impl Into<String>) -> acp::Error {
    acp::Error::new(-32603, msg)
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
}

#[async_trait(?Send)]
impl acp::Client for KiroClient {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> acp::Result<acp::RequestPermissionResponse> {
        let (tx, rx) = oneshot::channel();
        let _ = self.event_tx.send(AppEvent::PermissionRequest {
            request: args,
            responder: tx,
        });

        rx.await.map_err(|_| internal_err("Permission request channel closed"))
    }

    async fn session_notification(
        &self,
        args: acp::SessionNotification,
    ) -> acp::Result<()> {
        let session_id = args.session_id.clone();
        match args.update {
            acp::SessionUpdate::AgentMessageChunk(chunk) => {
                let _ = self.event_tx.send(AppEvent::AgentMessage {
                    session_id,
                    chunk,
                });
            }
            acp::SessionUpdate::AgentThoughtChunk(chunk) => {
                let _ = self.event_tx.send(AppEvent::AgentThought {
                    session_id,
                    chunk,
                });
            }
            acp::SessionUpdate::ToolCall(tool_call) => {
                let _ = self.event_tx.send(AppEvent::ToolCallStarted {
                    session_id,
                    tool_call,
                });
            }
            acp::SessionUpdate::ToolCallUpdate(update) => {
                let _ = self.event_tx.send(AppEvent::ToolCallUpdated {
                    session_id,
                    update,
                });
            }
            acp::SessionUpdate::Plan(plan) => {
                let _ = self.event_tx.send(AppEvent::PlanUpdated {
                    session_id,
                    plan,
                });
            }
            acp::SessionUpdate::AvailableCommandsUpdate(commands) => {
                let _ = self.event_tx.send(AppEvent::CommandsUpdated {
                    session_id,
                    commands,
                });
            }
            acp::SessionUpdate::CurrentModeUpdate(mode) => {
                let _ = self.event_tx.send(AppEvent::ModeChanged {
                    session_id,
                    mode,
                });
            }
            _ => {
                tracing::debug!("Unhandled session notification variant");
            }
        }
        Ok(())
    }

    async fn read_text_file(
        &self,
        args: acp::ReadTextFileRequest,
    ) -> acp::Result<acp::ReadTextFileResponse> {
        let win_path = path::wsl_to_win(&args.path.to_string_lossy());
        tracing::info!("fs.readTextFile: {} -> {}", args.path.display(), win_path.display());

        let hook_ctx = HookContext {
            target: HookTarget::FsRead,
            timing: HookTiming::Before,
            path: Some(win_path.clone()),
            content: None,
            command: None,
        };
        if let HookResult::Blocked { reason } = self.hooks.borrow().run_before(&hook_ctx).await {
            return Err(internal_err(reason));
        }

        let content = capabilities::fs::read_text_file(&win_path)
            .await
            .map_err(|e| internal_err(e.to_string()))?;

        Ok(acp::ReadTextFileResponse::new(content))
    }

    async fn write_text_file(
        &self,
        args: acp::WriteTextFileRequest,
    ) -> acp::Result<acp::WriteTextFileResponse> {
        let win_path = path::wsl_to_win(&args.path.to_string_lossy());
        tracing::info!("fs.writeTextFile: {} -> {}", args.path.display(), win_path.display());

        let mut content = args.content.clone();

        let hook_ctx = HookContext {
            target: HookTarget::FsWrite,
            timing: HookTiming::Before,
            path: Some(win_path.clone()),
            content: Some(content.clone()),
            command: None,
        };
        match self.hooks.borrow().run_before(&hook_ctx).await {
            HookResult::Blocked { reason } => {
                return Err(internal_err(reason));
            }
            HookResult::ModifiedArgs { content: Some(new_content) } => {
                content = new_content;
            }
            _ => {}
        }

        capabilities::fs::write_text_file(&win_path, &content)
            .await
            .map_err(|e| internal_err(e.to_string()))?;

        // Run after hooks
        let after_ctx = HookContext {
            target: HookTarget::FsWrite,
            timing: HookTiming::After,
            path: Some(win_path),
            content: Some(content),
            command: None,
        };
        let after_results = self.hooks.borrow().run_after(&after_ctx).await;
        for result in after_results {
            if let HookResult::FeedbackPrompt { text } = result {
                let _ = self.event_tx.send(AppEvent::HookFeedback { text });
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

        Ok(acp::CreateTerminalResponse::new(id))
    }

    async fn terminal_output(
        &self,
        args: acp::TerminalOutputRequest,
    ) -> acp::Result<acp::TerminalOutputResponse> {
        let output = self
            .terminal_manager
            .borrow_mut()
            .get_output(&args.terminal_id.to_string())
            .map_err(|e| internal_err(e.to_string()))?;

        Ok(acp::TerminalOutputResponse::new(output, false))
    }

    async fn wait_for_terminal_exit(
        &self,
        args: acp::WaitForTerminalExitRequest,
    ) -> acp::Result<acp::WaitForTerminalExitResponse> {
        let exit_code = self
            .terminal_manager
            .borrow_mut()
            .wait_for_exit(&args.terminal_id.to_string())
            .await
            .map_err(|e| internal_err(e.to_string()))?;

        let exit_status = acp::TerminalExitStatus::new()
            .exit_code(exit_code as u32);

        Ok(acp::WaitForTerminalExitResponse::new(exit_status))
    }

    async fn release_terminal(
        &self,
        args: acp::ReleaseTerminalRequest,
    ) -> acp::Result<acp::ReleaseTerminalResponse> {
        self.terminal_manager
            .borrow_mut()
            .release(&args.terminal_id.to_string())
            .map_err(|e| internal_err(e.to_string()))?;

        Ok(acp::ReleaseTerminalResponse::new())
    }

    async fn kill_terminal_command(
        &self,
        args: acp::KillTerminalCommandRequest,
    ) -> acp::Result<acp::KillTerminalCommandResponse> {
        self.terminal_manager
            .borrow_mut()
            .kill(&args.terminal_id.to_string())
            .await
            .map_err(|e| internal_err(e.to_string()))?;

        Ok(acp::KillTerminalCommandResponse::new())
    }
}
