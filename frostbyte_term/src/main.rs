pub mod threaded_writer;
mod ui;

use ui::UI;

fn main() {
    unsafe {
        // I need to actually add layershell support. Until then, we'll just fallback to X11
        std::env::remove_var("WAYLAND_DISPLAY");
    }

    iced::daemon(UI::title, UI::update, UI::view)
        .font(include_bytes!("../fonts/RobotoMonoNerdFont-Regular.ttf"))
        .subscription(UI::subscription)
        .theme(|_, _| iced::Theme::Dark)
        .antialiasing(true)
        .run_with(UI::start)
        .unwrap();
}
