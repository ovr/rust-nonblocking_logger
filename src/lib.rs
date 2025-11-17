#[cfg(feature = "colored")]
use colored::Colorize;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
#[cfg(feature = "timestamps")]
use time::{OffsetDateTime, UtcOffset, format_description::FormatItem};

mod io;
mod worker;

#[cfg(feature = "timestamps")]
#[derive(PartialEq)]
enum Timestamps {
    None,
    Utc,
    UtcOffset(UtcOffset),
}

#[cfg(feature = "timestamps")]
const TIMESTAMP_FORMAT_OFFSET: &[FormatItem] = time::macros::format_description!(
    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3][offset_hour sign:mandatory]:[offset_minute]"
);

#[cfg(feature = "timestamps")]
const TIMESTAMP_FORMAT_UTC: &[FormatItem] = time::macros::format_description!(
    "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z"
);

#[derive(Clone, Debug)]
pub struct NonBlockingOptions {
    /// The default logging level
    default_level: LevelFilter,

    /// The specific logging level for each module
    ///
    /// This is used to override the default value for some specific modules.
    ///
    /// This must be sorted from most-specific to least-specific, so that [`enabled`](#method.enabled) can scan the
    /// vector for the first match to give us the desired log level for a module.
    module_levels: Vec<(String, LevelFilter)>,

    #[cfg(feature = "colors")]
    colors: bool,

    #[cfg(feature = "timestamps")]
    timestamps: Timestamps,

    #[cfg(feature = "timestamps")]
    timestamps_format: Option<&'static [FormatItem<'static>]>,
}

pub struct NonBlockingLoggerBuilder {
    options: NonBlockingOptions,
}

impl NonBlockingLoggerBuilder {
    pub fn new() -> Self {
        Self {
            options: NonBlockingOptions {
                default_level: LevelFilter::Trace,
                module_levels: Vec::new(),

                #[cfg(feature = "threads")]
                threads: false,

                #[cfg(feature = "timestamps")]
                timestamps: Timestamps::Utc,

                #[cfg(feature = "timestamps")]
                timestamps_format: None,

                #[cfg(feature = "colors")]
                colors: true,
            },
        }
    }

    #[must_use = "You must call init() to begin logging"]
    pub fn with_module_level(mut self, target: &str, level: LevelFilter) -> Self {
        self.options.module_levels.push((target.to_string(), level));
        self.options
            .module_levels
            .sort_by_key(|(name, _level)| name.len().wrapping_neg());
        self
    }

    /// Control whether messages are colored or not.
    ///
    /// This method is only available if the `colored` feature is enabled.
    #[must_use = "You must call init() to begin logging"]
    #[cfg(feature = "colors")]
    pub fn with_colors(mut self, colors: bool) -> Self {
        self.options.colors = colors;
        self
    }

    pub fn init(self) -> Result<NonBlockingLogger, SetLoggerError> {
        #[cfg(all(feature = "colored", feature = "stderr"))]
        use_stderr_for_colors();

        #[cfg(not(feature = "stderr"))]
        {
            if let Err(err) = io::set_nonblocking(std::io::stdout().as_raw_fd()) {
                println!("Failed to set STDOUT to non-blocking mode: {}", err);
            }
        }

        #[cfg(feature = "stderr")]
        {
            if let Err(err) = io::set_nonblocking(std::io::stdout().as_raw_fd()) {
                eprintln!("Failed to set STDERR to non-blocking mode: {}", err);
            }
        }

        let (sender, receiver) = crossbeam_channel::bounded(1024);

        let (worker, running) = worker::LogWorker::new(receiver);
        if let Err(err) = worker.spawn() {
            println!("Failed to spawn logger worker: {}", err);
        };

        let logger = NonBlockingLogger {
            options: self.options,
            sender,
            running,
        };

        log::set_max_level(logger.max_level());
        log::set_boxed_logger(Box::new(logger.clone()))?;

        Ok(logger)
    }
}

#[derive(Debug)]
pub enum NonBlockingLoggerError {
    Error { reason: String },
}

impl std::fmt::Display for NonBlockingLoggerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NonBlockingLoggerError::Error { reason } => write!(f, "NonBlockingLoggerError: {}", reason),
        }
    }
}

impl std::error::Error for NonBlockingLoggerError {}

#[derive(Clone, Debug)]
pub struct NonBlockingLogger {
    options: NonBlockingOptions,
    sender: crossbeam_channel::Sender<worker::WorkerMessage>,
    running: Arc<AtomicBool>,
}

impl NonBlockingLogger {
    pub fn max_level(&self) -> LevelFilter {
        let max_level = self
            .options
            .module_levels
            .iter()
            .map(|(_name, level)| level)
            .copied()
            .max();
        max_level
            .map(|lvl| lvl.max(self.options.default_level))
            .unwrap_or(self.options.default_level)
    }

    pub fn shutdown(self) -> Result<(), NonBlockingLoggerError> {
        let compare = self.running.compare_exchange(
            true,
            false,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        );

        if compare.is_err() {
            Err(NonBlockingLoggerError::Error {
                reason: "Failed to shutdown logger: It was already shutted down".to_string(),
            })
        } else {
            Ok(())
        }
    }
}

impl Log for NonBlockingLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        &metadata.level().to_level_filter()
            <= self
                .options
                .module_levels
                .iter()
                /* At this point the Vec is already sorted so that we can simply take
                 * the first match
                 */
                .find(|(name, _level)| metadata.target().starts_with(name))
                .map(|(_name, level)| level)
                .unwrap_or(&self.options.default_level)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level_string = {
                #[cfg(feature = "colors")]
                {
                    if self.colors {
                        match record.level() {
                            Level::Error => format!("{:<5}", record.level().to_string())
                                .red()
                                .to_string(),
                            Level::Warn => format!("{:<5}", record.level().to_string())
                                .yellow()
                                .to_string(),
                            Level::Info => format!("{:<5}", record.level().to_string())
                                .cyan()
                                .to_string(),
                            Level::Debug => format!("{:<5}", record.level().to_string())
                                .purple()
                                .to_string(),
                            Level::Trace => format!("{:<5}", record.level().to_string())
                                .normal()
                                .to_string(),
                        }
                    } else {
                        format!("{:<5}", record.level().to_string())
                    }
                }
                #[cfg(not(feature = "colors"))]
                {
                    format!("{:<5}", record.level().to_string())
                }
            };

            let target = if !record.target().is_empty() {
                record.target()
            } else {
                record.module_path().unwrap_or_default()
            };

            let thread = {
                #[cfg(feature = "threads")]
                if self.options.threads {
                    let thread = std::thread::current();

                    format!("@{}", {
                        #[cfg(feature = "nightly")]
                        {
                            thread.name().unwrap_or(&thread.id().as_u64().to_string())
                        }

                        #[cfg(not(feature = "nightly"))]
                        {
                            thread.name().unwrap_or("?")
                        }
                    })
                } else {
                    "".to_string()
                }

                #[cfg(not(feature = "threads"))]
                ""
            };

            let timestamp = {
                #[cfg(feature = "timestamps")]
                match self.options.timestamps {
                    Timestamps::None => "".to_string(),
                    Timestamps::Utc => format!(
                        "{} ",
                        OffsetDateTime::now_utc()
                            .format(
                                &self
                                    .options
                                    .timestamps_format
                                    .unwrap_or(TIMESTAMP_FORMAT_UTC)
                            )
                            .unwrap()
                    ),
                    Timestamps::UtcOffset(offset) => format!(
                        "{} ",
                        OffsetDateTime::now_utc()
                            .to_offset(offset)
                            .format(
                                &self
                                    .options
                                    .timestamps_format
                                    .unwrap_or(TIMESTAMP_FORMAT_OFFSET)
                            )
                            .unwrap()
                    ),
                }

                #[cfg(not(feature = "timestamps"))]
                ""
            };

            let message = format!(
                "{}{} [{}{}] {}\r\n",
                timestamp,
                level_string,
                target,
                thread,
                record.args()
            );

            self.sender.send(worker::WorkerMessage::Log(message)).unwrap();
        }
    }

    fn flush(&self) {
        let (done_tx, done_rx) = crossbeam_channel::bounded(1);
        self.sender.send(worker::WorkerMessage::Flush(done_tx)).unwrap();

        // Block until flush completes
        let _ = done_rx.recv();
    }
}

/// The colored crate will disable colors when STDOUT is not a terminal. This method overrides this
/// behavior to check the status of STDERR instead.
#[cfg(all(feature = "colored", feature = "stderr"))]
fn use_stderr_for_colors() {
    use std::io::{IsTerminal, stderr};

    colored::control::set_override(stderr().is_terminal());
}
