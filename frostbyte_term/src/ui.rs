use std::{collections::BTreeMap, fmt::Debug};

#[cfg(target_os = "linux")]
use std::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[cfg(target_os = "linux")]
use signal_hook::consts::signal::SIGUSR1;
#[cfg(target_os = "linux")]
use signal_hook::flag as signal_flag;

use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey};
use iced::{
    Element, Font, Length, Subscription, Task,
    futures::SinkExt,
    keyboard,
    stream::channel,
    widget::{button, center, column, row, text},
    window,
};
#[cfg(target_os = "linux")]
use iced_layershell::reexport::{Anchor, NewLayerShellSettings};
use image::GenericImageView;
use local_terminal::LocalTerminal;
use sipper::Stream;
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder};

mod local_terminal;

/// Messages emitted by the application and its widgets.
#[cfg_attr(target_os = "linux", iced_layershell::to_layer_message(multi))]
#[derive(Debug, Clone)]
pub enum Message {
    LocalTerminal {
        id: u32,
        message: local_terminal::Message,
    },
    OpenTab,
    SwitchTab(u32),
    CloseTab(u32),
    Hotkey,
    WindowOpened(window::Id),
    CloseWindow,
    WindowClosed,
    Shutdown,
    // This does nothing as is only here to trigger a redraw
    Redraw,
}

enum Mode {
    Winit,
    #[cfg(target_os = "linux")]
    Layershell,
}

const ICON: &'static [u8] = include_bytes!("../assets/icon.png");

pub struct UI {
    terminals: BTreeMap<u32, LocalTerminal>,
    window_id: Option<window::Id>,
    selected_tab: u32,
    new_terminal_id: u32,
    _hotkey_manager: GlobalHotKeyManager,
    hotkey: Hotkey,
    hotkey_id: u32,
    _tray_icon: Option<TrayIcon>,
    mode: Mode,
}

impl Debug for UI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UI")
            .field("window_id", &self.window_id)
            .field("selected_tab", &self.selected_tab)
            .field("new_terminal_id", &self.new_terminal_id)
            .field("hotkey_id", &self.hotkey_id)
            .finish()
    }
}

impl UI {
    fn create_tray_icon() -> TrayIcon {
        let close_item = tray_icon::menu::MenuItem::new("Exit Frostbyte", true, None);
        let tray_menu = tray_icon::menu::Menu::new();
        tray_menu.append(&close_item).unwrap();

        let icon = image::load_from_memory_with_format(ICON, image::ImageFormat::Png).unwrap();
        let (width, height) = icon.dimensions();
        let icon_data = icon.into_rgba8().to_vec();

        TrayIconBuilder::new()
            .with_tooltip("Frostbyte")
            .with_menu(Box::new(tray_menu))
            .with_menu_on_left_click(false)
            .with_icon(tray_icon::Icon::from_rgba(icon_data, width, height).unwrap())
            .build()
            .unwrap()
    }

    pub fn start_winit() -> (Self, Task<Message>) {
        Self::start_in_mode(Mode::Winit)
    }

    #[cfg(target_os = "linux")]
    pub fn start_layershell() -> (Self, Task<Message>) {
        Self::start_in_mode(Mode::Layershell)
    }

    fn start_in_mode(mode: Mode) -> (Self, Task<Message>) {
        #[cfg(target_os = "linux")]
        std::thread::spawn(|| {
            gtk::init().unwrap();
            let _tray_icon = Self::create_tray_icon();

            gtk::main();
        });
        #[cfg(target_os = "linux")]
        let tray_icon = None;
        #[cfg(not(target_os = "linux"))]
        let tray_icon = Some(Self::create_tray_icon());

        let terminals = BTreeMap::new();

        let hotkey = Hotkey::default();
        let global_hotkey = hotkey.global_hotkey();
        let hotkey_id = global_hotkey.id;
        let hotkey_manager = GlobalHotKeyManager::new().unwrap();
        hotkey_manager.register(global_hotkey).unwrap();

        (
            Self {
                terminals,
                window_id: None,
                selected_tab: 1,
                new_terminal_id: 1,
                _hotkey_manager: hotkey_manager,
                hotkey_id,
                hotkey,
                _tray_icon: tray_icon,
                mode,
            },
            Task::none(),
        )
    }

    #[must_use]
    pub fn update<'a>(&'a mut self, message: Message) -> Task<Message> {
        match message {
            Message::LocalTerminal { id, message } => {
                let term = match self.terminals.get_mut(&id) {
                    None => return Task::none(),
                    Some(term) => term,
                };

                let action = term.update(message);

                match action {
                    local_terminal::Action::Close => self.close_tab(id),
                    local_terminal::Action::Run(task) => {
                        task.map(move |message| Message::LocalTerminal { id, message })
                    }
                    local_terminal::Action::IdChanged => self.focus_tab(),
                    local_terminal::Action::None => Task::none(),
                }
            }
            Message::OpenTab => self.open_tab(),
            Message::SwitchTab(id) => {
                self.switch_tab(id);
                Task::none()
            }
            Message::CloseTab(id) => self.close_tab(id),
            Message::Hotkey => {
                return if self.window_id.is_some() {
                    self.close_window()
                } else {
                    self.open_window()
                };
            }
            Message::WindowOpened(id) => {
                if let Some(term) = self.terminals.get(&self.selected_tab) {
                    Task::batch([window::gain_focus(id), term.focus()])
                } else {
                    Task::none()
                }
            }
            Message::CloseWindow => self.close_window(),
            Message::WindowClosed => {
                self.window_id = None;
                Task::none()
            }
            Message::Shutdown => iced::exit(),
            // only here to trigger a redraw
            Message::Redraw => Task::none(),
            #[cfg(target_os = "linux")]
            Message::AnchorChange { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::SetInputRegion { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::AnchorSizeChange { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::LayerChange { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::MarginChange { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::SizeChange { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::VirtualKeyboardPressed { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::NewLayerShell { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::NewPopUp { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::NewMenu { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::RemoveWindow(_) => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::ForgetLastOutput => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::ExclusiveZoneChange { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::NewInputPanel { .. } => unreachable!(),
            #[cfg(target_os = "linux")]
            Message::NewBaseWindow { .. } => unreachable!(),
        }
    }

    fn open_window(&mut self) -> Task<Message> {
        if let Some(id) = self.window_id {
            window::gain_focus(id)
        } else {
            let task = match self.mode {
                Mode::Winit => {
                    let settings = window::Settings {
                        decorations: false,
                        resizable: false,
                        position: window::Position::SpecificWith(|window_size, monitor_res| {
                            let x = (monitor_res.width - window_size.width) / 2.0;
                            iced::Point::new(x, 0.0)
                        }),
                        size: iced::window::Size::FromScreensize(|monitor_res| {
                            iced::Size::new(monitor_res.width * 0.8, monitor_res.height * 0.45)
                        }),
                        level: window::Level::AlwaysOnTop,

                        ..Default::default()
                    };

                    let (id, task) = window::open(settings);
                    self.window_id = Some(id);

                    task.map(Message::WindowOpened)
                }
                #[cfg(target_os = "linux")]
                Mode::Layershell => {
                    let id = window::Id::unique();

                    self.window_id = Some(id);
                    Task::done(Message::NewLayerShell {
                        settings: NewLayerShellSettings {
                            anchor: Anchor::Top | Anchor::Left | Anchor::Right,
                            margin: Some((0, 200, 0, 200)),
                            size: Some((0, 600)),
                            ..Default::default()
                        },
                        id,
                    })
                    .chain(Task::done(Message::WindowOpened(id)))
                }
            };

            if self.terminals.is_empty() {
                Task::batch([task, self.open_tab()])
            } else {
                task
            }
        }
    }

    fn close_window(&mut self) -> Task<Message> {
        if let Some(id) = self.window_id {
            self.window_id = None;
            window::close(id)
        } else {
            Task::none()
        }
    }

    fn open_tab(&mut self) -> Task<Message> {
        let (local_terminal, terminal_task) = LocalTerminal::start(
            Some(Font::with_name("RobotoMono Nerd Font")),
            self.hotkey.filter(),
        );
        let id = self.new_terminal_id;
        self.new_terminal_id += 1;

        self.terminals.insert(id, local_terminal);
        self.selected_tab = id;

        terminal_task.map(move |message| Message::LocalTerminal { id, message })
    }

    fn focus_tab(&self) -> Task<Message> {
        if let Some(term) = self.terminals.get(&self.selected_tab) {
            // the chained redraw message is required for the layer shell implementation
            term.focus().chain(Task::done(Message::Redraw))
        } else {
            Task::none()
        }
    }

    fn close_tab(&mut self, id: u32) -> Task<Message> {
        self.terminals.remove(&id);

        if let Some((id, _term)) = self.terminals.iter().next() {
            self.selected_tab = *id;
            Task::none()
        } else {
            self.close_window()
        }
    }

    fn switch_tab(&mut self, id: u32) {
        if let Some(_terminal) = self.terminals.get(&id) {
            self.selected_tab = id;
        }
    }

    pub fn view<'a>(&'a self, _id: window::Id) -> Element<'a, Message> {
        let selected_terminal = self.terminals.get(&self.selected_tab);

        let tab_view: Element<Message> = match selected_terminal {
            Some(terminal) => terminal
                .view()
                .map(move |message| Message::LocalTerminal {
                    id: self.selected_tab,
                    message,
                })
                .into(),
            None => text("terminal closed").into(),
        };

        let tab_bar = row(self.terminals.iter().map(|(id, terminal)| {
            let style = if id == &self.selected_tab {
                button::secondary
            } else {
                button::primary
            };
            button(row![
                center(text(terminal.get_title())),
                button(text("X").center())
                    .on_press(Message::CloseTab(id.clone()))
                    .width(30)
                    .style(button::danger)
            ])
            .on_press(Message::SwitchTab(id.clone()))
            .style(style)
            .width(200)
            .height(Length::Fill)
            .into()
        }))
        .spacing(5);

        column![
            tab_view,
            tab_bar
                .push(
                    button(center(text("New Tab")))
                        .width(200)
                        .height(Length::Fill)
                        .on_press(Message::OpenTab),
                )
                .push(iced::widget::horizontal_space())
                .push(
                    button(center(text("X")))
                        .style(button::danger)
                        .width(40)
                        .height(Length::Fill)
                        .on_press(Message::CloseWindow)
                )
                .height(40)
        ]
        .height(40)
        .height(Length::Fill)
        .into()
    }

    pub fn title(&self, _id: window::Id) -> String {
        let selected_terminal = self.terminals.get(&self.selected_tab);

        match selected_terminal {
            Some(terminal) => terminal.get_title().to_string(),
            None => "frozen_term".to_string(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            window::close_events().map(|_| Message::WindowClosed),
            Subscription::run(poll_events_sub),
            keyboard::on_key_press(|key, modifiers| match key {
                keyboard::Key::Named(keyboard::key::Named::Pause) => None,
                keyboard::Key::Character(c) => match c.as_str() {
                    "t" | "T" => {
                        if modifiers.control() && modifiers.shift() {
                            Some(Message::OpenTab)
                        } else {
                            None
                        }
                    }
                    _ => None,
                },
                keyboard::Key::Named(_named) => None,
                keyboard::Key::Unidentified => None,
            }),
        ])
    }
}

/// Stolen from the tauri global hotkey example for iced
fn poll_events_sub() -> impl Stream<Item = Message> {
    channel(32, async |mut sender| {
        let hotkey_receiver = GlobalHotKeyEvent::receiver();

        let tray_menu_receiver = tray_icon::menu::MenuEvent::receiver();
        let tray_icon_receiver = tray_icon::TrayIconEvent::receiver();

        #[cfg(target_os = "linux")]
        let mut flag_counter = Arc::new(AtomicUsize::new(0));
        #[cfg(target_os = "linux")]
        const SIGUSR1_U: usize = SIGUSR1 as usize;
        #[cfg(target_os = "linux")]
        signal_flag::register_usize(SIGUSR1, Arc::clone(&flag_counter), SIGUSR1_U).unwrap();

        // poll for global hotkey events every 50ms
        loop {
            // You need to zero out and reset listener in loop
            #[cfg(target_os = "linux")]
            if flag_counter.load(Ordering::Relaxed) == SIGUSR1_U {
                if let Err(err) = sender.send(Message::Hotkey).await {
                    eprintln!("Error sending hotkey message: {}", err);
                }
                flag_counter = Arc::new(AtomicUsize::new(0));
                signal_flag::register_usize(SIGUSR1, Arc::clone(&flag_counter), SIGUSR1_U).unwrap();
            }

            if let Ok(event) = hotkey_receiver.try_recv() {
                if event.state() == HotKeyState::Pressed {
                    if let Err(err) = sender.send(Message::Hotkey).await {
                        eprintln!("Error sending hotkey message: {}", err);
                    }
                }
            }
            if let Ok(_event) = tray_menu_receiver.try_recv() {
                if let Err(err) = sender.send(Message::Shutdown).await {
                    eprintln!("Error sending tray message: {}", err);
                }
            }
            if let Ok(event) = tray_icon_receiver.try_recv() {
                match event {
                    tray_icon::TrayIconEvent::Click {
                        button,
                        button_state,
                        ..
                    } => {
                        if button == MouseButton::Left && button_state == MouseButtonState::Down {
                            if let Err(err) = sender.send(Message::Hotkey).await {
                                eprintln!("Error sending tray message: {}", err);
                            }
                        }
                    }
                    _ => (),
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
}

enum Hotkey {
    #[allow(dead_code)]
    F12,
    #[allow(dead_code)]
    AltF12,
    Pause,
}

impl Default for Hotkey {
    fn default() -> Self {
        if std::env::var_os("DEBUG").is_some() {
            return Self::Pause;
        }
        #[cfg(target_os = "linux")]
        return Self::F12;
        #[cfg(not(target_os = "linux"))]
        return Self::AltF12;
    }
}

impl Hotkey {
    fn global_hotkey(&self) -> hotkey::HotKey {
        match self {
            Self::F12 => hotkey::HotKey::new(None, hotkey::Code::F12),
            Self::AltF12 => hotkey::HotKey::new(Some(hotkey::Modifiers::ALT), hotkey::Code::F12),
            Self::Pause => hotkey::HotKey::new(None, hotkey::Code::Pause),
        }
    }

    fn iced(&self) -> (iced::keyboard::Key, iced::keyboard::Modifiers) {
        match self {
            Self::F12 => (
                iced::keyboard::Key::Named(iced::keyboard::key::Named::F12),
                iced::keyboard::Modifiers::empty(),
            ),
            Self::AltF12 => (
                iced::keyboard::Key::Named(iced::keyboard::key::Named::F12),
                iced::keyboard::Modifiers::ALT,
            ),
            Self::Pause => (
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Pause),
                iced::keyboard::Modifiers::empty(),
            ),
        }
    }

    fn filter(
        &self,
    ) -> impl 'static + Fn(&iced::keyboard::Key, &iced::keyboard::Modifiers) -> bool {
        let (hotkey, hotkey_modifiers) = self.iced();
        move |key: &iced::keyboard::Key, modifiers: &iced::keyboard::Modifiers| {
            if key == &iced::keyboard::Key::Character("T".into())
                && modifiers.control()
                && modifiers.shift()
            {
                return true;
            };

            if key == &hotkey && modifiers == &hotkey_modifiers {
                return true;
            }

            false
        }
    }
}
