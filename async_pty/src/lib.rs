use std::sync::LazyLock;

use anyhow::Result;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

static SHELL: LazyLock<String> = LazyLock::new(|| {
    let shell = CommandBuilder::new_default_prog().get_shell();
    if &shell == r"C:\WINDOWS\system32\cmd.exe" {
        return "powershell.exe".to_string();
    }

    shell
});

pub enum TerminalInput {
    Input(Vec<u8>),
    Resize(TerminalSize),
}

#[derive(Debug)]
pub struct PtyProcess {
    write: mpsc::Sender<TerminalInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

impl From<TerminalSize> for PtySize {
    fn from(value: TerminalSize) -> Self {
        Self {
            rows: value.rows,
            cols: value.cols,
            ..Default::default()
        }
    }
}

impl PtyProcess {
    pub async fn shell(size: TerminalSize) -> Result<(Self, mpsc::Receiver<Vec<u8>>)> {
        tokio::task::spawn_blocking(move || {
            let pty_system = native_pty_system();

            let pair = pty_system.openpty(size.into())?;

            let shell_cmd = CommandBuilder::new(SHELL.to_owned());

            let child = pair.slave.spawn_command(shell_cmd)?;
            drop(pair.slave);

            let master = pair.master;
            let mut reader = master.try_clone_reader()?;
            let (writer_send, writer_recv) = mpsc::channel::<TerminalInput>(10);
            let (helper_send, helper_recv) = mpsc::channel::<TerminalInput>(10);

            let cancel = CancellationToken::new();
            let cancel2 = cancel.clone();

            tokio::spawn(async move {
                let cancel = cancel2;
                let mut writer_recv = writer_recv;
                cancel
                    .run_until_cancelled(async move {
                        while let Some(input) = writer_recv.recv().await {
                            if helper_send.send(input).await.is_err() {
                                break;
                            }
                        }
                    })
                    .await;
            });

            // writer thread
            std::thread::spawn(move || {
                let mut helper_recv = helper_recv;
                if let Ok(mut writer) = master.take_writer() {
                    while let Some(input) = helper_recv.blocking_recv() {
                        match input {
                            TerminalInput::Input(input) => {
                                if let Err(_err) = writer.write_all(&input) {
                                    return;
                                }
                            }
                            TerminalInput::Resize(size) => {
                                if let Err(_err) = master.resize(size.into()) {
                                    return;
                                }
                            }
                        }
                    }
                }
            });

            let (reader_send, reader_recv) = mpsc::channel::<Vec<u8>>(100);

            // reader thread
            std::thread::spawn(move || {
                let mut buffer = [0u8; 1024];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(bytes) => {
                            if bytes == 0 {
                                break;
                            }

                            let mut chunk = Vec::new();
                            chunk.extend_from_slice(&buffer[0..bytes]);
                            if let Err(_err) = reader_send.blocking_send(chunk) {
                                break;
                            }
                        }
                        Err(_err) => {
                            break;
                        }
                    }
                }
            });

            // For win specifically, the explicit child does implement Future.
            // Unfortunately, it doesn't work.
            // As I'm not all that great of a programmer, this is what I'll use for now.
            std::thread::spawn(move || {
                let mut child = child;
                let _ = child.wait();

                cancel.cancel();
            });

            Ok((Self { write: writer_send }, reader_recv))
        })
        .await?
    }

    pub async fn resize(&self, size: TerminalSize) -> Result<()> {
        self.write.send(TerminalInput::Resize(size)).await?;

        Ok(())
    }

    pub fn try_resize(&self, size: TerminalSize) -> Result<()> {
        self.write.try_send(TerminalInput::Resize(size))?;

        Ok(())
    }

    pub async fn write(&self, content: Vec<u8>) -> Result<()> {
        self.write.send(TerminalInput::Input(content)).await?;

        Ok(())
    }

    pub fn try_write(&self, content: Vec<u8>) -> Result<()> {
        self.write.try_send(TerminalInput::Input(content))?;

        Ok(())
    }
}
