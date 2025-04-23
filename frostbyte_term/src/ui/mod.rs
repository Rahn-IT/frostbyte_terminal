use iced::{Element, Subscription, Task, window};
use local_terminal::LocalTerminal;
use tokio::sync::{mpsc, oneshot};

mod local_terminal;

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    LocalTerminal(local_terminal::Message),
    Opened(window::Id),
}

pub struct UI {
    term: LocalTerminal,
}

impl Drop for UI {
    fn drop(&mut self) {
        println!("Dropping UI");
    }
}

impl UI {
    pub fn start() -> (Self, Task<Message>) {
        let (local_terminal, terminal_task) = LocalTerminal::start();

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
        let (id, window_task) = window::open(settings);

        (
            Self {
                term: local_terminal,
            },
            Task::batch(vec![
                window_task.map(Message::Opened),
                terminal_task.map(Message::LocalTerminal),
            ]),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::LocalTerminal(message) => {
                let action = self.term.update(message);

                match action {
                    local_terminal::Action::Run(task) => task.map(Message::LocalTerminal),
                    local_terminal::Action::None => Task::none(),
                }
            }
            Message::Opened(_) => {
                let focus_task = self.term.focus();

                focus_task
            }
        }
    }

    pub fn view(&self, id: window::Id) -> Element<Message> {
        self.term.view().map(Message::LocalTerminal)
    }

    pub fn title(&self, id: window::Id) -> String {
        self.term.get_title().to_string()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }
}
