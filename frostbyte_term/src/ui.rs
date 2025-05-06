use std::{collections::BTreeMap, fmt::Debug, time::Duration};

use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey};
use iced::{
    Color, Element, Font, Length, Subscription, Task,
    futures::SinkExt,
    keyboard,
    stream::channel,
    widget::{button, center, column, row, text},
    window,
};
use iced_aw::{TabLabel, tab_bar};
use local_terminal::LocalTerminal;
use sipper::Stream;
use tray_icon::TrayIconBuilder;

mod local_terminal;

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    LocalTerminal {
        id: u32,
        message: local_terminal::Message,
    },
    OpenTab,
    SwitchTab(u32),
    FocusTab(u32),
    CloseTab(u32),
    Hotkey,
    WindowOpened(window::Id),
    CloseWindow,
    WindowClosed,
    Shutdown,
    None,
}

pub struct UI {
    terminals: BTreeMap<u32, LocalTerminal>,
    window_id: Option<window::Id>,
    selected_tab: u32,
    new_terminal_id: u32,
    _hotkey_manager: GlobalHotKeyManager,
    hotkey: Hotkey,
    hotkey_id: u32,
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
    pub fn start() -> (Self, Task<Message>) {
        std::thread::spawn(|| {
            gtk::init().unwrap();

            let close_item = tray_icon::menu::MenuItem::new("Exit Frostbyte", true, None);
            let tray_menu = tray_icon::menu::Menu::new();
            tray_menu.append(&close_item).unwrap();
            let _tray_icon = TrayIconBuilder::new()
                .with_tooltip("Frostbyte")
                .with_menu(Box::new(tray_menu))
                .build()
                .unwrap();

            gtk::main();
        });

        let terminals = BTreeMap::new();

        let hotkey = Hotkey::F12;
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
            },
            Task::none(),
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Task<Message> {
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
                    local_terminal::Action::None => Task::none(),
                }
            }
            Message::OpenTab => self.open_tab(),
            Message::SwitchTab(id) => self.switch_tab(id),
            Message::FocusTab(id) => {
                if let Some(term) = self.terminals.get(&id) {
                    term.focus()
                } else {
                    Task::none()
                }
            }
            Message::CloseTab(id) => self.close_tab(id),
            Message::Hotkey => {
                return if let Some(id) = self.window_id {
                    window::close(id)
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
            Message::CloseWindow => {
                if let Some(id) = self.window_id {
                    self.window_id = None;
                    return window::close(id);
                } else {
                    Task::none()
                }
            }
            Message::WindowClosed => {
                self.window_id = None;
                Task::none()
            }
            Message::Shutdown => iced::exit(),
            Message::None => Task::none(),
        }
    }

    fn open_window(&mut self) -> Task<Message> {
        if let Some(id) = self.window_id {
            window::gain_focus(id)
        } else {
            let settings = window::Settings {
                decorations: false,
                resizable: false,
                position: window::Position::SpecificWith(|window_size, monitor_res| {
                    let x = (monitor_res.width - window_size.width) / 2.0;
                    iced::Point::new(x, 0.0)
                }),
                size: iced::Size {
                    width: 2000.0,
                    height: 600.0,
                },
                level: window::Level::AlwaysOnTop,

                ..Default::default()
            };

            let (id, task) = window::open(settings);
            self.window_id = Some(id);

            let task = task.map(Message::WindowOpened);

            if self.terminals.is_empty() {
                Task::batch([self.open_tab(), task])
            } else {
                task
            }
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

        Task::batch([
            terminal_task.map(move |message| Message::LocalTerminal { id, message }),
            self.focus_tab(id),
        ])
    }

    fn focus_tab(&self, id: u32) -> Task<Message> {
        Task::future(async move {
            tokio::time::sleep(Duration::from_micros(300)).await;
            Message::FocusTab(id)
        })
    }

    fn close_tab(&mut self, id: u32) -> Task<Message> {
        self.terminals.remove(&id);

        if let Some((id, _term)) = self.terminals.iter().next() {
            self.selected_tab = *id;
            self.focus_tab(*id)
        } else {
            let id = self.window_id.clone();
            if let Some(id) = id {
                self.window_id = None;
                window::close(id)
            } else {
                Task::none()
            }
        }
    }

    fn switch_tab(&mut self, id: u32) -> Task<Message> {
        if let Some(_terminal) = self.terminals.get(&id) {
            self.selected_tab = id;
            self.focus_tab(id)
        } else {
            Task::none()
        }
    }

    pub fn view(&self, _id: window::Id) -> Element<Message> {
        let selected_terminal = self.terminals.get(&self.selected_tab);

        let tab_view = match selected_terminal {
            Some(terminal) => terminal.view(),
            None => text("terminal closed").into(),
        };

        let current_id = self.selected_tab;

        let tab_bar = tab_bar::TabBar::with_tab_labels(
            self.terminals
                .iter()
                .map(|(id, terminal)| {
                    (id.clone(), TabLabel::Text(terminal.get_title().to_string()))
                })
                .collect(),
            Message::SwitchTab,
        )
        .set_active_tab(&self.selected_tab)
        // .width(Length::Shrink)
        .height(Length::Fill)
        // .tab_width(Length::Fixed(444.0))
        .on_close(Message::CloseTab);
        column![
            tab_view.map(move |message| {
                Message::LocalTerminal {
                    id: current_id,
                    message,
                }
            }),
            row![
                tab_bar,
                button(center(text("New Tab")))
                    .width(200)
                    .height(Length::Fill)
                    .on_press(Message::OpenTab),
                button(center(text("X")))
                    .style(|_, status| {
                        let color = match status {
                            button::Status::Active | button::Status::Pressed => {
                                Color::from_rgb(0.8, 0.0, 0.0)
                            }
                            button::Status::Hovered => Color::from_rgb(0.8, 0.2, 0.2),
                            button::Status::Disabled => Color::from_rgb(0.5, 0.5, 0.5),
                        };
                        button::Style {
                            background: Some(color.into()),
                            text_color: Color::WHITE,
                            ..Default::default()
                        }
                    })
                    .width(40)
                    .height(Length::Fill)
                    .on_press(Message::CloseWindow)
            ]
            .height(40)
        ]
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
            window::events().map(|(_id, event)| match event {
                window::Event::Closed => Message::WindowClosed,
                _ => Message::None,
            }),
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
    channel(32, |mut sender| async move {
        let hotkey_receiver = GlobalHotKeyEvent::receiver();

        let tray_receiver = tray_icon::menu::MenuEvent::receiver();
        // poll for global hotkey events every 50ms
        loop {
            if let Ok(event) = hotkey_receiver.try_recv() {
                if event.state() == HotKeyState::Released {
                    if let Err(err) = sender.send(Message::Hotkey).await {
                        eprintln!("Error sending hotkey message: {}", err);
                    }
                }
            }
            if let Ok(_event) = tray_receiver.try_recv() {
                if let Err(err) = sender.send(Message::Shutdown).await {
                    eprintln!("Error sending tray message: {}", err);
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
}

enum Hotkey {
    F12,
}

impl Hotkey {
    fn global_hotkey(&self) -> hotkey::HotKey {
        match self {
            Hotkey::F12 => hotkey::HotKey::new(None, hotkey::Code::F12),
        }
    }

    fn iced(&self) -> (iced::keyboard::Key, iced::keyboard::Modifiers) {
        match self {
            Hotkey::F12 => (
                iced::keyboard::Key::Named(iced::keyboard::key::Named::F12),
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
