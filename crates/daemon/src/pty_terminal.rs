use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize, SlavePty};
use std::io::{Read, Write};
use tokio::sync::mpsc;

pub struct PtyHandle {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
}

impl PtyHandle {
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        self.writer.write_all(data).context("writing to terminal")?;
        self.writer.flush().context("flushing terminal input")
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("resizing terminal")
    }
}

pub type BoxedChild = Box<dyn Child + Send + Sync>;

pub fn spawn_shell(
    cols: u16,
    rows: u16,
) -> Result<(PtyHandle, mpsc::Receiver<Vec<u8>>, BoxedChild)> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("opening native terminal")?;

    let child = spawn_native_shell(pair.slave.as_ref())?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .context("reading from terminal")?;
    let writer = pair.master.take_writer().context("writing to terminal")?;

    let (tx, rx) = mpsc::channel::<Vec<u8>>(64);
    std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.blocking_send(buffer[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    Ok((
        PtyHandle {
            writer,
            master: pair.master,
        },
        rx,
        child,
    ))
}

pub async fn shutdown(mut child: BoxedChild) {
    let _ = child.kill();
    let _ = tokio::task::spawn_blocking(move || child.wait()).await;
}

#[cfg(windows)]
fn spawn_native_shell(slave: &dyn SlavePty) -> Result<BoxedChild> {
    slave
        .spawn_command(CommandBuilder::new("pwsh.exe"))
        .or_else(|_| slave.spawn_command(CommandBuilder::new("powershell.exe")))
        .context("spawning PowerShell")
}

#[cfg(not(windows))]
fn spawn_native_shell(slave: &dyn SlavePty) -> Result<BoxedChild> {
    let preferred = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_owned());
    slave
        .spawn_command(CommandBuilder::new(&preferred))
        .or_else(|_| slave.spawn_command(CommandBuilder::new("/bin/bash")))
        .or_else(|_| slave.spawn_command(CommandBuilder::new("/bin/sh")))
        .context("spawning shell")
}
