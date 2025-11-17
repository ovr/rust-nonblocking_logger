use log::Log;
use rust_nonblocking_logger::NonBlockingLoggerBuilder;
use std::thread;

fn main() {
    let logger = NonBlockingLoggerBuilder::new().init().unwrap();

    log::warn!("This is an example message.");

    let large_string =
        "[super large message that should crash fd in non blocking mode]".repeat(100000);
    log::warn!("{}", large_string);

    thread::sleep(std::time::Duration::from_secs(2));
    log::warn!("This is an example message.");
    thread::sleep(std::time::Duration::from_secs(2));

    log::logger().flush();

    thread::sleep(std::time::Duration::from_secs(2));
}
