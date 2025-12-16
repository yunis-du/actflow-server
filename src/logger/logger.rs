use std::{fs, path::Path};

use anyhow::Result;
use flexi_logger::{Age, Cleanup, Criterion, Duplicate, FileSpec, Logger, Naming, colored_opt_format};

use crate::config;

/// Initializes the application's logging system
pub fn init_logger(log_config: &config::LogConfig) -> Result<Logger> {
    let base_path = match Path::new(&log_config.log_file).parent() {
        Some(base_path) => base_path,
        None => {
            return Err(anyhow::Error::msg(format!(
                "Init logger failure, the log path({}) may be incorrectly configured",
                log_config.log_file
            )));
        }
    };
    let write_to_file = if base_path.exists() {
        base_path.metadata().ok().map(|meta| !meta.permissions().readonly()).unwrap_or(false)
    } else {
        fs::create_dir_all(base_path).is_ok()
    };

    let crate_name = env!("CARGO_PKG_NAME").replace("-", "_");
    let log_level = format!("{},{}={}", log_config.third_party_log_level, crate_name, log_config.level);
    let logger = Logger::try_with_env_or_str(&log_level)?.format(colored_opt_format);

    let logger = if write_to_file {
        logger
            .log_to_file(FileSpec::try_from(&log_config.log_file)?)
            // .duplicate_to_stdout(Duplicate::All)
            .duplicate_to_stderr(Duplicate::All)
            .rotate(
                Criterion::Age(Age::Day),
                Naming::Timestamps,
                Cleanup::KeepLogFiles(log_config.retention),
            )
            .create_symlink(&log_config.log_file)
            .append()
    } else {
        eprintln!(
            "Log file path '{}' access denied, logs will not be written to file",
            log_config.log_file
        );
        logger
    };

    Ok(logger)
}
