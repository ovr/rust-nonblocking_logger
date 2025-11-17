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
