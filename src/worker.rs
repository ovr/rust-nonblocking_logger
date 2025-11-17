use crossbeam_channel::{Receiver, Sender, TryRecvError};
use std::collections::VecDeque;
use std::io;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub enum WorkerMessage {
    /// Log message to be written
    Log(String),
    /// Request to flush the output
    Flush,
}

/// Worker thread that handles non-blocking writes to stdout/stderr
pub(crate) struct LogWorker {
    receiver: Receiver<WorkerMessage>,
    running: Arc<AtomicBool>,
}

impl LogWorker {
    pub fn new(receiver: Receiver<WorkerMessage>) -> (Self, Arc<AtomicBool>) {
        let running = Arc::new(AtomicBool::new(false));

        (
            Self {
                receiver,
                running: running.clone(),
            },
            running,
        )
    }

    pub fn spawn(mut self) -> io::Result<JoinHandle<()>> {
        self.running.store(true, Ordering::SeqCst);

        Ok(thread::spawn(move || {
            self.run();
        }))
    }

    /// Main worker loop
    fn run(&mut self) {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        let mut buf = VecDeque::<u8>::new();

        while self.running.load(Ordering::SeqCst) {
            // block until at least one message
            match self.receiver.recv() {
                Ok(msg) => match msg {
                    WorkerMessage::Log(msg) => buf.extend(msg.as_bytes()),
                    WorkerMessage::Flush => {
                        // TODO: Fix me
                    }
                },
                Err(_) => break, // channel closed
            }

            // pipe one more message into the buffer
            while let Ok(msg) = self.receiver.try_recv() {
                match msg {
                    WorkerMessage::Log(msg) => buf.extend(msg.as_bytes()),
                    WorkerMessage::Flush => {
                        // TODO: Fix me
                    }
                };
            }

            while !buf.is_empty() {
                let (front, _) = buf.as_slices();
                match out.write(front) {
                    Ok(0) => break, // nothing accepted, try later
                    Ok(n) => {
                        for _ in 0..n {
                            buf.pop_front();
                        }
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // stdout “busy”
                        thread::sleep(Duration::from_millis(1));
                        break;
                    }
                    Err(ref err) => {
                        // hard error, give up
                        if cfg!(debug_assertions) {
                            eprintln!("Error writing to stdout: {}", err);
                        }
                    }
                }
            }
        }
    }
}
