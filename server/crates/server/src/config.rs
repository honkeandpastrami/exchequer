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
    "REMOTE_AI_KEY",
    "TIINGO_API_KEY",
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
    /// `EXCHEQUER_LOCAL_AI`: the local AI method (design §8), by its ID,
    /// default the newest (`lexical-knn-v2`); `off` for none.
    pub local_ai: Option<String>,
    /// Remote AI (design §8), when its endpoint, deployment and key are all
    /// given; none of them, no remote AI.
    pub remote_ai: Option<RemoteAi>,
    pub prices: PriceConfig,
}

/// Price sources (design §3.4).
#[derive(Debug)]
pub struct PriceConfig {
    /// `EXCHEQUER_PRICE_SOURCES`: names in order, default `tiingo,ecb`;
    /// `off` for none. A source whose key isn't given sits out.
    pub sources: Vec<PriceSource>,
    /// Listed but without their key: they sit out.
    pub unkeyed: Vec<&'static str>,
    /// `EXCHEQUER_PRICE_FRESH_DAYS`: a price at most this many days older
    /// than the date asked is fresh, default 5.
    pub fresh_days: u32,
}

#[derive(Debug)]
pub enum PriceSource {
    /// `TIINGO_API_KEY_FILE`.
    Tiingo {
        key: Secret,
    },
    Ecb,
}

/// A decision model on Azure AI Foundry.
#[derive(Debug)]
pub struct RemoteAi {
    /// `EXCHEQUER_REMOTE_AI_ENDPOINT`: the Foundry resource's endpoint,
    /// `https://<resource>.services.ai.azure.com`.
    pub endpoint: String,
    /// `EXCHEQUER_REMOTE_AI_DEPLOYMENT`: the model's deployment name.
    pub deployment: String,
    /// `REMOTE_AI_KEY_FILE`: the resource's key.
    pub key: Secret,
    /// `EXCHEQUER_REMOTE_AI_CONCURRENCY`: requests at once, default 4.
    pub concurrency: usize,
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

/// Where the server listens unless `EXCHEQUER_BIND` says otherwise.
pub const DEFAULT_BIND: &str = "127.0.0.1:7438";

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

        let remote_ai = match (
            env("EXCHEQUER_REMOTE_AI_ENDPOINT"),
            env("EXCHEQUER_REMOTE_AI_DEPLOYMENT"),
            secret("REMOTE_AI_KEY")?,
        ) {
            (None, None, None) => None,
            (Some(endpoint), Some(deployment), Some(key)) => {
                if !endpoint.starts_with("https://") {
                    return Err(ConfigError::Invalid {
                        name: "EXCHEQUER_REMOTE_AI_ENDPOINT",
                        value: endpoint,
                    });
                }
                let concurrency = parsed("EXCHEQUER_REMOTE_AI_CONCURRENCY", "4")?;
                Some(RemoteAi {
                    endpoint,
                    deployment,
                    key,
                    concurrency: match concurrency.parse() {
                        Ok(n @ 1..=32) => n,
                        _ => {
                            return Err(ConfigError::Invalid {
                                name: "EXCHEQUER_REMOTE_AI_CONCURRENCY",
                                value: concurrency,
                            });
                        }
                    },
                })
            }
            (None, _, _) => return Err(ConfigError::Missing("EXCHEQUER_REMOTE_AI_ENDPOINT")),
            (_, None, _) => return Err(ConfigError::Missing("EXCHEQUER_REMOTE_AI_DEPLOYMENT")),
            (_, _, None) => return Err(ConfigError::Missing("REMOTE_AI_KEY_FILE")),
        };
        let mut price_sources = Vec::new();
        let mut unkeyed = Vec::new();
        let names = parsed("EXCHEQUER_PRICE_SOURCES", "tiingo,ecb")?;
        if names != "off" {
            let mut seen = Vec::new();
            for name in names.split(',').map(str::trim) {
                if seen.contains(&name) {
                    return Err(ConfigError::Invalid {
                        name: "EXCHEQUER_PRICE_SOURCES",
                        value: names.clone(),
                    });
                }
                seen.push(name);
                match name {
                    "tiingo" => match secret("TIINGO_API_KEY")? {
                        Some(key) => price_sources.push(PriceSource::Tiingo { key }),
                        None => unkeyed.push("tiingo"),
                    },
                    "ecb" => price_sources.push(PriceSource::Ecb),
                    _ => {
                        return Err(ConfigError::Invalid {
                            name: "EXCHEQUER_PRICE_SOURCES",
                            value: names.clone(),
                        });
                    }
                }
            }
        }
        let fresh_days = parsed("EXCHEQUER_PRICE_FRESH_DAYS", "5")?;
        let prices = PriceConfig {
            sources: price_sources,
            unkeyed,
            fresh_days: match fresh_days.parse() {
                Ok(n @ 0..=31) => n,
                _ => {
                    return Err(ConfigError::Invalid {
                        name: "EXCHEQUER_PRICE_FRESH_DAYS",
                        value: fresh_days,
                    });
                }
            },
        };
        let bind = parsed("EXCHEQUER_BIND", DEFAULT_BIND)?;
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
            local_ai: match env("EXCHEQUER_LOCAL_AI") {
                Some(off) if off == "off" => None,
                Some(id) => match heuristics::method(&id) {
                    Some(m) => Some(m.id()),
                    None => {
                        return Err(ConfigError::Invalid {
                            name: "EXCHEQUER_LOCAL_AI",
                            value: id,
                        });
                    }
                },
                None => heuristics::METHODS.first().map(|m| m.id()),
            },
            remote_ai,
            prices,
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
        assert_eq!(c.local_ai.as_deref(), Some("lexical-knn-v2"));
        assert!(c.remote_ai.is_none());
        // Tiingo sits out without its key.
        assert!(matches!(c.prices.sources[..], [PriceSource::Ecb]));
        assert_eq!(c.prices.unkeyed, ["tiingo"]);
        assert_eq!(c.prices.fresh_days, 5);
    }

    #[test]
    fn price_sources() {
        let dir = std::env::temp_dir().join(format!("exchequer-prices-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("key");
        std::fs::write(&file, "t1ingo\n").unwrap();
        let path = file.to_str().unwrap().to_owned();
        let with = |extra: &[(&'static str, &str)]| {
            let mut vars = DB.to_vec();
            vars.extend_from_slice(extra);
            config(&vars)
        };
        let c = with(&[("TIINGO_API_KEY_FILE", &path)]).unwrap();
        match &c.prices.sources[..] {
            [PriceSource::Tiingo { key }, PriceSource::Ecb] => assert_eq!(key.expose(), "t1ingo"),
            other => panic!("{other:?}"),
        }
        let c = with(&[
            ("TIINGO_API_KEY_FILE", &path),
            ("EXCHEQUER_PRICE_SOURCES", "ecb, tiingo"),
        ])
        .unwrap();
        assert!(matches!(
            c.prices.sources[..],
            [PriceSource::Ecb, PriceSource::Tiingo { .. }]
        ));
        let c = with(&[("EXCHEQUER_PRICE_SOURCES", "off")]).unwrap();
        assert!(c.prices.sources.is_empty() && c.prices.unkeyed.is_empty());
        for bad in ["yahoo", "ecb,ecb", ""] {
            assert!(
                matches!(
                    with(&[("EXCHEQUER_PRICE_SOURCES", bad)]),
                    Err(ConfigError::Invalid { .. })
                ),
                "{bad}"
            );
        }
        assert!(matches!(
            with(&[("TIINGO_API_KEY", "plain")]),
            Err(ConfigError::PlainSecret("TIINGO_API_KEY"))
        ));
        assert!(with(&[("EXCHEQUER_PRICE_FRESH_DAYS", "40")]).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remote_ai_needs_all_three() {
        let dir = std::env::temp_dir().join(format!("exchequer-remote-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("key");
        std::fs::write(&file, "k3y-value\n").unwrap();
        let path = file.to_str().unwrap().to_owned();
        let mut vars = DB.to_vec();
        vars.push((
            "EXCHEQUER_REMOTE_AI_ENDPOINT",
            "https://x.services.ai.azure.com",
        ));
        assert!(matches!(
            config(&vars),
            Err(ConfigError::Missing("EXCHEQUER_REMOTE_AI_DEPLOYMENT"))
        ));
        vars.push(("EXCHEQUER_REMOTE_AI_DEPLOYMENT", "decision"));
        assert!(matches!(
            config(&vars),
            Err(ConfigError::Missing("REMOTE_AI_KEY_FILE"))
        ));
        vars.push(("REMOTE_AI_KEY_FILE", &path));
        let c = config(&vars).unwrap();
        let r = c.remote_ai.as_ref().unwrap();
        assert_eq!(
            (r.deployment.as_str(), r.key.expose(), r.concurrency),
            ("decision", "k3y-value", 4)
        );
        assert!(!format!("{c:?}").contains("k3y"));
        vars[3] = (
            "EXCHEQUER_REMOTE_AI_ENDPOINT",
            "http://x.services.ai.azure.com",
        );
        let plain = config(&vars);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(matches!(
            plain,
            Err(ConfigError::Invalid {
                name: "EXCHEQUER_REMOTE_AI_ENDPOINT",
                ..
            })
        ));
    }

    #[test]
    fn local_ai_by_method() {
        let mut vars = DB.to_vec();
        vars.push(("EXCHEQUER_LOCAL_AI", "off"));
        assert_eq!(config(&vars).unwrap().local_ai, None);
        vars.pop();
        vars.push(("EXCHEQUER_LOCAL_AI", "magic-v9"));
        assert!(matches!(
            config(&vars),
            Err(ConfigError::Invalid {
                name: "EXCHEQUER_LOCAL_AI",
                ..
            })
        ));
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
