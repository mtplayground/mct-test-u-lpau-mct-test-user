use std::{
    env,
    num::ParseIntError,
    path::PathBuf,
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub anthropic_api_key: String,
    pub chromium_path: PathBuf,
    pub scan_timeout: Duration,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = required_var("DATABASE_URL")?;
        let anthropic_api_key = required_var("ANTHROPIC_API_KEY")?;
        let chromium_path = PathBuf::from(required_var("CHROMIUM_PATH")?);
        let scan_timeout_secs = parse_u64_var("SCAN_TIMEOUT_SECS")?;
        let port = parse_u16_var("PORT")?;

        Ok(Self {
            database_url,
            anthropic_api_key,
            chromium_path,
            scan_timeout: Duration::from_secs(scan_timeout_secs),
            port,
        })
    }
}

fn required_var(name: &'static str) -> Result<String, ConfigError> {
    match env::var(name) {
        Ok(value) if value.trim().is_empty() => Err(ConfigError::EmptyVar { name }),
        Ok(value) => Ok(value),
        Err(env::VarError::NotPresent) => Err(ConfigError::MissingVar { name }),
        Err(source) => Err(ConfigError::UnreadableVar { name, source }),
    }
}

fn parse_u16_var(name: &'static str) -> Result<u16, ConfigError> {
    let value = required_var(name)?;
    value
        .parse::<u16>()
        .map_err(|source| ConfigError::InvalidU16 { name, value, source })
}

fn parse_u64_var(name: &'static str) -> Result<u64, ConfigError> {
    let value = required_var(name)?;
    value
        .parse::<u64>()
        .map_err(|source| ConfigError::InvalidU64 { name, value, source })
}

#[derive(Debug)]
pub enum ConfigError {
    EmptyVar {
        name: &'static str,
    },
    InvalidU16 {
        name: &'static str,
        value: String,
        source: ParseIntError,
    },
    InvalidU64 {
        name: &'static str,
        value: String,
        source: ParseIntError,
    },
    MissingVar {
        name: &'static str,
    },
    UnreadableVar {
        name: &'static str,
        source: env::VarError,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyVar { name } => write!(f, "{name} is set but empty"),
            Self::InvalidU16 {
                name,
                value,
                source,
            } => write!(f, "{name} must be a valid u16, got '{value}': {source}"),
            Self::InvalidU64 {
                name,
                value,
                source,
            } => write!(f, "{name} must be a valid u64, got '{value}': {source}"),
            Self::MissingVar { name } => write!(f, "missing required environment variable {name}"),
            Self::UnreadableVar { name, source } => {
                write!(f, "failed to read environment variable {name}: {source}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}
