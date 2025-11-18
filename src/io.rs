use std::io;
use std::os::fd::RawFd;

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

pub fn write_stderr_with_retry(msg: &str) {
    use io::Write;
    use std::os::fd::AsRawFd;

    let formatted = format!("[log_nonblock error] {}\n", msg);
    let bytes = formatted.as_bytes();

    let stderr = io::stderr();
    let mut out = stderr.lock();
    let mut written = 0;

    while written < bytes.len() {
        match out.write(&bytes[written..]) {
            Ok(0) => {
                #[cfg(unix)]
                {
                    // Nothing accepted, wait for stderr to become writable
                    if wait_writable(stderr.as_raw_fd()).is_err() {
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
                    // Wait for stderr to become writable
                    if wait_writable(stderr.as_raw_fd()).is_err() {
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
