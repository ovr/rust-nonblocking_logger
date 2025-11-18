/// Blocking Behavior Test
///
/// This program tests how log_nonblock and simple_logger handle scenarios
/// where output is slow or blocked (e.g., when piped to a slow consumer).
///
/// Usage:
///   # Test with fast output (baseline):
///   cargo run --example blocking_test log_nonblock
///   cargo run --example blocking_test simple_logger
///
///   # Test with slow output (pipe to slow consumer):
///   cargo run --example blocking_test log_nonblock | pv -L 10K > /dev/null
///   cargo run --example blocking_test simple_logger | pv -L 10K > /dev/null
///
///   The pv command limits throughput to 10KB/s to simulate a slow consumer.
///   Without pv installed, you can use: | dd bs=1K > /dev/null
use log::{LevelFilter, info};
use log_nonblock::NonBlockingLoggerBuilder;
use log_nonblock::{eprintln, println};
use std::env;
use std::time::Instant;

const MESSAGE_COUNT: usize = 10_000;
const MESSAGE_SIZE: usize = 1024; // 1KB messages

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <log_nonblock|simple_logger>", args[0]);
        eprintln!("\nExample:");
        eprintln!("  {} log_nonblock", args[0]);
        eprintln!("  {} simple_logger", args[0]);
        eprintln!("\nTo test blocking behavior, pipe to a slow consumer:");
        eprintln!("  {} log_nonblock | pv -L 10K > /dev/null", args[0]);
        std::process::exit(1);
    }

    let logger_type = &args[1];
    let message = "x".repeat(MESSAGE_SIZE);

    let logger = match logger_type.as_str() {
        "log_nonblock" => {
            eprintln!("Initializing log_nonblock...");
            let logger = NonBlockingLoggerBuilder::new()
                .with_level(LevelFilter::Info)
                .without_timestamps()
                .init()
                .expect("Failed to initialize log_nonblock");
            Some(logger)
        }
        "simple_logger" => {
            eprintln!("Initializing simple_logger...");
            simple_logger::SimpleLogger::new()
                .with_level(LevelFilter::Info)
                .without_timestamps()
                .init()
                .expect("Failed to initialize simple_logger");
            None
        }
        _ => {
            eprintln!("Unknown logger type: {}", logger_type);
            eprintln!("Valid options: log_nonblock, simple_logger");
            std::process::exit(1);
        }
    };

    println!(
        "\nStarting test with {} {}KB messages...",
        MESSAGE_COUNT,
        MESSAGE_SIZE / 1024
    );
    println!(
        "Total data: {} MB",
        (MESSAGE_COUNT * MESSAGE_SIZE) / (1024 * 1024)
    );

    let start = Instant::now();

    // Log messages
    for i in 0..MESSAGE_COUNT {
        info!("Message {} {}", i, message);
    }

    let log_duration = start.elapsed();

    // For log_nonblock, measure flush time separately
    if let Some(logger) = logger {
        let flush_start = Instant::now();
        log::logger().flush();
        let flush_duration = flush_start.elapsed();

        eprintln!("\nResults:");
        eprintln!("  Logging time: {:.3}s", log_duration.as_secs_f64());
        eprintln!("  Flush time: {:.3}s", flush_duration.as_secs_f64());
        eprintln!("  Total time: {:.3}s", start.elapsed().as_secs_f64());
        eprintln!(
            "  Throughput (logging): {:.2} messages/sec",
            MESSAGE_COUNT as f64 / log_duration.as_secs_f64()
        );
        eprintln!(
            "  Throughput (total): {:.2} messages/sec",
            MESSAGE_COUNT as f64 / start.elapsed().as_secs_f64()
        );
        eprintln!(
            "  Throughput (logging): {:.2} MB/sec",
            (MESSAGE_COUNT * MESSAGE_SIZE) as f64 / (1024.0 * 1024.0) / log_duration.as_secs_f64()
        );
        eprintln!(
            "  Throughput (total): {:.2} MB/sec",
            (MESSAGE_COUNT * MESSAGE_SIZE) as f64
                / (1024.0 * 1024.0)
                / start.elapsed().as_secs_f64()
        );

        log::logger().flush();
        logger.shutdown().expect("Failed to shutdown logger");
    } else {
        eprintln!("\nResults:");
        eprintln!("  Total time: {:.3}s", log_duration.as_secs_f64());
        eprintln!(
            "  Throughput: {:.2} messages/sec",
            MESSAGE_COUNT as f64 / log_duration.as_secs_f64()
        );
        eprintln!(
            "  Throughput: {:.2} MB/sec",
            (MESSAGE_COUNT * MESSAGE_SIZE) as f64 / (1024.0 * 1024.0) / log_duration.as_secs_f64()
        );
    }

    eprintln!("\nTest completed!");
}
