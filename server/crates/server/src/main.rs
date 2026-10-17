//! `exchequer-server`: runs the API, prints its OpenAPI document or schema
//! version, or checks a running server's health.

use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

mod config;

use config::Config;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the API server (the default).
    Serve,
    /// Print the OpenAPI document as JSON.
    Openapi,
    /// Print the newest schema migration this binary knows. A restore checks a
    /// dump against it (deploy/exchequer-backup).
    SchemaVersion,
    /// Exit 0 if the server at `EXCHEQUER_BIND` answers `/healthz` with 200
    /// (up, and reaching its database); 1 otherwise. The container's health
    /// check: the image has no shell or curl.
    Healthcheck,
}

#[tokio::main]
async fn main() -> ExitCode {
    match Cli::parse().command.unwrap_or(Command::Serve) {
        Command::Openapi => {
            let doc = api::openapi();
            match doc.to_pretty_json() {
                Ok(json) => {
                    println!("{json}");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("error: {err}");
                    ExitCode::FAILURE
                }
            }
        }
        Command::SchemaVersion => {
            println!("{}", store::schema_version());
            ExitCode::SUCCESS
        }
        Command::Healthcheck => match healthcheck().await {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("unhealthy: {err}");
                ExitCode::FAILURE
            }
        },
        Command::Serve => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
                )
                // Plain text under journald; colors only in a terminal.
                .with_ansi(std::io::stdout().is_terminal())
                .init();
            match serve().await {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    tracing::error!("{err}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;

    // The database container may still be starting: retry for a while.
    let pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(30))
        .connect_with(config.db.connect_options())
        .await?;
    store::MIGRATOR.run(&pool).await?;
    tracing::info!(schema = store::schema_version(), "database ready");

    match config.zone.iana_name() {
        Some(zone) if zone != "UTC" => tracing::info!(zone, "time zone"),
        _ => tracing::warn!("no time zone (set TZ): dates are UTC"),
    }
    match &config.local_ai {
        Some(method) => tracing::info!(method, "local AI"),
        None => tracing::info!("local AI off"),
    }
    let remote = match &config.remote_ai {
        Some(r) => {
            let client = outbound::decision::Client::new(
                &r.endpoint,
                &r.deployment,
                r.key.expose(),
                std::time::Duration::from_secs(30),
            )
            .map_err(|e| format!("remote AI: {e}"))?;
            tracing::info!(
                deployment = r.deployment,
                concurrency = r.concurrency,
                "remote AI"
            );
            Some(std::sync::Arc::new(remote::Remote::new(
                std::sync::Arc::new(client),
                r.concurrency,
            )))
        }
        None => {
            tracing::info!("remote AI off");
            None
        }
    };
    let timeout = std::time::Duration::from_secs(15);
    let mut price_sources: Vec<std::sync::Arc<dyn outbound::prices::PriceSource>> = Vec::new();
    for s in &config.prices.sources {
        price_sources.push(match s {
            config::PriceSource::Tiingo { key } => std::sync::Arc::new(
                outbound::prices::Tiingo::new(outbound::prices::Tiingo::URL, key.expose(), timeout)
                    .map_err(|e| format!("tiingo: {e}"))?,
            ),
            config::PriceSource::Ecb => std::sync::Arc::new(
                outbound::prices::Ecb::new(outbound::prices::Ecb::URL, timeout)
                    .map_err(|e| format!("ecb: {e}"))?,
            ),
        });
    }
    let pricing = api::Pricing::new(price_sources, config.prices.fresh_days);
    for name in &config.prices.unkeyed {
        tracing::info!(source = name, "price source off: no key");
    }
    tracing::info!(sources = pricing.names().join(","), "price sources");
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!(bind = %config.bind, "listening");
    let state = api::AppState {
        pool,
        bootstrap_token: config.bootstrap_token.map(|s| s.expose().into()),
        zone: config.zone,
        suggestions: api::Suggestions::new(config.local_ai.as_deref().and_then(heuristics::method))
            .with_remote(remote),
        pricing,
    };
    let app = match &config.web_dir {
        Some(dir) => api::app_with_web(state, dir),
        None => api::app(state),
    };
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    tracing::info!("stopped");
    Ok(())
}

/// `GET /healthz` on this container's own server, with a plain HTTP/1.0
/// request: a wildcard bind address is reached on loopback.
async fn healthcheck() -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let bind = std::env::var("EXCHEQUER_BIND").unwrap_or_else(|_| config::DEFAULT_BIND.into());
    let mut addr: std::net::SocketAddr = bind.parse()?;
    if addr.ip().is_unspecified() {
        addr.set_ip(match addr {
            std::net::SocketAddr::V4(_) => std::net::Ipv4Addr::LOCALHOST.into(),
            std::net::SocketAddr::V6(_) => std::net::Ipv6Addr::LOCALHOST.into(),
        });
    }
    let status = tokio::time::timeout(Duration::from_secs(5), async {
        let mut stream = tokio::net::TcpStream::connect(addr).await?;
        stream
            .write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")
            .await?;
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await?;
        Ok::<_, std::io::Error>(String::from_utf8_lossy(&response).into_owned())
    })
    .await??;
    let line = status.lines().next().unwrap_or_default();
    if line.split(' ').nth(1) == Some("200") {
        Ok(())
    } else {
        Err(format!("{addr}: {line}").into())
    }
}

/// Resolves on Ctrl-C or SIGTERM (`podman stop`).
async fn shutdown() {
    let ctrl_c = tokio::signal::ctrl_c();
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("can install a SIGTERM handler");
    tokio::select! {
        _ = ctrl_c => {}
        _ = term.recv() => {}
    }
}
