use std::sync::Arc;

use iced::{
    Element, Task,
    widget::{center, text},
};
use sipper::sipper;
use svalin_sysctl::pty::{self, PtyProcess};

#[derive(Debug, Clone)]
pub enum Message {
    Opened(Arc<(PtyProcess, tokio::sync::mpsc::Receiver<Vec<u8>>)>),
    Terminal(frozen_term::Message),
    Output(Vec<u8>),
    Closed,
}

pub enum Action {
    Run(Task<Message>),
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
    pub fn start() -> (Self, Task<Message>) {
        let size = pty::TerminalSize { cols: 80, rows: 24 };
        let (display, display_task) = frozen_term::Terminal::new(size.rows, size.cols);
        let display = display.random_id();

        let start_task = Task::future(async {
            let (process, output) = PtyProcess::shell(size).await.unwrap();
            Message::Opened(Arc::new((process, output)))
        });

        (
            Self {
                state: State::Starting,
                display,
            },
            Task::batch(vec![display_task.map(Message::Terminal), start_task]),
        )
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Opened(arc) => {
                let (process, output) = Arc::into_inner(arc).unwrap();

                let stream = sipper(|mut sender| async move {
                    let mut output = output;
                    while let Some(chunk) = output.recv().await {
                        sender.send(Message::Output(chunk)).await;
                    }
                    println!("no more messages");

                    sender.send(Message::Closed).await;
                });

                let task = Task::stream(stream);

                self.state = State::Active(process);

                Action::Run(task)
            }
            Message::Terminal(message) => {
                let action = self.display.update(message);

                match action {
                    frozen_term::Action::None => (),
                    frozen_term::Action::Input(input) => {
                        if let State::Active(pty) = &self.state {
                            pty.try_write(input).unwrap();
                        }
                    }
                    frozen_term::Action::Resize(size) => {
                        if let State::Active(pty) = &self.state {
                            pty.try_resize(pty::TerminalSize {
                                rows: size.rows as u16,
                                cols: size.cols as u16,
                            })
                            .unwrap();
                        }
                    }
                }

                Action::None
            }
            Message::Output(output) => {
                self.display.advance_bytes(output);

                Action::None
            }
            Message::Closed => {
                self.state = State::Closed;

                Action::None
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        match &self.state {
            State::Starting => center(text!("opening pty...")).into(),
            State::Active(_) => self.display.view().map(Message::Terminal),
            State::Closed => center(text!("pty closed")).into(),
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
