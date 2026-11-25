// SPDX-License-Identifier: AGPL-3.0-or-later
#![forbid(unsafe_code)]
//! HTTP side of trafficlights-webapp.
//!
//! Serves the single-page app embedded from `web/dist` and `GET /api/health`.
//! The WebSocket endpoints, session registry and statistics logging described
//! in "Architecture and design" are added on top of [`build_router`].

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde_json::json;

/// The built frontend, baked into the binary in release builds (and read from
/// disk in debug builds). `build.rs` guarantees the folder exists.
#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct WebAssets;

/// Where the project's source code is published, for AGPL Section 13.
pub const SOURCE_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// The project's license, for `--version` and the AGPL Section 13 notice.
pub const LICENSE: &str = env!("CARGO_PKG_LICENSE");

/// The address the server binds to: `BIND`, defaulting to `127.0.0.1:3000`.
pub fn bind_address() -> String {
    std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:3000".to_string())
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

/// The application router: `GET /api/health` plus the embedded single-page
/// app for every other `GET`.
pub fn build_router() -> Router {
    Router::new()
        .route("/api/health", get(health))
        .fallback_service(get(static_fallback))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

/// Embedded files by exact path; any path without a file extension is a
/// client-side route and gets `index.html`. Unknown `/api/...` paths and
/// missing files are real 404s, so a stale asset never turns into HTML.
async fn static_fallback(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": { "code": "not_found", "message": "not found" } })),
        )
            .into_response();
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
    use super::{bind_port, content_type, format_version, has_extension, LICENSE, SOURCE_URL};

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
