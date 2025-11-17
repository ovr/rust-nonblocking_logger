use rust_nonblocking_logger::NonBlockingLogger;

fn main() {
    NonBlockingLogger::new().init().unwrap();

    log::warn!("This is an example message.");
}