mod scrollbar;
mod terminal;
mod terminal_grid;
mod wezterm;

#[cfg(feature = "local-terminal")]
pub mod local_terminal;

pub use terminal::{
    Action, Message, Terminal,
    style::{CursorShape, Palette256, Style},
};
pub use terminal_grid::Size;
