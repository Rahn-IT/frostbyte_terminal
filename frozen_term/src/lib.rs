#[cfg(any(feature = "iced-master", feature = "iced-013"))]
mod terminal;

#[cfg(any(feature = "iced-master", feature = "iced-013"))]
pub use terminal::Action;
#[cfg(any(feature = "iced-master", feature = "iced-013"))]
pub use terminal::MessageWrapper as Message;
#[cfg(any(feature = "iced-master", feature = "iced-013"))]
pub use terminal::Terminal;
#[cfg(any(feature = "iced-master", feature = "iced-013"))]
pub use terminal::TerminalSize;
