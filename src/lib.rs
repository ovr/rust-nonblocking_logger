use colored::Colorize;
use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use time::{OffsetDateTime, UtcOffset};
use time::format_description::FormatItem;

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
const TIMESTAMP_FORMAT_UTC: &[FormatItem] =
    time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

pub struct NonBlockingLogger {
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

impl NonBlockingLogger {
    pub fn new() -> Self {
        Self {
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
        }
    }

    #[must_use = "You must call init() to begin logging"]
    pub fn with_module_level(mut self, target: &str, level: LevelFilter) -> Self {
        self.module_levels.push((target.to_string(), level));
        self.module_levels
            .sort_by_key(|(name, _level)| name.len().wrapping_neg());
        self
    }

    /// Control whether messages are colored or not.
    ///
    /// This method is only available if the `colored` feature is enabled.
    #[must_use = "You must call init() to begin logging"]
    #[cfg(feature = "colors")]
    pub fn with_colors(mut self, colors: bool) -> Self {
        self.colors = colors;
        self
    }

    /// Configure the logger
    pub fn max_level(&self) -> LevelFilter {
        let max_level = self.module_levels.iter().map(|(_name, level)| level).copied().max();
        max_level
            .map(|lvl| lvl.max(self.default_level))
            .unwrap_or(self.default_level)
    }

    pub fn init(self) -> Result<(), SetLoggerError> {
        #[cfg(all(feature = "colored", feature = "stderr"))]
        use_stderr_for_colors();

        log::set_max_level(self.max_level());
        log::set_boxed_logger(Box::new(self))
    }
}

impl Log for NonBlockingLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        &metadata.level().to_level_filter()
            <= self
            .module_levels
            .iter()
            /* At this point the Vec is already sorted so that we can simply take
             * the first match
             */
            .find(|(name, _level)| metadata.target().starts_with(name))
            .map(|(_name, level)| level)
            .unwrap_or(&self.default_level)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let level_string = {
                #[cfg(feature = "colors")]
                {
                    if self.colors {
                        match record.level() {
                            Level::Error => format!("{:<5}", record.level().to_string()).red().to_string(),
                            Level::Warn => format!("{:<5}", record.level().to_string()).yellow().to_string(),
                            Level::Info => format!("{:<5}", record.level().to_string()).cyan().to_string(),
                            Level::Debug => format!("{:<5}", record.level().to_string()).purple().to_string(),
                            Level::Trace => format!("{:<5}", record.level().to_string()).normal().to_string(),
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
                if self.threads {
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
                match self.timestamps {
                    Timestamps::None => "".to_string(),
                    Timestamps::Utc => format!(
                        "{} ",
                        OffsetDateTime::now_utc()
                            .format(&self.timestamps_format.unwrap_or(TIMESTAMP_FORMAT_UTC))
                            .unwrap()
                    ),
                    Timestamps::UtcOffset(offset) => format!(
                        "{} ",
                        OffsetDateTime::now_utc()
                            .to_offset(offset)
                            .format(&self.timestamps_format.unwrap_or(TIMESTAMP_FORMAT_OFFSET))
                            .unwrap()
                    ),
                }

                #[cfg(not(feature = "timestamps"))]
                ""
            };

            let message = format!("{}{} [{}{}] {}", timestamp, level_string, target, thread, record.args());

            #[cfg(not(feature = "stderr"))]
            println!("{}", message);

            #[cfg(feature = "stderr")]
            eprintln!("{}", message);
        }
    }

    fn flush(&self) {
        todo!()
    }
}

/// The colored crate will disable colors when STDOUT is not a terminal. This method overrides this
/// behaviour to check the status of STDERR instead.
#[cfg(all(feature = "colored", feature = "stderr"))]
fn use_stderr_for_colors() {
    use std::io::{stderr, IsTerminal};

    colored::control::set_override(stderr().is_terminal());
}