# log-nonblock

[![Crates.io](https://img.shields.io/crates/v/rust-nonblocking_logger.svg)](https://crates.io/crates/rust-nonblocking_logger)
[![Documentation](https://docs.rs/rust-nonblocking_logger/badge.svg)](https://docs.rs/rust-nonblocking_logger)

> A high-performance Rust logging library that implements truly non-blocking writes to STDOUT/STDERR.

## Motivation

### Problem #1: STDOUT/STDERR Writes Are Synchronous Blocking Operations

<details>
<summary>Click to expand explanation</summary>

Writing to STDOUT or STDERR is a **slow I/O operation**, and by default in Rust (and most languages), these are **synchronous blocking calls**:

```rust
println!("Log message");  // Your thread STOPS here until the write completes
log::info!("Log message"); // Same - blocks until written
```

When you write to STDOUT/STDERR, your application thread **stops and waits** until the operating system completes the write operation. This might seem fast on your terminal, but it becomes a critical bottleneck when:

- **STDOUT is piped to a slow consumer**: Files on slow disks, network streams, terminals that can't keep up
- **Large log messages**: Writing megabytes of data can take hundreds of milliseconds
- **High-frequency logging**: Each log call blocks your thread, multiplying the cost
- **Performance-critical applications**: Web servers, real-time systems, high-throughput data processing

**The impact**: Each log operation can take 1-10ms or more, during which your application is doing nothing but waiting for I/O to complete.

</details>

### Problem #2: Rust's Standard Library Doesn't Support Non-Blocking STDOUT

<details>
<summary>Click to expand explanation</summary>

You might think: "I'll just set STDOUT to non-blocking mode at the OS level!" Unfortunately, this doesn't work with Rust's standard library:

```rust
// Set STDOUT to non-blocking mode (using fcntl)
set_nonblocking(stdout);

// This will PANIC when the buffer is full!
println!("Large message");  // L thread panicked: failed printing to stdout: Resource temporarily unavailable
```

**The problem**: When STDOUT/STDERR is in non-blocking mode, the OS returns `WouldBlock` errors when the output buffer is full. Rust's `std::io::Stdout` and the `println!`/`eprintln!` macros are **not designed to handle this** - they will panic immediately.

This happens with:
- **Large messages** that don't fit in the kernel buffer (~64KB on most systems)
- **Parallel usage** from multiple threads overwhelming the output buffer
- **Any situation** where the consumer can't keep up with your write rate

You cannot simply use non-blocking I/O with Rust's standard library - you need proper handling of `WouldBlock` errors.

</details>

### Problem #3: The Performance Impact

<details>
<summary>Click to expand explanation</summary>

In a typical web application logging at INFO level:

```rust
// Each request logs ~5-10 times
log::info!("Request received: {}", request);
// ... blocked for 1-5ms ...
log::debug!("Processing: {}", data);
// ... blocked for 1-5ms ...
log::info!("Response sent: {}", response);
// ... blocked for 1-5ms ...
```

**Result**: 5-25ms of your request latency is spent waiting for I/O operations. In a system handling 1000 req/s, this can be the difference between meeting your SLA and missing it.

</details>

### License

MIT

Formating and options part are based on [rust-simple_logger](https://github.com/borntyping/rust-simple_logger), which is authored by [Sam Clements](https://github.com/borntyping) under MIT license
