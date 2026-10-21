//! Session-bound MCP inspection and read-only interaction. No bridge or wire types.

mod arguments;
mod integration;
mod presentation;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cyril_core::types::{
    McpCatalog, McpOperation, McpPromptArgument, McpRequest, McpServer, McpServerStatus, McpState,
    SessionId,
};

use arguments::ArgumentValue;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Servers,
    Tools,
    Prompts,
    Resources,
    Templates,
}

impl Tab {
    fn step(self, forward: bool) -> Self {
        match (self, forward) {
            (Self::Servers, true) | (Self::Prompts, false) => Self::Tools,
            (Self::Tools, true) | (Self::Resources, false) => Self::Prompts,
            (Self::Prompts, true) | (Self::Templates, false) => Self::Resources,
            (Self::Resources, true) | (Self::Servers, false) => Self::Templates,
            (Self::Templates, true) | (Self::Tools, false) => Self::Servers,
        }
    }
}

#[derive(Debug, Clone)]
struct Field {
    argument: McpPromptArgument,
    value: ArgumentValue,
}

#[derive(Debug, Clone)]
struct ArgumentForm {
    session_id: SessionId,
    server_name: String,
    prompt_name: String,
    prompt_title: Option<String>,
    fields: Vec<Field>,
    focused: usize,
}

#[derive(Debug, Clone, Default)]
enum Page {
    #[default]
    Catalog,
    Arguments(ArgumentForm),
    Read(Arc<McpRequest>),
}

#[derive(Debug, Clone)]
pub(crate) struct Caret {
    pub(crate) row: usize,
    pub(crate) column: usize,
    pub(crate) bytes: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct McpPanelState {
    state: Arc<McpState>,
    selected_server: Option<String>,
    selected_entry: Option<String>,
    tab: Tab,
    page: Page,
    notice: Option<String>,
    submitted: Option<Arc<McpRequest>>,
    pub(crate) scroll: usize,
    pub(crate) document: Vec<String>,
    pub(crate) focus_line: usize,
    pub(crate) caret: Option<Caret>,
    pub(crate) heading: String,
    pub(crate) footer: String,
    width: usize,
    window: usize,
}

pub(crate) enum PanelAction {
    Close,
    Request(McpRequest),
}

impl McpPanelState {
    pub(crate) fn new(state: Arc<McpState>) -> Self {
        let mut panel = Self {
            state,
            selected_server: None,
            selected_entry: None,
            tab: Tab::Servers,
            page: Page::Catalog,
            notice: None,
            submitted: None,
            scroll: 0,
            document: Vec::new(),
            focus_line: 0,
            caret: None,
            heading: String::new(),
            footer: String::new(),
            width: 80,
            window: 12,
        };
        panel.clamp_selection();
        panel.rebuild(true);
        panel
    }

    pub(crate) fn refresh(&mut self, state: Arc<McpState>) -> bool {
        if Arc::ptr_eq(&self.state, &state) {
            return false;
        }
        if self.state.session_id != state.session_id {
            *self = Self::new(state);
            return true;
        }
        self.state = state;
        if self.submitted.as_ref().is_some_and(|request| {
            self.state.pending.contains_key(&request.server_name)
                || self
                    .state
                    .outcomes
                    .get(&request.server_name)
                    .is_some_and(|outcome| outcome.request == **request)
        }) {
            self.submitted = None;
        }
        self.clamp_selection();
        // An edit/read retains its original identity even when a replacement
        // snapshot moves the browser selection to a different server.
        self.rebuild(!matches!(self.page, Page::Read(_)));
        true
    }

    pub(crate) fn geometry(&mut self, width: usize, window: usize) {
        let width = width.max(1);
        if self.width != width {
            self.width = width;
            self.window = window;
            self.rebuild(!matches!(self.page, Page::Read(_)));
        } else if self.window != window {
            self.window = window;
            self.clamp_scroll(!matches!(self.page, Page::Read(_)));
        }
    }

    fn servers(&self) -> &[McpServer] {
        self.state
            .snapshot
            .as_ref()
            .map_or(&[], |snapshot| snapshot.servers.as_slice())
    }

    fn selected(&self) -> Option<&McpServer> {
        self.servers()
            .iter()
            .find(|server| Some(&server.name) == self.selected_server.as_ref())
    }

    fn catalog(&self) -> Option<&McpCatalog> {
        match &self.selected()?.status {
            McpServerStatus::Connected { catalog } => Some(catalog),
            _ => None,
        }
    }

    fn entry_count(&self) -> usize {
        self.catalog().map_or(0, |catalog| match self.tab {
            Tab::Servers => 0,
            Tab::Tools => catalog.tools.len(),
            Tab::Prompts => catalog.prompts.len(),
            Tab::Resources => catalog.resources.len(),
            Tab::Templates => catalog.resource_templates.len(),
        })
    }

    fn entry_key(&self, index: usize) -> Option<&str> {
        let catalog = self.catalog()?;
        match self.tab {
            Tab::Servers => None,
            Tab::Tools => catalog.tools.get(index).map(|tool| tool.name.as_str()),
            Tab::Prompts => catalog
                .prompts
                .get(index)
                .map(|prompt| prompt.name.as_str()),
            Tab::Resources => catalog
                .resources
                .get(index)
                .map(|resource| resource.uri.as_str()),
            Tab::Templates => catalog
                .resource_templates
                .get(index)
                .map(|template| template.uri_template.as_str()),
        }
    }

    fn entry_index(&self) -> Option<usize> {
        let selected = self.selected_entry.as_deref()?;
        let catalog = self.catalog()?;
        match self.tab {
            Tab::Servers => None,
            Tab::Tools => catalog.tools.iter().position(|tool| tool.name == selected),
            Tab::Prompts => catalog
                .prompts
                .iter()
                .position(|prompt| prompt.name == selected),
            Tab::Resources => catalog
                .resources
                .iter()
                .position(|resource| resource.uri == selected),
            Tab::Templates => catalog
                .resource_templates
                .iter()
                .position(|template| template.uri_template == selected),
        }
    }

    fn clamp_selection(&mut self) {
        if !self
            .servers()
            .iter()
            .any(|server| Some(&server.name) == self.selected_server.as_ref())
        {
            self.selected_server = self.servers().first().map(|server| server.name.clone());
            self.selected_entry = None;
        }
        if self.entry_index().is_none() {
            self.selected_entry = self.entry_key(0).map(str::to_owned);
        }
    }

    fn move_selection(&mut self, forward: bool) {
        let (index, len) = if self.tab == Tab::Servers {
            (
                self.servers()
                    .iter()
                    .position(|server| Some(&server.name) == self.selected_server.as_ref())
                    .unwrap_or(0),
                self.servers().len(),
            )
        } else {
            (self.entry_index().unwrap_or(0), self.entry_count())
        };
        let next = if forward {
            index.saturating_add(1).min(len.saturating_sub(1))
        } else {
            index.saturating_sub(1)
        };
        let selected = if self.tab == Tab::Servers {
            self.servers().get(next).map(|server| server.name.clone())
        } else {
            self.entry_key(next).map(str::to_owned)
        };
        if self.tab == Tab::Servers {
            self.selected_server = selected;
            self.selected_entry = None;
            self.clamp_selection();
        } else {
            self.selected_entry = selected;
        }
    }

    fn request(&mut self, request: McpRequest) -> Option<PanelAction> {
        if self.state.session_id.as_ref() != Some(&request.session_id) {
            self.notice =
                Some("Session changed; reopen the advertised entry before sending.".into());
            return None;
        }
        if self.state.pending.contains_key(&request.server_name) || self.submitted.is_some() {
            self.notice = Some("An MCP operation is pending; wait for its result.".into());
            return None;
        }
        self.notice = None;
        self.submitted = Some(Arc::new(request.clone()));
        Some(PanelAction::Request(request))
    }

    fn read(&mut self, request: McpRequest) -> Option<PanelAction> {
        let action = self.request(request)?;
        if let Some(request) = &self.submitted {
            self.page = Page::Read(Arc::clone(request));
        }
        Some(action)
    }

    fn catalog_request(&self, operation: McpOperation) -> Option<McpRequest> {
        Some(McpRequest {
            session_id: self.state.session_id.clone()?,
            server_name: self.selected_server.clone()?,
            operation,
        })
    }

    fn target_visible(&self) -> bool {
        self.window > 0
            && self.focus_line >= self.scroll
            && self.focus_line < self.scroll.saturating_add(self.window)
    }

    fn activate(&mut self) -> Option<PanelAction> {
        // PgUp/PgDn deliberately leave selection fixed. The first activation
        // restores that exact target; only a later visible activation may act.
        if !self.target_visible() {
            return None;
        }
        match self.tab {
            Tab::Prompts => {
                let prompt = self.catalog().and_then(|catalog| {
                    catalog
                        .prompts
                        .iter()
                        .find(|prompt| Some(&prompt.name) == self.selected_entry.as_ref())
                })?;
                let session_id = self.state.session_id.clone()?;
                let server_name = self.selected_server.clone()?;
                self.page = Page::Arguments(ArgumentForm {
                    session_id,
                    server_name,
                    prompt_name: prompt.name.clone(),
                    prompt_title: prompt.title.clone(),
                    fields: prompt
                        .arguments
                        .iter()
                        .map(|argument| Field {
                            argument: argument.clone(),
                            value: ArgumentValue::default(),
                        })
                        .collect(),
                    focused: 0,
                });
                self.notice = None;
                None
            }
            Tab::Resources => {
                let uri = self.selected_entry.clone()?;
                let request = self.catalog_request(McpOperation::GetResource { uri })?;
                self.read(request)
            }
            _ => None,
        }
    }

    fn submit_arguments(&mut self) -> Option<PanelAction> {
        let Page::Arguments(form) = &self.page else {
            return None;
        };
        if let Some(field) = form
            .fields
            .iter()
            .find(|field| field.argument.required && !field.value.present)
        {
            self.notice = Some(format!(
                "Required argument {} is omitted. Ctrl+E supplies an explicit empty string.",
                field.argument.name
            ));
            return None;
        }
        let request = McpRequest {
            session_id: form.session_id.clone(),
            server_name: form.server_name.clone(),
            operation: McpOperation::GetPrompt {
                name: form.prompt_name.clone(),
                arguments: form
                    .fields
                    .iter()
                    .filter(|field| field.value.present)
                    .map(|field| (field.argument.name.clone(), field.value.text.clone()))
                    .collect::<BTreeMap<_, _>>(),
            },
        };
        self.read(request)
    }

    pub(crate) fn paste(&mut self, text: &str) -> bool {
        let Page::Arguments(form) = &mut self.page else {
            return false;
        };
        let Some(field) = form.fields.get_mut(form.focused) else {
            return false;
        };
        field.value.insert(text);
        self.rebuild(true);
        true
    }

    pub(crate) fn dispatch_failed(&mut self, message: String) {
        self.submitted = None;
        self.notice = Some(format!("MCP command could not be sent: {message}"));
        self.rebuild(true);
    }

    pub(crate) fn key(&mut self, key: KeyEvent) -> Option<PanelAction> {
        let mut action = None;
        let mut follow_focus = true;
        match (&mut self.page, key.code) {
            (Page::Catalog, KeyCode::Esc) => return Some(PanelAction::Close),
            (_, KeyCode::Esc) => {
                self.page = Page::Catalog;
                self.notice = None;
            }
            (Page::Arguments(_), KeyCode::Enter) if key.modifiers == KeyModifiers::NONE => {
                action = self.submit_arguments()
            }
            (Page::Arguments(form), KeyCode::Tab | KeyCode::Down) => {
                form.focused = (form.focused + 1).min(form.fields.len().saturating_sub(1))
            }
            (Page::Arguments(form), KeyCode::BackTab | KeyCode::Up) => {
                form.focused = form.focused.saturating_sub(1)
            }
            (_, KeyCode::PageUp) => {
                self.scroll = self.scroll.saturating_sub(self.window.max(1));
                follow_focus = false;
            }
            (_, KeyCode::PageDown) => {
                self.scroll = self.scroll.saturating_add(self.window.max(1));
                follow_focus = false;
            }
            (Page::Arguments(form), _) => {
                if let Some(field) = form.fields.get_mut(form.focused) {
                    field.value.key(key);
                }
            }
            (Page::Read(_), KeyCode::Up) => {
                self.scroll = self.scroll.saturating_sub(1);
                follow_focus = false;
            }
            (Page::Read(_), KeyCode::Down) => {
                self.scroll = self.scroll.saturating_add(1);
                follow_focus = false;
            }
            (Page::Catalog, KeyCode::Tab | KeyCode::Right | KeyCode::BackTab | KeyCode::Left) => {
                self.tab = self
                    .tab
                    .step(matches!(key.code, KeyCode::Tab | KeyCode::Right));
                self.selected_entry = None;
                self.clamp_selection();
            }
            (Page::Catalog, KeyCode::Up | KeyCode::Down) => {
                self.move_selection(key.code == KeyCode::Down)
            }
            (Page::Catalog, KeyCode::Enter) if key.modifiers == KeyModifiers::NONE => {
                action = self.activate()
            }
            (Page::Catalog, KeyCode::Char('r' | 'a')) if key.modifiers == KeyModifiers::NONE => {
                if self.target_visible()
                    && let Some(request) = self.catalog_request(McpOperation::Reset {
                        start_oauth: key.code == KeyCode::Char('a'),
                    })
                {
                    action = self.request(request);
                }
            }
            (Page::Catalog, KeyCode::Char('v')) if key.modifiers == KeyModifiers::NONE => {
                if let Some(outcome) = self
                    .selected_server
                    .as_ref()
                    .and_then(|server| self.state.outcomes.get(server))
                {
                    self.page = Page::Read(Arc::new(outcome.request.clone()));
                }
            }
            _ => return None,
        }
        if follow_focus {
            self.rebuild(true);
        } else {
            self.clamp_scroll(false);
        }
        action
    }

    fn rebuild(&mut self, follow_focus: bool) {
        let (document, focus_line, heading, footer, caret) =
            presentation::document(self, self.width);
        self.document = document;
        self.focus_line = focus_line;
        self.caret = caret;
        self.heading = crate::mcp_notice::terminal_safe(&heading).to_string();
        self.footer = footer;
        self.clamp_scroll(follow_focus);
    }

    fn clamp_scroll(&mut self, follow_focus: bool) {
        self.scroll = self
            .scroll
            .min(self.document.len().saturating_sub(self.window.max(1)));
        if follow_focus
            && (self.focus_line < self.scroll
                || self.focus_line >= self.scroll + self.window.max(1))
        {
            self.scroll = self.focus_line.saturating_sub(self.window.max(1) - 1);
        }
    }
}
