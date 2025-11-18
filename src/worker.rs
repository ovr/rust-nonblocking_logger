use crossbeam_channel::{Receiver, Sender, TryRecvError};
use std::io;
use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

#[cfg(unix)]
use std::os::fd::AsRawFd;

#[cfg(not(unix))]
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

    fn write_buffer(buf: &[u8]) -> Result<(), io::Error> {
        let mut cursor = 0;

        let mut pipe = {
            #[cfg(not(feature = "stderr"))]
            {
                io::stdout()
            }

            #[cfg(feature = "stderr")]
            {
                io::stderr()
            }
        };

        // Write all buffered data
        while cursor < buf.len() {
            let slice = &buf[cursor..];
            match pipe.write(slice) {
                Ok(0) => {
                    #[cfg(unix)]
                    {
                        // Nothing accepted, wait for stdout to become writable using poll
                        crate::io::wait_writable(pipe.as_raw_fd())?
                    }

                    #[cfg(not(unix))]
                    thread::sleep(Duration::from_millis(1));
                }
                Ok(n) => {
                    // Advance cursor by number of bytes written
                    cursor += n;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    #[cfg(unix)]
                    {
                        // Wait for stdout to become writable usig poll
                        crate::io::wait_writable(pipe.as_raw_fd())?
                    }

                    #[cfg(not(unix))]
                    thread::sleep(Duration::from_millis(1));
                }
                Err(err) => {
                    // Hard error, give up
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    fn run(&mut self) {
        let stdout = io::stdout();

        let mut pipe_buffer = Vec::with_capacity(2 * 1024);

        while self.running.load(Ordering::SeqCst) {
            // block until at least one message
            let first_message_to_pipe = match self.receiver.recv() {
                Ok(msg) => match msg {
                    WorkerMessage::Log(msg) => {
                        if msg.len() < 1280 {
                            msg
                        } else {
                            if let Err(err) = Self::write_buffer(msg.as_bytes()) {
                                crate::io::write_stderr_with_retry_internal(&format!(
                                    "Error waiting for stdout: {}",
                                    err
                                ))
                            }

                            continue;
                        }
                    }
                    WorkerMessage::Flush(done) => {
                        if let Err(err) = stdout.lock().flush() {
                            crate::io::write_stderr_with_retry_internal(&format!(
                                "Error flushing stdout: {}",
                                err
                            ));
                        }

                        // Signal completion (ignore if receiver was dropped)
                        let _ = done.send(());

                        continue;
                    }
                },
                Err(_) => break, // channel closed
            };

            // pipe one more message into the buffer (optimization)
            match self.receiver.try_recv() {
                Ok(msg) => match msg {
                    WorkerMessage::Log(second_message_to_pipe) => {
                        pipe_buffer.extend_from_slice(first_message_to_pipe.as_bytes());
                        drop(first_message_to_pipe);

                        pipe_buffer.extend_from_slice(second_message_to_pipe.as_bytes());
                        drop(second_message_to_pipe);

                        let res = Self::write_buffer(pipe_buffer.as_slice());

                        pipe_buffer.clear();

                        if let Err(err) = res {
                            crate::io::write_stderr_with_retry_internal(&format!(
                                "Error waiting for stdout: {}",
                                err
                            ))
                        }
                    }
                    WorkerMessage::Flush(done) => {
                        let res = Self::write_buffer(first_message_to_pipe.as_bytes());
                        let flush_res = stdout.lock().flush();

                        // Signal completion (ignore if receiver was dropped)
                        let _ = done.send(());

                        if let Err(err) = res {
                            crate::io::write_stderr_with_retry_internal(&format!(
                                "Error waiting for stdout: {}",
                                err
                            ))
                        }

                        if let Err(err) = flush_res {
                            crate::io::write_stderr_with_retry_internal(&format!(
                                "Error flushing stdout: {}",
                                err
                            ));
                        }

                        continue;
                    }
                },
                Err(TryRecvError::Empty) => {
                    if let Err(err) = Self::write_buffer(first_message_to_pipe.as_bytes()) {
                        crate::io::write_stderr_with_retry_internal(&format!(
                            "Error waiting for stdout: {}",
                            err
                        ))
                    }
                }
                Err(TryRecvError::Disconnected) => break, // channel closed
            }
        }
    }
}
