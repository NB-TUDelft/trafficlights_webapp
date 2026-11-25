// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! The trafficlights-webapp server, as described in "Architecture and
//! design" in the knowledge base:
//!
//! - `GET /api/health` for the container health check;
//! - `GET /api/sessions/{id}/log`, the creator-authorized streaming
//!   download of a session's statistics `.jsonl` file;
//! - `WS /ws/lobby` and `WS /ws/sessions/{id}` (see [`ws`]), backed by the
//!   in-memory session registry in [`AppState`];
//! - the single-page app embedded from `web/dist` for every other `GET`.
//!
//! [`build_router_with_state`] wires these to one [`AppState`];
//! [`AppState::shutdown`] ends every session cleanly when the server stops.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde_json::json;
use tokio_util::io::ReaderStream;

mod board;
mod client;
pub mod config;
mod log;
mod session;
mod state;
pub mod ws;

pub use config::{ConfigError, ServerConfig};
pub use state::AppState;

/// Read-buffer size for streaming a statistics log download.
const LOG_STREAM_CHUNK_BYTES: usize = 16 * 1024;

/// The built frontend, baked into the binary in release builds (and read from
/// disk in debug builds). `build.rs` guarantees the folder exists.
#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct WebAssets;

/// Where the project's source code is published, for AGPL Section 13.
pub const SOURCE_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// The project's license, for `--version` and the AGPL Section 13 notice.
pub const LICENSE: &str = env!("CARGO_PKG_LICENSE");

/// The address the server binds to: `BIND`, defaulting to `127.0.0.1:3000`
/// when unset or blank (the same rule as [`ServerConfig::from_env`]).
pub fn bind_address() -> String {
    std::env::var("BIND")
        .ok()
        .map(|bind| bind.trim().to_string())
        .filter(|bind| !bind.is_empty())
        .unwrap_or_else(|| config::DEFAULT_BIND.to_string())
}

/// The commit this binary was built from, when the image build passes it in
/// `TRAFFICLIGHTS_COMMIT`; absent in a local development build.
pub fn build_commit() -> Option<&'static str> {
    option_env!("TRAFFICLIGHTS_COMMIT").filter(|commit| !commit.trim().is_empty())
}

/// What `trafficlights-webapp --version` prints: the version (with the short
/// commit when known), the license and the link to the Corresponding Source.
pub fn version_text() -> String {
    format_version(env!("CARGO_PKG_VERSION"), build_commit())
}

fn format_version(version: &str, commit: Option<&str>) -> String {
    let stamp = match commit {
        Some(commit) => {
            let short: String = commit.trim().chars().take(7).collect();
            format!("{version} ({short})")
        }
        None => version.to_string(),
    };
    format!("trafficlights-webapp {stamp}\nLicense: {LICENSE}\nSource: {SOURCE_URL}")
}

/// The application router with a default configuration and fresh state;
/// see [`build_router_with_state`].
pub fn build_router() -> Router {
    build_router_with_state(Arc::new(AppState::new(ServerConfig::default())))
}

/// The application router over `state`: `GET /api/health`,
/// `GET /api/sessions/{id}/log`, `WS /ws/lobby`, `WS /ws/sessions/{id}`,
/// plus the embedded single-page app for every other `GET`.
pub fn build_router_with_state(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/sessions/{id}/log", get(session_log))
        .route("/ws/lobby", get(ws::lobby))
        .route("/ws/sessions/{id}", get(ws::session))
        .fallback_service(get(static_fallback))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

/// A JSON error response: `{"error": {"code": ..., "message": ...}}`.
pub(crate) fn api_error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
        .into_response()
}

/// The credentials of an `Authorization: Bearer <token>` header.
fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.trim().split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then_some(token.trim())
}

/// `GET /api/sessions/{id}/log`: stream a live or recently ended session's
/// statistics `.jsonl` file to its creator.
///
/// The ID must look like a session ID, and the file path always comes from
/// the registry, never from the request, so no request can reach another
/// file. Unknown or expired sessions are `404`; a missing or wrong
/// `Authorization: Bearer <creator_token>` is `401` (compared in constant
/// time). The file is streamed in fixed-size chunks, so a download costs the
/// same small amount of memory however long the class ran (R15).
async fn session_log(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let not_found = || api_error(StatusCode::NOT_FOUND, "not_found", "Session log not found.");
    if !ws::valid_session_id(&id) {
        return not_found();
    }
    let Some((token, path)) = state.log_access(&id) else {
        return not_found();
    };
    if !bearer_token(&headers).is_some_and(|provided| token.matches(provided)) {
        let mut response = api_error(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "The session's creator token is required.",
        );
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static("Bearer"),
        );
        return response;
    }
    let file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return not_found(),
        Err(error) => {
            tracing::warn!(session_id = %id, %error, "cannot open a session log");
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "The session log could not be read.",
            );
        }
    };
    let body = Body::from_stream(ReaderStream::with_capacity(file, LOG_STREAM_CHUNK_BYTES));
    (
        [
            (
                header::CONTENT_TYPE,
                "application/x-ndjson; charset=utf-8".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"trafficlights-{id}.jsonl\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
        ],
        body,
    )
        .into_response()
}

/// Embedded files by exact path; any path without a file extension is a
/// client-side route and gets `index.html`. Unknown `/api/...` and `/ws/...`
/// paths and missing files are real 404s, so a stale asset never turns into
/// HTML.
async fn static_fallback(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let reserved = ["api", "ws"]
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")));
    if reserved {
        return api_error(StatusCode::NOT_FOUND, "not_found", "not found");
    }
    if let Some(response) = embedded(path) {
        return response;
    }
    if !has_extension(path) {
        if let Some(response) = embedded("index.html") {
            return response;
        }
    }
    (StatusCode::NOT_FOUND, "not found").into_response()
}

fn embedded(path: &str) -> Option<Response> {
    let file = WebAssets::get(path)?;
    Some(
        (
            [(header::CONTENT_TYPE, content_type(path))],
            file.data.into_owned(),
        )
            .into_response(),
    )
}

fn has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

/// The `Content-Type` for an embedded file. Browsers refuse to stream-compile
/// WebAssembly unless it is served as `application/wasm`.
fn content_type(path: &str) -> String {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    if mime.type_() == mime_guess::mime::TEXT {
        format!("{mime}; charset=utf-8")
    } else {
        mime.to_string()
    }
}

/// The TCP port of a `BIND` value such as `0.0.0.0:3000` or `[::]:3000`.
fn bind_port(bind: &str) -> Option<u16> {
    bind.rsplit_once(':')?.1.parse().ok()
}

/// Probe this server's own `/api/health` on the loopback interface, for the
/// container's `HEALTHCHECK` (the distroless image has no `curl` or `wget`).
/// `bind` is the server's `BIND` value; only its port is used.
pub fn healthcheck(bind: &str) -> bool {
    let Some(port) = bind_port(bind) else {
        return false;
    };
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let timeout = Duration::from_secs(3);
    let Ok(mut stream) = TcpStream::connect_timeout(&address, timeout) else {
        return false;
    };
    if stream.set_read_timeout(Some(timeout)).is_err()
        || stream.set_write_timeout(Some(timeout)).is_err()
    {
        return false;
    }
    let request =
        format!("GET /api/health HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() {
        return false;
    }
    response.starts_with("HTTP/1.1 200") && response.contains(r#""status":"ok""#)
}

#[cfg(test)]
mod tests {
    use axum::http::{header, HeaderMap, HeaderValue};

    use super::{
        bearer_token, bind_port, content_type, format_version, has_extension, LICENSE, SOURCE_URL,
    };

    fn authorization(value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, HeaderValue::from_static(value));
        headers
    }

    #[test]
    fn bearer_tokens_are_read_from_the_authorization_header() {
        assert_eq!(bearer_token(&authorization("Bearer abc")), Some("abc"));
        assert_eq!(bearer_token(&authorization("bearer  abc ")), Some("abc"));
        assert_eq!(bearer_token(&authorization("Basic abc")), None);
        assert_eq!(bearer_token(&authorization("Bearer")), None);
        assert_eq!(bearer_token(&HeaderMap::new()), None);
    }

    #[test]
    fn version_names_the_license_and_the_source() {
        let text = format_version("1.2.3", None);
        assert_eq!(
            text,
            format!("trafficlights-webapp 1.2.3\nLicense: {LICENSE}\nSource: {SOURCE_URL}")
        );
        assert_eq!(LICENSE, "AGPL-3.0-or-later");
        assert_eq!(
            SOURCE_URL,
            "https://github.com/NB-TUDelft/trafficlights_webapp"
        );
    }

    #[test]
    fn version_shortens_the_commit_to_seven_characters() {
        let text = format_version("1.2.3", Some("0123456789abcdef\n"));
        assert!(text.starts_with("trafficlights-webapp 1.2.3 (0123456)\n"));
    }

    #[test]
    fn bind_port_reads_ipv4_and_ipv6_binds() {
        assert_eq!(bind_port("0.0.0.0:3000"), Some(3000));
        assert_eq!(bind_port("127.0.0.1:8080"), Some(8080));
        assert_eq!(bind_port("[::]:3000"), Some(3000));
        assert_eq!(bind_port("3000"), None);
        assert_eq!(bind_port("host:notaport"), None);
    }

    #[test]
    fn web_assets_get_the_content_types_browsers_need() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("assets/app.css"), "text/css; charset=utf-8");
        assert_eq!(content_type("icon.svg"), "image/svg+xml");
        assert_eq!(
            content_type("assets/trafficlights_wasm_bg.wasm"),
            "application/wasm"
        );
        assert_eq!(
            content_type("manifest.webmanifest"),
            "application/manifest+json"
        );
        assert!(content_type("assets/app.js").starts_with("text/javascript"));
    }

    #[test]
    fn only_paths_with_a_file_name_extension_are_files() {
        assert!(has_extension("assets/app.js"));
        assert!(has_extension("icon.svg"));
        assert!(!has_extension(""));
        assert!(!has_extension("s/abc123"));
        assert!(!has_extension("v1.2/session"));
    }
}
