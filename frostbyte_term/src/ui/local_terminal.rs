use std::sync::Arc;

use async_pty::PtyProcess;
use iced::{
    Element, Length, Task,
    widget::{center, text},
};
use sipper::sipper;

#[derive(Debug, Clone)]
pub enum Message {
    Opened(Arc<(PtyProcess, tokio::sync::mpsc::Receiver<Vec<u8>>)>),
    Terminal(frozen_term::Message),
    Output(Vec<u8>),
    Closed,
}

pub enum Action {
    Run(Task<Message>),
    IdChanged,
    Close,
    None,
}

enum State {
    Starting,
    Active(PtyProcess),
    Closed,
}

pub struct LocalTerminal {
    state: State,
    display: frozen_term::Terminal,
}

impl LocalTerminal {
    pub fn start(
        font: Option<iced::Font>,
        key_filter: impl 'static + Fn(&iced::keyboard::Key, &iced::keyboard::Modifiers) -> bool,
    ) -> (Self, Task<Message>) {
        let size = async_pty::TerminalSize { cols: 80, rows: 24 };
        let (display, display_task) = frozen_term::Terminal::new(size.rows, size.cols);
        let mut display = display.key_filter(key_filter);

        if let Some(font) = font {
            display = display.font(font);
        }

        let start_task = Task::future(async {
            let (process, output) = PtyProcess::shell(size).await.unwrap();
            Message::Opened(Arc::new((process, output)))
        });

        (
            Self {
                state: State::Starting,
                display,
            },
            Task::batch([display_task.map(Message::Terminal), start_task]),
        )
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Opened(arc) => {
                let (process, output) = Arc::into_inner(arc).unwrap();

                let stream = sipper(|mut sender| async move {
                    let mut output = output;
                    while let Some(chunk) = output.recv().await {
                        sender.send(Message::Output(chunk)).await;
                    }

                    sender.send(Message::Closed).await;
                });

                let task = Task::stream(stream);

                self.state = State::Active(process);

                Action::Run(task)
            }
            Message::Terminal(message) => {
                let action = self.display.update(message);

                match action {
                    frozen_term::Action::None => Action::None,
                    frozen_term::Action::Run(task) => Action::Run(task.map(Message::Terminal)),
                    frozen_term::Action::IdChanged => Action::IdChanged,
                    frozen_term::Action::Input(input) => {
                        if let State::Active(pty) = &self.state {
                            pty.try_write(input).unwrap();
                        }
                        Action::None
                    }
                    frozen_term::Action::Resize(size) => {
                        if let State::Active(pty) = &self.state {
                            pty.try_resize(async_pty::TerminalSize {
                                rows: size.rows as u16,
                                cols: size.cols as u16,
                            })
                            .unwrap();
                        }
                        Action::None
                    }
                }
            }
            Message::Output(output) => {
                self.display.advance_bytes(output);

                Action::None
            }
            Message::Closed => {
                self.state = State::Closed;

                Action::Close
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        match &self.state {
            State::Starting => center(text!("opening pty...")).into(),
            State::Active(_) => center(self.display.view().map(Message::Terminal))
                .padding(10)
                .into(),
            State::Closed => center(text!("pty closed")).height(Length::Fill).into(),
        }
    }

    pub fn get_title(&self) -> &str {
        self.display.get_title()
    }

    pub fn focus<T>(&self) -> Task<T>
    where
        T: Send + 'static,
    {
        self.display.focus()
    }
}
