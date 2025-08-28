use frozen_term::local_terminal::{self, InputSequence, LocalTerminal};
use iced::{
    Length, Task,
    widget::{button, column, row, text},
};

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
    GotoHome,
    GotoRoot,
    GotoEtc,
}

struct UI {
    terminal: LocalTerminal,
}

impl UI {
    fn start() -> (Self, Task<Message>) {
        let (mut terminal, task) = LocalTerminal::start(|_, _| false);

        terminal.set_style(frozen_term::Style {
            ..Default::default()
        });

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
            Message::GotoHome => self
                .terminal
                .inject_input(InputSequence::AbortAndCommand("cd ~".into()))
                .map(Message::Terminal),
            Message::GotoRoot => self
                .terminal
                .inject_input(InputSequence::AbortAndCommand("cd /".into()))
                .map(Message::Terminal),
            Message::GotoEtc => self
                .terminal
                .inject_input(InputSequence::AbortAndCommand("cd /etc".into()))
                .map(Message::Terminal),
        }
    }

    fn view(&'_ self) -> iced::Element<'_, Message> {
        row![
            self.terminal.view().map(Message::Terminal),
            column![
                button(text("Home"))
                    .width(Length::Fill)
                    .on_press(Message::GotoHome),
                button(text("Root"))
                    .width(Length::Fill)
                    .on_press(Message::GotoRoot),
                button(text("Etc"))
                    .width(Length::Fill)
                    .on_press(Message::GotoEtc)
            ]
            .width(120)
            .padding(10)
            .spacing(20)
        ]
        .into()
    }

    fn title(&self) -> String {
        self.terminal.get_title().to_string()
    }
}
