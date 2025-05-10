pub mod threaded_writer;
mod ui;

use iced_layershell::settings::{LayerShellSettings, StartMode};
use ui::UI;

fn main() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        iced_layershell::build_pattern::daemon(
            "frostbyte_terminal",
            UI::update,
            UI::view,
            UI::remove_id,
        )
        .font(include_bytes!("../fonts/RobotoMonoNerdFont-Regular.ttf"))
        .subscription(UI::subscription)
        .theme(|_| iced::Theme::Dark)
        .antialiasing(true)
        .layer_settings(LayerShellSettings {
            start_mode: StartMode::Background,
            ..Default::default()
        })
        .run_with(UI::start_layershell)
        .unwrap();
    } else {
        unsafe {
            // I need to actually add layershell support. Until then, we'll just fallback to X11
            std::env::remove_var("WAYLAND_DISPLAY");
        }

        iced::daemon(UI::title, UI::update, UI::view)
            .font(include_bytes!("../fonts/RobotoMonoNerdFont-Regular.ttf"))
            .subscription(UI::subscription)
            .theme(|_, _| iced::Theme::Dark)
            .antialiasing(true)
            .run_with(UI::start_winit)
            .unwrap();
    }
}
