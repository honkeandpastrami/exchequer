//! Configuration from environment variables (design §14.1).
//!
//! Secrets never arrive as plain environment variables: for each secret
//! setting `X`, the server reads the file named by `X_FILE` (a podman secret
//! under `/run/secrets/`) and refuses to start if `X` itself is set.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;

use jiff::tz::TimeZone;
use sqlx::postgres::PgConnectOptions;

/// Settings that may only be given as files.
const SECRETS: [&str; 4] = [
    "DB_PASSWORD",
    "BOOTSTRAP_TOKEN",
    "LLM_API_KEY",
    "PRICE_API_KEY",
];

/// `openssl rand -base64 32` gives 44 characters.
const MIN_BOOTSTRAP_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0} must not be set directly; put the secret in a file and set {0}_FILE")]
    PlainSecret(&'static str),
    #[error("{0} is required")]
    Missing(&'static str),
    #[error("{name} is invalid: {value:?}")]
    Invalid { name: &'static str, value: String },
    #[error("can't read {name}_FILE ({path}): {source}")]
    SecretFile {
        name: &'static str,
        path: String,
        source: std::io::Error,
    },
}

/// A secret value. Its `Debug` output is redacted, so it can't be logged by
/// accident.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

#[derive(Debug)]
pub struct Config {
    /// `EXCHEQUER_BIND`, default `127.0.0.1:7438`.
    pub bind: SocketAddr,
    pub db: DbConfig,
    /// `BOOTSTRAP_TOKEN_FILE`: accepted only for managing tokens (design §10).
    pub bootstrap_token: Option<Secret>,
    /// `EXCHEQUER_WEB_DIR`: the web app's static build, served at `/`. The
    /// image sets it; unset, only the API is served.
    pub web_dir: Option<PathBuf>,
    /// `TZ` (an IANA name such as `America/New_York`), else the system's
    /// zone: what "today" and a bank file's dates mean. The image has no
    /// time zone files; the names come with the binary.
    pub zone: TimeZone,
}

#[derive(Debug)]
pub struct DbConfig {
    /// `DB_HOST`: a host name, or an absolute path to a socket directory.
    pub host: String,
    /// `DB_PORT`, default 5432.
    pub port: u16,
    /// `DB_USER`.
    pub user: String,
    /// `DB_NAME`.
    pub name: String,
    /// `DB_PASSWORD_FILE`; omitted for local socket connections.
    pub password: Option<Secret>,
}

impl DbConfig {
    pub fn connect_options(&self) -> PgConnectOptions {
        let opts = PgConnectOptions::new()
            .port(self.port)
            .username(&self.user)
            .database(&self.name);
        let opts = if self.host.starts_with('/') {
            opts.socket(&self.host)
        } else {
            opts.host(&self.host)
        };
        match &self.password {
            Some(p) => opts.password(p.expose()),
            None => opts,
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// `env` returns the value of an environment variable, if set.
    pub fn from_lookup(env: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        if let Some(name) = SECRETS.into_iter().find(|n| env(n).is_some()) {
            return Err(ConfigError::PlainSecret(name));
        }
        let required = |name: &'static str| env(name).ok_or(ConfigError::Missing(name));
        let parsed = |name: &'static str, default: &str| -> Result<String, ConfigError> {
            Ok(env(name).unwrap_or_else(|| default.to_owned()))
        };
        let secret = |name: &'static str| -> Result<Option<Secret>, ConfigError> {
            let Some(path) = env(&format!("{name}_FILE")) else {
                return Ok(None);
            };
            let value = std::fs::read_to_string(PathBuf::from(&path)).map_err(|source| {
                ConfigError::SecretFile {
                    name,
                    path: path.clone(),
                    source,
                }
            })?;
            // `podman secret create` keeps a trailing newline if one was piped in.
            Ok(Some(Secret(
                value.trim_end_matches(['\n', '\r']).to_owned(),
            )))
        };

        let bind = parsed("EXCHEQUER_BIND", "127.0.0.1:7438")?;
        let port = parsed("DB_PORT", "5432")?;
        Ok(Self {
            bind: bind.parse().map_err(|_| ConfigError::Invalid {
                name: "EXCHEQUER_BIND",
                value: bind,
            })?,
            db: DbConfig {
                host: required("DB_HOST")?,
                port: port.parse().map_err(|_| ConfigError::Invalid {
                    name: "DB_PORT",
                    value: port,
                })?,
                user: required("DB_USER")?,
                name: required("DB_NAME")?,
                password: secret("DB_PASSWORD")?,
            },
            bootstrap_token: match secret("BOOTSTRAP_TOKEN")? {
                Some(t) if t.0.chars().count() < MIN_BOOTSTRAP_LEN => {
                    return Err(ConfigError::Invalid {
                        name: "BOOTSTRAP_TOKEN_FILE",
                        value: format!("the token must be at least {MIN_BOOTSTRAP_LEN} characters"),
                    });
                }
                t => t,
            },
            web_dir: env("EXCHEQUER_WEB_DIR").map(PathBuf::from),
            zone: match env("TZ") {
                Some(name) => TimeZone::get(name.trim_start_matches(':')).map_err(|_| {
                    ConfigError::Invalid {
                        name: "TZ",
                        value: name,
                    }
                })?,
                None => TimeZone::system(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn config(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Config::from_lookup(|k| map.get(k).cloned())
    }

    const DB: [(&str, &str); 3] = [
        ("DB_HOST", "exchequer-db"),
        ("DB_USER", "exchequer"),
        ("DB_NAME", "exchequer"),
    ];

    #[test]
    fn defaults() {
        let c = config(&DB).unwrap();
        assert_eq!(c.bind.to_string(), "127.0.0.1:7438");
        assert_eq!(c.db.port, 5432);
        assert!(c.db.password.is_none());
        assert!(c.web_dir.is_none());
    }

    #[test]
    fn plain_secrets_are_refused() {
        for name in SECRETS {
            let mut vars = DB.to_vec();
            vars.push((name, "hunter2"));
            assert!(
                matches!(config(&vars), Err(ConfigError::PlainSecret(n)) if n == name),
                "{name}"
            );
        }
    }

    #[test]
    fn secrets_come_from_files_and_never_print() {
        let dir = std::env::temp_dir().join(format!("exchequer-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("db_password");
        std::fs::write(&file, "s3cret-value\n").unwrap();

        let mut vars = DB.to_vec();
        let path = file.to_str().unwrap().to_owned();
        vars.push(("DB_PASSWORD_FILE", &path));
        let c = config(&vars).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();

        assert_eq!(c.db.password.as_ref().unwrap().expose(), "s3cret-value");
        assert!(!format!("{c:?}").contains("s3cret"));
    }

    #[test]
    fn short_bootstrap_token_is_refused() {
        let dir = std::env::temp_dir().join(format!("exchequer-boot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("token");
        std::fs::write(&file, "too-short\n").unwrap();
        let path = file.to_str().unwrap().to_owned();
        let mut vars = DB.to_vec();
        vars.push(("BOOTSTRAP_TOKEN_FILE", &path));
        let result = config(&vars);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(matches!(result, Err(ConfigError::Invalid { .. })));
    }

    #[test]
    fn missing_secret_file_is_an_error() {
        let mut vars = DB.to_vec();
        vars.push(("DB_PASSWORD_FILE", "/nonexistent/exchequer"));
        assert!(matches!(config(&vars), Err(ConfigError::SecretFile { .. })));
    }

    #[test]
    fn time_zone_from_tz() {
        let mut vars = DB.to_vec();
        vars.push(("TZ", "Asia/Tokyo"));
        assert_eq!(config(&vars).unwrap().zone.iana_name(), Some("Asia/Tokyo"));
        vars.pop();
        vars.push(("TZ", "Nowhere/Atlantis"));
        assert!(matches!(
            config(&vars),
            Err(ConfigError::Invalid { name: "TZ", .. })
        ));
    }

    #[test]
    fn required_and_invalid() {
        assert!(matches!(config(&[]), Err(ConfigError::Missing("DB_HOST"))));
        let mut vars = DB.to_vec();
        vars.push(("DB_PORT", "abc"));
        assert!(matches!(
            config(&vars),
            Err(ConfigError::Invalid {
                name: "DB_PORT",
                ..
            })
        ));
    }
}
