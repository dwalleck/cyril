//! `/powers` panel: the agent's installed KAS powers, as it reported them.
//!
//! Three lines per power, so a row is a fixed height and the viewport window is
//! exact arithmetic rather than a layout guess:
//!
//! ```text
//!   Build AWS infrastructure with CDK and CloudFormation
//!   aws-infrastructure-as-code · mcp awslabs.aws-iac-mcp-server
//!   Build well-architected AWS infrastructure with CDK using latest …
//! ```
//!
//! Line 1 is the display name (or the identifier when the agent gave none),
//! line 2 the identifier plus one `mcp <server>` token per MCP server the power
//! contributes plus `steering` when the agent reported steering files, line 3
//! the agent's description. The description line is always drawn — blank when
//! the agent omitted it — because a variable-height row would break the
//! viewport window.
//!
//! Nothing here decides content: ordering is `UiState`'s, and every value is
//! the agent's. The widget only chooses cells.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::text::{truncate, truncate_and_pad};
use crate::theme::Theme;
use crate::traits::{MAX_VISIBLE_POWERS, PowersPanelState};

/// Rows one power occupies, always.
const LINES_PER_POWER: usize = 3;
/// Two-cell indent on every content line.
const INDENT: usize = 2;
/// Rows the block spends on its own frame: the two borders, nothing else.
///
/// Deliberately NOT the hooks panel's +4 — that panel draws a header row
/// inside its frame, while this one puts the count in the title. Reserving
/// rows this widget does not draw drops the last power that fits whenever
/// `place` clamps the popup (review finding 5).
const BORDER_ROWS: usize = 2;
/// Marker for a power that ships steering files, appended to the meta line.
/// ASCII, so its byte length is its cell width.
const STEERING_TOKEN: &str = " · steering";

/// The popup's rect and the number of whole powers it can show.
///
/// `None` when [`crate::widgets::modal::place`] finds no room above the input:
/// there is nowhere to draw and nothing to scroll. The window is what the
/// PLACED popup fits — `place` clamps the popup to the rows above the input, so
/// a short terminal or a tall input leaves less than [`MAX_VISIBLE_POWERS`]
/// powers' worth of rows.
///
/// That number is also the keyboard's scroll bound, through
/// [`crate::render::powers_window`]: bounding it at `len - MAX_VISIBLE_POWERS`
/// instead strands the tail of a long catalog, because no offset the bound
/// allows starts the window over the last powers (review finding 8).
pub(crate) fn placement(len: usize, area: Rect, input_top: u16) -> Option<(Rect, usize)> {
    let listed = len.clamp(1, MAX_VISIBLE_POWERS);
    let desired_height = (listed * LINES_PER_POWER) as u16 + BORDER_ROWS as u16;
    let popup = crate::widgets::modal::place(area, input_top, 96, desired_height)?;
    let window = ((popup.height as usize).saturating_sub(BORDER_ROWS) / LINES_PER_POWER).max(1);
    Some((popup, window))
}

/// Render the powers panel overlay (input-protected popup).
///
/// `input_top` is the absolute row of the input box's top border; placement
/// goes through [`crate::widgets::modal::place`] so the popup never covers the
/// input. A zero-height area or one with no room above the input paints
/// nothing at all — not even `Clear`.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &PowersPanelState,
    theme: &Theme,
) {
    let Some((popup_area, window)) = placement(state.powers.len(), area, input_top) else {
        return; // no rows above the input can hold the popup
    };
    let width = popup_area.width;

    frame.render_widget(Clear, popup_area);

    // Content width inside the borders and the two-cell indent. Floor division
    // is deliberate: a trailing partial line is left unpainted rather than
    // clipped mid-row.
    let inner_width = (width as usize).saturating_sub(2);
    let text_width = inner_width.saturating_sub(INDENT).max(1);
    // The viewport must start inside the catalog. `UiState` bounds the keyboard
    // to the same window this returns, so the clamp only bites for a panel
    // state built by hand or one whose catalog shrank under it (review
    // finding 8).
    let first_visible = state
        .scroll_offset
        .min(state.powers.len().saturating_sub(window));

    let title = if state.powers.is_empty() {
        " /powers · 0 powers ".to_owned()
    } else if state.powers.len() > window {
        // Without this a twelve-power catalog reads as five rows and stops:
        // the panel has no `+N more` row and no key footer, so nothing says
        // the other seven exist (review finding 6).
        format!(
            " /powers · {} powers · showing {}–{} ",
            state.powers.len(),
            first_visible + 1,
            (first_visible + window).min(state.powers.len())
        )
    } else {
        format!(
            " /powers · {} power{} ",
            state.powers.len(),
            if state.powers.len() == 1 { "" } else { "s" }
        )
    };
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(theme.accent_quinary)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_quinary));

    if state.powers.is_empty() {
        // Reached only from a push that carried zero powers: `/powers` says
        // "not reported yet" itself and never opens this panel without a
        // catalog.
        let empty = Paragraph::new(Line::styled(
            "  No powers installed",
            Style::default().fg(theme.subdued),
        ))
        .block(block);
        frame.render_widget(empty, popup_area);
        return;
    }

    let mut lines: Vec<Line> = Vec::with_capacity(state.powers.len() * LINES_PER_POWER);
    for power in state.powers.iter().skip(first_visible).take(window) {
        lines.push(Line::styled(
            format!("  {}", truncate_and_pad(power.title(), text_width)),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ));

        // The identifier is the stable half of the row: a display name can be
        // edited upstream, the id is what `~/.kiro/powers/` holds.
        //
        // The steering token is budgeted BEFORE the server list, not appended
        // after it: the row is truncated to the panel width, so a long or
        // multi-server MCP list used to eat ` · steering` whole and the panel
        // then stated by omission that the power ships no steering files
        // (review finding 7).
        let mut meta = format!("  {}", power.name());
        for server in power.mcp_server_names() {
            meta.push_str(&format!(" · mcp {server}"));
        }
        // A panel too narrow for the token plus at least one cell of identifier
        // drops the token rather than painting past its own border; the row is
        // an ellipsis at that width anyway.
        let steering = if power.has_steering_files() && inner_width > STEERING_TOKEN.len() {
            STEERING_TOKEN
        } else {
            ""
        };
        let meta = format!(
            "{}{steering}",
            truncate(&meta, inner_width.saturating_sub(steering.len()))
        );
        lines.push(Line::styled(
            meta,
            Style::default().fg(theme.text_secondary),
        ));

        lines.push(Line::styled(
            match power.description() {
                Some(description) => format!("  {}", truncate_and_pad(description, text_width)),
                None => String::new(),
            },
            Style::default().fg(theme.subdued),
        ));
    }

    let popup = Paragraph::new(lines).block(block);
    frame.render_widget(popup, popup_area);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::state::UiState;
    use crate::traits::TuiState;
    use cyril_core::types::PowerInfo;
    use ratatui::backend::TestBackend;

    fn power(
        name: &str,
        title: Option<&str>,
        description: Option<&str>,
        servers: &[&str],
        steering: bool,
    ) -> PowerInfo {
        PowerInfo::new(
            name,
            title.map(str::to_owned),
            description.map(str::to_owned),
            servers.iter().map(|s| (*s).to_owned()).collect(),
            steering,
        )
    }

    fn draw(state: &PowersPanelState, width: u16, height: u16) -> Terminal<TestBackend> {
        draw_at(state, width, height, height)
    }

    /// Draw with an explicit input top, so a test can reach the branch where
    /// `place` has to clamp the popup instead of centering it.
    fn draw_at(
        state: &PowersPanelState,
        width: u16,
        height: u16,
        input_top: u16,
    ) -> Terminal<TestBackend> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    input_top,
                    state,
                    &crate::theme::resolve(
                        crate::theme::ThemeId::CyrilDark,
                        crate::theme::ColorMode::TrueColor,
                    ),
                )
            })
            .unwrap();
        terminal
    }

    /// Draw the WHOLE frame — chrome, chat, input, overlays — the way the app
    /// does, so a fence can assert on the geometry the real layout produces
    /// instead of a hand-passed input row.
    fn draw_frame(state: &UiState, width: u16, height: u16) -> Terminal<TestBackend> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| crate::render::draw(frame, state))
            .unwrap();
        terminal
    }

    fn rendered_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// The popup is centered by `modal::place`, so its rows move with the
    /// terminal height. Tests locate its top border (`┌`, which also carries
    /// the title) rather than assuming a row, and index content from there.
    fn popup_top(terminal: &Terminal<TestBackend>) -> u16 {
        let buffer = terminal.backend().buffer();
        (0..buffer.area().height)
            .find(|y| (0..buffer.area().width).any(|x| buffer[(x, *y)].symbol() == "┌"))
            .expect("the popup's top border is drawn")
    }

    /// Content row `index` inside the popup (0 = the first row below the top
    /// border).
    fn row_text(terminal: &Terminal<TestBackend>, index: u16) -> String {
        let y = popup_top(terminal) + 1 + index;
        let buffer = terminal.backend().buffer();
        (0..buffer.area().width)
            .map(|x| buffer[(x, y)].symbol())
            .collect::<String>()
    }

    /// Nine powers: two more than the widest window, so any offset bound short
    /// of the real one is visible as an unreachable tail.
    fn nine_powers() -> Vec<PowerInfo> {
        (0..9)
            .map(|n| {
                power(
                    &format!("power-{n:02}"),
                    Some(&format!("Power {n:02}")),
                    None,
                    &[],
                    false,
                )
            })
            .collect()
    }

    /// The three powers installed on the capture machine, in the display order
    /// `UiState` produces.
    fn three_powers() -> Vec<PowerInfo> {
        vec![
            power(
                "aws-infrastructure-as-code",
                Some("Build AWS infrastructure with CDK and CloudFormation"),
                Some("Build well-architected AWS infrastructure with CDK."),
                &["awslabs.aws-iac-mcp-server"],
                false,
            ),
            power(
                "datadog",
                Some("Datadog Observability"),
                Some("Query logs, metrics, traces from Datadog."),
                &["datadog"],
                true,
            ),
            power("markdownlint", None, None, &["markdownlint"], true),
        ]
    }

    /// REGRESSION FENCE (cyril-v19o C6): the approved row layout, at exact
    /// cell positions rather than by substring alone.
    #[test]
    fn layout_matches_the_approved_row_shape() {
        let state = PowersPanelState {
            powers: three_powers(),
            scroll_offset: 0,
        };
        let terminal = draw(&state, 100, 24);
        let text = rendered_text(&terminal);
        assert!(text.contains("3 powers"), "counted in the title: {text}");
        assert!(text.contains("Build AWS infrastructure with CDK and CloudFormation"));
        assert!(text.contains("aws-infrastructure-as-code"));
        assert!(text.contains("mcp awslabs.aws-iac-mcp-server"));
        assert!(text.contains("Datadog Observability"));
        assert!(text.contains("mcp datadog"));
        assert!(text.contains("Query logs, metrics, traces from Datadog."));
        // A power with no display name falls back to its identifier.
        assert!(text.contains("markdownlint"));

        // Three content lines per power, no separator: title, meta, description.
        assert!(row_text(&terminal, 0).contains("Build AWS infrastructure"));
        assert!(row_text(&terminal, 1).contains("mcp awslabs.aws-iac-mcp-server"));
        assert!(row_text(&terminal, 2).contains("Build well-architected AWS infrastructure"));
        assert!(row_text(&terminal, 3).contains("Datadog Observability"));
        assert!(row_text(&terminal, 4).contains("mcp datadog"));
        assert!(row_text(&terminal, 5).contains("Query logs, metrics, traces from Datadog."));
        assert!(row_text(&terminal, 6).contains("markdownlint"));
        // A power the agent gave no description for leaves that line blank
        // rather than dropping it — the row height is constant.
        assert_eq!(
            row_text(&terminal, 8).trim_matches(['│', ' ']),
            "",
            "the description line of a power with none is blank inside the borders"
        );

        // Two MCP servers on one power both render, and an over-long
        // description truncates with a marker instead of drifting into the
        // next row (S12, S17).
        let crowded = PowersPanelState {
            powers: vec![power(
                "multi",
                Some("Multi"),
                Some(&"d".repeat(400)),
                &["first-mcp-server", "second-mcp-server"],
                false,
            )],
            scroll_offset: 0,
        };
        let crowded_terminal = draw(&crowded, 60, 24);
        let crowded_text = rendered_text(&crowded_terminal);
        assert!(crowded_text.contains("mcp first-mcp-server"));
        assert!(crowded_text.contains("mcp second-mcp-server"));
        assert!(
            crowded_text.contains('…'),
            "an over-long description is truncated with a marker"
        );
        let inner = row_text(&crowded_terminal, 2)
            .trim()
            .trim_matches('│')
            .trim_end()
            .to_owned();
        assert!(
            inner.ends_with('…'),
            "the truncated description ends at the marker, inside the border: {inner:?}"
        );

        // `steering` marks exactly the two powers whose wire flag was true.
        assert_eq!(
            text.matches("· steering").count(),
            2,
            "steering marks the two powers that ship steering files: {text}"
        );
        // …and the aws row is the one without it.
        assert!(!row_text(&terminal, 1).contains("steering"));
        assert!(row_text(&terminal, 4).contains("steering"));
    }

    /// REGRESSION FENCE (cyril-v19o C6): a known-empty catalog states that
    /// fact; it is never blank, and never a placeholder row.
    #[test]
    fn empty_catalog_shows_placeholder() {
        let state = PowersPanelState {
            powers: Vec::new(),
            scroll_offset: 0,
        };
        let terminal = draw(&state, 100, 24);
        let text = rendered_text(&terminal);
        assert!(text.contains("No powers installed"));
        assert!(text.contains("0 powers"));
    }

    /// REGRESSION FENCE (cyril-v19o C6): wide characters clamp to the panel.
    /// Without the width clamp the row would be hard-clipped by ratatui with no
    /// truncation marker, so the ellipsis is the observable difference.
    #[test]
    fn wide_title_clamps_to_the_panel() {
        let state = PowersPanelState {
            powers: vec![power(
                "wide",
                Some("日本語のとても長いタイトルでありパネル幅を超えるもの"),
                None,
                &[],
                false,
            )],
            scroll_offset: 0,
        };
        let terminal = draw(&state, 40, 24);
        let title_row = row_text(&terminal, 0);
        let buffer = terminal.backend().buffer();
        let y = popup_top(&terminal) + 1;
        let popup_right = (0..buffer.area().width)
            .rev()
            .find(|x| buffer[(*x, y)].symbol() == "│")
            .expect("the popup's right border is drawn on the title row");
        assert!(
            title_row.contains('…'),
            "an over-long title is truncated with a marker: {title_row:?}"
        );
        assert!(
            !title_row.contains("超"),
            "content must stop before the panel's own width: {title_row:?}"
        );
        assert_eq!(
            buffer[(popup_right, y)].symbol(),
            "│",
            "the border survives the widest row"
        );
    }

    /// REGRESSION FENCE (cyril-v19o review finding 6). A catalog taller than
    /// the window says so in the title: the panel has no `+N more` row and no
    /// key footer, so without this it reads as "these five are all of them".
    #[test]
    fn title_states_the_window_when_the_catalog_overflows() {
        let powers: Vec<PowerInfo> = (0..12)
            .map(|n| {
                power(
                    &format!("power-{n:02}"),
                    Some(&format!("Power {n:02}")),
                    None,
                    &[],
                    false,
                )
            })
            .collect();

        let state = PowersPanelState {
            powers: powers.clone(),
            scroll_offset: 0,
        };
        let text = rendered_text(&draw(&state, 100, 24));
        assert!(
            text.contains("12 powers"),
            "the count is still stated: {text}"
        );
        assert!(
            text.contains("showing 1–5"),
            "the window is stated when it is not the whole catalog: {text}"
        );

        let state = PowersPanelState {
            powers,
            scroll_offset: 7,
        };
        let text = rendered_text(&draw(&state, 100, 24));
        assert!(text.contains("Power 07"), "the window starts at the offset");
        assert!(
            text.contains("showing 8–12"),
            "…and the title follows the scroll: {text}"
        );
    }

    /// REGRESSION FENCE (cyril-v19o review finding 5). `place`'s clamp branch
    /// is the only geometry where the chrome arithmetic shows: the popup is
    /// capped to the rows above the input, so a window computed with the hooks
    /// panel's +4 — a header row THIS widget does not draw — drops the last
    /// power that fits. Five powers are 15 content rows plus two borders, so
    /// all five render.
    #[test]
    fn clamped_popup_still_shows_every_power_that_fits() {
        let powers: Vec<PowerInfo> = (0..5)
            .map(|n| {
                power(
                    &format!("power-{n:02}"),
                    Some(&format!("Power {n:02}")),
                    None,
                    &[],
                    false,
                )
            })
            .collect();
        let state = PowersPanelState {
            powers,
            scroll_offset: 0,
        };
        // 100x24 with the input top at row 18: the geometry the constant's own
        // comment cites, where `place` clamps the popup to rows 1..17.
        let text = rendered_text(&draw_at(&state, 100, 24, 18));
        assert!(
            text.contains("Power 00"),
            "the first power renders in the clamped popup: {text}"
        );
        assert!(
            text.contains("Power 04"),
            "the fifth fits in 15 content rows and must not be dropped: {text}"
        );
    }

    /// REGRESSION FENCE (cyril-v19o review finding 7). The steering marker is
    /// budgeted before the MCP list, so it survives a meta line that has to be
    /// truncated — losing it states by omission that the power ships no
    /// steering files.
    #[test]
    fn steering_marker_survives_a_truncated_meta_line() {
        let state = PowersPanelState {
            powers: vec![power(
                "aws-infrastructure-as-code",
                Some("Build AWS"),
                None,
                &["awslabs.aws-iac-mcp-server", "awslabs.cdk-mcp-server"],
                true,
            )],
            scroll_offset: 0,
        };
        let terminal = draw(&state, 60, 24);
        let meta = row_text(&terminal, 1);
        assert!(
            meta.contains("· steering"),
            "the marker survives the width budget: {meta:?}"
        );
        assert!(
            meta.contains('…'),
            "the server list is what gives way, not the marker: {meta:?}"
        );
    }

    /// REGRESSION FENCE (cyril-v19o C6): the viewport shows whole powers and
    /// clamps at the end, so a stale scroll offset cannot strand the panel past
    /// the catalog.
    #[test]
    fn viewport_window_clamps_scroll() {
        let powers: Vec<PowerInfo> = (0..12)
            .map(|n| {
                power(
                    &format!("power-{n:02}"),
                    Some(&format!("Power {n:02}")),
                    None,
                    &[],
                    false,
                )
            })
            .collect();
        let state = PowersPanelState {
            powers: powers.clone(),
            scroll_offset: 11,
        };
        let terminal = draw(&state, 100, 24);
        let text = rendered_text(&terminal);
        assert!(text.contains("Power 11"), "the last power renders");
        assert!(
            text.contains("Power 07"),
            "an offset past the last full window is pulled back to it, so the \
             viewport still shows five powers (review finding 8): {text}"
        );
        assert!(
            !text.contains("Power 06"),
            "…and the window is whole: {text}"
        );

        // Scrolled to the front, the window is exactly MAX_VISIBLE_POWERS tall.
        let state = PowersPanelState {
            powers,
            scroll_offset: 0,
        };
        let terminal = draw(&state, 100, 24);
        let text = rendered_text(&terminal);
        assert!(text.contains("Power 04"));
        assert!(!text.contains("Power 05"), "window caps at five powers");
    }

    /// REGRESSION FENCE (cyril-v19o review finding 8, advisory follow-up).
    ///
    /// The keyboard's scroll bound is the window the popup ACTUALLY has, not
    /// `MAX_VISIBLE_POWERS`: `modal::place` squeezes the popup into the rows
    /// above the input, and nine powers in an 18-row frame show three at a
    /// time. Bounded at `len - MAX_VISIBLE_POWERS` the keyboard stops at index
    /// 4, and since the widget clamps the viewport into the last full window
    /// (index 6), no reachable offset ever starts the window over the last
    /// powers — they are unreachable, not merely scrolled past.
    ///
    /// This drives the real `UiState` scroll and draws the real frame, so the
    /// state's bound and the widget's placement are compared where they meet.
    #[test]
    fn squeezed_viewport_reaches_the_last_power() {
        let mut ui = UiState::new(500);
        // An 18-row frame: chat gets 13 rows, so the input starts at row 14 and
        // the popup is clamped to rows 1..13 — three powers, not five.
        ui.set_terminal_size(100, 18);
        ui.show_powers_panel(nine_powers());
        ui.powers_panel_scroll_down(usize::MAX);
        assert_eq!(
            ui.powers_panel().expect("open").scroll_offset,
            9 - 3,
            "the bound is the squeezed window (9 powers, 3 visible), so the \
             viewport can still start over the last powers"
        );

        let terminal = draw_frame(&ui, 100, 18);
        let text = rendered_text(&terminal);
        assert!(
            text.contains("9 powers") && text.contains("showing 7–9"),
            "the frame really is squeezed to three powers: {text}"
        );
        assert!(
            text.contains("Power 08"),
            "the last power is reachable at the extreme scroll: {text}"
        );
        // …and the window is whole: the three visible powers hold the three
        // content rows each, ending on the last one.
        assert!(row_text(&terminal, 0).contains("Power 06"));
        assert!(row_text(&terminal, 3).contains("Power 07"));
        assert!(row_text(&terminal, 6).contains("Power 08"));
        assert!(
            !row_text(&terminal, 9).contains("Power"),
            "nothing is painted past the window's last power"
        );
    }

    /// REGRESSION FENCE (cyril-v19o review finding 8, second advisory).
    ///
    /// The stored offset can outlive the window it was clamped for.
    /// `set_terminal_size` only records the size, so a terminal that GROWS
    /// leaves the offset past the new last full window while the widget renders
    /// the view from that window's end. Up must normalize into the current
    /// window before it moves, or the first keypresses walk the stale number
    /// back into range with the viewport standing still.
    #[test]
    fn scroll_up_moves_after_the_window_grows() {
        let mut ui = UiState::new(500);
        ui.set_terminal_size(100, 18);
        ui.show_powers_panel(nine_powers());
        ui.powers_panel_scroll_down(usize::MAX);
        assert_eq!(
            ui.powers_panel().expect("open").scroll_offset,
            9 - 3,
            "the extreme offset for a three-power window"
        );

        // The terminal grows: five powers fit, so the old offset of 6 is past
        // the new bound of 4 and the widget renders the view from 4.
        ui.set_terminal_size(100, 24);
        ui.powers_panel_scroll_up(1);
        assert_eq!(
            ui.powers_panel().expect("open").scroll_offset,
            3,
            "Up normalizes into the wider window first, then moves by one"
        );

        let terminal = draw_frame(&ui, 100, 24);
        let text = rendered_text(&terminal);
        assert!(
            text.contains("showing 4–8"),
            "one Up moves the viewport one power up from the bottom of the \
             list: {text}"
        );
        assert!(
            row_text(&terminal, 0).contains("Power 03"),
            "…and the first visible row follows the offset: {}",
            row_text(&terminal, 0)
        );
    }

    /// REGRESSION FENCE (cyril-v19o C6): with no room above the input the panel
    /// paints nothing — in particular it must not `Clear` over the input.
    #[test]
    fn tiny_area_paints_nothing() {
        let state = PowersPanelState {
            powers: three_powers(),
            scroll_offset: 0,
        };
        let terminal = draw(&state, 40, 4);
        let text = rendered_text(&terminal);
        assert!(
            !text.contains("powers") && !text.contains("Build AWS"),
            "nothing is painted when there is no room above the input: {text:?}"
        );
    }
}
