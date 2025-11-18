/// Non-blocking `println!` macro that uses write_stdout_with_retry.
///
/// This macro mimics the behavior of `std::println!` but uses non-blocking I/O
/// with retry logic to prevent panics and blocking on slow output.
///
/// Unlike the standard `println!`, this macro:
/// - Handles `WouldBlock` errors gracefully using poll() on Unix systems
/// - Retries writes until completion instead of panicking
/// - Never blocks indefinitely on full buffers
///
/// # Examples
///
/// ```
/// use log_nonblock::println;
///
/// println!("Hello, world!");
/// println!("The answer is {}", 42);
/// println!("{:?}", some_struct);
/// println!(); // Just a newline
/// ```
///
/// # Note
///
/// This macro writes directly to stdout synchronously (with retry logic).
/// It is independent of the `NonBlockingLogger` and doesn't use the
/// background worker thread. For high-frequency logging, consider using
/// the logger with its background worker instead.
#[macro_export]
macro_rules! println {
    () => {
        $crate::io::write_stdout_with_retry("\n")
    };
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        let message_with_newline = format!("{}\n", message);
        $crate::io::write_stdout_with_retry(&message_with_newline)
    }};
}

/// Non-blocking `eprintln!` macro that uses write_stderr_with_retry.
///
/// This macro mimics the behavior of `std::eprintln!` but uses non-blocking I/O
/// with retry logic to prevent panics and blocking on slow output.
///
/// Unlike the standard `eprintln!`, this macro:
/// - Handles `WouldBlock` errors gracefully using poll() on Unix systems
/// - Retries writes until completion instead of panicking
/// - Never blocks indefinitely on full buffers
///
/// # Examples
///
/// ```
/// use log_nonblock::eprintln;
///
/// eprintln!("Error occurred!");
/// eprintln!("Error code: {}", 500);
/// eprintln!("{:?}", error);
/// eprintln!(); // Just a newline
/// ```
///
/// # Note
///
/// This macro writes directly to stderr synchronously (with retry logic).
/// It is independent of the `NonBlockingLogger` and doesn't use the
/// background worker thread. For high-frequency logging, consider using
/// the logger with its background worker instead.
#[macro_export]
macro_rules! eprintln {
    () => {
        $crate::io::write_stderr_with_retry("\n")
    };
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        let message_with_newline = format!("{}\n", message);
        $crate::io::write_stderr_with_retry(&message_with_newline)
    }};
}
