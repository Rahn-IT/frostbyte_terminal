#![windows_subsystem = "windows"]

// pub mod threaded_writer;
mod ui;

#[cfg(unix)]
use iced_layershell::settings::{LayerShellSettings, StartMode};
use ui::UI;

fn main() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        #[cfg(unix)]
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
        iced::daemon(UI::start_winit, UI::update, UI::view)
            .font(include_bytes!("../fonts/RobotoMonoNerdFont-Regular.ttf"))
            .subscription(UI::subscription)
            .title(UI::title)
            .theme(|_, _| iced::Theme::Dark)
            .antialiasing(true)
            .run()
            .unwrap();
    }
}
