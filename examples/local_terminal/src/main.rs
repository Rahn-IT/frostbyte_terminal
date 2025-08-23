use frozen_term::local_terminal::{self, LocalTerminal};
use iced::Task;

pub fn main() {
    iced::application(UI::start, UI::update, UI::view)
        .title(UI::title)
        .antialiasing(true)
        .run()
        .unwrap()
}

#[derive(Debug, Clone)]
enum Message {
    Terminal(local_terminal::Message),
}

struct UI {
    terminal: LocalTerminal,
}

impl UI {
    fn start() -> (Self, Task<Message>) {
        let (mut terminal, task) = LocalTerminal::start(|_, _| false);

        terminal.set_style(frozen_term::Style::default());

        (Self { terminal }, task.map(Message::Terminal))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Terminal(message) => {
                let action = self.terminal.update(message);

                match action {
                    local_terminal::Action::Run(task) => task.map(Message::Terminal),
                    local_terminal::Action::IdChanged => Task::none(),
                    local_terminal::Action::Close => iced::exit(),
                    local_terminal::Action::None => Task::none(),
                }
            }
        }
    }

    fn view(&'_ self) -> iced::Element<'_, Message> {
        self.terminal.view().map(Message::Terminal)
    }

    fn title(&self) -> String {
        self.terminal.get_title().to_string()
    }
}
