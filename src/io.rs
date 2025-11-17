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

/// On Windows, we can't easily set stdout/stderr to non-blocking mode
/// This is a no-op that returns success
#[cfg(not(unix))]
pub fn set_nonblocking(_fd: RawFd) -> io::Result<()> {
    // Windows doesn't support non-blocking mode for console handles
    // We'll rely on the worker thread and channel for async behavior
    Ok(())
}

/// Helper function to write error messages to stderr with WouldBlock retry logic.
/// Uses the same retry pattern as write_buffer for consistency.
#[cfg(unix)]
pub fn write_stderr_with_retry(msg: &str) {
    use io::Write;
    use std::thread;
    use std::time::Duration;

    let formatted = format!("[log_nonblock error] {}\n", msg);
    let bytes = formatted.as_bytes();

    let stderr = io::stderr();
    let mut out = stderr.lock();
    let mut written = 0;

    while written < bytes.len() {
        match out.write(&bytes[written..]) {
            Ok(0) => {
                // Nothing accepted, retry after short sleep
                thread::sleep(Duration::from_millis(1));
            }
            Ok(n) => {
                // Remove written bytes
                written += n;
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Retry after short sleep
                thread::sleep(Duration::from_millis(1));
            }
            Err(_) => {
                // Hard error, give up
                break;
            }
        }
    }
}

/// On Windows, use eprintln! as fallback (no recursion risk without non-blocking mode)
#[cfg(not(unix))]
pub fn write_stderr_with_retry(msg: &str) {
    eprintln!("[log_nonblock error] {}", msg);
}
