//! `exchequer-server`: runs the API, or prints its OpenAPI document.

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

    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!(bind = %config.bind, "listening");
    axum::serve(listener, api::app(api::AppState { pool }))
        .with_graceful_shutdown(shutdown())
        .await?;
    tracing::info!("stopped");
    Ok(())
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
