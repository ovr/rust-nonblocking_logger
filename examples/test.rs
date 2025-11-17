use log_nonblock::NonBlockingLoggerBuilder;

fn main() {
    let logger = NonBlockingLoggerBuilder::new().init().unwrap();

    log::warn!("This is an example message.");

    let large_string =
        "[super large message that should crash fd in non blocking mode]".repeat(100000);
    log::warn!("{}", large_string);
    log::logger().flush();

    log::warn!("This is an example message.");
    log::logger().flush();

    logger.shutdown().unwrap();
}
