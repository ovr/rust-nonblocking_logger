use std::io;
use std::io::Write;
use std::os::fd::{AsRawFd, RawFd};

/// Sets a file descriptor to non-blocking mode on Unix systems
#[cfg(unix)]
pub fn set_nonblocking(fd: RawFd) -> Result<(), io::Error> {
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags == -1 {
            return Err(io::Error::last_os_error());
        }

        if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Waits for a file descriptor to become writable using poll().
/// This is more efficient than sleeping when handling WouldBlock errors.
/// Returns Ok(()) if the fd becomes writable, or Err if poll fails.
#[cfg(unix)]
pub(crate) fn wait_writable(fd: RawFd) -> Result<(), io::Error> {
    unsafe {
        let mut pollfd = libc::pollfd {
            fd,
            events: libc::POLLOUT,
            revents: 0,
        };

        // Wait indefinitely for the fd to become writable
        let ret = libc::poll(&mut pollfd as *mut libc::pollfd, 1, -1);

        if ret == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }
}

/// On Windows, we can't easily set stdout/stderr to non-blocking mode
/// This is a no-op that returns success
#[cfg(not(unix))]
pub fn set_nonblocking(_fd: RawFd) -> io::Result<()> {
    // Windows doesn't support non-blocking mode for console handles
    // We'll rely on the worker thread and channel for async behavior
    Ok(())
}

/// Internal function for writing error messages to STDERR with retry logic.
#[allow(unused)]
pub(crate) fn write_stderr_with_retry_internal(msg: &str) {
    let out = io::stderr();
    write_with_retry_internal(
        out.lock(),
        out.as_raw_fd(),
        &format!("[log_nonblock error] {}\n", msg),
    )
}

/// Internal function for writing error messages to STDOUT with retry logic.
#[allow(unused)]
pub(crate) fn write_stdout_with_retry_internal(msg: &str) {
    let out = io::stdout();
    write_with_retry_internal(
        out.lock(),
        out.as_raw_fd(),
        &format!("[log_nonblock error] {}\n", msg),
    )
}

fn write_with_retry_internal<W: Write>(mut out: W, raw_fd: RawFd, msg: &str) {
    let bytes = msg.as_bytes();

    let mut written = 0;

    while written < bytes.len() {
        match out.write(&bytes[written..]) {
            Ok(0) => {
                #[cfg(unix)]
                {
                    // Nothing accepted, wait for fd to become writable
                    if wait_writable(raw_fd).is_err() {
                        // If poll fails, give up
                        break;
                    }
                }
            }
            Ok(n) => {
                // Remove written bytes
                written += n;
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                #[cfg(unix)]
                {
                    // Wait for fd to become writable
                    if wait_writable(raw_fd).is_err() {
                        // If poll fails, give up
                        break;
                    }
                }
            }
            Err(_) => {
                // Hard error, give up
                break;
            }
        }
    }
}

/// Writes a message to stdout with retry logic, without adding any prefix.
/// This function is used by the `println!` macro when the `macros` feature is enabled.
#[doc(hidden)]
#[cfg(feature = "macros")]
pub fn write_stdout_with_retry(msg: &str) {
    let out = io::stdout();
    write_with_retry_internal(out.lock(), out.as_raw_fd(), msg)
}

/// Writes a message to stderr with retry logic, without adding any prefix.
/// This function is used by the `eprintln!` macro when the `macros` feature is enabled.
#[doc(hidden)]
#[cfg(feature = "macros")]
pub fn write_stderr_with_retry(msg: &str) {
    let out = io::stderr();
    write_with_retry_internal(out.lock(), out.as_raw_fd(), msg)
}
