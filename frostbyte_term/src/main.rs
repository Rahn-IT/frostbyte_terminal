pub mod threaded_writer;
mod ui;

use ui::UI;

fn main() {
    iced::daemon(UI::title, UI::update, UI::view)
        .font(include_bytes!("../fonts/RobotoMonoNerdFont-Regular.ttf"))
        .subscription(UI::subscription)
        .theme(|_, _| iced::Theme::Dark)
        .antialiasing(true)
        .run_with(UI::start)
        .unwrap();
}
