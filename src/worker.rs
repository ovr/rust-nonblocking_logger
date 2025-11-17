use crossbeam_channel::{Receiver, Sender};
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
    /// Request to flush the output, with a sender to signal completion
    Flush(Sender<()>),
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

    fn write_buffer(out: &mut io::StdoutLock, buf: &mut VecDeque<u8>) {
        // Write all buffered data
        while !buf.is_empty() {
            let (front, _) = buf.as_slices();
            match out.write(front) {
                Ok(0) => {
                    // Nothing accepted, retry after short sleep
                    thread::sleep(Duration::from_millis(1));
                }
                Ok(n) => {
                    // Remove written bytes
                    for _ in 0..n {
                        buf.pop_front();
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Retry after short sleep
                    thread::sleep(Duration::from_millis(1));
                }
                Err(ref err) => {
                    // Hard error, give up
                    if cfg!(debug_assertions) {
                        eprintln!("Error flushing to stdout: {}", err);
                    }
                    break;
                }
            }
        }
    }

    fn write_buffer_and_flush(out: &mut io::StdoutLock, buf: &mut VecDeque<u8>) {
        Self::write_buffer(out, buf);

        if let Err(err) = out.flush() {
            if cfg!(debug_assertions) {
                eprintln!("Error flushing stdout: {}", err);
            }
        }
    }

    fn run(&mut self) {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        let mut buf = VecDeque::<u8>::new();

        while self.running.load(Ordering::SeqCst) {
            // block until at least one message
            match self.receiver.recv() {
                Ok(msg) => match msg {
                    WorkerMessage::Log(msg) => buf.extend(msg.as_bytes()),
                    WorkerMessage::Flush(done) => {
                        Self::write_buffer_and_flush(&mut out, &mut buf);
                        // Signal completion (ignore if receiver was dropped)
                        let _ = done.send(());

                        continue;
                    }
                },
                Err(_) => break, // channel closed
            }

            // pipe one more message into the buffer (optimization)
            while let Ok(msg) = self.receiver.try_recv() {
                match msg {
                    WorkerMessage::Log(msg) => buf.extend(msg.as_bytes()),
                    WorkerMessage::Flush(done) => {
                        Self::write_buffer_and_flush(&mut out, &mut buf);
                        // Signal completion (ignore if receiver was dropped)
                        let _ = done.send(());

                        continue;
                    }
                };
            }

            Self::write_buffer(&mut out, &mut buf);
        }
    }
}
