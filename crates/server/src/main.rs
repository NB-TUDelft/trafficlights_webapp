// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use std::process::ExitCode;

use trafficlights_server::{bind_address, build_commit, build_router, healthcheck, version_text};

const USAGE: &str = "usage: trafficlights-webapp [--version | --healthcheck]";

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        None => run_server(),
        Some("--version") => {
            println!("{}", version_text());
            ExitCode::SUCCESS
        }
        Some("--healthcheck") => {
            if healthcheck(&bind_address()) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Some(other) => {
            eprintln!("unknown argument: {other}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run_server() -> ExitCode {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("cannot start the async runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(serve()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("server failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn serve() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("LOG").unwrap_or_else(|_| "info".into()),
        )
        .init();

    // The build first, so even a start that fails to bind names its version.
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        commit = build_commit(),
        "trafficlights-webapp starting"
    );
    let bind = bind_address();
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "trafficlights-webapp listening");
    axum::serve(listener, build_router())
        .with_graceful_shutdown(shutdown_signal())
        .await
}

/// Docker stops containers with SIGTERM, and as PID 1 an unhandled SIGTERM is
/// ignored until the runtime's SIGKILL, so it must end the server too.
async fn shutdown_signal() {
    let interrupt = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::warn!(%error, "no SIGTERM handler; stop with SIGINT");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = interrupt => {}
        () = terminate => {}
    }
    tracing::info!("shutting down");
}
