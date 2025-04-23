use iced::{Element, Subscription, Task};
use local_terminal::LocalTerminal;

mod local_terminal;

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    LocalTerminal(local_terminal::Message),
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
        let (local_terminal, task) = LocalTerminal::start();

        (
            Self {
                term: local_terminal,
            },
            task.map(Message::LocalTerminal),
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
        }
    }

    pub fn view(&self) -> Element<Message> {
        self.term.view().map(Message::LocalTerminal)
    }

    pub fn title(&self) -> String {
        self.term.get_title().to_string()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }
}
