#[cfg(any(feature = "iced-master", feature = "iced-013"))]
mod terminal;

#[cfg(all(
    any(feature = "iced-master", feature = "iced-013"),
    feature = "local-terminal"
))]
pub mod local_terminal;

#[cfg(any(feature = "iced-master", feature = "iced-013"))]
pub use terminal::{Action, Message, Terminal, TerminalSize, style::Style};
pub use wezterm_term::color::ColorPalette;

#[cfg(feature = "iced-master")]
pub(crate) use iced_master as iced;

#[cfg(feature = "iced-013")]
pub(crate) use iced_013 as iced;
