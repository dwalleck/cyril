use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use cyril_core::types::{McpRequest, McpState, McpUpdate};

use crate::state::UiState;
use crate::traits::Overlay;

use super::{McpPanelState, PanelAction, presentation};

impl UiState {
    pub fn show_mcp_panel(&mut self, state: Arc<McpState>) {
        self.mcp_panel = Some(McpPanelState::new(state));
        self.sync_mcp_geometry();
    }

    /// Pushes never open a panel. Core retains operation outcomes across close.
    pub fn refresh_mcp_panel(&mut self, state: Arc<McpState>) -> bool {
        let changed = self
            .mcp_panel
            .as_mut()
            .is_some_and(|panel| panel.refresh(state));
        if changed {
            self.sync_mcp_geometry();
        }
        changed
    }

    pub fn hide_mcp_panel(&mut self) {
        self.mcp_panel = None;
    }

    pub fn handle_mcp_key(&mut self, key: KeyEvent) -> Option<McpRequest> {
        if key.kind == KeyEventKind::Release {
            return None;
        }
        if self.topmost_overlay() != Some(Overlay::Mcp) {
            return None;
        }
        self.sync_mcp_geometry();
        if key.code != KeyCode::Esc
            && crate::render::mcp_geometry(self).is_none_or(|placed| placed.body.height == 0)
        {
            return None;
        }
        match self.mcp_panel.as_mut().and_then(|panel| panel.key(key)) {
            Some(PanelAction::Close) => {
                self.hide_mcp_panel();
                None
            }
            Some(PanelAction::Request(request)) => Some(request),
            None => None,
        }
    }

    /// Only the topmost MCP argument editor accepts paste; previews are inert.
    pub fn mcp_argument_paste(&mut self, text: &str) -> bool {
        self.topmost_overlay() == Some(Overlay::Mcp)
            && crate::render::mcp_geometry(self).is_some_and(|placed| placed.body.height > 0)
            && self
                .mcp_panel
                .as_mut()
                .is_some_and(|panel| panel.paste(text))
    }

    pub fn mcp_dispatch_failed(&mut self, message: String) {
        if let Some(panel) = &mut self.mcp_panel {
            panel.dispatch_failed(message);
        }
    }

    pub(crate) fn sync_mcp_geometry(&mut self) {
        if self.mcp_panel.is_none() {
            return;
        }
        let geometry = crate::render::mcp_geometry(self);
        if let Some(panel) = &mut self.mcp_panel {
            panel.geometry(
                geometry.map_or(1, |placed| usize::from(placed.body.width)),
                geometry.map_or(0, |placed| usize::from(placed.body.height)),
            );
        }
    }

    pub(crate) fn apply_mcp_notice(&mut self, update: &McpUpdate) -> bool {
        match update {
            McpUpdate::ElicitationDeclined(notice) => {
                self.add_system_message(presentation::elicitation_notice(notice));
                true
            }
            _ => false,
        }
    }
}
