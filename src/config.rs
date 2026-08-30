use std::env::{self, VarError};
use std::path::PathBuf;
use std::time::Duration;

/// Minimum length for a `JWT_SECRET` supplied via the environment.
const MIN_SECRET_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("environment variable {name} is not valid UTF-8")]
    NotUnicode { name: &'static str },

    #[error("environment variable {name} has an invalid value: {value}")]
    InvalidValue { name: &'static str, value: String },

    #[error("JWT_SECRET must be at least 32 characters long")]
    WeakSecret,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub jwt_secret: Vec<u8>,
    pub token_ttl: Duration,
    pub static_dir: PathBuf,
    pub environment: String,
}

impl Config {
    /// Reads configuration from the process environment, falling back to
    /// development-friendly defaults for everything except the JWT secret,
    /// which is randomised per process when unset.
    pub fn from_env() -> Result<Self, ConfigError> {
        let host = var("HOST")?.unwrap_or_else(|| "127.0.0.1".to_string());
        let port = parse_var("PORT", 8080)?;
        let ttl_minutes: u64 = parse_var("TOKEN_TTL_MINUTES", 60)?;

        let database_url =
            var("DATABASE_URL")?.unwrap_or_else(|| "sqlite://data/casino.db".to_string());
        let static_dir = var("STATIC_DIR")?
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("static"));
        let environment = var("APP_ENV")?.unwrap_or_else(|| "development".to_string());

        let jwt_secret = match var("JWT_SECRET")? {
            Some(secret) if secret.len() < MIN_SECRET_LEN => return Err(ConfigError::WeakSecret),
            Some(secret) => secret.into_bytes(),
            None => {
                tracing::warn!(
                    "JWT_SECRET is unset; generating a random one. \
                     Issued tokens will be invalidated on restart."
                );
                random_secret()
            }
        };

        Ok(Self {
            host,
            port,
            database_url,
            jwt_secret,
            token_ttl: Duration::from_secs(ttl_minutes * 60),
            static_dir,
            environment,
        })
    }
}

fn var(name: &'static str) -> Result<Option<String>, ConfigError> {
    match env::var(name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => Err(ConfigError::NotUnicode { name }),
    }
}

fn parse_var<T>(name: &'static str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
{
    match var(name)? {
        None => Ok(default),
        Some(raw) => raw
            .parse()
            .map_err(|_| ConfigError::InvalidValue { name, value: raw }),
    }
}

fn random_secret() -> Vec<u8> {
    use rand_core::{OsRng, RngCore};

    let mut bytes = vec![0u8; 64];
    OsRng.fill_bytes(&mut bytes);
    bytes
}
