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
