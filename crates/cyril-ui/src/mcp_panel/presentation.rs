use std::borrow::Cow;

use cyril_core::types::{
    McpAccessMode, McpContent, McpElicitationMode, McpElicitationNotice, McpOperation,
    McpOperationResult, McpPolicy, McpPreview, McpProvenance, McpRequest, McpServerStatus,
};
use ratatui::buffer::CellWidth;
use unicode_segmentation::UnicodeSegmentation;

use super::{Caret, McpPanelState, Page, Tab};

struct Document {
    rows: Vec<String>,
    width: usize,
}

impl Document {
    fn text(&mut self, value: &str) -> usize {
        self.text_with_focus(value, 0)
    }

    fn text_with_focus(&mut self, value: &str, focus_offset: usize) -> usize {
        let (display, display_focus) = Self::display(value, focus_offset);
        let mut focus = self.rows.len();
        let mut row = String::new();
        let mut used = 0;
        for (offset, grapheme) in display.grapheme_indices(true) {
            self.grapheme(&mut row, &mut used, grapheme);
            // Pin by source byte offset through projection and wrapping, even
            // when it falls inside a grapheme; never search for a cursor glyph.
            if grapheme != "\n" && (offset..offset + grapheme.len()).contains(&display_focus) {
                focus = self.rows.len();
            }
        }
        self.rows.push(row);
        focus
    }

    fn text_with_caret(&mut self, value: &str, caret_offset: usize) -> Caret {
        let (display, display_caret) = Self::display(value, caret_offset);
        let mut row = String::new();
        let mut used = 0;
        // The editor's caret is a semantic one-cell token, not user data.
        // Segment both sides independently so neither SpacingMark nor Prepend
        // can absorb it; the widget preserves the same three paint boundaries.
        for grapheme in display[..display_caret].graphemes(true) {
            self.grapheme(&mut row, &mut used, grapheme);
        }
        const CARET: &str = "▏";
        self.grapheme(&mut row, &mut used, CARET);
        let caret = Caret {
            row: self.rows.len(),
            column: used - 1,
            bytes: row.len() - CARET.len()..row.len(),
        };
        for grapheme in display[display_caret..].graphemes(true) {
            self.grapheme(&mut row, &mut used, grapheme);
        }
        self.rows.push(row);
        caret
    }

    fn grapheme(&mut self, row: &mut String, used: &mut usize, grapheme: &str) {
        if grapheme == "\n" {
            self.rows.push(std::mem::take(row));
            *used = 0;
            return;
        }
        // Match Buffer::set_stringn's extended grapheme boundaries and
        // cell-width policy, including halfwidth voiced sound marks.
        let width = usize::from(grapheme.cell_width());
        if *used + width > self.width && !row.is_empty() {
            self.rows.push(std::mem::take(row));
            *used = 0;
        }
        row.push_str(grapheme);
        *used += width;
    }

    fn display(value: &str, focus_offset: usize) -> (Cow<'_, str>, usize) {
        // Project the complete text before segmenting: an escape can itself
        // join a following variation selector. Ordinary printable data borrows
        // its source; escaping uses one buffer, never one per grapheme.
        let mut display_focus = focus_offset;
        let display = if let Some((first, _)) = value.char_indices().find(|(_, character)| {
            *character != '\n' && crate::mcp_notice::needs_escape(*character)
        }) {
            let mut escaped = String::with_capacity(value.len());
            escaped.push_str(&value[..first]);
            for (relative, character) in value[first..].char_indices() {
                let offset = first + relative;
                if offset == focus_offset {
                    display_focus = escaped.len();
                }
                if character == '\n' {
                    escaped.push(character);
                } else {
                    escaped.extend(crate::mcp_notice::characters(
                        &value[offset..offset + character.len_utf8()],
                    ));
                }
            }
            if focus_offset == value.len() {
                display_focus = escaped.len();
            }
            Cow::Owned(escaped)
        } else {
            Cow::Borrowed(value)
        };
        (display, display_focus)
    }

    fn optional(&mut self, label: &str, value: Option<&str>) {
        if let Some(value) = value {
            self.text(&format!("{label}: {value}"));
        }
    }

    fn provenance(&mut self, provenance: Option<&McpProvenance>) {
        if let Some(provenance) = provenance {
            self.text("Resource provenance:");
            self.optional("  type", provenance.resource_type.as_deref());
            self.optional("  origin", provenance.origin.as_deref());
            self.optional("  KRN", provenance.krn.as_deref());
            self.optional("  layer", provenance.layer.as_deref());
            self.text(&provenance.details);
        }
    }

    fn content(&mut self, content: &McpContent) {
        match content {
            McpContent::Text {
                text,
                mime_type,
                uri,
            } => {
                self.text("Content kind: text");
                self.optional("MIME", mime_type.as_deref());
                self.optional("URI", uri.as_deref());
                self.text(text);
            }
            McpContent::NonText {
                kind,
                mime_type,
                uri,
                name,
                description,
            } => {
                self.text(&format!(
                    "Content kind: {kind} (nontext; not executed or decoded)"
                ));
                self.optional("MIME", mime_type.as_deref());
                self.optional("URI", uri.as_deref());
                self.optional("Name", name.as_deref());
                self.optional("Description", description.as_deref());
            }
        }
    }

    fn preview(&mut self, preview: &McpPreview) {
        self.text("READ-ONLY EXTERNAL DATA — not sent to the model, executed, or saved.");
        match preview {
            McpPreview::Prompt {
                description,
                messages,
            } => {
                self.optional("Description", description.as_deref());
                if messages.is_empty() {
                    self.text("Prompt returned zero messages.");
                }
                for message in messages {
                    self.text(&format!("Message role: {}", message.role));
                    self.content(&message.content);
                    self.text("");
                }
            }
            McpPreview::Resource { contents } => {
                if contents.is_empty() {
                    self.text("Resource returned zero content entries.");
                }
                for content in contents {
                    self.content(content);
                    self.text("");
                }
            }
        }
    }
}

fn operation(request: &McpRequest) -> String {
    match &request.operation {
        McpOperation::Reset { start_oauth: true } => "Authenticate (reset with OAuth)".into(),
        McpOperation::Reset { start_oauth: false } => "Reset (no OAuth)".into(),
        McpOperation::GetPrompt { name, .. } => format!("Read prompt {name}"),
        McpOperation::GetResource { uri } => format!("Read resource {uri}"),
    }
}

pub(crate) fn elicitation_notice(notice: &McpElicitationNotice) -> String {
    let mode = match &notice.mode {
        McpElicitationMode::Form => "form",
        McpElicitationMode::Url => "URL",
        McpElicitationMode::Unknown(mode) => mode,
    };
    format!(
        "MCP {} elicitation declined (session {}, tool {}): Cyril does not offer interactive elicitation. {}{}",
        crate::mcp_notice::terminal_safe(mode),
        crate::mcp_notice::terminal_safe(notice.session_id.as_str()),
        crate::mcp_notice::terminal_safe(notice.tool_call_id.as_str()),
        crate::mcp_notice::terminal_safe(&notice.message),
        notice.url.as_ref().map_or(String::new(), |url| format!(
            " Requested URL (not opened): {}",
            crate::mcp_notice::terminal_safe(url)
        )),
    )
}

pub(super) fn document(
    panel: &McpPanelState,
    width: usize,
) -> (Vec<String>, usize, String, String, Option<Caret>) {
    let mut doc = Document {
        rows: Vec::new(),
        width: width.max(1),
    };
    let mut focus = 0;
    let mut caret = None;
    doc.text(&format!(
        "Session: {}",
        panel
            .state
            .session_id
            .as_ref()
            .map_or("not established", |session| session.as_str())
    ));
    if let Some(policy) = &panel.state.policy {
        doc.text(&match policy {
            McpPolicy::ApiFailure => "MCP governance unavailable: policy API failed. This is not an empty configuration.".into(),
            McpPolicy::AdminDisabled => "MCP disabled by your administrator. Cyril cannot override this policy.".into(),
            McpPolicy::NoEndpoint => "MCP governance reports no endpoint.".into(),
            McpPolicy::Unknown(reason) => format!("MCP governance disabled: unrecognized reason {reason}. No policy bypass is offered."),
        });
    }
    if let Some(diagnostic) = &panel.state.diagnostic {
        doc.text(&format!(
            "STATUS ERROR — last valid inventory may be stale: {diagnostic}"
        ));
    }
    if let Some(notice) = &panel.state.elicitation {
        doc.text(&elicitation_notice(notice));
    }
    if let Some(notice) = &panel.notice {
        doc.text(&format!("ERROR: {notice}"));
    }
    for pending in panel.state.pending.values() {
        doc.text(&format!(
            "PENDING [{}]: {}",
            pending.request.server_name,
            operation(&pending.request)
        ));
    }
    if let Some(request) = &panel.submitted {
        doc.text(&format!(
            "SUBMITTED [{}]: {}",
            request.server_name,
            operation(request)
        ));
    }
    for outcome in panel.state.outcomes.values() {
        match &outcome.result {
            Err(error) => {
                doc.text(&format!(
                    "FAILED [{}] {} ({:?}): {}",
                    outcome.request.server_name,
                    operation(&outcome.request),
                    error.kind,
                    error.message
                ));
            }
            Ok(McpOperationResult::Acknowledged) => {
                doc.text(&format!("ACKNOWLEDGED [{}]: {}. Await authoritative status; this does not mean connected.", outcome.request.server_name, operation(&outcome.request)));
            }
            Ok(McpOperationResult::Preview(_)) => {
                doc.text(&format!(
                    "READ COMPLETE [{}]: {} (v to view)",
                    outcome.request.server_name,
                    operation(&outcome.request)
                ));
            }
        }
    }
    let (heading, footer) = match &panel.page {
        Page::Arguments(form) => {
            doc.text(&format!(
                "Prompt {} on {} (original session {})",
                form.prompt_title.as_deref().unwrap_or(&form.prompt_name),
                form.server_name,
                form.session_id.as_str()
            ));
            doc.text("Values are strings. Omitted differs from explicitly empty. Enter fetches read-only data.");
            if form.fields.is_empty() {
                doc.text("No advertised arguments; Enter fetches the prompt.");
            }
            for (index, field) in form.fields.iter().enumerate() {
                let active = index == form.focused;
                let label = format!(
                    "{} {} [{}; {}] = ",
                    if active { ">" } else { " " },
                    field.argument.name,
                    if field.argument.required {
                        "required"
                    } else {
                        "optional"
                    },
                    if field.value.present {
                        "present"
                    } else {
                        "omitted"
                    }
                );
                let logical_cursor = field.value.cursor;
                // Only the visual caret snaps after its owning DATA grapheme.
                // Labels and escape projections must not redefine boundaries,
                // and the editor's logical byte cursor remains untouched.
                let visual_cursor =
                    if active && logical_cursor > 0 && logical_cursor < field.value.text.len() {
                        field
                            .value
                            .text
                            .grapheme_indices(true)
                            .map(|(start, grapheme)| (start, start + grapheme.len()))
                            .find(|(_, end)| *end >= logical_cursor)
                            .filter(|(start, end)| *start < logical_cursor && logical_cursor < *end)
                            .map_or(logical_cursor, |(_, end)| end)
                    } else {
                        logical_cursor
                    };
                let prefix = format!(
                    "{label}{}",
                    crate::mcp_notice::terminal_safe(&field.value.text[..visual_cursor])
                );
                let value = format!(
                    "{prefix}{}",
                    crate::mcp_notice::terminal_safe(&field.value.text[visual_cursor..])
                );
                if active {
                    let position = doc.text_with_caret(&value, prefix.len());
                    focus = position.row;
                    caret = Some(position);
                } else {
                    doc.text(&value);
                }
                doc.optional("  Description", field.argument.description.as_deref());
            }
            (format!("Arguments: {}", form.prompt_title.as_deref().unwrap_or(&form.prompt_name)), "Enter fetch · Tab/↑↓ field · Ctrl+E empty · Ctrl+U omit · Ctrl+J newline · Esc back".into())
        }
        Page::Read(request) => {
            focus = doc.text(&format!(
                "{} — server {} — original session {}",
                operation(request),
                request.server_name,
                request.session_id.as_str()
            ));
            if panel.submitted.as_ref() == Some(request) {
                doc.text("Request submitted; awaiting the correlated result.");
            } else if let Some(outcome) = panel
                .state
                .outcomes
                .get(&request.server_name)
                .filter(|outcome| outcome.request == **request)
            {
                match &outcome.result {
                    Ok(McpOperationResult::Preview(preview)) => doc.preview(preview),
                    Ok(McpOperationResult::Acknowledged) => {
                        doc.text(
                            "Reset acknowledged. Only a status snapshot can report connected.",
                        );
                    }
                    Err(error) => {
                        doc.text(&format!(
                            "Operation failed ({:?}): {}",
                            error.kind, error.message
                        ));
                    }
                }
            } else {
                doc.text(
                    "Waiting for the pinned request result. Status notifications remain live.",
                );
            }
            (
                "Read-only preview".into(),
                "↑↓ scroll · PgUp/PgDn page · Esc back (never sends content to chat)".into(),
            )
        }
        Page::Catalog => {
            let Some(snapshot) = &panel.state.snapshot else {
                doc.text("MCP inventory not reported yet (unknown, not zero configured servers).");
                return (
                    doc.rows,
                    focus,
                    "MCP · inventory unreported".into(),
                    "Esc close · status pushes refresh this panel only while open".into(),
                    caret,
                );
            };
            doc.text(&format!(
                "Access mode: {}",
                match &snapshot.access_mode {
                    None => "not reported",
                    Some(McpAccessMode::Registry) => "registry",
                    Some(McpAccessMode::NonRegistry) => "nonRegistry",
                    Some(McpAccessMode::Unknown(mode)) => mode,
                }
            ));
            doc.provenance(snapshot.provenance.as_ref());
            if let Some(filtered) = &snapshot.access_mode_filtered_servers {
                doc.text("Access-mode filtered servers (excluded by policy, not disconnected):");
                if filtered.is_empty() {
                    doc.text("  none");
                }
                for server in filtered {
                    doc.text(server);
                }
            }
            if let Some(unresolved) = &snapshot.unresolved_registry_servers {
                doc.text("Unresolved registry names:");
                if unresolved.is_empty() {
                    doc.text("  none");
                }
                for server in unresolved {
                    doc.text(server);
                }
            }
            if let Some(registry) = &snapshot.registry_servers {
                doc.text("Registry catalog:");
                if registry.is_empty() {
                    doc.text("  empty");
                }
                for server in registry {
                    doc.text(&format!(
                        "{} — {}",
                        server.name,
                        if server.enabled {
                            "enabled / in pool"
                        } else {
                            "not enabled in pool"
                        }
                    ));
                    doc.optional("  Version", server.version.as_deref());
                    doc.optional("  Description", server.description.as_deref());
                }
            }
            doc.text("Servers · Tools · Prompts · Resources · Templates (Tab/←→)");
            if snapshot.servers.is_empty() {
                doc.text("Known empty inventory: zero servers in the reported pool. See policy/registry explanations above.");
            }
            for server in &snapshot.servers {
                let selected = Some(&server.name) == panel.selected_server.as_ref();
                let status = match &server.status {
                    McpServerStatus::Connecting => "connecting",
                    McpServerStatus::Connected { .. } => "connected",
                    McpServerStatus::Failed {
                        failed_authorization: true,
                        ..
                    } => "authentication required",
                    McpServerStatus::Failed { .. } => "transport failed",
                    McpServerStatus::Disabled => "disabled",
                    McpServerStatus::Unknown { .. } => "unrecognized",
                };
                let row = doc.text(&format!(
                    "{} {} — {status}",
                    if selected { ">" } else { " " },
                    server.name
                ));
                if selected {
                    focus = row;
                }
                if let McpServerStatus::Unknown { status } = &server.status {
                    doc.text(&format!("  Reported status: {status}"));
                }
            }
            if let Some(server) = panel.selected() {
                doc.text(&format!("Selected server: {}", server.name));
                doc.optional("Authentication type", server.auth_type.as_deref());
                doc.provenance(server.provenance.as_ref());
                if let McpServerStatus::Failed {
                    error_message,
                    error_code,
                    ..
                } = &server.status
                {
                    doc.text(&format!("Failure: {error_message}"));
                    doc.optional("Error code", error_code.as_deref());
                }
                doc.optional(
                    "Manual sign-in URL (not opened)",
                    panel
                        .state
                        .authorization_urls
                        .get(&server.name)
                        .map(String::as_str),
                );
            }
            if let Some(catalog) = panel.catalog() {
                doc.text(&format!(
                    "Advertised: {} tools, {} prompts, {} resources, {} templates",
                    catalog.tools.len(),
                    catalog.prompts.len(),
                    catalog.resources.len(),
                    catalog.resource_templates.len()
                ));
                match panel.tab {
                    Tab::Servers => {}
                    Tab::Tools => {
                        doc.text("TOOLS — inspection only; no tool execution control");
                        for tool in &catalog.tools {
                            let selected = Some(&tool.name) == panel.selected_entry.as_ref();
                            let row = doc.text(&format!(
                                "{} {} — {}",
                                if selected { ">" } else { " " },
                                tool.name,
                                if tool.disabled { "disabled" } else { "enabled" }
                            ));
                            if selected {
                                focus = row;
                                doc.optional("Description", tool.description.as_deref());
                                doc.text(&format!(
                                    "Input schema (read-only): {}",
                                    tool.input_schema
                                ));
                            }
                        }
                        if catalog.tools.is_empty() {
                            doc.text("No advertised tools.");
                        }
                    }
                    Tab::Prompts => {
                        for prompt in &catalog.prompts {
                            let selected = Some(&prompt.name) == panel.selected_entry.as_ref();
                            let row = doc.text(&format!(
                                "{} {}",
                                if selected { ">" } else { " " },
                                prompt.title.as_deref().unwrap_or(&prompt.name)
                            ));
                            if selected {
                                focus = row;
                                if prompt.title.is_some() {
                                    doc.text(&format!("Prompt name: {}", prompt.name));
                                }
                                doc.optional("Description", prompt.description.as_deref());
                                for argument in &prompt.arguments {
                                    doc.text(&format!(
                                        "Argument {} ({})",
                                        argument.name,
                                        if argument.required {
                                            "required"
                                        } else {
                                            "optional"
                                        }
                                    ));
                                    doc.optional("Description", argument.description.as_deref());
                                }
                            }
                        }
                        if catalog.prompts.is_empty() {
                            doc.text("No advertised prompts.");
                        }
                    }
                    Tab::Resources => {
                        for resource in &catalog.resources {
                            let selected = Some(&resource.uri) == panel.selected_entry.as_ref();
                            let row = doc.text(&format!(
                                "{} {} — {}",
                                if selected { ">" } else { " " },
                                resource.name,
                                resource.uri
                            ));
                            if selected {
                                focus = row;
                                doc.optional("Title", resource.title.as_deref());
                                doc.optional("Description", resource.description.as_deref());
                                doc.optional("MIME", resource.mime_type.as_deref());
                            }
                        }
                        if catalog.resources.is_empty() {
                            doc.text("No advertised resources.");
                        }
                    }
                    Tab::Templates => {
                        for template in &catalog.resource_templates {
                            let selected =
                                Some(&template.uri_template) == panel.selected_entry.as_ref();
                            let row = doc.text(&format!(
                                "{} {} — {}",
                                if selected { ">" } else { " " },
                                template.name,
                                template.uri_template
                            ));
                            if selected {
                                focus = row;
                                doc.optional("Title", template.title.as_deref());
                                doc.optional("Description", template.description.as_deref());
                                doc.optional("MIME", template.mime_type.as_deref());
                            }
                        }
                        doc.text("Templates are advertised metadata, not directly fetched URIs.");
                    }
                }
            } else if panel.tab != Tab::Servers {
                doc.text("Catalog unavailable: selected server is not connected.");
            }
            let busy = panel
                .selected_server
                .as_ref()
                .is_some_and(|server| panel.state.pending.contains_key(server))
                || panel.submitted.is_some();
            let footer = if busy {
                "PENDING · actions disabled · ↑↓/Tab inspect · PgUp/PgDn scroll · Esc close"
            } else {
                match panel.tab {
                    Tab::Prompts => {
                        "Enter arguments · r Reset · a Auth · ↑↓ select · Tab · v result · PgUp/PgDn · Esc"
                    }
                    Tab::Resources => {
                        "Enter read · r Reset · a Auth · ↑↓ select · Tab · v result · PgUp/PgDn · Esc"
                    }
                    _ => {
                        "r Reset · a Authenticate · ↑↓ select · Tab catalog · v result · PgUp/PgDn · Esc"
                    }
                }
            };
            (
                format!(
                    "MCP · {:?} · {}",
                    panel.tab,
                    panel.selected_server.as_deref().unwrap_or("no server")
                ),
                footer.into(),
            )
        }
    };
    (doc.rows, focus, heading, footer, caret)
}

#[cfg(test)]
mod tests {
    use super::Document;

    #[test]
    fn grapheme_width_wraps_preview_tail_at_two_and_four_columns() {
        for (width, value, first_row) in [
            (2, "#\u{fe0f}Z", "#\u{fe0f}"),
            (4, "#\u{fe0f}#\u{fe0f}Z", "#\u{fe0f}#\u{fe0f}"),
        ] {
            let mut doc = Document {
                rows: Vec::new(),
                width,
            };
            doc.text(value);
            assert_eq!(doc.rows, [first_row, "Z"], "width {width}");
        }
    }

    #[test]
    fn grapheme_width_focus_uses_source_offset_at_two_and_four_columns() {
        for (width, prefix) in [(2, "#\u{fe0f}"), (4, "#\u{fe0f}#\u{fe0f}")] {
            let mut doc = Document {
                rows: Vec::new(),
                width,
            };
            let focus = doc.text_with_focus(&format!("{prefix}▏"), prefix.len());
            assert_eq!(doc.rows, [prefix, "▏"], "width {width}");
            assert_eq!(focus, 1, "cursor must wrap after the complete graphemes");
        }
    }

    #[test]
    fn grapheme_width_preserves_compound_symbols_that_fit_one_row() {
        for symbol in ["👩\u{200d}💻", "👍🏽", "e\u{301}"] {
            let mut doc = Document {
                rows: Vec::new(),
                width: 2,
            };
            doc.text(symbol);
            assert_eq!(doc.rows, [symbol], "compound symbol {symbol:?}");
        }
    }

    #[test]
    fn focus_uses_source_offset_not_a_user_entered_cursor_glyph() {
        let mut doc = Document {
            rows: Vec::new(),
            width: 3,
        };
        let focus = doc.text_with_focus("▏日日▏日▏", "▏日日".len());
        assert_eq!(doc.rows, ["▏日", "日▏", "日▏"]);
        assert_eq!(focus, 1);
    }

    #[test]
    fn focus_tracks_newlines_escaped_controls_and_combining_characters() {
        let mut doc = Document {
            rows: vec!["prior".into()],
            width: 3,
        };
        let focus = doc.text_with_focus("日\n\u{1b}e\u{301}▏日", "日\n\u{1b}e\u{301}".len());
        assert_eq!(doc.rows, ["prior", "日", "\\u{", "1b}", "e\u{301}▏", "日"]);
        assert_eq!(focus, 4);
        assert_eq!(doc.text("\n"), 6);
        assert_eq!(&doc.rows[6..], ["", ""]);
    }

    fn assert_sound_mark_document_width_contract(symbol: &str, expected_width: usize) {
        // Released core 0.1.2 adds one cell only for U+FF9E/U+FF9F:
        // https://github.com/ratatui/ratatui/blob/e665c36cb14752a61cd777fbd06dbef8474f2add/ratatui-core/src/buffer/cell_width.rs
        // Standalone halfwidth marks occupy one cell. Combining U+3099/U+309A
        // stay attached: one cell with halfwidth bases, two with Hiragana.
        assert!(matches!(symbol.len(), 3 | 6));
        let with_tail = format!("{symbol}Z");
        for (width, value, expected_rows, source_focus, expected_focus) in [
            (expected_width, symbol, vec![symbol], 0, 0),
            (
                expected_width,
                with_tail.as_str(),
                vec![symbol, "Z"],
                symbol.len(),
                1,
            ),
            (
                expected_width + 1,
                with_tail.as_str(),
                vec![with_tail.as_str()],
                symbol.len(),
                0,
            ),
        ] {
            let mut doc = Document {
                rows: Vec::new(),
                width,
            };
            let focus = doc.text_with_focus(value, source_focus);
            assert_eq!(doc.rows, expected_rows, "symbol {symbol:?}, width {width}");
            assert_eq!(
                focus, expected_focus,
                "source focus {source_focus}, symbol {symbol:?}, width {width}"
            );
        }
    }

    #[test]
    fn halfwidth_dakuten_document_uses_released_two_cell_contract() {
        assert_sound_mark_document_width_contract("\u{ff76}\u{ff9e}", 2);
    }

    #[test]
    fn halfwidth_handakuten_document_uses_released_two_cell_contract() {
        assert_sound_mark_document_width_contract("\u{ff8a}\u{ff9f}", 2);
    }

    #[test]
    fn ordinary_combining_dakuten_document_keeps_two_cell_contract() {
        assert_sound_mark_document_width_contract("\u{304b}\u{3099}", 2);
    }

    #[test]
    fn ordinary_combining_handakuten_document_keeps_two_cell_contract() {
        assert_sound_mark_document_width_contract("\u{306f}\u{309a}", 2);
    }

    #[test]
    fn standalone_halfwidth_dakuten_document_uses_released_one_cell_contract() {
        assert_sound_mark_document_width_contract("\u{ff9e}", 1);
    }

    #[test]
    fn standalone_halfwidth_handakuten_document_uses_released_one_cell_contract() {
        assert_sound_mark_document_width_contract("\u{ff9f}", 1);
    }

    #[test]
    fn halfwidth_base_combining_dakuten_document_keeps_one_cell_contract() {
        assert_sound_mark_document_width_contract("\u{ff76}\u{3099}", 1);
    }

    #[test]
    fn halfwidth_base_combining_handakuten_document_keeps_one_cell_contract() {
        assert_sound_mark_document_width_contract("\u{ff8a}\u{309a}", 1);
    }
}
