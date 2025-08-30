use std::{ops::Range, sync::Arc};

use termwiz::surface::CursorVisibility;
use tokio::sync::mpsc;
use tokio_stream::Stream;
use wezterm_term::{PhysRowIndex, TerminalConfiguration, TerminalSize, color::ColorPalette};

use crate::{
    terminal_grid::{Size, TerminalGrid, VisiblePosition},
    wezterm::selection::{SelectionPosition, SelectionState, is_selected},
};

pub mod prerenderer;
pub mod selection;

pub struct BridgedWriter {
    send: mpsc::Sender<Vec<u8>>,
}

impl std::io::Write for BridgedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.send.blocking_send(buf.to_vec()).is_ok() {
            Ok(buf.len())
        } else {
            Ok(0)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct Config {}

impl TerminalConfiguration for Config {
    fn color_palette(&self) -> wezterm_term::color::ColorPalette {
        ColorPalette::default()
    }
}

pub struct WeztermGrid {
    terminal: wezterm_term::Terminal,
    scroll_offset: usize,
    size: Size,
    selection: SelectionState,
}

impl WeztermGrid {
    pub fn new() -> (Self, impl Stream<Item = Vec<u8>>) {
        let term_size = wezterm_term::TerminalSize::default();
        let size = Size {
            rows: term_size.rows,
            cols: term_size.cols,
        };

        let (send, recv) = mpsc::channel(100);
        let recv = tokio_stream::wrappers::ReceiverStream::new(recv);

        let term = wezterm_term::Terminal::new(
            term_size,
            Arc::new(Config {}),
            "frozen_term",
            env!("CARGO_PKG_VERSION"),
            Box::new(BridgedWriter { send }),
        );

        (
            Self {
                terminal: term,
                scroll_offset: 0,
                selection: SelectionState::new(),
                size,
            },
            recv,
        )
    }

    fn invalidate_lines(&mut self, mut invalidate: Range<PhysRowIndex>) {
        self.terminal.increment_seqno();
        let seqno = self.terminal.current_seqno();
        invalidate.start = invalidate.start.max(self.min_scroll());
        invalidate.end = invalidate
            .end
            .min(self.max_scroll() + self.terminal.screen().physical_rows);

        let screen = self.terminal.screen_mut();
        let invalidate = screen.stable_range(&(invalidate.start as isize..invalidate.end as isize));
        screen.with_phys_lines_mut(invalidate, |lines| {
            for line in lines {
                line.update_last_change_seqno(seqno);
            }
        });
    }

    fn min_scroll(&self) -> usize {
        let screen = self.terminal.screen();
        let max_stable_index = screen.phys_to_stable_row_index(screen.scrollback_rows()) as usize;
        max_stable_index.saturating_sub(screen.scrollback_rows())
    }

    fn max_scroll(&self) -> usize {
        let screen = self.terminal.screen();
        let max_stable_index = screen.phys_to_stable_row_index(screen.scrollback_rows()) as usize;
        max_stable_index.saturating_sub(screen.physical_rows)
    }

    fn inverse_offset(&self) -> usize {
        self.max_scroll().saturating_sub(self.scroll_offset)
    }

    fn update_scroll(&mut self, new_offset: usize) {
        self.scroll_offset = new_offset.min(self.max_scroll()).max(self.min_scroll());
        if let Some(invalidate) = self.selection.set_scroll(self.scroll_offset) {
            self.invalidate_lines(invalidate);
        }
    }

    fn screen_lines(&self, range: Range<usize>) -> Vec<wezterm_term::Line> {
        let screen = self.terminal.screen();
        let range = screen.stable_range(&(range.start as isize..range.end as isize));
        screen.lines_in_phys_range(range)
    }
}

impl TerminalGrid for WeztermGrid {
    fn advance_bytes(&mut self, bytes: &[u8]) {
        let auto_scroll = self.scroll_offset == self.max_scroll();
        self.terminal.advance_bytes(bytes);
        if auto_scroll {
            self.update_scroll(self.max_scroll());
        } else {
            self.update_scroll(self.scroll_offset);
        }
    }

    fn resize(&mut self, size: Size) {
        let diff = self.size.rows.abs_diff(size.rows);

        let new_scroll = if size.rows > self.size.rows {
            self.scroll_offset - diff
        } else {
            self.scroll_offset + diff
        };

        self.terminal.resize(TerminalSize {
            cols: size.cols,
            rows: size.rows,
            ..Default::default()
        });
        self.size = size;
        self.update_scroll(new_scroll);
    }

    fn press_key(
        &mut self,
        key: iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    ) -> Option<Vec<u8>> {
        if let Some((key, modifiers)) = transform_key(key, modifiers) {
            let _ = self.terminal.key_down(key, modifiers);
            self.update_scroll(self.max_scroll());
        }
        None
    }

    fn paste(&mut self, text: &str) -> Option<Vec<u8>> {
        let _ = self.terminal.send_paste(text);
        None
    }

    fn scroll(&mut self, lines: isize) {
        self.update_scroll(
            self.scroll_offset
                .saturating_add_signed(-lines)
                .min(self.max_scroll()),
        );
    }

    fn get_scroll(&self) -> usize {
        self.scroll_offset
    }

    fn available_lines(&self) -> usize {
        self.terminal.screen().scrollback_rows()
    }

    fn start_selection(&mut self, start: VisiblePosition) {
        if let Some(invalidate) = self.selection.start(start) {
            self.invalidate_lines(invalidate);
        }
    }

    fn move_selection(&mut self, end: VisiblePosition) {
        if let Some(invalidate) = self.selection.move_end(end) {
            self.invalidate_lines(invalidate);
        }
    }

    fn end_selection(&mut self) {
        self.selection.finish()
    }

    fn currently_selecting(&self) -> bool {
        self.selection.is_active()
    }

    fn selected_text(&self) -> Option<String> {
        let selection = self.selection.get_selection()?;
        let screen = self.terminal.screen();

        let range = selection.start.y..(selection.end.y + 1).min(screen.scrollback_rows());

        let mut clipboard = String::new();

        for (offset, line) in self.screen_lines(range.clone()).iter().enumerate() {
            let index = range.start + offset;
            for (cell_index, cell) in line.visible_cells().enumerate() {
                if is_selected(
                    &selection,
                    SelectionPosition {
                        x: cell_index,
                        y: index,
                    },
                ) {
                    clipboard.push_str(&cell.str());
                }
            }
            clipboard.push('\n');
        }

        let clipboard = clipboard.trim().to_string();

        if !clipboard.is_empty() {
            Some(clipboard)
        } else {
            None
        }
    }

    fn get_title(&self) -> &str {
        self.terminal.get_title()
    }

    fn get_size(&self) -> Size {
        self.size
    }

    fn get_cursor(&self) -> Option<VisiblePosition> {
        let pos = self.terminal.cursor_pos();
        let y = (pos.y as usize) + self.inverse_offset();

        if y < self.size.rows && pos.visibility == CursorVisibility::Visible {
            Some(VisiblePosition { x: pos.x, y })
        } else {
            None
        }
    }
}

fn transform_key(
    key: iced::keyboard::Key,
    modifiers: iced::keyboard::Modifiers,
) -> Option<(wezterm_term::KeyCode, wezterm_term::KeyModifiers)> {
    let wez_key = match key {
        iced::keyboard::Key::Character(c) => {
            let c = c.chars().next().unwrap();
            Some(wezterm_term::KeyCode::Char(c))
        }
        iced::keyboard::Key::Named(named) => match named {
            iced::keyboard::key::Named::Enter => Some(wezterm_term::KeyCode::Enter),
            iced::keyboard::key::Named::Space => Some(wezterm_term::KeyCode::Char(' ')),
            iced::keyboard::key::Named::Backspace => Some(wezterm_term::KeyCode::Backspace),
            iced::keyboard::key::Named::Delete => Some(wezterm_term::KeyCode::Delete),
            iced::keyboard::key::Named::ArrowLeft => Some(wezterm_term::KeyCode::LeftArrow),
            iced::keyboard::key::Named::ArrowRight => Some(wezterm_term::KeyCode::RightArrow),
            iced::keyboard::key::Named::ArrowUp => Some(wezterm_term::KeyCode::UpArrow),
            iced::keyboard::key::Named::ArrowDown => Some(wezterm_term::KeyCode::DownArrow),
            iced::keyboard::key::Named::Tab => Some(wezterm_term::KeyCode::Tab),
            iced::keyboard::key::Named::Escape => Some(wezterm_term::KeyCode::Escape),
            iced::keyboard::key::Named::Home => Some(wezterm_term::KeyCode::Home),
            iced::keyboard::key::Named::End => Some(wezterm_term::KeyCode::End),
            iced::keyboard::key::Named::PageUp => Some(wezterm_term::KeyCode::PageUp),
            iced::keyboard::key::Named::PageDown => Some(wezterm_term::KeyCode::PageDown),
            iced::keyboard::key::Named::F1 => Some(wezterm_term::KeyCode::Function(1)),
            iced::keyboard::key::Named::F2 => Some(wezterm_term::KeyCode::Function(2)),
            iced::keyboard::key::Named::F3 => Some(wezterm_term::KeyCode::Function(3)),
            iced::keyboard::key::Named::F4 => Some(wezterm_term::KeyCode::Function(4)),
            iced::keyboard::key::Named::F5 => Some(wezterm_term::KeyCode::Function(5)),
            iced::keyboard::key::Named::F6 => Some(wezterm_term::KeyCode::Function(6)),
            iced::keyboard::key::Named::F7 => Some(wezterm_term::KeyCode::Function(7)),
            iced::keyboard::key::Named::F8 => Some(wezterm_term::KeyCode::Function(8)),
            iced::keyboard::key::Named::F9 => Some(wezterm_term::KeyCode::Function(9)),
            iced::keyboard::key::Named::F10 => Some(wezterm_term::KeyCode::Function(10)),
            iced::keyboard::key::Named::F11 => Some(wezterm_term::KeyCode::Function(11)),
            iced::keyboard::key::Named::F12 => Some(wezterm_term::KeyCode::Function(12)),
            iced::keyboard::key::Named::F13 => Some(wezterm_term::KeyCode::Function(13)),
            iced::keyboard::key::Named::F14 => Some(wezterm_term::KeyCode::Function(14)),
            iced::keyboard::key::Named::F15 => Some(wezterm_term::KeyCode::Function(15)),
            iced::keyboard::key::Named::F16 => Some(wezterm_term::KeyCode::Function(16)),
            iced::keyboard::key::Named::F17 => Some(wezterm_term::KeyCode::Function(17)),
            iced::keyboard::key::Named::F18 => Some(wezterm_term::KeyCode::Function(18)),
            iced::keyboard::key::Named::F19 => Some(wezterm_term::KeyCode::Function(19)),
            iced::keyboard::key::Named::F20 => Some(wezterm_term::KeyCode::Function(20)),
            iced::keyboard::key::Named::F21 => Some(wezterm_term::KeyCode::Function(21)),
            iced::keyboard::key::Named::F22 => Some(wezterm_term::KeyCode::Function(22)),
            iced::keyboard::key::Named::F23 => Some(wezterm_term::KeyCode::Function(23)),
            iced::keyboard::key::Named::F24 => Some(wezterm_term::KeyCode::Function(24)),
            iced::keyboard::key::Named::F25 => Some(wezterm_term::KeyCode::Function(25)),
            iced::keyboard::key::Named::F26 => Some(wezterm_term::KeyCode::Function(26)),
            iced::keyboard::key::Named::F27 => Some(wezterm_term::KeyCode::Function(27)),
            iced::keyboard::key::Named::F28 => Some(wezterm_term::KeyCode::Function(28)),
            iced::keyboard::key::Named::F29 => Some(wezterm_term::KeyCode::Function(29)),
            iced::keyboard::key::Named::F30 => Some(wezterm_term::KeyCode::Function(30)),
            iced::keyboard::key::Named::F31 => Some(wezterm_term::KeyCode::Function(31)),
            iced::keyboard::key::Named::F32 => Some(wezterm_term::KeyCode::Function(32)),
            iced::keyboard::key::Named::F33 => Some(wezterm_term::KeyCode::Function(33)),
            iced::keyboard::key::Named::F34 => Some(wezterm_term::KeyCode::Function(34)),
            iced::keyboard::key::Named::F35 => Some(wezterm_term::KeyCode::Function(35)),
            _ => None,
        },
        _ => None,
    };

    match wez_key {
        None => None,
        Some(key) => {
            let mut wez_modifiers = wezterm_term::KeyModifiers::empty();

            if modifiers.shift() {
                wez_modifiers |= wezterm_term::KeyModifiers::SHIFT;
            }
            if modifiers.alt() {
                wez_modifiers |= wezterm_term::KeyModifiers::ALT;
            }
            if modifiers.control() {
                wez_modifiers |= wezterm_term::KeyModifiers::CTRL;
            }
            if modifiers.logo() {
                wez_modifiers |= wezterm_term::KeyModifiers::SUPER;
            }

            Some((key, wez_modifiers))
        }
    }
}
