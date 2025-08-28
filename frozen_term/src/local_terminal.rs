use std::{sync::Arc, time::Duration};

use crate::{Style, terminal};
use async_pty::PtyProcess;
use iced::{
    self, Element, Length, Task,
    task::sipper,
    widget::{center, text},
};

#[derive(Debug, Clone)]
pub struct Message(InnerMessage);

#[derive(Debug, Clone)]
enum InnerMessage {
    Opened(Arc<(PtyProcess, tokio::sync::mpsc::Receiver<Vec<u8>>)>),
    Terminal(terminal::Message),
    Output(Vec<u8>),
    InjectInput(Vec<u8>),
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
    display: terminal::Terminal,
}

impl LocalTerminal {
    pub fn start(
        key_filter: impl 'static + Fn(&iced::keyboard::Key, &iced::keyboard::Modifiers) -> bool,
    ) -> (Self, Task<Message>) {
        let size = async_pty::TerminalSize { cols: 80, rows: 24 };
        let (display, display_task) = terminal::Terminal::new();
        let display = display.key_filter(key_filter);

        let start_task = Task::future(async {
            let (process, output) = PtyProcess::shell(size).await.unwrap();
            Message(InnerMessage::Opened(Arc::new((process, output))))
        });

        (
            Self {
                state: State::Starting,
                display,
            },
            Task::batch([
                display_task.map(InnerMessage::Terminal).map(Message),
                start_task,
            ]),
        )
    }

    pub fn style(mut self, style: Style) -> Self {
        self.set_style(style);
        self
    }

    pub fn set_style(&mut self, style: Style) {
        self.display.set_style(style);
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message.0 {
            InnerMessage::Opened(arc) => {
                let (process, output) = Arc::into_inner(arc).unwrap();

                let stream = sipper(|mut sender| async move {
                    let mut output = output;
                    while let Some(chunk) = output.recv().await {
                        sender.send(InnerMessage::Output(chunk)).await;
                    }

                    sender.send(InnerMessage::Closed).await;
                });

                let task = Task::stream(stream).map(Message);

                self.state = State::Active(process);

                Action::Run(task)
            }
            InnerMessage::Terminal(message) => {
                let action = self.display.update(message);

                match action {
                    terminal::Action::None => Action::None,
                    terminal::Action::Run(task) => {
                        Action::Run(task.map(InnerMessage::Terminal).map(Message))
                    }
                    terminal::Action::IdChanged => Action::IdChanged,
                    terminal::Action::Input(input) => {
                        if let State::Active(pty) = &self.state {
                            pty.try_write(input).unwrap();
                        }
                        Action::None
                    }
                    terminal::Action::Resize(size) => {
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
            InnerMessage::InjectInput(input) => {
                if let State::Active(pty) = &self.state {
                    pty.try_write(input).unwrap();
                }
                Action::None
            }
            InnerMessage::Output(output) => {
                self.display.advance_bytes(output);

                Action::None
            }
            InnerMessage::Closed => {
                self.state = State::Closed;

                Action::Close
            }
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        match &self.state {
            State::Starting => center(text!("opening pty...")).into(),
            State::Active(_) => {
                center(self.display.view().map(InnerMessage::Terminal).map(Message))
                    .padding(10)
                    .into()
            }
            State::Closed => center(text!("pty closed")).height(Length::Fill).into(),
        }
    }

    pub fn get_title(&self) -> &str {
        self.display.get_title()
    }

    #[must_use]
    pub fn focus<T>(&self) -> Task<T>
    where
        T: Send + 'static,
    {
        self.display.focus()
    }

    /// !!!WARNING!!!
    ///
    /// injected input will be directly injected into the stdin of the terminal process.
    /// If the user has typed something, that input will still be there!
    /// When writing commands manually, you'll need to ensure that they are not influenced by what the user has typed
    /// and you will also have to handle key encoding and control characters yourself.
    #[must_use]
    pub fn inject_input(&self, input: InputSequence) -> Task<Message> {
        if let State::Active(ref pty) = self.state {
            match input {
                InputSequence::Raw(input) => {
                    let _ = pty.try_write(input);
                    Task::none()
                }
                InputSequence::AbortAndRaw(input) => {
                    let _ = pty.try_write(b"\x03".to_vec());
                    // While I'd love to skip this weird helper task, my shell just doesn't clear the current line without it.
                    // 
                    Task::future(async move {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Message(InnerMessage::InjectInput(input))
                    })
                }
                InputSequence::AbortAndCommand(mut input) => {
                    let _ = pty.try_write(b"\x03".to_vec());
                    input.push('\n');
                    let input = input.into_bytes();
                    Task::future(async move {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Message(InnerMessage::InjectInput(input))
                    })
                }
            }
        } else {
            Task::none()
        }
    }
}

pub enum InputSequence {
    /// !!!WARNING!!!
    ///
    /// Is is very rare to need a raw input sequence.
    /// Please ensure you absolutely know what you are doing before using this method.
    Raw(Vec<u8>),
    /// This will send the equivalent of Ctrl+C to the terminal process.
    /// Before adding your input.
    AbortAndRaw(Vec<u8>),
    /// This will send the equivalent of Ctrl+C to the terminal process,
    /// add your command and finally send a newline.
    ///
    /// Be aware that your command will not be sanitized!.
    AbortAndCommand(String),
}
