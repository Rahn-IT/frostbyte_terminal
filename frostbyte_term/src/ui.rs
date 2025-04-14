use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use frozen_term::Terminal;
use iced::{Element, Subscription, Task, futures::SinkExt, stream::channel};
use portable_pty::{Child, PtyPair, PtySize};
use tokio::task::{JoinHandle, spawn_blocking};

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    Terminal(frozen_term::Message),
}

pub struct UI {
    term: Terminal,
    child: Box<dyn Child + Send + Sync>,
    copy_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    pty: PtyPair,
}

impl Drop for UI {
    fn drop(&mut self) {
        println!("Dropping UI");
        self.child.kill().unwrap();
        if let Some(handle) = self.copy_handle.lock().unwrap().deref() {
            handle.abort();
        }
    }
}

impl UI {
    #[cfg(unix)]
    fn get_shell() -> String {
        std::env::var("SHELL").unwrap_or("/bin/bash".to_string())
    }

    #[cfg(windows)]
    fn get_shell() -> String {
        "powershell.exe".to_string()
    }

    pub fn start() -> (Self, Task<Message>) {
        // let grid = AnsiGrid::new(120, 40);
        let cols = 80;
        let rows = 25;

        let command = portable_pty::CommandBuilder::new(Self::get_shell());

        let pty = portable_pty::native_pty_system()
            .openpty(PtySize {
                cols,
                rows,
                ..Default::default()
            })
            .unwrap();

        let child = pty.slave.spawn_command(command).unwrap();

        let writer = pty.master.take_writer().unwrap();

        let (term, task) = Terminal::new(rows, cols);

        (
            Self {
                // grid,
                term,
                pty,
                child,
                copy_handle: Arc::new(Mutex::new(None)),
            },
            task.map(Message::Terminal),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Terminal(msg) => {
                let action = self.term.update(msg);

                match action {
                    frozen_term::Action::None => Task::none(),
                    frozen_term::Action::Resize(size) => {
                        let pty_size = PtySize {
                            rows: size.rows as u16,
                            cols: size.cols as u16,
                            pixel_height: size.pixel_height as u16,
                            pixel_width: size.pixel_width as u16,
                        };
                        self.pty.master.resize(pty_size).unwrap();

                        Task::none()
                    }
                    frozen_term::Action::Input(input) => {
                        
                    }
                }
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        self.term.view().map(Message::Terminal)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut reader = self.pty.master.try_clone_reader().unwrap();
        let copy_handle = self.copy_handle.clone();
        Subscription::batch(vec![Subscription::run_with_id(
            1,
            channel(1, |mut output| async move {
                let (send, mut recv) = tokio::sync::mpsc::unbounded_channel();

                let handle = spawn_blocking(move || {
                    let mut buf = vec![0u8; 1024];
                    loop {
                        let read = reader.read(&mut buf).unwrap();
                        if read == 0 {
                            println!("EOF");
                            break;
                        }
                        send.send(buf[..read].to_vec()).unwrap();
                    }
                });

                {
                    *copy_handle.lock().unwrap() = Some(handle);
                }

                while let Some(s) = recv.recv().await {
                    let message = Message::Terminal(frozen_term::Message::AdvanceBytes(s));
                    output.send(message).await.unwrap();
                }
            }),
        )])
    }

    pub fn title(&self) -> String {
        self.term.title().to_string()
    }
}
