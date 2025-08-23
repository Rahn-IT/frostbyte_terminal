mod terminal;

#[cfg(feature = "local-terminal")]
pub mod local_terminal;

pub use terminal::{Action, Message, Terminal, TerminalSize, style::Style};
pub use wezterm_term::color::ColorPalette;
