mod terminal;
mod terminal2;
mod terminal_grid;
mod wezterm;

#[cfg(feature = "local-terminal")]
pub mod local_terminal;
#[cfg(feature = "local-terminal")]
pub mod local_terminal2;

pub use terminal::{Action, Message, Terminal, TerminalSize, style::Style};
pub use terminal_grid::Size;
