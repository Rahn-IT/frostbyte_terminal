pub mod threaded_writer;
mod ui;

use iced::Font;
use ui::UI;

fn main() {
    iced::daemon(UI::title, UI::update, UI::view)
        .font(include_bytes!("../fonts/3270NerdFontMono-Regular.ttf"))
        .subscription(UI::subscription)
        .theme(|_, _| iced::Theme::Dark)
        .antialiasing(true)
        .run_with(UI::start)
        .unwrap();
}
