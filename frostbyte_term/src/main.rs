mod ui;

use ui::UI;

fn main() {
    iced::application(UI::title, UI::update, UI::view)
        .subscription(UI::subscription)
        .theme(|_| iced::Theme::Dark)
        .antialiasing(true)
        .centered()
        .run_with(UI::start)
        .unwrap();
}
