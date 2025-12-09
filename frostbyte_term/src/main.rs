// #![windows_subsystem = "windows"]

// pub mod threaded_writer;
mod ui;

#[cfg(target_os = "linux")]
use iced_layershell::settings::{LayerShellSettings, StartMode};
use ui::UI;

const FONT: &[u8] = include_bytes!("../fonts/RobotoMonoNerdFont-Regular.ttf");

fn main() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("DEBUG").is_none() {
        run_layershell();
    } else {
        run_iced();
    }
    #[cfg(any(windows, target_os = "macos"))]
    run_iced();
}

fn run_iced() {
    iced::daemon(UI::start_winit, UI::update, UI::view)
        .font(FONT)
        .subscription(UI::subscription)
        .title(UI::title)
        .theme(iced::Theme::Dark)
        .antialiasing(true)
        .run()
        .unwrap();
}

#[cfg(target_os = "linux")]
fn run_layershell() {
    iced_layershell::build_pattern::daemon(
        UI::start_layershell,
        "frostbyte_terminal",
        UI::update,
        UI::view,
    )
    .font(FONT)
    .subscription(UI::subscription)
    .theme(|_: &'_ UI, _| iced::Theme::Dark)
    .antialiasing(true)
    .layer_settings(LayerShellSettings {
        start_mode: StartMode::Background,
        ..Default::default()
    })
    .run()
    .unwrap();
}
