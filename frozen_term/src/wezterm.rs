use std::{io::Write, sync::Arc};

use termwiz::surface::CursorVisibility;
use wezterm_term::{TerminalConfiguration, TerminalSize, color::ColorPalette};

use crate::terminal_grid::{Cursor, Size, TerminalGrid};

pub mod prerenderer;

pub struct VoidWriter {}

impl Write for VoidWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
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
}

impl WeztermGrid {
    pub fn new() -> Self {
        let term_size = wezterm_term::TerminalSize::default();
        let size = Size {
            rows: term_size.rows,
            cols: term_size.cols,
        };

        let term = wezterm_term::Terminal::new(
            term_size,
            Arc::new(Config {}),
            "frozen_term",
            env!("CARGO_PKG_VERSION"),
            Box::new(VoidWriter {}),
        );

        Self {
            terminal: term,
            scroll_offset: 0,
            size,
        }
    }

    fn max_scroll(&self) -> usize {
        let screen = self.terminal.screen();
        screen
            .scrollback_rows()
            .saturating_sub(screen.physical_rows)
    }
}

impl TerminalGrid for WeztermGrid {
    fn advance_bytes(&mut self, bytes: &[u8]) {
        let auto_scroll = self.scroll_offset == self.max_scroll();
        self.terminal.advance_bytes(bytes);
        if auto_scroll {
            self.scroll_offset = self.max_scroll();
        }
    }

    fn resize(&mut self, size: Size) {
        println!("resizing to {:?}", size);
        self.terminal.resize(TerminalSize {
            cols: size.cols,
            rows: size.rows,
            ..Default::default()
        });
        self.size = size;
        self.scroll_offset = self.scroll_offset.min(self.max_scroll());
    }

    fn press_key(
        &mut self,
        key: iced::keyboard::Key,
        modifiers: iced::keyboard::Modifiers,
    ) -> Option<Vec<u8>> {
        if let Some((wez_key, wez_mods)) = transform_key(key, modifiers) {
            if let Some(encoded) = wez_key
                .encode(
                    wez_mods,
                    termwiz::input::KeyCodeEncodeModes {
                        #[cfg(unix)]
                        encoding: termwiz::input::KeyboardEncoding::Xterm,
                        #[cfg(windows)]
                        encoding: termwiz::input::KeyboardEncoding::Win32,
                        application_cursor_keys: false,
                        newline_mode: false,
                        modify_other_keys: None,
                    },
                    true,
                )
                .ok()
            {
                Some(encoded.into_bytes())
            } else {
                None
            }
        } else {
            None
        }
    }

    fn paste(&mut self, text: &str) -> Vec<u8> {
        text.replace("\x1b[200~", "")
            .replace("\x1b[201~", "")
            .as_bytes()
            .to_vec()
    }

    fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self
            .scroll_offset
            .saturating_add(lines)
            .min(self.max_scroll());
    }

    fn get_title(&self) -> &str {
        self.terminal.get_title()
    }

    fn get_size(&self) -> Size {
        self.size
    }

    fn get_cursor(&self) -> Cursor {
        let pos = self.terminal.cursor_pos();
        Cursor {
            x: pos.x,
            y: pos.y as usize,
            visible: pos.visibility == CursorVisibility::Visible,
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
