#!/usr/bin/env bash
set -euo pipefail

readonly PINNED_COMMIT=80f3ffa5a7ced20e33c9b98c782c08af704407d5
readonly ROOT="$(git rev-parse --show-toplevel)"
readonly OUTPUT="${1:-$ROOT/crates/cyril-ui/src/fixtures/conversation-theme-baseline.tsv}"
readonly WORKTREE="$(mktemp -d "$ROOT/target/cyril-ghuu-baseline-worktree.XXXXXX")"
readonly RAW_OUTPUT="$ROOT/target/cyril-ghuu-baseline-raw.txt"

cleanup() {
  git -C "$ROOT" worktree remove --force "$WORKTREE" >/dev/null 2>&1 || true
}
trap cleanup EXIT

git -C "$ROOT" worktree add --detach "$WORKTREE" "$PINNED_COMMIT" >/dev/null

cat >> "$WORKTREE/crates/cyril-ui/src/render.rs" <<'RUST'

#[cfg(test)]
mod cyril_ghuu_baseline {
    use std::fmt::Write as _;
    use std::time::Duration;

    use cyril_core::types::Plan;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;
    use ratatui::Terminal;

    use crate::traits::test_support::MockTuiState;
    use crate::traits::{Activity, ChatMessage, ChatMessageKind, SteerEchoStatus};

    fn steer(text: &str, status: SteerEchoStatus) -> ChatMessage {
        ChatMessage {
            kind: ChatMessageKind::SteerEcho {
                text: text.to_string(),
                status,
            },
            timestamp: std::time::Instant::now(),
        }
    }

    fn message_state() -> MockTuiState {
        MockTuiState {
            messages: vec![
                ChatMessage::user_text("user".into()),
                ChatMessage::agent_text(String::new()),
                ChatMessage::thought("thought".into()),
                ChatMessage::plan(Plan::new(Vec::new())),
                ChatMessage::system("system".into()),
                ChatMessage::command_output("context".into(), String::new()),
                steer("queued", SteerEchoStatus::Queued),
                steer("applied", SteerEchoStatus::Applied),
                steer("cleared", SteerEchoStatus::Cleared),
                steer("unsupported", SteerEchoStatus::Unsupported),
            ],
            activity: Activity::ToolRunning,
            activity_elapsed: Some(Duration::from_secs(1)),
            ..MockTuiState::default()
        }
    }

    fn render_message_scene() -> anyhow::Result<Buffer> {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend)?;
        let state = message_state();
        terminal.draw(|frame| crate::widgets::chat::render(frame, frame.area(), &state))?;
        Ok(terminal.backend().buffer().clone())
    }

    fn normalize_color(color: Color) -> String {
        let rgb = match color {
            Color::Reset => return "DEFAULT".to_string(),
            Color::Black => (0x00, 0x00, 0x00),
            Color::Red => (0x80, 0x00, 0x00),
            Color::Green => (0x00, 0x80, 0x00),
            Color::Yellow => (0x80, 0x80, 0x00),
            Color::Blue => (0x00, 0x00, 0x80),
            Color::Magenta => (0x80, 0x00, 0x80),
            Color::Cyan => (0x00, 0x80, 0x80),
            Color::Gray => (0xc0, 0xc0, 0xc0),
            Color::DarkGray => (0x80, 0x80, 0x80),
            Color::LightRed => (0xff, 0x00, 0x00),
            Color::LightGreen => (0x00, 0xff, 0x00),
            Color::LightYellow => (0xff, 0xff, 0x00),
            Color::LightBlue => (0x00, 0x00, 0xff),
            Color::LightMagenta => (0xff, 0x00, 0xff),
            Color::LightCyan => (0x00, 0xff, 0xff),
            Color::White => (0xff, 0xff, 0xff),
            Color::Rgb(red, green, blue) => (red, green, blue),
            Color::Indexed(index) => return format!("INDEX:{index}"),
        };
        format!("RGB:{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2)
    }

    fn symbol_hex(symbol: &str) -> String {
        let mut encoded = String::with_capacity(symbol.len() * 2);
        for byte in symbol.as_bytes() {
            write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
        }
        encoded
    }

    fn normalized_rows(scene: &str, buffer: &Buffer) -> String {
        let mut output = String::new();
        for y in 0..24 {
            for x in 0..80 {
                let cell = buffer.cell((x, y)).expect("80x24 cell exists");
                writeln!(
                    &mut output,
                    "{scene}\t{x}\t{y}\t{}\t{}\t{}\t{}",
                    symbol_hex(cell.symbol()),
                    normalize_color(cell.fg),
                    normalize_color(cell.bg),
                    cell.modifier.bits(),
                )
                .expect("writing to String cannot fail");
            }
        }
        output
    }

    #[test]
    fn emit_message_scene() -> anyhow::Result<()> {
        let first = render_message_scene()?;
        let second = render_message_scene()?;
        let first_rows = normalized_rows("messages", &first);
        assert_eq!(first_rows, normalized_rows("messages", &second));
        assert_eq!(first_rows.lines().count(), 1_920);

        let symbols = first
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for label in [
            "You:",
            "Kiro:",
            "thought",
            "Plan:",
            "system",
            "/context:",
            "queued",
            "applied",
            "cleared",
            "not supported",
            "Running... 1s",
        ] {
            assert!(symbols.contains(label), "missing message-scene label {label:?}");
        }

        assert_eq!(normalize_color(Color::Cyan), "RGB:008080");
        assert_eq!(normalize_color(Color::LightCyan), "RGB:00ffff");
        assert_eq!(normalize_color(Color::Reset), "DEFAULT");
        assert_eq!(normalize_color(Color::Rgb(1, 2, 3)), "RGB:010203");
        assert_eq!(normalize_color(Color::Indexed(42)), "INDEX:42");

        println!("BEGIN_CYRIL_GHUU_BASELINE");
        print!("{first_rows}");
        println!("END_CYRIL_GHUU_BASELINE");
        Ok(())
    }
}
RUST

mkdir -p "$(dirname "$OUTPUT")" "$(dirname "$RAW_OUTPUT")"
CARGO_TARGET_DIR="$ROOT/target/cyril-ghuu-baseline" \
  cargo test --manifest-path "$WORKTREE/Cargo.toml" -p cyril-ui \
  render::cyril_ghuu_baseline::emit_message_scene -- --exact --nocapture > "$RAW_OUTPUT"

{
  printf 'commit\t%s\n' "$PINNED_COMMIT"
  printf 'scene\tx\ty\tsymbol_hex\tforeground\tbackground\tmodifier_bits\n'
  awk '/BEGIN_CYRIL_GHUU_BASELINE/{capture=1; next} /END_CYRIL_GHUU_BASELINE/{capture=0} capture' "$RAW_OUTPUT"
} > "$OUTPUT"

data_rows=$(awk 'NR > 2 {count++} END {print count + 0}' "$OUTPUT")
if [[ "$data_rows" -ne 1920 ]]; then
  printf 'expected 1920 baseline cells, found %s\n' "$data_rows" >&2
  exit 1
fi
if [[ "$(head -n 1 "$OUTPUT")" != $'commit\t'"$PINNED_COMMIT" ]]; then
  printf 'baseline commit header mismatch\n' >&2
  exit 1
fi
printf 'generated %s cells at %s\n' "$data_rows" "$OUTPUT" >&2
