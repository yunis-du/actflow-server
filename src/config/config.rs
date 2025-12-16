use std::{env, fs, path::Path};

use serde::Deserialize;
use thiserror::Error;

use crate::common::consts::{DEFAULT_LOG_FILE, DEFAULT_LOG_LEVEL, DEFAULT_LOG_RETENTION, DEFAULT_THIRD_PARTY_LOG_LEVEL};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config file is empty")]
    ConfigFileEmpty,
    #[error("yaml config invalid: {0}")]
    YamlConfigInvalid(String),
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    pub server: ServerConfig,
    pub log: LogConfig,
    pub async_worker_thread_number: u16,
}

impl Config {
    /// Load configuration from a file path
    pub fn load_from_file<T: AsRef<Path>>(path: T) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path).map_err(|e| ConfigError::YamlConfigInvalid(e.to_string()))?;
        Self::load(&contents)
    }

    /// Load configuration from a string
    pub fn load<C: AsRef<str>>(contents: C) -> Result<Self, ConfigError> {
        let contents = contents.as_ref();
        if contents.len() == 0 {
            // parsing empty string leads to EOF error
            Ok(Self::default())
        } else {
            let mut cfg: Self = serde_yaml::from_str(contents).map_err(|e| ConfigError::YamlConfigInvalid(e.to_string()))?;

            if cfg.log.log_file.is_empty() {
                cfg.log.log_file = DEFAULT_LOG_FILE.to_owned();
            }
            // convert relative path to absolute
            if Path::new(&cfg.log.log_file).is_relative() {
                let Ok(mut pb) = env::current_dir() else {
                    return Err(ConfigError::YamlConfigInvalid("get cwd failed".to_owned()));
                };
                pb.push(&cfg.log.log_file);
                match pb.to_str() {
                    Some(s) => cfg.log.log_file = s.to_owned(),
                    None => {
                        return Err(ConfigError::YamlConfigInvalid(format!("invalid log path {}", cfg.log.log_file)));
                    }
                }
            }

            Ok(cfg)
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            log: LogConfig::default(),
            async_worker_thread_number: 16,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "kebab-case")]
pub struct ServerConfig {
    pub port: u16,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, rename_all = "kebab-case")]
pub struct LogConfig {
    pub level: String,
    pub third_party_log_level: String,
    pub log_file: String,
    pub retention: usize,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: DEFAULT_LOG_LEVEL.into(),
            third_party_log_level: DEFAULT_THIRD_PARTY_LOG_LEVEL.into(),
            log_file: DEFAULT_LOG_FILE.into(),
            retention: DEFAULT_LOG_RETENTION,
        }
    }
}
