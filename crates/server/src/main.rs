// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]

use std::future::IntoFuture;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use trafficlights_server::config::is_loopback_bind;
use trafficlights_server::{
    bind_address, build_commit, build_router_with_state, healthcheck, version_text, AppState,
    ServerConfig,
};

const USAGE: &str = "usage: trafficlights-webapp [--version | --healthcheck]";

/// Once a stop signal has ended every session, how long in-flight HTTP
/// requests (a log download over a slow link, a half-sent request) may
/// still run before the process exits anyway; well inside Docker's default
/// ten-second stop timeout.
const HTTP_DRAIN: Duration = Duration::from_secs(5);

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
    let config = match ServerConfig::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("invalid configuration: {error}");
            return ExitCode::from(2);
        }
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("cannot start the async runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(serve(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("server failed: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Serve until `SIGTERM` or `SIGINT`, then end every active session with
/// `shutdown` so each statistics log gets its `session_end` line.
///
/// The sessions end as soon as the signal arrives, before the HTTP server
/// drains: axum waits for every in-flight request, and a single slow client
/// must not delay the statistics until the container is killed.
async fn serve(config: ServerConfig) -> std::io::Result<()> {
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
    // `Debug` redacts the creation PIN.
    tracing::info!(?config, "configuration");
    if config.public_origins.is_empty() && !is_loopback_bind(&config.bind) {
        tracing::warn!(
            "PUBLIC_ORIGIN is not set while BIND is not loopback; set PUBLIC_ORIGIN in production to restrict WebSocket origins"
        );
    }
    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!(address = %listener.local_addr()?, "trafficlights-webapp listening");
    let state = Arc::new(AppState::new(config));
    let (sessions_ended, ended) = tokio::sync::oneshot::channel::<()>();
    let stopping = Arc::clone(&state);
    let server = axum::serve(listener, build_router_with_state(Arc::clone(&state)))
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            stopping.shutdown().await;
            let _ = sessions_ended.send(());
        })
        .into_future();
    let drain_limit = async {
        match ended.await {
            Ok(()) => tokio::time::sleep(HTTP_DRAIN).await,
            // The server stopped without a signal; nothing to limit.
            Err(_) => std::future::pending().await,
        }
    };
    let served = tokio::select! {
        served = server => served,
        () = drain_limit => {
            tracing::warn!("in-flight HTTP requests did not finish; exiting anyway");
            Ok(())
        }
    };
    // Idempotent: ends any session still active if the server stopped for
    // another reason.
    state.shutdown().await;
    tracing::info!("trafficlights-webapp stopped");
    served
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
