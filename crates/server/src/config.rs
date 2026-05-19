use std::{
    env,
    num::ParseIntError,
    path::PathBuf,
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub anthropic_api_key: Option<String>,
    pub chromium_path: PathBuf,
    pub scan_timeout: Duration,
    pub port: u16,
    pub allow_private_urls: bool,
    pub e2e_fixture_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = required_var("DATABASE_URL")?;
        let anthropic_api_key = optional_var("ANTHROPIC_API_KEY")?;
        let chromium_path = PathBuf::from(required_var("CHROMIUM_PATH")?);
        let scan_timeout_secs = parse_u64_var("SCAN_TIMEOUT_SECS")?;
        let port = parse_u16_var("PORT")?;
        let allow_private_urls = parse_bool_var("ZEROCLAW_ALLOW_PRIVATE_URLS")?.unwrap_or(false);
        let e2e_fixture_url = optional_var("ZEROCLAW_E2E_FIXTURE_URL")?;

        Ok(Self {
            database_url,
            anthropic_api_key,
            chromium_path,
            scan_timeout: Duration::from_secs(scan_timeout_secs),
            port,
            allow_private_urls,
            e2e_fixture_url,
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

fn optional_var(name: &'static str) -> Result<Option<String>, ConfigError> {
    match env::var(name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(source) => Err(ConfigError::UnreadableVar { name, source }),
    }
}

fn parse_bool_var(name: &'static str) -> Result<Option<bool>, ConfigError> {
    match optional_var(name)? {
        Some(value) => parse_bool(name, value).map(Some),
        None => Ok(None),
    }
}

fn parse_bool(name: &'static str, value: String) -> Result<bool, ConfigError> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(ConfigError::InvalidBool { name, value }),
    }
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
    InvalidBool {
        name: &'static str,
        value: String,
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
            Self::InvalidBool { name, value } => {
                write!(f, "{name} must be a boolean flag, got '{value}'")
            }
            Self::MissingVar { name } => write!(f, "missing required environment variable {name}"),
            Self::UnreadableVar { name, source } => {
                write!(f, "failed to read environment variable {name}: {source}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}
