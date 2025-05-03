pub mod threaded_writer;
mod ui;

use iced::Font;
use ui::UI;

fn main() {
    iced::daemon(UI::title, UI::update, UI::view)
        .font(include_bytes!("../fonts/3270NerdFontMono-Regular.ttf"))
        .default_font(Font::with_name("3270 Nerd Font Mono"))
        .subscription(UI::subscription)
        .theme(|_, _| iced::Theme::Dark)
        .antialiasing(true)
        .run_with(UI::start)
        .unwrap();
}
